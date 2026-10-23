//! Device-wide enterprise relation, settled by the sole communication owner.
//! ref: reqwest v0.13.5 src/async_impl/client.rs; rusqlite v0.32.1 src/transaction.rs.
use crate::{
    store::{encode, hash},
    wire, Client, Clock, Error, IdentityProvider,
};
use execution_contract::NativeContextRef;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authenticated local owner; never supplied by the renderer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    /// OS-authenticated peer scope.
    pub peer: String,
    /// Exact native login incarnation.
    pub context: NativeContextRef,
}
/// Safe synchronization projection.
pub use execution_contract::CurrentUserSync as Status;
/// Durable local phase from the shared product contract.
pub use execution_contract::CurrentUserSyncPhase as Phase;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    owner: Owner,
    registration: String,
    input: wire::CurrentUserChange,
    user: Option<wire::SelfServiceUser>,
    proof: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Confirmed {
    owner: Owner,
    registration: String,
    proof: String,
    value: wire::CurrentUser,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    phase: Phase,
    pending: Option<Pending>,
    confirmed: Option<Confirmed>,
    // Bounded evidence: an abandoned PUT remains unknown even after cleanup settles.
    unresolved: Option<Pending>,
}
fn same_user(a: &Option<wire::SelfServiceUser>, b: &Option<wire::SelfServiceUser>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.tenant_id == b.tenant_id
                && a.instance_id == b.instance_id
                && a.principal_id == b.principal_id
        }
        _ => false,
    }
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    fn relation_state(&self) -> Result<State, Error> {
        Ok(self.store.get("current_user")?.unwrap_or_default())
    }
    fn relation_registration(&self) -> Result<String, Error> {
        let active = self.identity_state()?.active.ok_or(Error::Identity)?;
        // Includes the exact certificate, not merely the unchanged registration generation.
        Ok(hash(&encode(&(self.registration()?, active.chain))?))
    }
    fn save_relation(&self, state: &State) -> Result<(), Error> {
        self.store.put("current_user", state)
    }
    /// Last durable progress; opening this projection performs no network IO.
    pub fn current_user_status(&self) -> Result<Status, Error> {
        let state = self.relation_state()?;
        let registration = self.relation_registration()?;
        let stale = state
            .pending
            .as_ref()
            .is_some_and(|p| p.registration != registration)
            || state
                .confirmed
                .as_ref()
                .is_some_and(|c| c.registration != registration);
        Ok(Status {
            phase: if stale {
                Phase::Invalidated
            } else {
                state.phase
            },
            current: state.confirmed.map(|c| c.value),
        })
    }
    /// Retained original owner, used only by authenticated service retirement/recovery.
    pub fn current_user_owner(&self) -> Result<Option<Owner>, Error> {
        let state = self.relation_state()?;
        Ok(state
            .pending
            .map(|p| p.owner)
            .or_else(|| state.confirmed.map(|c| c.owner)))
    }
    /// Read the server's public relation using the actual device mTLS identity.
    pub async fn current_user(&self) -> Result<wire::CurrentUser, Error> {
        let value: wire::CurrentUser = self
            .self_service_json(None, Method::GET, "current-user", None)
            .await?;
        Self::check_relation(&value)?;
        Ok(value)
    }
    fn check_relation(value: &wire::CurrentUser) -> Result<(), Error> {
        if value.revision >= i64::MAX as u64
            || value.confirmed_at.is_some_and(|v| v < 0)
            || (value.state == wire::CurrentUserState::Active) != value.user.is_some()
            || value.user.as_ref().is_some_and(|u| {
                u.tenant_id.is_nil() || u.instance_id.is_nil() || u.principal_id.is_nil()
            })
        {
            return Err(Error::Protocol);
        }
        Ok(())
    }
    /// Freeze or resume this exact enterprise session. A new proof is never a replay.
    pub async fn bind_current_user(
        &self,
        owner: &Owner,
        user: &wire::SelfServiceUser,
        proof: &str,
    ) -> Result<wire::CurrentUser, Error> {
        self.active()?;
        if user.tenant_id != self.configuration().tenant {
            return Err(Error::Identity);
        }
        let registration = self.relation_registration()?;
        let fingerprint = hash(proof.as_bytes());
        let mut state = self.relation_state()?;
        if state
            .pending
            .as_ref()
            .is_some_and(|p| p.registration != registration)
            || state
                .confirmed
                .as_ref()
                .is_some_and(|c| c.registration != registration)
        {
            state.unresolved = state.pending.take();
            state.confirmed = None;
            state.phase = Phase::Unbound;
            self.save_relation(&state)?;
        }
        if let Some(p) = &state.pending {
            if p.owner != *owner
                || p.user.is_none()
                || p.proof.as_ref() != Some(&fingerprint)
                || !same_user(&p.user, &Some(user.clone()))
            {
                return Err(Error::Conflict);
            }
            if state.phase == Phase::Conflict {
                return Err(Error::Conflict);
            }
            return self.flush_relation(Some(proof)).await;
        }
        if let Some(c) = &state.confirmed {
            if c.owner == *owner && c.proof == fingerprint {
                let current = self.current_user().await?;
                if current.revision != c.value.revision
                    || current.state != wire::CurrentUserState::Active
                    || !same_user(&current.user, &c.value.user)
                {
                    return Err(Error::Conflict);
                }
                return Ok(current);
            }
        }
        let current = self.current_user().await?;
        state.pending = Some(Pending {
            owner: owner.clone(),
            registration,
            input: wire::CurrentUserChange {
                operation_id: Uuid::new_v4(),
                expected_revision: current.revision,
            },
            user: Some(user.clone()),
            proof: Some(fingerprint),
        });
        state.phase = Phase::BindingUnknown;
        self.save_relation(&state)?;
        self.flush_relation(Some(proof)).await
    }
    /// Drain an owned relation. Recovery explicitly abandons an unreplayable PUT, never claims it succeeded.
    pub async fn retire_current_user(
        &self,
        owner: &Owner,
        proof: Option<&str>,
        recover: bool,
    ) -> Result<(), Error> {
        self.active()?;
        let registration = self.relation_registration()?;
        let mut state = self.relation_state()?;
        if state
            .pending
            .as_ref()
            .is_some_and(|p| p.registration != registration)
            || state
                .confirmed
                .as_ref()
                .is_some_and(|c| c.registration != registration)
        {
            state.phase = Phase::Invalidated;
            self.save_relation(&state)?;
            return Err(Error::Identity);
        }
        if let Some(p) = state.pending.as_ref() {
            if p.owner != *owner {
                return Err(Error::Conflict);
            }
            if p.user.is_none() && state.phase != Phase::Conflict {
                self.flush_relation(None).await?;
                return Ok(());
            }
            if p.user.is_some()
                && proof.is_some_and(|proof| p.proof.as_ref() == Some(&hash(proof.as_bytes())))
                && !recover
            {
                self.flush_relation(proof).await?;
                state = self.relation_state()?;
            } else if !recover {
                return Err(Error::Unavailable);
            }
        }
        if !recover && state.confirmed.as_ref().is_none_or(|c| c.owner != *owner) {
            return Ok(()); // Another peer's later login must survive this peer's logout.
        }
        let revision = if recover {
            self.current_user().await?.revision
        } else {
            state
                .confirmed
                .as_ref()
                .ok_or(Error::Conflict)?
                .value
                .revision
        };
        if state.pending.is_some() {
            state.unresolved = state.pending.take();
        }
        state.pending = Some(Pending {
            owner: owner.clone(),
            registration,
            input: wire::CurrentUserChange {
                operation_id: Uuid::new_v4(),
                expected_revision: revision,
            },
            user: None,
            proof: None,
        });
        state.phase = Phase::CleanupPending;
        self.save_relation(&state)?;
        self.flush_relation(None).await?;
        Ok(())
    }
    async fn flush_relation(&self, proof: Option<&str>) -> Result<wire::CurrentUser, Error> {
        let mut state = self.relation_state()?;
        let pending = state.pending.as_ref().ok_or(Error::Conflict)?;
        if pending.registration != self.relation_registration()? {
            return Err(Error::Identity);
        }
        pending.input.validate()?;
        if pending.user.is_some()
            && proof.is_none_or(|proof| pending.proof.as_ref() != Some(&hash(proof.as_bytes())))
        {
            return Err(Error::Identity);
        }
        let response = self
            .self_service_json::<wire::CurrentUser>(
                proof,
                if pending.user.is_some() {
                    Method::PUT
                } else {
                    Method::DELETE
                },
                "current-user",
                Some(encode(&pending.input)?),
            )
            .await;
        let value = match response {
            Ok(value) => value,
            Err(Error::Conflict) => {
                state.phase = Phase::Conflict;
                self.save_relation(&state)?;
                return Err(Error::Conflict);
            }
            Err(error) => return Err(error),
        };
        Self::check_relation(&value)?;
        if pending.registration != self.relation_registration()?
            || value.revision != pending.input.expected_revision + 1
            || !same_user(&value.user, &pending.user)
            || value.state
                != if pending.user.is_some() {
                    wire::CurrentUserState::Active
                } else {
                    wire::CurrentUserState::Unbound
                }
        {
            return Err(Error::Protocol);
        }
        state.confirmed = if let Some(fingerprint) = &pending.proof {
            Some(Confirmed {
                owner: pending.owner.clone(),
                registration: pending.registration.clone(),
                proof: fingerprint.clone(),
                value: value.clone(),
            })
        } else {
            None
        };
        state.pending = None;
        state.phase = if state.confirmed.is_some() {
            Phase::Bound
        } else {
            Phase::Unbound
        };
        self.save_relation(&state)?;
        Ok(value)
    }
}

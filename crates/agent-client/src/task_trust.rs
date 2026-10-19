//! One durable task authority in the communication owner's existing SQLite state.
//! ref: rusqlite 0.32.1 src/transaction.rs; reqwest 0.13.5 src/async_impl/client.rs.
use crate::{Client, Clock, Error, IdentityProvider};
use base64::Engine;
use rss_mdm_agent_wire::task_trust::{
    Acknowledgement, Observation, ObservationReason, ObservationState, Trust,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    document: Option<Trust>,
    ack: Option<Acknowledgement>,
    confirmed: bool,
    refresh_at: i64,
    next_drive: i64,
    sequence: u64,
    latest: Option<Observation>,
    pending: Option<Observation>,
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    fn task_trust_state(&self) -> Result<State, Error> {
        Ok(self.store.get("task_trust")?.unwrap_or_default())
    }
    fn bind_task_trust(&self, trust: &Trust) -> Result<(), Error> {
        trust.validate()?;
        if trust.tenant != self.store.cfg.tenant
            || trust.origin != self.store.cfg.origin.origin().ascii_serialization()
        {
            return Err(Error::Untrusted);
        }
        Ok(())
    }
    pub(crate) fn task_trust_ready(&self) -> Result<(), Error> {
        let state = self.task_trust_state()?;
        self.bind_task_trust(state.document.as_ref().ok_or(Error::Untrusted)?)
    }
    pub(crate) fn task_verification_key(&self, id: &str) -> Result<Vec<u8>, Error> {
        let state = self.task_trust_state()?;
        let trust = state.document.ok_or(Error::Untrusted)?;
        self.bind_task_trust(&trust)?;
        let key = trust
            .keys
            .iter()
            .find(|key| key.id == id)
            .ok_or(Error::Untrusted)?;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&key.public_key)
            .map_err(|_| Error::Untrusted)
    }
    /// Last local task-trust observation. It contains no key material or credential references.
    pub fn task_trust_observation(&self) -> Result<Option<Observation>, Error> {
        Ok(self.task_trust_state()?.latest)
    }
    /// ACK confirmation is separate from local durable application.
    pub fn task_trust_acknowledged(&self) -> Result<bool, Error> {
        let state = self.task_trust_state()?;
        let Some(document) = state.document.as_ref() else {
            return Ok(false);
        };
        self.bind_task_trust(document)?;
        let registration = self.registration()?;
        Ok(state.confirmed
            && state.ack.as_ref().is_some_and(|ack| {
                ack.registration == registration.registration_id
                    && ack.generation == registration.generation
                    && ack.matches(document).unwrap_or(false)
            }))
    }
    fn observe_task_trust(
        &self,
        target: Option<&Trust>,
        state: ObservationState,
        reason: Option<ObservationReason>,
    ) -> Result<(), Error> {
        let registration = self.registration()?;
        let mut stored = self.task_trust_state()?;
        stored.sequence = stored
            .sequence
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(Error::Capacity)?;
        let actual = stored.document.as_ref();
        let observation = Observation {
            operation: Uuid::new_v4(),
            version: 1,
            registration: registration.registration_id,
            generation: registration.generation,
            sequence: stored.sequence,
            target_revision: target.map(|v| v.revision),
            target_digest: target.map(Trust::installation_digest).transpose()?,
            actual_revision: actual.map(|v| v.revision),
            actual_digest: actual.map(Trust::installation_digest).transpose()?,
            state,
            reason,
            observed_at: self.now()?,
        };
        observation.validate()?;
        if stored.pending.is_none() {
            stored.pending = Some(observation.clone());
        }
        stored.latest = Some(observation);
        self.store.put("task_trust", &stored)
    }
    async fn flush_task_observation(&self) -> Result<(), Error> {
        let mut state = self.task_trust_state()?;
        let Some(pending) = state.pending.as_ref() else {
            return Ok(());
        };
        let reply: Observation = self
            .json(
                reqwest::Method::POST,
                "task-trust/observations",
                Some(&crate::store::encode(pending)?),
            )
            .await?;
        if &reply != pending {
            return Err(Error::Protocol);
        }
        state.pending = state.latest.clone().filter(|v| v.sequence > reply.sequence);
        self.store.put("task_trust", &state)
    }
    async fn acknowledge_task_trust(&self) -> Result<(), Error> {
        // A fresh readback is mandatory even when replaying a lost response after restart.
        let mut state = self.task_trust_state()?;
        let Some(ack) = state.ack.as_ref() else {
            return Ok(());
        };
        let trust = state.document.as_ref().ok_or(Error::Storage)?;
        self.bind_task_trust(trust)?;
        let registration = self.registration()?;
        if !ack.matches(trust)?
            || ack.registration != registration.registration_id
            || ack.generation != registration.generation
        {
            return Err(Error::Untrusted);
        }
        if state.confirmed {
            return Ok(());
        }
        let reply: Acknowledgement = self
            .json(
                reqwest::Method::POST,
                "task-trust",
                Some(&crate::store::encode(ack)?),
            )
            .await?;
        if &reply != ack {
            return Err(Error::Protocol);
        }
        state.confirmed = true;
        self.store.put("task_trust", &state)
    }
    async fn refresh_task_trust(&self) -> Result<(), Error> {
        let trust: Trust = self.json(reqwest::Method::GET, "task-trust", None).await?;
        self.bind_task_trust(&trust)?;
        let mut state = self.task_trust_state()?;
        if let Some(installed) = &state.document {
            trust.validate_successor(installed)?;
        }
        self.observe_task_trust(Some(&trust), ObservationState::Received, None)?;
        state = self.task_trust_state()?;
        let registration = self.registration()?;
        if state.ack.as_ref().is_none_or(|a| {
            a.registration != registration.registration_id
                || a.generation != registration.generation
                || !a.matches(&trust).unwrap_or(false)
        }) {
            state.ack = Some(Acknowledgement {
                operation: Uuid::new_v4(),
                version: 1,
                registration: registration.registration_id,
                generation: registration.generation,
                revision: trust.revision,
                digest: trust.installation_digest()?,
            });
            state.confirmed = false;
        }
        state.document = Some(trust.clone());
        state.refresh_at = self.now()?.saturating_add(60);
        self.store.put("task_trust", &state)?;
        let readback = self.task_trust_state()?;
        if readback.document.as_ref() != Some(&trust) {
            return Err(Error::Storage);
        }
        self.observe_task_trust(Some(&trust), ObservationState::Applied, None)?;
        self.acknowledge_task_trust().await
    }
    /// Bounded drive from the existing service loop; failure preserves the enrolled identity and prior authority.
    pub async fn drive_task_trust(&mut self) -> Result<(), Error> {
        if let Err(error) = self.active() {
            self.runtime_gate.close();
            let _ = self.observe_task_trust(
                None,
                ObservationState::Failed,
                Some(ObservationReason::Credential),
            );
            return Err(error);
        }
        let mut schedule = self.task_trust_state()?;
        if self.task_trust_started && self.now()? < schedule.next_drive {
            return Ok(());
        }
        schedule.next_drive = self.now()?.saturating_add(2);
        self.store.put("task_trust", &schedule)?;
        let _ = self.flush_task_observation().await;
        let state = self.task_trust_state()?;
        let refresh =
            !self.task_trust_started || state.document.is_none() || self.now()? >= state.refresh_at;
        self.task_trust_started = true;
        let result = if refresh {
            // Settle the old immutable ACK before replacing its intent with a newer revision.
            match self.acknowledge_task_trust().await {
                Ok(()) => self.refresh_task_trust().await,
                Err(e) => Err(e),
            }
        } else {
            self.acknowledge_task_trust().await
        };
        if let Err(error) = result {
            let reason = match error {
                Error::Storage => ObservationReason::Storage,
                Error::Unavailable => ObservationReason::Transport,
                Error::Identity
                | Error::CredentialRevoked
                | Error::CredentialRetired
                | Error::CertificateExpired => ObservationReason::Credential,
                _ => ObservationReason::Validation,
            };
            // A failed disk write cannot be represented as applied, even if reporting must wait.
            let _ = self.observe_task_trust(None, ObservationState::Failed, Some(reason));
        }
        self.publish_runtime_gate();
        let _ = self.flush_task_observation().await;
        result
    }
}

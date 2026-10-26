//! Product gate proof bound to a verified backend task, never supplied by IPC or the model.
//! ref: rustls 0.23.45 rustls/src/verify.rs (2976d90): private verification markers.
use super::{offered, request_id};
use agent_client::{Error, Offer, Start};
use execution_contract::*;
use rss_mdm_agent_wire::runtime as wire;

/// Private startup authority derived only from an authenticated payload.
/// Presentation is retained separately for display and confirmation binding.
#[derive(Clone)]
pub(crate) struct TrustedStart {
    revision: Digest,
}
impl TrustedStart {
    pub(crate) fn from_offer(offer: &Offer) -> Option<Self> {
        match offer.payload() {
            wire::TaskPayload::Software(spec)
                if matches!(
                    spec.authorization,
                    wire::TaskAuthorization::SelfService { .. }
                ) && matches!(
                    spec.intent,
                    wire::SoftwareTaskIntent::Install | wire::SoftwareTaskIntent::Uninstall
                ) =>
            {
                Some(Self {
                    revision: offered(offer).ok()?.revision,
                })
            }
            _ => None,
        }
    }
    pub(crate) fn permits(&self, revision: &Digest) -> bool {
        &self.revision == revision
    }
}

/// A decoded backend risk decision tied to its authenticated full offer revision.
/// Backend scripts and built-in software rules have distinct authority identities.
#[derive(Clone)]
pub(crate) struct TrustedRisk {
    revision: Digest,
    decision: ExecutionRisk,
}
impl TrustedRisk {
    pub(crate) fn from_offer(offer: &Offer) -> Option<Self> {
        let revision = offered(offer).ok()?.revision;
        let expires_at_unix_ms = u64::try_from(offer.payload().expires_at())
            .ok()?
            .checked_mul(1000)?;
        let (level, source) = match offer.payload() {
            wire::TaskPayload::Script(spec) => match &spec.authorization {
                wire::TaskAuthorization::SelfService {
                    policy_id,
                    policy_revision,
                    allow_ai: true,
                    risk_level,
                    source: wire::SelfServiceCaller::Ai,
                    ..
                } => (
                    match risk_level {
                        1 => RiskLevel::One,
                        2 => RiskLevel::Two,
                        _ => return None,
                    },
                    RiskSource::Policy {
                        policy: VersionedRef {
                            id: Id::new(policy_id.to_string()).ok()?,
                            revision: Id::new(policy_revision.to_string()).ok()?,
                        },
                    },
                ),
                _ => return None,
            },
            wire::TaskPayload::Enrollment(spec) => match &spec.authorization {
                wire::TaskAuthorization::SelfService {
                    policy_id,
                    policy_revision,
                    allow_ai: true,
                    risk_level,
                    source: wire::SelfServiceCaller::Ai,
                    ..
                } => (
                    match risk_level {
                        1 => RiskLevel::One,
                        2 => RiskLevel::Two,
                        _ => return None,
                    },
                    RiskSource::Policy {
                        policy: VersionedRef {
                            id: Id::new(policy_id.to_string()).ok()?,
                            revision: Id::new(policy_revision.to_string()).ok()?,
                        },
                    },
                ),
                _ => return None,
            },
            wire::TaskPayload::Software(spec) => match &spec.authorization {
                wire::TaskAuthorization::SelfService {
                    policy_id,
                    policy_revision,
                    allow_ai: true,
                    risk_level,
                    source: wire::SelfServiceCaller::Ai,
                    ..
                } => (
                    match risk_level {
                        1 => RiskLevel::One,
                        2 => RiskLevel::Two,
                        _ => return None,
                    },
                    RiskSource::Policy {
                        policy: VersionedRef {
                            id: Id::new(policy_id.to_string()).ok()?,
                            revision: Id::new(policy_revision.to_string()).ok()?,
                        },
                    },
                ),
                _ => return None,
            },
            _ => return None,
        };
        Some(Self {
            revision,
            decision: ExecutionRisk {
                level,
                source,
                expires_at_unix_ms,
            },
        })
    }
    pub(crate) fn decision(&self, revision: &Digest) -> Option<&ExecutionRisk> {
        (&self.revision == revision).then_some(&self.decision)
    }
    #[cfg(test)]
    pub(crate) fn fixture(offer: &Offer, decision: ExecutionRisk) -> Self {
        Self {
            revision: offered(offer).unwrap().revision,
            decision,
        }
    }
}

/// The only evidence that the production product gate was satisfied.
/// Constructed after HTTP returns, from the original current journal and signed Start.
pub(crate) struct ProductGateProof {
    digest: Digest,
    trigger: BackendTrigger,
    until: u64,
}
impl ProductGateProof {
    pub(crate) fn verify_start(
        offer: &Offer,
        start: &Start,
        plan: &FrozenExecution,
        record: Option<&BackendRequest>,
        now: u64,
    ) -> Result<Self, Error> {
        if start.payload().permit() != wire::TaskPermit::Start
            || !start.matches_payload(offer.payload())
            || plan.spec().request.request_id != request_id(offer)?
        {
            return Err(Error::Untrusted);
        }
        let until = u64::try_from(start.payload().expires_at())
            .map_err(|_| Error::Clock)?
            .checked_mul(1000)
            .ok_or(Error::Clock)?;
        if now >= until {
            return Err(Error::Expired);
        }
        let view = offered(offer)?;
        let trigger = if view.user_initiated {
            let record = record.ok_or(Error::Denied)?;
            if record.action.offer() != Some(&view)
                || record.state != BackendRequestState::Submitting
                || !record.valid_product_gate()
            {
                return Err(Error::Denied);
            }
            match &record.trigger {
                BackendTrigger::Human { .. } => record.trigger.clone(),
                BackendTrigger::Ai { .. } => {
                    let authorization = match start.payload() {
                        wire::TaskPayload::Script(p) => &p.authorization,
                        wire::TaskPayload::Software(p) => &p.authorization,
                        wire::TaskPayload::Enrollment(p) => &p.authorization,
                        _ => return Err(Error::Denied),
                    };
                    let wire::TaskAuthorization::SelfService {
                        source: wire::SelfServiceCaller::Ai,
                        allow_ai: true,
                        risk_level,
                        confirmed,
                        ..
                    } = authorization
                    else {
                        return Err(Error::Denied);
                    };
                    if !matches!(risk_level, 1 | 2) || (*risk_level == 2 && !confirmed) {
                        return Err(Error::Denied);
                    }
                    record.trigger.clone()
                }
                _ => return Err(Error::Denied),
            }
        } else {
            // Signed product authorization determines whether an original local request is required.
            if record.is_some() {
                return Err(Error::Denied);
            }
            BackendTrigger::Automatic {}
        };
        if !matches!(&plan.spec().request.initiator, Initiator::Backend { trigger: actual, .. } if actual == &trigger)
        {
            return Err(Error::Untrusted);
        }
        Ok(Self {
            digest: plan.digest().clone(),
            trigger,
            until,
        })
    }
    pub(crate) fn verify(&self, plan: &FrozenExecution, now: u64) -> Result<(), Error> {
        if plan.digest() != &self.digest
            || now >= self.until
            || !matches!(&plan.spec().request.initiator, Initiator::Backend { trigger, .. } if trigger == &self.trigger)
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    pub(crate) fn until(&self) -> u64 {
        self.until
    }
}

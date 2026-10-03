//! Product gate proof bound to a verified backend task, never supplied by IPC or the model.
//! ref: rustls 0.23.45 rustls/src/verify.rs (2976d90): private verification markers.
use super::{offered, request_id};
use agent_client::{wire, Error, Offer, Start};
use execution_admission::{ai_execution_gate, ExecutionGate};
use execution_contract::*;

/// A decoded backend risk decision tied to its authenticated full offer revision.
/// Current wire has no such decision (#2654); formal builds have no alternate source.
#[derive(Clone)]
pub(crate) struct TrustedRisk {
    revision: Digest,
    decision: BackendRiskDecision,
}
impl TrustedRisk {
    pub(crate) fn from_offer(_offer: &Offer) -> Option<Self> {
        // The producer must publish its signed contract before this adapter can decode it.
        None
    }
    pub(crate) fn decision(&self, revision: &Digest) -> Option<&BackendRiskDecision> {
        (&self.revision == revision).then_some(&self.decision)
    }
    #[cfg(test)]
    pub(crate) fn fixture(offer: &Offer, decision: BackendRiskDecision) -> Self {
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
    risk: Option<BackendRiskDecision>,
    until: u64,
}
impl ProductGateProof {
    pub(crate) fn verify_start(
        offer: &Offer,
        start: &Start,
        plan: &FrozenExecution,
        record: Option<&BackendRequest>,
        risk: Option<&TrustedRisk>,
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
        let (trigger, classification, until) = if view.user_initiated {
            let record = record.ok_or(Error::Denied)?;
            if record.offer != view
                || record.state != BackendRequestState::Submitting
                || !record.valid_product_gate()
            {
                return Err(Error::Denied);
            }
            match &record.trigger {
                BackendTrigger::Human { .. } => (record.trigger.clone(), None, until),
                BackendTrigger::Ai { os_session, .. } => {
                    let decision = risk
                        .and_then(|r| r.decision(&view.revision))
                        .ok_or(Error::Denied)?;
                    if record.risk.as_ref() != Some(decision) || now >= decision.expires_at_unix_ms
                    {
                        return Err(Error::Denied);
                    }
                    let until = until.min(decision.expires_at_unix_ms);
                    let until = match ai_execution_gate(Some(decision.level)) {
                        ExecutionGate::Direct if record.confirmation.is_none() => until,
                        ExecutionGate::Confirmation => {
                            let answer = record.confirmation.as_ref().ok_or(Error::Denied)?;
                            if answer.os_session != *os_session
                                || now < answer.confirmed_at_unix_ms
                                || now >= answer.expires_at_unix_ms
                            {
                                return Err(Error::Denied);
                            }
                            until.min(answer.expires_at_unix_ms)
                        }
                        _ => return Err(Error::Denied),
                    };
                    (record.trigger.clone(), Some(decision.clone()), until)
                }
                _ => return Err(Error::Denied),
            }
        } else {
            // Absence of a local record is not sufficient: signed start mode is authoritative.
            if record.is_some() {
                return Err(Error::Denied);
            }
            (BackendTrigger::Automatic {}, None, until)
        };
        if !matches!(&plan.spec().request.initiator, Initiator::Backend { trigger: actual, .. } if actual == &trigger)
        {
            return Err(Error::Untrusted);
        }
        Ok(Self {
            digest: plan.digest().clone(),
            trigger,
            risk: classification,
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
    pub(crate) fn risk(&self) -> Option<RiskLevel> {
        self.risk.as_ref().map(|r| r.level)
    }
    pub(crate) fn until(&self) -> u64 {
        self.until
    }
}

use crate::*;
use execution_contract::{AttemptId, EvidenceRef, FrozenExecution};
use execution_lifecycle::{ObservationError, ObservationFacts, ObservationVerifier};

pub(crate) struct Host<'a, H> {
    inner: &'a H,
    binding: &'a ServiceBinding,
    caller: Option<&'a RequestContext>,
    config: AppConfig,
}
impl<'a, H> Host<'a, H> {
    pub(crate) fn new(
        inner: &'a H,
        binding: &'a ServiceBinding,
        config: &AppConfig,
        caller: Option<&'a RequestContext>,
    ) -> Self {
        Self {
            inner,
            binding,
            caller,
            config: *config,
        }
    }
}
pub(crate) fn check_budget(plan: &FrozenExecution, config: AppConfig) -> Result<(), Error> {
    let budget = plan.spec().budget;
    if budget.total_timeout_ms > config.max_timeout_ms
        || budget.total_output_bytes > config.max_output_bytes
    {
        return Err(Error::Configuration);
    }
    Ok(())
}
pub(crate) struct ObservationEvidence<'a>(pub(crate) &'a ObservationFacts);
impl ObservationVerifier for ObservationEvidence<'_> {
    fn verify(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        evidence: &EvidenceRef,
        now: u64,
    ) -> Result<ObservationFacts, ObservationError> {
        let facts = self.0;
        if facts.request_id != plan.spec().request.request_id
            || &facts.content_digest != plan.digest()
            || &facts.attempt_id != attempt
            || &facts.evidence != evidence
            || facts.observed_at_unix_ms > now
        {
            return Err(ObservationError::Untrusted);
        }
        Ok(facts.clone())
    }
}
impl<H: AppHost> JournalHost for Host<'_, H> {
    fn authorize(&self, request: AccessRequest<'_>) -> Result<(), JournalError> {
        let actual = self
            .inner
            .service_binding()
            .map_err(|_| JournalError::Denied)?;
        if &actual != self.binding
            || self
                .caller
                .is_some_and(|caller| request.scope.actor != caller.actor)
        {
            return Err(JournalError::Denied);
        }
        self.inner
            .binding_for_authority(&request.scope.authority)
            .map_err(|_| JournalError::Denied)?;
        match self.caller {
            Some(caller) => self.inner.authorize(caller, request),
            None => self.inner.authorize_service(request),
        }
    }
    fn reliable_now(&self) -> Result<u64, JournalError> {
        self.inner.reliable_now()
    }
    fn admit(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        now: u64,
    ) -> Result<AdmissionGate, JournalError> {
        check_budget(plan, self.config).map_err(|_| JournalError::Configuration)?;
        self.inner.admit(plan, attempt, now)
    }
}

/// A capture written by the authenticated runner remains readable after its in-memory ack.
pub(crate) fn process_observation(
    plan: &FrozenExecution,
    facts: &execution_contract::ProcessEvidence,
    now: u64,
) -> Option<ObservationFacts> {
    use execution_contract::{EvidenceKind, EvidenceRef, Id, ProcessScope, VersionedRef};
    use execution_lifecycle::Observation;
    if !facts.finished || !facts.quiescent || &facts.content_digest != plan.digest() {
        return None;
    }
    let never = matches!(facts.scope, ProcessScope::NotStarted {});
    let observation = if never {
        Observation::NeverDispatched {
            total_output_bytes: facts.total_output_bytes,
        }
    } else {
        Observation::Exited {
            exit_code: facts.exit_code?,
            total_output_bytes: facts.total_output_bytes,
        }
    };
    Some(ObservationFacts {
        request_id: plan.spec().request.request_id.clone(),
        content_digest: plan.digest().clone(),
        attempt_id: facts.attempt_id.clone(),
        observed_at_unix_ms: now,
        evidence: EvidenceRef {
            reference: VersionedRef {
                id: Id::new(facts.attempt_id.as_str()).ok()?,
                revision: Id::new("process-final").ok()?,
            },
            kind: if never {
                EvidenceKind::StateObserved
            } else {
                EvidenceKind::ProcessExited
            },
            runner: facts.runner.clone(),
        },
        observation,
    })
}

//! Installation-owned identities and permits for code-owned local capabilities.
//! ref: rustls rustls/src/verify.rs@0.23.45 (non-deserializable verification markers).
use agent_client::Error;
use execution_app::{RequestContext, ServiceBinding};
use execution_contract::*;
use sha2::{Digest as _, Sha256};

pub(crate) fn hash(bytes: &[u8]) -> Result<Digest, Error> {
    Digest::new(format!("{:x}", Sha256::digest(bytes))).map_err(|_| Error::Protocol)
}
pub(crate) fn binding(
    service: uuid::Uuid,
    tenant: uuid::Uuid,
    installation: uuid::Uuid,
) -> Result<(ServiceBinding, ActorId), Error> {
    if installation.is_nil() {
        return Err(Error::Configuration);
    }
    let identity = crate::collected_data::digest(&(service, tenant, installation))?;
    Ok((
        ServiceBinding {
            authority: Authority::Local {
                id: Id::new(format!("installation:{}", identity.as_str()))
                    .map_err(|_| Error::Configuration)?,
            },
            device: DeviceId::new(format!("local:{}", identity.as_str()))
                .map_err(|_| Error::Configuration)?,
        },
        ActorId::new(format!("local:{}", identity.as_str())).map_err(|_| Error::Configuration)?,
    ))
}
/// Only the service's fixed native compiler supplies this value, never IPC input.
pub(crate) struct ReadPermit(FrozenExecution);
impl ReadPermit {
    pub(crate) fn new(plan: FrozenExecution) -> Result<Self, Error> {
        if !matches!(plan.spec().request.authority, Authority::Local { .. })
            || plan.spec().launch.interpreter.profile.id.as_str()
                != crate::collection_worker::PROFILE
            || plan.spec().request.operation.action.as_str() != "local-query"
        {
            return Err(Error::Denied);
        }
        Ok(Self(plan))
    }
    pub(crate) fn verify(&self, plan: &FrozenExecution, now: u64) -> Result<(), Error> {
        if plan.digest() != self.0.digest() || now >= plan.spec().validity.expires_at_unix_ms {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
impl crate::service::Core {
    pub(crate) fn local_caller(&self) -> RequestContext {
        RequestContext {
            actor: self.host.local_actor.clone(),
        }
    }
    pub(crate) fn request_device(&self, request: &RequestId) -> Result<DeviceId, Error> {
        match self.app.service_frozen_input(request) {
            Ok(input) => Ok(input.spec().request.target.device.clone()),
            Err(execution_app::Error::NotFound) => Ok(self.host.root.device.clone()),
            Err(error) => Err(crate::error::app_error(error)),
        }
    }
    pub(crate) fn request_caller(&self, request: &RequestId) -> Result<RequestContext, Error> {
        let input = self
            .app
            .service_frozen_input(request)
            .map_err(crate::error::app_error)?;
        Ok(RequestContext {
            actor: input.spec().request.actor.clone(),
        })
    }
}

/// Product effect evidence is constructed only from facts retained by the trusted runner.
struct NativeEffectEvidence {
    plan: FrozenExecution,
    facts: ProcessEvidence,
    assessment: execution_lifecycle::EffectAssessment,
}
impl execution_lifecycle::ObservationVerifier for NativeEffectEvidence {
    fn verify(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        reference: &EvidenceRef,
        now: u64,
    ) -> Result<execution_lifecycle::ObservationFacts, execution_lifecycle::ObservationError> {
        if self.plan.digest() != plan.digest()
            || self.facts.content_digest != *plan.digest()
            || &self.facts.attempt_id != attempt
            || !self.facts.finished
            || !self.facts.quiescent
        {
            return Err(execution_lifecycle::ObservationError::Untrusted);
        }
        Ok(execution_lifecycle::ObservationFacts {
            request_id: plan.spec().request.request_id.clone(),
            content_digest: plan.digest().clone(),
            attempt_id: attempt.clone(),
            evidence: reference.clone(),
            observed_at_unix_ms: now,
            observation: execution_lifecycle::Observation::Effect {
                assessment: self.assessment,
            },
        })
    }
}
impl crate::service::Core {
    pub(crate) fn assess_local(
        &mut self,
        plan: &FrozenExecution,
        facts: ProcessEvidence,
        assessment: execution_lifecycle::EffectAssessment,
    ) -> Result<(), Error> {
        if plan.spec().request.authority != self.host.root.authority {
            return Err(Error::Denied);
        }
        let revision = crate::collected_data::digest(&(
            plan.digest(),
            assessment,
            hash(&facts.stdout)?,
            hash(&facts.stderr)?,
        ))?;
        let reference = crate::backend::plan::reference("local-native-effect", revision.as_str())?;
        let verifier = NativeEffectEvidence {
            plan: plan.clone(),
            facts,
            assessment,
        };
        self.app
            .service_assess(
                &plan.spec().request.request_id,
                &verifier.facts.attempt_id,
                reference,
                &verifier,
            )
            .map_err(crate::error::app_error)?;
        Ok(())
    }
    /// Consume local projections only after their cache/evidence writes have succeeded.
    /// Enterprise receipts are confirmed solely by the signed transport/ACK bridge.
    pub(crate) fn finish_local_result(&mut self, request: &RequestId) -> Result<(), Error> {
        let plan = self
            .app
            .service_frozen_input(request)
            .map_err(crate::error::app_error)?;
        if plan.spec().request.authority != self.host.root.authority {
            return Ok(());
        }
        let profile = plan.spec().launch.interpreter.profile.id.as_str();
        if let Some(facts) = self
            .app
            .service_process_evidence(request)
            .map_err(crate::error::app_error)?
            .filter(|p| p.finished && p.quiescent)
        {
            let assessment = if profile == crate::collection_worker::PROFILE
                && plan.spec().request.operation.action.as_str() == "local-query"
            {
                Some(execution_lifecycle::EffectAssessment::NoEffect)
            } else if profile == crate::local_software::native::PROFILE
                && facts.exit_code == Some(0)
                && facts.quality == OutputQuality::Complete
            {
                serde_json::from_slice::<SoftwareWorkerResult>(&facts.stdout)
                    .ok()
                    .filter(|r| r.closed)
                    .and_then(|r| match plan.spec().request.operation.action.as_str() {
                        "software.detect" | "software.launch" => {
                            Some(execution_lifecycle::EffectAssessment::NoEffect)
                        }
                        "software.uninstall"
                            if matches!(r.detected, Some(SoftwareState::Absent {})) =>
                        {
                            Some(execution_lifecycle::EffectAssessment::Satisfied)
                        }
                        _ => None,
                    })
            } else {
                None
            };
            if let Some(assessment) = assessment {
                self.assess_local(&plan, facts, assessment)?;
            }
        }
        let status = self
            .app
            .status(&self.local_caller(), request)
            .map_err(crate::error::app_error)?;
        if matches!(
            status.phase,
            execution_app::TaskPhase::Verified
                | execution_app::TaskPhase::Cancelled
                | execution_app::TaskPhase::AdmissionDenied
                | execution_app::TaskPhase::FailedBeforeDispatch
        ) {
            let consumer = crate::backend::plan::id(crate::service::RESULT_CONSUMER)?;
            for item in self
                .app
                .service_delivery(request, &consumer, 32)
                .map_err(crate::error::app_error)?
            {
                self.app
                    .service_confirm(request, &consumer, &item.receipt.event_id)
                    .map_err(crate::error::app_error)?;
            }
        }
        Ok(())
    }
}

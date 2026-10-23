//! Compile a verified native plan into the sole runner's fixed service worker.
use super::plan::{self, hex};
use agent_client::{Error, Materials, Offer};
use execution_contract::*;
use execution_runner::{staging::MaterialRoot, Artifacts};
use rss_mdm_agent_wire::runtime as w;
use sha2::{Digest as _, Sha256};

pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    payload: &w::CredentialTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    (config, root): (&crate::ExecutionConfig, &mut MaterialRoot),
    delegate: Option<std::sync::Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let w::TaskPayload::Credential(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if compared != **original
        || payload.device_id != binding.device.as_str()
        || !materials.files().is_empty()
    {
        return Err(Error::Untrusted);
    }
    materials.validate_offer(offer)?;
    payload.validate()?;
    if payload.platform
        != match plan::platform()? {
            Platform::Windows => w::TaskPlatform::Windows,
            Platform::Macos => w::TaskPlatform::Macos,
            Platform::Linux => return Err(Error::Unsupported),
        }
    {
        return Err(Error::Unsupported);
    }
    if delegate.is_some() {
        return Err(Error::Denied);
    }
    let state_root = crate::credential_worker::prepare_root(config)?;
    super::native_worker::compile(
        offer,
        binding,
        actor,
        (config, root),
        super::native_worker::Worker {
            profile: crate::credential_worker::PROFILE,
            argument: "--credential-worker",
            action: "backend-credential",
            artifact: "native-credential-input",
            root_key: "credential-root",
            root: state_root.clone(),
            deadline: payload.deadline,
            expires_at: payload.expires_at,
            definition_digest: payload.definition_digest,
            output: crate::credential_worker::OUTPUT_BYTES,
            stdin: StandardInput::Closed {},
            parameters: Default::default(),
        },
        |image_sha256, run_as, session| {
            serde_json::to_vec(&crate::credential_worker::Input {
                task: payload.clone(),
                root: state_root,
                image_sha256,
                run_as,
                session,
            })
            .map_err(|_| Error::Protocol)
        },
    )
}

pub(crate) fn verify_input(
    plan: &FrozenExecution,
    task: &w::CredentialTaskSpec,
) -> Result<(), Error> {
    let root = match plan.spec().request.parameters.get("credential-root") {
        Some(InputValue::Literal { value }) => {
            serde_json::from_value(value.clone()).map_err(|_| Error::Protocol)?
        }
        _ => return Err(Error::Untrusted),
    };
    let input = crate::credential_worker::Input {
        task: task.clone(),
        root,
        image_sha256: plan.spec().launch.interpreter.artifact.sha256.clone(),
        run_as: plan.spec().run_as.clone(),
        session: plan.spec().session_requirement.clone(),
    };
    if hex(&Sha256::digest(
        serde_json::to_vec(&input).map_err(|_| Error::Protocol)?,
    )) != plan.spec().launch.artifact.sha256.as_str()
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}

/// Proof comes from the system worker's protected result, never stdout or its exit code.
/// ref: tokio/tokio-1.47.1 tokio/src/process/mod.rs (wait only establishes ExitStatus).
pub(super) struct EffectEvidence<'a> {
    pub task: &'a w::CredentialTaskSpec,
    pub attempt: &'a AttemptId,
}
impl execution_lifecycle::ObservationVerifier for EffectEvidence<'_> {
    fn verify(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        evidence: &EvidenceRef,
        now: u64,
    ) -> Result<execution_lifecycle::ObservationFacts, execution_lifecycle::ObservationError> {
        use execution_lifecycle::{
            EffectAssessment, Observation, ObservationError as E, ObservationFacts,
        };
        if attempt != self.attempt
            || evidence.reference.id.as_str() != self.task.task_id.to_string()
            || evidence.reference.revision.as_str() != "credential-result"
        {
            return Err(E::Untrusted);
        }
        let mut started = self.task.clone();
        started.permit = w::TaskPermit::Start;
        let expires = plan
            .spec()
            .validity
            .expires_at_unix_ms
            .checked_sub(plan.spec().budget.total_timeout_ms)
            .ok_or(E::Untrusted)?;
        if !expires.is_multiple_of(1000) {
            return Err(E::Untrusted);
        }
        started.expires_at = i64::try_from(expires / 1000).map_err(|_| E::Untrusted)?;
        verify_input(plan, &started).map_err(|_| E::Untrusted)?;
        let root = crate::credential_worker::plan_root(plan).map_err(|_| E::Untrusted)?;
        let result = crate::credential_worker::stored_result(&root, self.task)
            .map_err(|_| E::Untrusted)?
            .ok_or(E::Unavailable)?;
        let assessment = crate::credential_worker::assessment(&result);
        if assessment == EffectAssessment::Satisfied {
            let escrow = crate::credential_worker::pending(&root, self.task)
                .map_err(|_| E::Untrusted)?
                .ok_or(E::Untrusted)?;
            if escrow.definition_digest != self.task.definition_digest
                || escrow.materials.len() != result.items.len()
                || escrow
                    .materials
                    .iter()
                    .any(|m| !result.items.iter().any(|i| i.subject == m.subject))
            {
                return Err(E::Untrusted);
            }
        }
        Ok(ObservationFacts {
            request_id: plan.spec().request.request_id.clone(),
            content_digest: plan.digest().clone(),
            attempt_id: attempt.clone(),
            evidence: evidence.clone(),
            observed_at_unix_ms: now,
            observation: Observation::Effect { assessment },
        })
    }
}

//! A signed Device Access task is compiled into the shared SYSTEM worker plan.
use super::plan::{self, hex, reference};
use agent_client::{Error, Materials, Offer};
use execution_contract::*;
use execution_runner::{staging::MaterialRoot, Artifacts};
use rss_mdm_agent_wire::runtime as w;
use sha2::{Digest as _, Sha256};

pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    task: &w::AccessTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    resources: (&crate::ExecutionConfig, &mut MaterialRoot),
    access: &crate::service::access::State,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let w::TaskPayload::DeviceAccess(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = task.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if compared != **original
        || task.device_id != binding.device.as_str()
        || !materials.files().is_empty()
    {
        return Err(Error::Untrusted);
    }
    task.validate()?;
    let mut normalized = task.clone();
    normalized.permit = w::TaskPermit::Offer;
    normalized.expires_at = 0;
    let task = &normalized;
    materials.validate_offer(offer)?;
    if !matches!(
        (task.platform, plan::platform()?),
        (w::TaskPlatform::Windows, Platform::Windows) | (w::TaskPlatform::Macos, Platform::Macos)
    ) {
        return Err(Error::Unsupported);
    }
    access.ready(task)?;
    let root = crate::access_worker::root(resources.0);
    let installation = access.installation;
    let mut parameters = std::collections::BTreeMap::new();
    parameters.insert(
        "access-installation".into(),
        InputValue::Literal {
            value: serde_json::to_value(installation).map_err(|_| Error::Protocol)?,
        },
    );
    let create = matches!(task.action, w::AccessAction::Create { .. });
    let controlled =
        create || (task.platform == w::TaskPlatform::Macos && access.password(task).is_ok());
    let input_reference = reference(task.task_id.to_string(), task.attempt_id.to_string())?;
    let (plan, mut artifacts) = super::native_worker::compile(
        offer,
        binding,
        actor,
        resources,
        super::native_worker::Worker {
            profile: crate::access_worker::PROFILE,
            argument: "--device-access-worker",
            action: "backend-device-access",
            artifact: "native-device-access-input",
            root_key: "access-root",
            root: root.clone(),
            deadline: task.deadline,
            definition_digest: task.definition_digest,
            output: crate::access_worker::OUTPUT_BYTES,
            stdin: if controlled {
                StandardInput::Controlled {
                    reference: input_reference,
                    encoding: TextEncoding::Utf8,
                    max_bytes: 1024,
                }
            } else {
                StandardInput::Closed {}
            },
            parameters,
        },
        |image_sha256, run_as, session| {
            serde_json::to_vec(&crate::access_worker::Input {
                task: task.clone(),
                root,
                installation,
                image_sha256,
                run_as,
                session,
            })
            .map_err(|_| Error::Protocol)
        },
    )?;
    if controlled {
        artifacts.controlled_input = Some(access.password(task)?);
    }
    Ok((plan, artifacts))
}
pub(crate) fn verify_input(plan: &FrozenExecution, task: &w::AccessTaskSpec) -> Result<(), Error> {
    let mut normalized = task.clone();
    normalized.permit = w::TaskPermit::Offer;
    normalized.expires_at = 0;
    let task = &normalized;

    let root = crate::access_worker::plan_root(plan)?;
    let installation: uuid::Uuid = match plan.spec().request.parameters.get("access-installation") {
        Some(InputValue::Literal { value }) => {
            serde_json::from_value(value.clone()).map_err(|_| Error::Untrusted)?
        }
        _ => return Err(Error::Untrusted),
    };
    let input = crate::access_worker::Input {
        task: task.clone(),
        root,
        installation,
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
pub(super) struct EffectEvidence<'a> {
    pub task: &'a w::AccessTaskSpec,
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
            EffectAssessment as A, Observation, ObservationError as E, ObservationFacts,
        };
        if attempt != self.attempt
            || evidence.reference.id.as_str() != self.task.task_id.to_string()
            || evidence.reference.revision.as_str() != "access-result"
        {
            return Err(E::Untrusted);
        }
        verify_input(plan, self.task).map_err(|_| E::Untrusted)?;
        let root = crate::access_worker::plan_root(plan).map_err(|_| E::Untrusted)?;
        let result = crate::access_worker::stored_result(&root, self.task)
            .map_err(|_| E::Untrusted)?
            .ok_or(E::Unavailable)?;
        let assessment = if crate::access_worker::complete(self.task, &result.effect) {
            A::Satisfied
        } else if matches!(result.effect, w::AccessEffect::Failed { .. }) {
            A::NoEffect
        } else {
            A::Unknown
        };
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

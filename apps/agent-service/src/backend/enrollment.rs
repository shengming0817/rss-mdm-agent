//! Standard OS enrollment runs through the existing journal, runner and authenticated user helper.
use super::plan::{self, hex, id, reference};
use agent_client::{Error, Materials, NativeEnrollmentMaterial, Offer};
use execution_contract::*;
use execution_runner::{staging::MaterialRoot, Artifacts};
use rss_mdm_agent_wire::runtime as w;
use sha2::{Digest as _, Sha256};
use std::sync::Arc;

pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    payload: &w::EnrollmentTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    root: &mut MaterialRoot,
    delegate: Option<Arc<execution_ipc::helper::Connection>>,
    material: Option<NativeEnrollmentMaterial>,
    profile_bytes: Option<Vec<u8>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let w::TaskPayload::Enrollment(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if compared != *original
        || payload.device_id != binding.device.as_str()
        || !materials.files().is_empty()
    {
        return Err(Error::Untrusted);
    }
    materials.validate_offer(offer)?;
    let helper = delegate.as_ref().ok_or(Error::Unavailable)?;
    let platform = plan::platform()?;
    let account = OsAccountRef {
        platform,
        subject: id(&helper.context().subject)?,
    };
    let run_as = RunAs::User {
        account: account.clone(),
    };
    let session = SessionRequirement::ActiveUser {
        account,
        session: helper.context().binding.clone(),
    };
    let work_root = helper.context().work_root.clone();
    let image = plan::pin(&std::env::current_exe()?)?;
    let profile = profile_bytes
        .map(|bytes| {
            let digest = Digest::new(hex(&Sha256::digest(&bytes))).map_err(|_| Error::Protocol)?;
            let source = root
                .publish(
                    Some(&helper.context().subject),
                    std::io::Cursor::new(&bytes),
                    &digest,
                    bytes.len() as u64,
                )
                .map_err(crate::error::app_error)?;
            let path = root
                .native_export(
                    &source,
                    &digest,
                    "enrollment.mobileconfig",
                    Some(&helper.context().subject),
                )
                .map_err(crate::error::app_error)?;
            Ok::<_, Error>((path, digest))
        })
        .transpose()?;
    let mut parameters = std::collections::BTreeMap::new();
    if let Some((_, digest)) = &profile {
        parameters.insert(
            "nativeProfile".into(),
            InputValue::Literal {
                value: digest.as_str().into(),
            },
        );
    }
    let bytes = serde_json::to_vec(&crate::enrollment_worker::Input {
        task: payload.clone(),
        material,
        profile,
        image_sha256: image.artifact.sha256.clone(),
        run_as: run_as.clone(),
        session: session.clone(),
    })
    .map_err(|_| Error::Protocol)?;
    let hash = hex(&Sha256::digest(&bytes));
    let artifact = ExactArtifactRef {
        resource: reference("native-enrollment-input", &hash)?,
        sha256: Digest::new(hash).map_err(|_| Error::Protocol)?,
    };
    let content = root
        .publish(
            Some(&helper.context().subject),
            std::io::Cursor::new(&bytes),
            &artifact.sha256,
            bytes.len() as u64,
        )
        .map_err(crate::error::app_error)?;
    let now = crate::wall()?;
    let task_hash = hex(&Sha256::digest(
        serde_json::to_vec(payload).map_err(|_| Error::Protocol)?,
    ));
    let frozen = FrozenExecution::freeze(
        ExecutionInput {
            schema_version: V8,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: super::request_id(offer)?,
                authority: binding.authority.clone(),
                actor: actor.clone(),
                initiator: Initiator::Backend {
                    task: id(payload.task_id.to_string())?,
                    attempt: id(payload.attempt_id.to_string())?,
                    trigger: BackendTrigger::Automatic {},
                },
                delegation: None,
                target: Target {
                    device: binding.device.clone(),
                    platform,
                    scope: TargetScope::Device {},
                },
                operation: Operation {
                    action: id("backend-enrollment")?,
                    resource: reference("backend-enrollment", task_hash)?,
                },
                parameters,
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: image.artifact,
                    profile: reference(crate::enrollment_worker::PROFILE, "1")?,
                },
                argv: vec![
                    LaunchArg::Literal {
                        value: "--enrollment-worker".into(),
                    },
                    LaunchArg::ArtifactPath {},
                ],
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output: OutputSpec {
                    format: OutputFormat::Json { max_rows: 1 },
                    stdout: TextEncoding::Utf8,
                    stderr: TextEncoding::Utf8,
                },
                cwd: work_root.to_str().ok_or(Error::Configuration)?.into(),
                env: Default::default(),
            },
            run_as,
            session_requirement: session,
            constraints: IsolationPolicy::OsIdentity {},
            budget: ExecutionBudget {
                total_timeout_ms: 600_000,
                total_output_bytes: 4096,
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: now,
                expires_at_unix_ms: u64::try_from(payload.expires_at)
                    .map_err(|_| Error::Clock)?
                    .checked_add(600)
                    .and_then(|v| v.checked_mul(1000))
                    .ok_or(Error::Clock)?,
            },
            policy: reference(
                format!("backend:{}", payload.task_id),
                payload.attempt_id.to_string(),
            )?,
        },
        &plan::storage_limits().input,
    )
    .map_err(|_| Error::Untrusted)?;
    Ok((
        frozen,
        Artifacts {
            program: vec![],
            delegate,
            interpreter: image.path.into(),
            content,
            work_root,
            controlled_input: None,
        },
    ))
}

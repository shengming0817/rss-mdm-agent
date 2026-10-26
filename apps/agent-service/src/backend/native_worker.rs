//! Shared fixed SYSTEM worker plan; task owners retain authorization and evidence semantics.
use super::plan::{self, hex, id, reference};
use agent_client::{Error, Offer};
use execution_contract::*;
use execution_runner::{staging::MaterialRoot, Artifacts};
use sha2::{Digest as _, Sha256};
pub(super) struct Worker {
    pub profile: &'static str,
    pub argument: &'static str,
    pub action: &'static str,
    pub artifact: &'static str,
    pub root_key: &'static str,
    pub root: std::path::PathBuf,
    pub deadline: i64,
    pub definition_digest: [u8; 32],
    pub output: u64,
    pub stdin: StandardInput,
    pub parameters: std::collections::BTreeMap<String, InputValue>,
}
pub(super) fn compile(
    offer: &Offer,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    (config, root): (&crate::ExecutionConfig, &mut MaterialRoot),
    worker: Worker,
    encode_input: impl FnOnce(Digest, RunAs, SessionRequirement) -> Result<Vec<u8>, Error>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let platform = plan::platform()?;
    let run_as = RunAs::System { platform };
    let session = SessionRequirement::NotRequired {};
    let work_root = config.work_root.clone();
    let state_root = worker.root.clone();

    let seconds = 3600u64;
    let compiled_at_ms = 0;
    let image = plan::pin(&std::env::current_exe()?)?;
    let bytes = encode_input(
        image.artifact.sha256.clone(),
        run_as.clone(),
        session.clone(),
    )?;
    let hash = hex(&Sha256::digest(&bytes));
    let artifact = ExactArtifactRef {
        resource: reference(worker.artifact, &hash)?,
        sha256: Digest::new(hash).map_err(|_| Error::Protocol)?,
    };
    let content = root
        .publish(
            None,
            std::io::Cursor::new(&bytes),
            &artifact.sha256,
            bytes.len() as u64,
        )
        .map_err(crate::error::app_error)?;
    let frozen = FrozenExecution::freeze(
        ExecutionInput {
            schema_version: V1,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: super::request_id(offer)?,
                authority: binding.authority.clone(),
                actor: actor.clone(),
                initiator: Initiator::Backend {
                    task: id(offer.task_id().to_string())?,
                    attempt: id(offer.attempt_id().to_string())?,
                    trigger: BackendTrigger::Automatic {},
                },
                delegation: None,
                target: Target {
                    device: binding.device.clone(),
                    platform,
                    scope: TargetScope::Device {},
                },
                operation: Operation {
                    action: id(worker.action)?,
                    resource: reference("backend-resource", hex(&worker.definition_digest))?,
                },
                parameters: {
                    let mut parameters = worker.parameters;
                    parameters.insert(
                        worker.root_key.into(),
                        InputValue::Literal {
                            value: serde_json::to_value(&state_root)
                                .map_err(|_| Error::Protocol)?,
                        },
                    );
                    parameters
                },
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: image.artifact,
                    profile: reference(worker.profile, "1")?,
                },
                argv: vec![
                    LaunchArg::Literal {
                        value: worker.argument.into(),
                    },
                    LaunchArg::ArtifactPath {},
                ],
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: worker.stdin,
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
                total_timeout_ms: seconds * 1000,
                total_output_bytes: worker.output,
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: compiled_at_ms,
                expires_at_unix_ms: u64::try_from(worker.deadline)
                    .map_err(|_| Error::Clock)?
                    .checked_mul(1000)
                    .ok_or(Error::Clock)?,
            },
            policy: reference(
                format!("backend:{}", offer.task_id()),
                offer.attempt_id().to_string(),
            )?,
        },
        &plan::storage_limits().input,
    )
    .map_err(|_| Error::Untrusted)?;
    Ok((
        frozen,
        Artifacts {
            program: vec![],
            delegate: None,
            interpreter: image.path.into(),
            content,
            work_root,
            controlled_input: None,
        },
    ))
}

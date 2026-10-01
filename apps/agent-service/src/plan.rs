use agent_client::{wire, Error, Materials, Offer};
use execution_contract::*;
use execution_runner::Artifacts;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Administrator-pinned interpreter; requests cannot select a path or a moving version.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interpreter {
    pub profile: wire::ExecutorProfile,
    pub image: local_service::Artifact,
}
/// Locally implemented native managers, independent of the server's software wire schema.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareManagerKind {
    Msi,
    PackageInstaller,
    Winget,
    Brew,
}
/// Protected native package-manager binary selected by a closed V5 executor.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareManager {
    pub executor: SoftwareManagerKind,
    pub image: local_service::Artifact,
}
pub(crate) fn id(value: impl Into<String>) -> Result<Id, Error> {
    Id::new(value).map_err(|_| Error::Protocol)
}
pub(crate) fn reference(
    name: impl Into<String>,
    revision: impl Into<String>,
) -> Result<VersionedRef, Error> {
    Ok(VersionedRef {
        id: id(name)?,
        revision: id(revision)?,
    })
}
pub(crate) fn platform() -> Result<Platform, Error> {
    if cfg!(target_os = "macos") {
        Ok(Platform::Macos)
    } else if cfg!(windows) {
        Ok(Platform::Windows)
    } else {
        Err(Error::Unsupported)
    }
}
pub(crate) fn context(
    origin: &str,
    tenant: uuid::Uuid,
    r: &wire::RegistrationReceipt,
) -> Result<(execution_app::ServiceBinding, ActorId), Error> {
    use sha2::{Digest as _, Sha256};
    let bytes = serde_json::to_vec(&(origin, r.registration_id, r.generation))
        .map_err(|_| Error::Protocol)?;
    Ok((
        execution_app::ServiceBinding {
            authority: Authority::Enterprise {
                id: id(format!("agent-{:x}", Sha256::digest(bytes)))?,
                tenant: id(tenant.to_string())?,
            },
            device: DeviceId::new(&r.device_id).map_err(|_| Error::Protocol)?,
        },
        ActorId::new(format!("registration:{}", r.registration_id)).map_err(|_| Error::Protocol)?,
    ))
}
/// The service's explicit, bounded storage envelope; not fixture defaults.
pub fn storage_limits() -> execution_sqlite::Limits {
    execution_sqlite::Limits {
        input: ExecutionLimits {
            max_input_bytes: 4 * 1024 * 1024,
            max_depth: 48,
            max_nodes: 131072,
            max_string_bytes: 65536,
            max_collection_items: 4096,
            max_timeout_ms: 86_400_000,
            max_output_bytes: 16_777_216,
            max_stdin_bytes: 1_048_576,
            max_attempts: 1,
        },
        lifecycle: execution_lifecycle::Limits {
            max_snapshot_bytes: 65536,
        },
        interaction: execution_interaction_limits(),
        max_approvals: 1,
        max_record_bytes: 8 * 1024 * 1024,
        max_receipts: 100_000,
        max_database_pages: 262144,
        max_consumers: 4,
        max_batch: 64,
        busy_timeout_ms: 1000,
    }
}
fn execution_interaction_limits() -> execution_interaction::Limits {
    execution_interaction::Limits {
        max_snapshot_bytes: 65536,
        max_lifetime_ms: 60000,
    }
}

pub(crate) fn script(
    offer: &Offer,
    materials: &Materials,
    payload: &wire::TaskSpec,
    (binding, actor): (&execution_app::ServiceBinding, &ActorId),
    interpreters: &[Interpreter],
    (work_root, content_path): (&Path, &Path),
    delegate: Option<std::sync::Arc<execution_runner::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let wire::TaskPayload::Script(original) = offer.payload() else {
        return Err(Error::Unsupported);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if &compared != original || payload.device_id != binding.device.as_str() {
        return Err(Error::Untrusted);
    }
    let interpreter = interpreters
        .iter()
        .find(|i| i.profile == payload.profile)
        .ok_or(Error::Unsupported)?;
    interpreter
        .image
        .verify(&interpreter.image.path)
        .map_err(|_| Error::Untrusted)?;
    let content = materials.files().first().ok_or(Error::Untrusted)?;
    if materials.files().len() != 1 {
        return Err(Error::Untrusted);
    }
    let _ = content.reader()?;
    let platform = platform()?;
    let run_as = match payload.run_as {
        wire::ExecutionIdentity::System => RunAs::System { platform },
        wire::ExecutionIdentity::LoggedInUser => RunAs::User {
            account: OsAccountRef {
                platform,
                subject: id(&delegate
                    .as_ref()
                    .ok_or(Error::Unavailable)?
                    .context()
                    .subject)?,
            },
        },
    };
    let policy = reference(
        format!("backend:{}", payload.task_id),
        payload.attempt_id.to_string(),
    )?;
    let resource = reference("backend-resource", hex(&payload.resource_digest))?;
    let artifact = ExactArtifactRef {
        resource: reference("backend-content", hex(&payload.content.sha256))?,
        sha256: Digest::new(hex(&payload.content.sha256)).map_err(|_| Error::Protocol)?,
    };
    let parameters: BTreeMap<_, _> = payload
        .sql_parameters
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|(k, value)| (k, InputValue::Literal { value }))
        .collect();
    let query_arguments = if payload.profile == wire::ExecutorProfile::Osquery {
        use std::io::Read;
        let mut bytes = Vec::new();
        content
            .reader()?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Storage)?;
        if bytes.len() > 65536 {
            return Err(Error::Protocol);
        }
        Some(
            execution_runner::osquery::arguments(&bytes, &parameters, payload.max_rows, platform)
                .map_err(|_| Error::Untrusted)?,
        )
    } else {
        None
    };
    let (profile, prefix) = match payload.profile {
        wire::ExecutorProfile::PowerShell7 => (
            "native-pwsh7-file",
            vec!["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
        ),
        wire::ExecutorProfile::PosixSh => ("native-posix-sh-file", vec![]),
        wire::ExecutorProfile::Bash => ("native-bash-file", vec!["--noprofile", "--norc"]),
        wire::ExecutorProfile::Osquery => (execution_runner::osquery::PROFILE, vec![]),
    };
    let mut argv = prefix
        .into_iter()
        .map(|s| LaunchArg::Literal { value: s.into() })
        .collect::<Vec<_>>();
    if let Some(args) = query_arguments {
        argv = args
            .into_iter()
            .map(|value| LaunchArg::Literal { value })
            .collect();
    } else {
        argv.push(LaunchArg::ArtifactPath {});
    }
    argv.extend(
        payload
            .arguments
            .iter()
            .map(|s| LaunchArg::Literal { value: s.clone() }),
    );
    let env = payload
        .environment
        .iter()
        .map(|(k, v)| {
            Ok((
                EnvironmentKey::new(k).map_err(|_| Error::Protocol)?,
                InputValue::Literal {
                    value: serde_json::Value::String(v.clone()),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    let session_requirement = match &run_as {
        RunAs::User { account } => SessionRequirement::ActiveUser {
            account: account.clone(),
            session: delegate
                .as_ref()
                .ok_or(Error::Unavailable)?
                .context()
                .binding
                .clone(),
        },
        _ => SessionRequirement::NotRequired {},
    };
    let plan = FrozenExecution::freeze(
        ExecutionInput {
            schema_version: V5,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: offer.request_id()?,
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
                    action: id("backend-script")?,
                    resource,
                },
                parameters,
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: ExactArtifactRef {
                        resource: reference(profile, &interpreter.image.sha256)?,
                        sha256: Digest::new(&interpreter.image.sha256)
                            .map_err(|_| Error::Configuration)?,
                    },
                    profile: reference(profile, "1")?,
                },
                argv,
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output: OutputSpec {
                    format: OutputFormat::Json {
                        max_rows: payload.max_rows,
                    },
                    stdout: TextEncoding::Utf8,
                    stderr: TextEncoding::Utf8,
                },
                cwd: work_root.to_str().ok_or(Error::Configuration)?.into(),
                env,
            },
            run_as,
            session_requirement,
            constraints: IsolationPolicy::OsIdentity {},
            budget: ExecutionBudget {
                total_timeout_ms: u64::from(payload.timeout_seconds) * 1000,
                total_output_bytes: u64::from(payload.output_bytes),
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: 0,
                expires_at_unix_ms: u64::try_from(payload.expires_at)
                    .map_err(|_| Error::Clock)?
                    .checked_mul(1000)
                    .and_then(|v| v.checked_add(u64::from(payload.timeout_seconds) * 1000))
                    .ok_or(Error::Clock)?,
            },
            policy,
        },
        &storage_limits().input,
    )
    .map_err(|_| Error::Unsupported)?;
    Ok((
        plan,
        Artifacts {
            program: vec![],
            delegate,
            interpreter: interpreter.image.path.clone(),
            content: content_path.to_owned(),
            work_root: PathBuf::from(work_root),
            controlled_input: None,
        },
    ))
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

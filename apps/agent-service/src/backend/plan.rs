use agent_client::{Error, Materials, Offer};
use execution_contract::*;
use execution_runner::Artifacts;
use rss_mdm_agent_wire::runtime as wire;
use script_plan::{compile_invocation, ScriptInvocationInput, ScriptProfile};
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
    pub image: installation_security::Artifact,
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
/// Protected native package-manager binary selected by a closed V1 executor.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareManager {
    pub executor: SoftwareManagerKind,
    pub image: installation_security::Artifact,
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
    service_id: uuid::Uuid,
    tenant: uuid::Uuid,
    r: &wire::BindingProfile,
) -> Result<(execution_app::ServiceBinding, ActorId), Error> {
    use sha2::{Digest as _, Sha256};
    let bytes = serde_json::to_vec(&(service_id, r.registration_id, r.generation))
        .map_err(|_| Error::Protocol)?;
    Ok((
        execution_app::ServiceBinding {
            authority: Authority::Enterprise {
                id: id(format!("agent-{:x}", Sha256::digest(bytes)))?,
                tenant: id(tenant.to_string())?,
            },
            device: DeviceId::new(&r.device_id).map_err(|_| Error::Protocol)?,
        },
        ActorId::new(format!(
            "registration:{}:{}",
            r.registration_id, r.generation
        ))
        .map_err(|_| Error::Protocol)?,
    ))
}
pub(crate) fn script_profile(profile: wire::ExecutorProfile) -> Result<ScriptProfile, Error> {
    match profile {
        wire::ExecutorProfile::PowerShell7 => Ok(ScriptProfile::PowerShell7),
        wire::ExecutorProfile::PosixSh => Ok(ScriptProfile::PosixSh),
        wire::ExecutorProfile::Bash => Ok(ScriptProfile::Bash),
        wire::ExecutorProfile::Osquery => Err(Error::Unsupported),
    }
}
pub(crate) fn script_interpreter(interpreter: &Interpreter) -> Result<ExactArtifactRef, Error> {
    let profile = script_profile(interpreter.profile)?.reference();
    Ok(ExactArtifactRef {
        resource: reference(profile.id.as_str(), &interpreter.image.sha256)?,
        sha256: Digest::new(&interpreter.image.sha256).map_err(|_| Error::Configuration)?,
    })
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
    delegate: Option<std::sync::Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let wire::TaskPayload::Script(original) = offer.payload() else {
        return Err(Error::Unsupported);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if compared != **original || payload.device_id != binding.device.as_str() {
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
    match (&payload.execution_target, payload.run_as, delegate.as_ref()) {
        (wire::SoftwareExecutionTarget::Device, wire::ExecutionIdentity::System, _) => (),
        (
            wire::SoftwareExecutionTarget::User {
                identity,
                session_id,
            },
            wire::ExecutionIdentity::LoggedInUser,
            Some(helper),
        ) if identity == &helper.context().subject
            && *session_id == crate::service::login_id(helper.context()) => {}
        _ => return Err(Error::Untrusted),
    }
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
    let env = environment(&payload.environment)?;
    let output = OutputSpec {
        format: OutputFormat::Json {
            max_rows: payload.max_rows,
        },
        stdout: TextEncoding::Utf8,
        stderr: TextEncoding::Utf8,
    };
    let cwd = work_root.to_str().ok_or(Error::Configuration)?.into();
    let launch = if let Some(arguments) = query_arguments {
        // Osquery retains its template semantics and independent execution validation.
        LaunchSpec {
            artifact,
            interpreter: InterpreterRef {
                artifact: ExactArtifactRef {
                    resource: reference(
                        execution_runner::osquery::PROFILE,
                        &interpreter.image.sha256,
                    )?,
                    sha256: Digest::new(&interpreter.image.sha256)
                        .map_err(|_| Error::Configuration)?,
                },
                profile: reference(execution_runner::osquery::PROFILE, "1")?,
            },
            argv: arguments
                .into_iter()
                .chain(payload.arguments.iter().cloned())
                .map(|value| LaunchArg::Literal { value })
                .collect(),
            artifact_encoding: ArtifactEncoding::Utf8,
            stdin: StandardInput::Closed {},
            output,
            cwd,
            env,
        }
    } else {
        compile_invocation(
            ScriptInvocationInput {
                platform,
                profile: script_profile(payload.profile)?,
                artifact,
                interpreter: script_interpreter(interpreter)?,
                arguments: payload.arguments.clone(),
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output,
                cwd,
                env,
            },
            &storage_limits().input,
        )
        .map_err(|_| Error::Unsupported)?
    };
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
            schema_version: V1,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: crate::backend::request_id(offer)?,
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
            launch,
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

pub(crate) fn pin(path: &Path) -> Result<SoftwareMaterial, Error> {
    use std::io::Read;
    installation_security::protected(path).map_err(|_| Error::Untrusted)?;
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > 256 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    use sha2::{Digest as _, Sha256};
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut file = file;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let digest: [u8; 32] = hash.finalize().into();
    let reference = ExactArtifactRef {
        resource: reference("backend-material", hex(&digest))?,
        sha256: Digest::new(hex(&digest)).map_err(|_| Error::Protocol)?,
    };
    execution_runner::staging::verify_retained(path, &reference.sha256)
        .map_err(crate::error::app_error)?;
    Ok(SoftwareMaterial {
        path: path.to_str().ok_or(Error::Configuration)?.into(),
        artifact: reference,
    })
}

/// Production always pins its installed service image. Unit tests may explicitly supply
/// a protected, non-native fixture image to test journal/API semantics without OS effects.
pub(crate) fn worker_image(config: &crate::ExecutionConfig) -> Result<SoftwareMaterial, Error> {
    let _ = config;
    #[cfg(test)]
    if let Some(image) = config.worker_image_fixture {
        return image(config);
    }
    pin(&std::env::current_exe()?)
}

/// Product-owned native startup inputs are frozen with the remote RSS_PARAM_ values.
pub(crate) fn environment(
    input: &BTreeMap<String, String>,
) -> Result<BTreeMap<EnvironmentKey, InputValue>, Error> {
    #[cfg(windows)]
    let system_root = Some(
        crate::os_version::system_root()?
            .into_string()
            .map_err(|_| Error::Configuration)?,
    );
    #[cfg(not(windows))]
    let system_root = None;
    freeze_environment(input, system_root)
}

fn freeze_environment(
    input: &BTreeMap<String, String>,
    system_root: Option<String>,
) -> Result<BTreeMap<EnvironmentKey, InputValue>, Error> {
    let mut values = input.clone();
    if let Some(system_root) = system_root {
        values.insert("SystemRoot".into(), system_root);
    }
    values
        .into_iter()
        .map(|(key, value)| {
            Ok((
                EnvironmentKey::new(key).map_err(|_| Error::Protocol)?,
                InputValue::Literal {
                    value: serde_json::Value::String(value),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    #[test]
    fn native_startup_value_is_frozen_without_inheriting_host_environment() {
        let input = BTreeMap::from([
            ("RSS_PARAM_NAME".into(), "value".into()),
            ("SystemRoot".into(), "foreign".into()),
        ]);
        let result = freeze_environment(&input, Some(r"C:\Windows".into())).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[&EnvironmentKey::new("SystemRoot").unwrap()],
            InputValue::Literal {
                value: r"C:\Windows".into()
            }
        );
        assert_eq!(
            result[&EnvironmentKey::new("RSS_PARAM_NAME").unwrap()],
            InputValue::Literal {
                value: "value".into()
            }
        );
    }
}

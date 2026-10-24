//! Deterministic V1 compiler. The backend has already selected sources and ordered dependencies.
use crate::{
    backend::plan::{self, hex, id, reference},
    ExecutionConfig, SoftwareManagerKind,
};
use agent_client::{Error, Materials, Offer};
use execution_contract::*;
use execution_runner::{Artifacts, SoftwareStepArtifacts};
use rss_mdm_agent_wire::runtime as wire;
use script_plan::{compile_invocation, ScriptInvocationInput};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

mod native;

fn package(value: &str) -> Result<PackageValue, Error> {
    PackageValue::new(value).map_err(|_| Error::Protocol)
}
fn artifact(bytes: &[u8; 32]) -> Result<ExactArtifactRef, Error> {
    Ok(ExactArtifactRef {
        resource: reference("backend-material", hex(bytes))?,
        sha256: Digest::new(hex(bytes)).map_err(|_| Error::Protocol)?,
    })
}
type CompiledCommand = (SoftwareInvocation, Artifacts);
struct Compiler<'a> {
    config: &'a ExecutionConfig,
    root: &'a mut execution_runner::staging::MaterialRoot,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
    files: BTreeMap<String, (PathBuf, ExactArtifactRef)>,
    bundle_root: Option<PathBuf>,
}
impl Compiler<'_> {
    fn command(&mut self, script: &wire::SoftwareTaskScript) -> Result<CompiledCommand, Error> {
        let command = &script.invocation;
        let (run_as, session, work_root, delegate) = match command.run_as {
            wire::ExecutionIdentity::System => (
                RunAs::System {
                    platform: plan::platform()?,
                },
                SessionRequirement::NotRequired {},
                self.config.work_root.clone(),
                None,
            ),
            wire::ExecutionIdentity::LoggedInUser => {
                let helper = self.helper.clone().ok_or(Error::Unavailable)?;
                let account = OsAccountRef {
                    platform: plan::platform()?,
                    subject: id(&helper.context().subject)?,
                };
                (
                    RunAs::User {
                        account: account.clone(),
                    },
                    SessionRequirement::ActiveUser {
                        account,
                        session: helper.context().binding.clone(),
                    },
                    helper.context().work_root.clone(),
                    Some(helper),
                )
            }
        };
        let profile = match script.interpreter {
            wire::SoftwareTaskInterpreter::PowerShell7 => wire::ExecutorProfile::PowerShell7,
            wire::SoftwareTaskInterpreter::PosixSh => wire::ExecutorProfile::PosixSh,
            wire::SoftwareTaskInterpreter::Bash => wire::ExecutorProfile::Bash,
        };
        let interpreter = self
            .config
            .interpreters
            .iter()
            .find(|i| i.profile == profile)
            .ok_or(Error::Unsupported)?;
        interpreter
            .image
            .verify(&interpreter.image.path)
            .map_err(|_| Error::Untrusted)?;
        let (content, content_ref) = self
            .files
            .get(&script.entry)
            .cloned()
            .ok_or(Error::Untrusted)?;
        let launch = script_launch(
            command,
            interpreter,
            content_ref,
            command.arguments.clone(),
            self.bundle_root.as_ref().unwrap_or(&work_root),
        )?;
        let invocation = SoftwareInvocation {
            run_as,
            session_requirement: session,
            timeout_ms: u64::from(command.timeout_seconds) * 1000,
            output_bytes: u64::from(command.output_bytes),
            exit_codes: SoftwareExitCodes {
                success: command.exit_codes.success.clone(),
                reboot: command.exit_codes.reboot.clone(),
            },
            launch,
        };
        Ok((
            invocation,
            Artifacts {
                program: Vec::new(),
                delegate,
                interpreter: interpreter.image.path.clone(),
                content,
                work_root,
                controlled_input: None,
            },
        ))
    }
}
// Wire arguments have already been expanded and signed. This conversion has no staging IO.
fn script_launch(
    command: &wire::SoftwareTaskInvocation,
    interpreter: &crate::Interpreter,
    artifact: ExactArtifactRef,
    arguments: Vec<String>,
    cwd: &std::path::Path,
) -> Result<LaunchSpec, Error> {
    compile_invocation(
        ScriptInvocationInput {
            platform: plan::platform()?,
            profile: plan::script_profile(interpreter.profile)?,
            artifact,
            interpreter: plan::script_interpreter(interpreter)?,
            arguments,
            artifact_encoding: ArtifactEncoding::Utf8,
            stdin: StandardInput::Closed {},
            output: OutputSpec {
                format: OutputFormat::Text {},
                stdout: TextEncoding::Utf8,
                stderr: TextEncoding::Utf8,
            },
            cwd: cwd.to_str().ok_or(Error::Configuration)?.into(),
            env: plan::environment(&command.environment)?,
        },
        &plan::storage_limits().input,
    )
    .map_err(|_| Error::Unsupported)
}
pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    payload: &wire::SoftwareTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    (config, root): (
        &ExecutionConfig,
        &mut execution_runner::staging::MaterialRoot,
    ),
    helper: Option<Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let wire::TaskPayload::Software(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if &compared != original.as_ref() || payload.device_id != binding.device.as_str() {
        return Err(Error::Untrusted);
    }
    payload.validate().map_err(|_| Error::Protocol)?;
    let platform = plan::platform()?;
    let architecture = package(match payload.architecture {
        wire::TaskArchitecture::Aarch64 => "aarch64",
        wire::TaskArchitecture::X86_64 => "x86_64",
    })?;
    let intent = match payload.intent {
        wire::SoftwareTaskIntent::Install => SoftwareOperation::Install,
        wire::SoftwareTaskIntent::Uninstall => SoftwareOperation::Uninstall,
        wire::SoftwareTaskIntent::Detect => SoftwareOperation::Detect,
    };
    let mut steps = Vec::new();
    let mut sources = Vec::new();
    for (index, step) in payload.steps.iter().enumerate() {
        match (&step.target, helper.as_ref()) {
            (wire::SoftwareExecutionTarget::Device, _) => (),
            (
                wire::SoftwareExecutionTarget::User {
                    identity,
                    session_id,
                },
                Some(h),
            ) if identity == &h.context().subject
                && *session_id == crate::service::login_id(h.context()) => {}
            _ => return Err(Error::Identity),
        }

        let (compiled, source) = native::compile(
            (step, index),
            payload,
            materials,
            (config, root),
            helper.clone(),
            intent,
            (&architecture, plan::worker_image(config)?),
        )?;
        steps.push(compiled);
        sources.push(source);
    }
    let (timeout, output) = crate::backend::commands::software_budget(payload)?;
    let mut controller = steps.first().ok_or(Error::Protocol)?.install.launch.clone();
    controller.interpreter.profile = reference("native-software-sequence", "1")?;
    let program = SoftwareProgram {
        definition_digest: Digest::new(hex(&payload.definition_digest))
            .map_err(|_| Error::Protocol)?,
        intent,
        steps,
    };
    let input = ExecutionInput {
        schema_version: V1,
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
                action: id(match intent {
                    SoftwareOperation::Install => "software.install",
                    SoftwareOperation::Uninstall => "software.uninstall",
                    SoftwareOperation::Detect => "software.detect",
                })?,
                resource: reference("backend-software", hex(&payload.definition_digest))?,
            },
            parameters: BTreeMap::new(),
        },
        execution: ExecutionSpec::SoftwareProgram {
            program: Box::new(program),
        },
        launch: controller,
        run_as: RunAs::System { platform },
        session_requirement: SessionRequirement::NotRequired {},
        constraints: IsolationPolicy::OsIdentity {},
        budget: ExecutionBudget {
            total_timeout_ms: timeout,
            total_output_bytes: output,
            max_attempts: 1,
        },
        validity: ValidityWindow {
            not_before_unix_ms: 0,
            expires_at_unix_ms: u64::try_from(payload.expires_at)
                .map_err(|_| Error::Clock)?
                .checked_mul(1000)
                .and_then(|v| v.checked_add(timeout))
                .ok_or(Error::Clock)?,
        },
        policy: reference(
            format!("backend:{}", payload.task_id),
            payload.attempt_id.to_string(),
        )?,
    };
    let frozen = FrozenExecution::freeze(input, &plan::storage_limits().input)
        .map_err(|_| Error::Untrusted)?;
    Ok((
        frozen,
        Artifacts {
            program: sources,
            delegate: helper,
            interpreter: PathBuf::new(),
            content: PathBuf::new(),
            work_root: config.work_root.clone(),
            controlled_input: None,
        },
    ))
}
#[cfg(test)]
mod review_tests {
    use super::*;
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn signed_software_and_wrapper_arguments_keep_original_calling_facts() {
        for (profile, name, prefix) in [
            (
                wire::ExecutorProfile::PowerShell7,
                "native-pwsh7-file",
                vec!["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
            ),
            (
                wire::ExecutorProfile::PosixSh,
                "native-posix-sh-file",
                vec![],
            ),
            (
                wire::ExecutorProfile::Bash,
                "native-bash-file",
                vec!["--noprofile", "--norc"],
            ),
        ] {
            if cfg!(windows) && profile != wire::ExecutorProfile::PowerShell7 {
                continue;
            }
            let interpreter = crate::Interpreter {
                profile,
                image: installation_security::Artifact {
                    path: "host-pinned-interpreter".into(),
                    sha256: "12".repeat(32),
                    cdhash: None,
                },
            };
            let command = wire::SoftwareTaskInvocation {
                run_as: wire::ExecutionIdentity::System,
                arguments: vec![
                    "".into(),
                    "-Name:".into(),
                    "true".into(),
                    " a; $(x) ".into(),
                ],
                environment: [("RSS_PARAM_VALUE".into(), " literal ; $(x) ".into())].into(),
                timeout_seconds: 30,
                output_bytes: 4096,
                exit_codes: wire::SoftwareTaskExitCodes {
                    success: [0].into(),
                    reboot: [3010].into(),
                },
            };
            for wrapper in [false, true] {
                let mut arguments = if wrapper {
                    vec![
                        "/exact/manager".into(),
                        "/exact/payload".into(),
                        "package".into(),
                        "version".into(),
                    ]
                } else {
                    vec![]
                };
                arguments.extend(command.arguments.clone());
                let material = artifact(&[3; 32]).unwrap();
                let cwd = if cfg!(windows) {
                    std::path::Path::new(r"C:\work")
                } else {
                    std::path::Path::new("/work")
                };
                let launch = script_launch(
                    &command,
                    &interpreter,
                    material.clone(),
                    arguments.clone(),
                    cwd,
                )
                .unwrap();
                let expected_argv: Vec<_> = prefix
                    .iter()
                    .map(|s| LaunchArg::Literal { value: (*s).into() })
                    .chain(Some(LaunchArg::ArtifactPath {}))
                    .chain(
                        arguments
                            .into_iter()
                            .map(|value| LaunchArg::Literal { value }),
                    )
                    .collect();
                assert_eq!(launch.argv, expected_argv);
                assert_eq!(launch.interpreter.profile, reference(name, "1").unwrap());
                assert_eq!(
                    launch.interpreter.artifact.resource,
                    reference(name, &interpreter.image.sha256).unwrap()
                );
                assert_eq!(launch.artifact, material);
                assert_eq!(launch.cwd, cwd.to_str().unwrap());
                assert_eq!(
                    launch.env[&EnvironmentKey::new("RSS_PARAM_VALUE").unwrap()],
                    InputValue::Literal {
                        value: serde_json::json!(" literal ; $(x) ")
                    }
                );
                assert_eq!(launch.stdin, StandardInput::Closed {});
                assert_eq!(launch.output.format, OutputFormat::Text {});
            }
        }
    }
}

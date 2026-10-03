//! Frozen physical recipes for EXE, MSIX and selected DMG payloads.
use super::*;
use crate::software_worker::{WorkerOperation as O, WorkerRequest};
use std::{io::Read, path::Path};

fn pin(path: &Path) -> Result<SoftwareMaterial, Error> {
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
    let reference = artifact(&digest)?;
    execution_runner::staging::verify_retained(path, &reference.sha256)?;
    Ok(SoftwareMaterial {
        path: path.to_str().ok_or(Error::Configuration)?.into(),
        artifact: reference,
    })
}
type NativeIdentity = (
    RunAs,
    SessionRequirement,
    PathBuf,
    Option<Arc<execution_ipc::helper::Connection>>,
);
fn identity(
    config: &ExecutionConfig,
    helper: Option<&Arc<execution_ipc::helper::Connection>>,
    command: &wire::SoftwareTaskInvocation,
) -> Result<NativeIdentity, Error> {
    match command.run_as {
        wire::ExecutionIdentity::System => Ok((
            RunAs::System {
                platform: plan::platform()?,
            },
            SessionRequirement::NotRequired {},
            config.work_root.clone(),
            None,
        )),
        wire::ExecutionIdentity::LoggedInUser => {
            let helper = helper.cloned().ok_or(Error::Unavailable)?;
            let account = OsAccountRef {
                platform: plan::platform()?,
                subject: id(&helper.context().subject)?,
            };
            Ok((
                RunAs::User {
                    account: account.clone(),
                },
                SessionRequirement::ActiveUser {
                    account,
                    session: helper.context().binding.clone(),
                },
                helper.context().work_root.clone(),
                Some(helper),
            ))
        }
    }
}
fn command(
    compiler: &Compiler<'_>,
    mut request: WorkerRequest,
    command: &wire::SoftwareTaskInvocation,
    image: &SoftwareMaterial,
) -> Result<(SoftwareInvocation, Artifacts), Error> {
    let (run_as, session, work_root, delegate) =
        identity(compiler.config, compiler.helper.as_ref(), command)?;
    request.run_as = run_as.clone();
    request.session = session.clone();
    request.output_bytes = u64::from(command.output_bytes);
    let bytes = serde_json::to_vec(&request).map_err(|_| Error::Protocol)?;
    use sha2::{Digest as _, Sha256};
    let hash: [u8; 32] = Sha256::digest(&bytes).into();
    let content_ref = artifact(&hash)?;
    let content = execution_runner::staging::publish(
        &compiler.config.material_root,
        delegate.as_ref().map(|h| h.context().subject.as_str()),
        std::io::Cursor::new(&bytes),
        &content_ref.sha256,
        bytes.len() as u64,
    )?;
    let invocation = SoftwareInvocation {
        launch: LaunchSpec {
            artifact: content_ref,
            interpreter: InterpreterRef {
                profile: reference("native-software-worker", "1")?,
                artifact: image.artifact.clone(),
            },
            argv: vec![
                LaunchArg::Literal {
                    value: "--software-worker".into(),
                },
                LaunchArg::ArtifactPath {},
            ],
            artifact_encoding: ArtifactEncoding::Utf8,
            stdin: if matches!(
                request.operation,
                O::Install | O::Upgrade | O::Uninstall | O::RemovePrevious
            ) {
                StandardInput::Controlled {
                    reference: reference("software-before", request.step.to_string())?,
                    encoding: TextEncoding::Utf8,
                    max_bytes: 4096,
                }
            } else {
                StandardInput::Closed {}
            },
            output: OutputSpec {
                format: OutputFormat::Text {},
                stdout: TextEncoding::Utf8,
                stderr: TextEncoding::Utf8,
            },
            cwd: work_root.to_str().ok_or(Error::Configuration)?.into(),
            env: command
                .environment
                .iter()
                .map(|(key, value)| {
                    Ok((
                        EnvironmentKey::new(key).map_err(|_| Error::Protocol)?,
                        InputValue::Literal {
                            value: serde_json::Value::String(value.clone()),
                        },
                    ))
                })
                .collect::<Result<_, Error>>()?,
        },
        run_as,
        session_requirement: session,
        timeout_ms: u64::from(command.timeout_seconds) * 1000,
        output_bytes: u64::from(command.output_bytes),
        exit_codes: SoftwareExitCodes {
            success: command.exit_codes.success.clone(),
            reboot: command.exit_codes.reboot.clone(),
        },
    };
    Ok((
        invocation,
        Artifacts {
            program: Vec::new(),
            delegate,
            interpreter: PathBuf::from(&image.path),
            content,
            work_root,
            controlled_input: None,
        },
    ))
}
fn msix_identity(v: &wire::SoftwareTaskMsixIdentity) -> Result<MsixIdentity, Error> {
    Ok(MsixIdentity {
        name: package(&v.name)?,
        publisher: package(&v.publisher)?,
        version: v.version,
        architecture: package(match v.architecture {
            wire::SoftwareTaskMsixArchitecture::X86_64 => "x86_64",
            wire::SoftwareTaskMsixArchitecture::Aarch64 => "aarch64",
            wire::SoftwareTaskMsixArchitecture::Neutral => "neutral",
        })?,
        resource_id: v.resource_id.clone(),
    })
}
pub(super) fn compile(
    (step, index): (&wire::SoftwareTaskStep, usize),
    payload: &wire::SoftwareTaskSpec,
    materials: &Materials,
    config: &ExecutionConfig,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
    intent: SoftwareOperation,
    architecture: &PackageValue,
) -> Result<(SoftwareProgramStep, SoftwareStepArtifacts), Error> {
    let action = &step.action;
    let commands = agent_client::software_commands(action)?;
    if let wire::SoftwareTaskBehavior::Exe(exe) = &action.behavior {
        if intent != SoftwareOperation::Detect {
            let key = if intent == SoftwareOperation::Uninstall {
                &exe.uninstall.as_ref().ok_or(Error::Unsupported)?.installer
            } else {
                &exe.installer
            };
            if !action.signatures.iter().any(|signature| {
                signature.artifact == *key
                    && signature.mechanism == wire::SoftwareTaskSignatureMechanism::Authenticode
            }) {
                return Err(Error::Unsupported);
            }
        }
    }

    let mut compiler = Compiler {
        config,
        helper: helper.clone(),
        files: BTreeMap::new(),
        bundle_root: None,
    };
    for declared in &step.artifacts {
        let file = materials
            .files()
            .iter()
            .find(|f| f.key() == declared.key)
            .ok_or(Error::Untrusted)?;
        let reference = artifact(&declared.sha256)?;
        let path = execution_runner::staging::publish(
            &config.material_root,
            helper.as_ref().map(|h| h.context().subject.as_str()),
            file.reader()?,
            &reference.sha256,
            declared.length,
        )?;
        let key = declared
            .key
            .strip_prefix(&format!("{index}/"))
            .ok_or(Error::Protocol)?
            .to_owned();
        compiler.files.insert(key, (path, reference));
    }
    let image = pin(&std::env::current_exe()?)?;
    let mut tools = BTreeMap::new();
    match &action.behavior {
        wire::SoftwareTaskBehavior::Dmg(d) => {
            for (name, path) in [
                ("diskutil", "/usr/sbin/diskutil"),
                ("hdiutil", "/usr/bin/hdiutil"),
                ("codesign", "/usr/bin/codesign"),
                ("spctl", "/usr/sbin/spctl"),
                ("lipo", "/usr/bin/lipo"),
            ] {
                tools.insert(name.into(), pin(Path::new(path))?);
            }
            if matches!(d.payload, wire::SoftwareTaskDmgPayload::ContainedPkg { .. }) {
                let manager = config
                    .managers
                    .iter()
                    .find(|m| m.executor == SoftwareManagerKind::PackageInstaller)
                    .ok_or(Error::Unsupported)?;
                manager
                    .image
                    .verify(&manager.image.path)
                    .map_err(|_| Error::Untrusted)?;
                tools.insert("installer".into(), pin(&manager.image.path)?);
                tools.insert("pkgutil".into(), pin(Path::new("/usr/sbin/pkgutil"))?);
            }
        }
        wire::SoftwareTaskBehavior::Exe(_) | wire::SoftwareTaskBehavior::Msix(_) => {
            if !action.signatures.is_empty() {
                let interpreter = config
                    .interpreters
                    .iter()
                    .find(|i| i.profile == wire::ExecutorProfile::PowerShell7)
                    .ok_or(Error::Unsupported)?;
                interpreter
                    .image
                    .verify(&interpreter.image.path)
                    .map_err(|_| Error::Untrusted)?;
                tools.insert("pwsh".into(), pin(&interpreter.image.path)?);
                let script = b"param([string]$Path,[string]$Publisher)\n$ErrorActionPreference='Stop'\n$s=Get-AuthenticodeSignature -LiteralPath $Path\nif($s.Status -ne 'Valid' -or $null -eq $s.SignerCertificate -or $s.SignerCertificate.Subject -cne $Publisher){exit 1}\nexit 0\n";
                use sha2::{Digest as _, Sha256};
                let digest: [u8; 32] = Sha256::digest(script).into();
                let reference = artifact(&digest)?;
                let path = execution_runner::staging::publish(
                    &config.material_root,
                    helper.as_ref().map(|h| h.context().subject.as_str()),
                    std::io::Cursor::new(script),
                    &reference.sha256,
                    script.len() as u64,
                )?;
                tools.insert(
                    "authenticode".into(),
                    SoftwareMaterial {
                        path: path.to_str().ok_or(Error::Configuration)?.into(),
                        artifact: reference,
                    },
                );
            }
        }
        _ => return Err(Error::Unsupported),
    }
    let primary = compiler
        .files
        .get(action.behavior.installer())
        .ok_or(Error::Untrusted)?
        .1
        .clone();
    let format = match &action.behavior {
        wire::SoftwareTaskBehavior::Exe(exe) => SoftwareFormat::Exe {
            ownership: exe_ownership(&exe.detect, &action.package)?,
            layout: exe
                .layout
                .iter()
                .map(|(path, key)| {
                    Ok((
                        path.clone(),
                        compiler.files.get(key).ok_or(Error::Untrusted)?.1.clone(),
                    ))
                })
                .collect::<Result<_, Error>>()?,
        },
        wire::SoftwareTaskBehavior::Msix(msix) => SoftwareFormat::Msix {
            container: match &msix.container {
                wire::SoftwareTaskMsixContainer::Package { .. } => MsixContainer::Package {},
                wire::SoftwareTaskMsixContainer::Bundle { members, .. } => MsixContainer::Bundle {
                    members: members
                        .iter()
                        .map(|m| {
                            Ok(MsixMember {
                                path: m.path.clone(),
                                identity: msix_identity(&m.identity)?,
                                length: m.length,
                                sha256: Digest::new(hex(&m.sha256)).map_err(|_| Error::Protocol)?,
                            })
                        })
                        .collect::<Result<_, Error>>()?,
                },
            },
            identity: msix_identity(&msix.identity)?,
            dependencies: msix
                .dependencies
                .iter()
                .map(msix_identity)
                .collect::<Result<_, _>>()?,
            deployment: match msix.deployment {
                wire::SoftwareTaskMsixDeployment::DeviceProvisioning => {
                    MsixDeployment::DeviceProvisioning {}
                }
                wire::SoftwareTaskMsixDeployment::TargetUserRegistration { .. } => {
                    MsixDeployment::TargetUserRegistration {}
                }
            },
            minimum_os: msix.minimum_os,
            require_sideload: msix.require_sideload,
            allow_unsigned: msix.allow_unsigned,
        },
        wire::SoftwareTaskBehavior::Dmg(dmg) => match &dmg.payload {
            wire::SoftwareTaskDmgPayload::AppCopy { application, .. } => SoftwareFormat::DmgApp {
                volume: package(&dmg.volume)?,
                path: application.path.clone(),
                bundle_id: package(&application.bundle_id)?,
                version: package(&application.version)?,
                target_name: application.target_name.clone(),
            },
            wire::SoftwareTaskDmgPayload::ContainedPkg {
                path,
                length,
                sha256,
                receipt,
                ..
            } => SoftwareFormat::DmgPkg {
                volume: package(&dmg.volume)?,
                path: path.clone(),
                length: *length,
                sha256: Digest::new(hex(sha256)).map_err(|_| Error::Protocol)?,
                receipt: package(receipt)?,
            },
        },
        _ => return Err(Error::Unsupported),
    };
    let source_command = if intent == SoftwareOperation::Uninstall {
        commands.uninstall.ok_or(Error::Unsupported)?
    } else {
        commands.install
    };
    let (run_as, session, work_root, _) = identity(config, helper.as_ref(), source_command)?;
    let request = WorkerRequest {
        external_pending: Default::default(),
        native_pending: Default::default(),
        native_output: Default::default(),
        action: action.clone(),
        operation: O::Install,
        materials: compiler
            .files
            .iter()
            .map(|(key, (path, reference))| {
                Ok((
                    key.clone(),
                    SoftwareMaterial {
                        path: path.to_str().ok_or(Error::Configuration)?.into(),
                        artifact: reference.clone(),
                    },
                ))
            })
            .collect::<Result<_, Error>>()?,
        tools: tools.clone(),
        resource_root: work_root.join(format!("software-{}-{index}", payload.attempt_id)),
        run_as,
        session,
        architecture: payload.architecture,
        output_bytes: u64::from(source_command.output_bytes),
        step: index.try_into().map_err(|_| Error::Protocol)?,
    };
    let build = |operation, invocation| {
        let mut request = request.clone();
        request.operation = operation;
        command(&compiler, request, invocation, &image)
    };
    let (install, install_source) = build(O::Install, commands.install)?;
    let (update, update_source) = build(O::Upgrade, commands.upgrade.unwrap_or(commands.install))?;
    let removal = commands
        .uninstall
        .map(|c| build(O::Uninstall, c))
        .transpose()?;
    let uninstall = removal.as_ref().map(|(c, _)| c.clone());
    let mut auxiliary = BTreeMap::new();
    let mut mutations = BTreeMap::new();
    if matches!(
        format,
        SoftwareFormat::DmgApp { .. } | SoftwareFormat::DmgPkg { .. }
    ) {
        let mut phases = vec![
            (SoftwarePhase::Attach, O::Attach),
            (SoftwarePhase::Cleanup, O::Cleanup),
        ];
        if matches!(format, SoftwareFormat::DmgApp { .. }) {
            phases.push((SoftwarePhase::Stage, O::Stage));
        }
        for (phase, operation) in phases {
            let (mut command, source) = build(operation, source_command)?;
            if phase == SoftwarePhase::Cleanup {
                // Reserve closure within the original source invocation before any image effect.
                command.timeout_ms = (command.timeout_ms / 4).min(30_000);
                command.output_bytes = (command.output_bytes / 4).min(65_536);
                if command.timeout_ms == 0 || command.output_bytes < 1024 {
                    return Err(Error::Unsupported);
                }
            }
            command.exit_codes = SoftwareExitCodes {
                success: [0].into(),
                reboot: Default::default(),
            };
            auxiliary.insert(phase, command);
            mutations.insert(phase, Box::new(source));
        }
    }
    match intent {
        SoftwareOperation::Install => {
            mutations.insert(SoftwarePhase::Mutation, Box::new(install_source));
            mutations.insert(SoftwarePhase::Upgrade, Box::new(update_source));
            if commands.upgrade_policy == wire::SoftwareTaskUpgrade::UninstallThenInstall {
                let c = commands.uninstall.ok_or(Error::Unsupported)?;
                let (removal, source) = build(O::RemovePrevious, c)?;
                auxiliary.insert(SoftwarePhase::Removal, removal);
                mutations.insert(SoftwarePhase::Removal, Box::new(source));
            }
        }
        SoftwareOperation::Uninstall => {
            let (_, source) = removal.clone().ok_or(Error::Unsupported)?;
            mutations.insert(SoftwarePhase::Mutation, Box::new(source));
        }
        SoftwareOperation::Detect => (),
    }
    let (detection, detector_source) =
        if let Some(wire::SoftwareTaskDetection::Script { command: script }) = commands.detection {
            let (command, source, _) = compiler.command(
                CommandKind::script(script),
                format.adapter(),
                compiler
                    .files
                    .get(action.behavior.installer())
                    .ok_or(Error::Untrusted)?,
                SoftwareOperation::Detect,
                &action.version,
                &action.package,
            )?;
            (
                SoftwareDetector::Script {
                    invocation: Box::new(command),
                },
                source,
            )
        } else {
            let (mut detector, source) = build(O::Detect, commands.install)?;
            detector.exit_codes = SoftwareExitCodes {
                success: [0].into(),
                reboot: Default::default(),
            };
            (
                SoftwareDetector::Script {
                    invocation: Box::new(detector),
                },
                source,
            )
        };
    let mut files = request
        .materials
        .values()
        .chain(tools.values())
        .cloned()
        .collect::<Vec<_>>();
    files.push(image);
    let signatures = action
        .signatures
        .iter()
        .map(|s| {
            let artifact = compiler
                .files
                .get(&s.artifact)
                .ok_or(Error::Untrusted)?
                .1
                .clone();
            let publisher = package(&s.publisher)?;
            Ok(match s.mechanism {
                wire::SoftwareTaskSignatureMechanism::Authenticode => {
                    SoftwareSignature::Authenticode {
                        artifact,
                        publisher,
                    }
                }
                wire::SoftwareTaskSignatureMechanism::AppleDeveloperId => {
                    SoftwareSignature::AppleDeveloperId {
                        artifact,
                        publisher,
                    }
                }
                wire::SoftwareTaskSignatureMechanism::Msix => SoftwareSignature::Msix {
                    artifact,
                    publisher,
                },
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok((
        SoftwareProgramStep {
            format,
            package: package(&action.package)?,
            version: package(&action.version)?,
            architecture: architecture.clone(),
            payload: primary,
            materials: files.clone(),
            signatures,
            install,
            auxiliary,
            uninstall,
            detection,
            upgrade: match commands.upgrade_policy {
                wire::SoftwareTaskUpgrade::InPlace => SoftwareUpgrade::InPlace {
                    invocation: Box::new(update),
                },
                wire::SoftwareTaskUpgrade::UninstallThenInstall => {
                    SoftwareUpgrade::UninstallThenInstall {}
                }
                wire::SoftwareTaskUpgrade::Deny => SoftwareUpgrade::Deny {},
            },
            existing: match action.ownership {
                wire::SoftwareTaskOwnership::ManagedOnly => ExistingSoftware::ManagedOnly,
                wire::SoftwareTaskOwnership::AllowUserExisting => {
                    ExistingSoftware::AllowUserExisting
                }
            },
            allow_downgrade: action.downgrade == wire::SoftwareTaskDowngrade::Allow,
            allow_reboot: action.reboot == wire::SoftwareTaskReboot::Report,
        },
        SoftwareStepArtifacts {
            mutations,
            detection: Some(Box::new(detector_source)),
            files: files
                .into_iter()
                .map(|m| (PathBuf::from(m.path), m.artifact))
                .collect(),
        },
    ))
}

fn exe_ownership(detector: &wire::SoftwareTaskDetection, package: &str) -> Result<Digest, Error> {
    let selector = match detector {
        wire::SoftwareTaskDetection::Registry {
            scope, key, value, ..
        } => serde_json::to_vec(&("registry", scope, key, value)),
        wire::SoftwareTaskDetection::File { scope, path, .. } => {
            serde_json::to_vec(&("file", scope, path))
        }
        wire::SoftwareTaskDetection::Script { .. } => serde_json::to_vec(&("script", package)),
        _ => return Err(Error::Unsupported),
    }
    .map_err(|_| Error::Protocol)?;
    use sha2::{Digest as _, Sha256};
    Digest::new(format!("{:x}", Sha256::digest(selector))).map_err(|_| Error::Protocol)
}

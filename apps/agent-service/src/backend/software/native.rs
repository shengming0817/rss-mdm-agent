//! One native worker recipe for every platform installer and package manager.
use super::*;
use crate::software_worker::{WorkerOperation as O, WorkerRequest};
use rss_mdm_agent_wire::runtime as wire;
use std::path::Path;

use super::plan::pin;
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
    compiler: &mut Compiler<'_>,
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
    let content = compiler
        .root
        .publish(
            delegate.as_ref().map(|h| h.context().subject.as_str()),
            std::io::Cursor::new(&bytes),
            &content_ref.sha256,
            bytes.len() as u64,
        )
        .map_err(crate::error::app_error)?;
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
                O::Prepare | O::Install | O::Upgrade | O::Uninstall | O::RemovePrevious
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
            env: plan::environment(&command.environment)?,
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
    (config, root): (
        &ExecutionConfig,
        &mut execution_runner::staging::MaterialRoot,
    ),
    helper: Option<Arc<execution_ipc::helper::Connection>>,
    intent: SoftwareOperation,
    (architecture, image): (&PackageValue, SoftwareMaterial),
) -> Result<(SoftwareProgramStep, SoftwareStepArtifacts), Error> {
    let action = &step.action;
    let commands = crate::backend::commands::software_commands(action)?;
    let mut compiler = Compiler {
        config,
        root,
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
        let path = compiler
            .root
            .publish(
                helper.as_ref().map(|h| h.context().subject.as_str()),
                file.reader()?,
                &reference.sha256,
                declared.length,
            )
            .map_err(crate::error::app_error)?;
        let key = declared
            .key
            .strip_prefix(&format!("{index}/"))
            .ok_or(Error::Protocol)?
            .to_owned();
        compiler.files.insert(key, (path, reference));
    }
    let platform = plan::platform()?;
    let bundle = action.behavior.bundle().map(|b| BundleManifest {
        schema: V1,
        platform,
        architecture: architecture.clone(),
        entries: b
            .entries
            .iter()
            .map(|(name, entry)| {
                (
                    name.clone(),
                    BundleFile {
                        length: entry.length,
                        sha256: entry.sha256,
                    },
                )
            })
            .collect(),
    });
    if let Some(manifest) = &bundle {
        let primary = compiler
            .files
            .get(action.behavior.installer())
            .cloned()
            .ok_or(Error::Untrusted)?;
        let members = compiler
            .root
            .bundle(
                helper.as_ref().map(|h| h.context().subject.as_str()),
                &primary.0,
                &primary.1.sha256,
                manifest,
            )
            .map_err(crate::error::app_error)?;
        let entry = &commands.install_script.ok_or(Error::Protocol)?.entry;
        let depth = Path::new(entry).components().count();
        let path = members.get(entry).ok_or(Error::Untrusted)?;
        let directory = path
            .ancestors()
            .nth(depth)
            .ok_or(Error::Untrusted)?
            .to_owned();
        if directory.join(entry) != *path {
            return Err(Error::Untrusted);
        }
        compiler.bundle_root = Some(directory);
        for (name, path) in members {
            compiler.files.insert(
                name.clone(),
                (
                    path,
                    artifact(&manifest.entries.get(&name).ok_or(Error::Untrusted)?.sha256)?,
                ),
            );
        }
    }
    let mut tools = BTreeMap::new();
    match &action.behavior {
        wire::SoftwareTaskBehavior::Winget(_) | wire::SoftwareTaskBehavior::Brew(_) => {
            let kind = if matches!(action.behavior, wire::SoftwareTaskBehavior::Brew(_)) {
                SoftwareManagerKind::Brew
            } else {
                SoftwareManagerKind::Winget
            };
            let manager = config
                .managers
                .iter()
                .find(|m| m.executor == kind)
                .ok_or(Error::Unsupported)?;
            manager
                .image
                .verify(&manager.image.path)
                .map_err(|_| Error::Untrusted)?;
            tools.insert("manager".into(), pin(&manager.image.path)?);
            if kind == SoftwareManagerKind::Brew {
                tools.insert("git".into(), pin(Path::new("/usr/bin/git"))?);
            }
            use sha2::{Digest as _, Sha256};
            use std::io::Read;
            let mut bytes = Vec::new();
            materials
                .native_source(index)?
                .reader()?
                .take(agent_client::native_source::MAX_PACK_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > agent_client::native_source::MAX_PACK_BYTES {
                return Err(Error::Capacity);
            }
            let digest: [u8; 32] = Sha256::digest(&bytes).into();
            let reference = artifact(&digest)?;
            let path = compiler
                .root
                .publish(
                    helper.as_ref().map(|h| h.context().subject.as_str()),
                    std::io::Cursor::new(&bytes),
                    &reference.sha256,
                    bytes.len() as u64,
                )
                .map_err(crate::error::app_error)?;
            compiler
                .files
                .insert("native-source".into(), (path, reference));
        }
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
        wire::SoftwareTaskBehavior::Msi(_) | wire::SoftwareTaskBehavior::Pkg(_) => {
            let kind = if matches!(action.behavior, wire::SoftwareTaskBehavior::Msi(_)) {
                SoftwareManagerKind::Msi
            } else {
                SoftwareManagerKind::PackageInstaller
            };
            let manager = config
                .managers
                .iter()
                .find(|m| m.executor == kind)
                .ok_or(Error::Unsupported)?;
            manager
                .image
                .verify(&manager.image.path)
                .map_err(|_| Error::Untrusted)?;
            tools.insert("installer".into(), pin(&manager.image.path)?);
            if matches!(action.behavior, wire::SoftwareTaskBehavior::Pkg(_)) {
                tools.insert("pkgutil".into(), pin(Path::new("/usr/sbin/pkgutil"))?);
                tools.insert("spctl".into(), pin(Path::new("/usr/sbin/spctl"))?);
            }
        }
        wire::SoftwareTaskBehavior::Bundle(b) => {
            for (name, script) in std::iter::once(("install-interpreter", &b.install))
                .chain(b.uninstall.as_ref().map(|s| ("uninstall-interpreter", s)))
            {
                let profile = match script.interpreter {
                    wire::SoftwareTaskInterpreter::PowerShell7 => {
                        wire::ExecutorProfile::PowerShell7
                    }
                    wire::SoftwareTaskInterpreter::PosixSh => wire::ExecutorProfile::PosixSh,
                    wire::SoftwareTaskInterpreter::Bash => wire::ExecutorProfile::Bash,
                };
                let interpreter = config
                    .interpreters
                    .iter()
                    .find(|i| i.profile == profile)
                    .ok_or(Error::Unsupported)?;
                interpreter
                    .image
                    .verify(&interpreter.image.path)
                    .map_err(|_| Error::Untrusted)?;
                tools.insert(name.into(), pin(&interpreter.image.path)?);
            }
            #[cfg(target_os = "macos")]
            for (name, path) in [
                ("codesign", "/usr/bin/codesign"),
                ("spctl", "/usr/sbin/spctl"),
            ] {
                tools.insert(name.into(), pin(Path::new(path))?);
            }
        }
        wire::SoftwareTaskBehavior::Exe(_) | wire::SoftwareTaskBehavior::Msix(_) => (),
    }
    if let Some(wire::SoftwareTaskDetection::Script { command }) = commands.detection {
        let profile = match command.interpreter {
            wire::SoftwareTaskInterpreter::PowerShell7 => wire::ExecutorProfile::PowerShell7,
            wire::SoftwareTaskInterpreter::PosixSh => wire::ExecutorProfile::PosixSh,
            wire::SoftwareTaskInterpreter::Bash => wire::ExecutorProfile::Bash,
        };
        let interpreter = config
            .interpreters
            .iter()
            .find(|i| i.profile == profile)
            .ok_or(Error::Unsupported)?;
        interpreter
            .image
            .verify(&interpreter.image.path)
            .map_err(|_| Error::Untrusted)?;
        tools.insert("detect-interpreter".into(), pin(&interpreter.image.path)?);
        #[cfg(target_os = "macos")]
        if action.signatures.iter().any(|s| {
            s.artifact == command.entry
                && s.mechanism == wire::SoftwareTaskSignatureMechanism::AppleDeveloperId
        }) {
            for (name, path) in [
                ("codesign", "/usr/bin/codesign"),
                ("spctl", "/usr/sbin/spctl"),
            ] {
                tools.insert(name.into(), pin(Path::new(path))?);
            }
        }
    }
    if action
        .signatures
        .iter()
        .any(|s| s.mechanism == wire::SoftwareTaskSignatureMechanism::Authenticode)
    {
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
        let path = compiler
            .root
            .publish(
                helper.as_ref().map(|h| h.context().subject.as_str()),
                std::io::Cursor::new(script),
                &reference.sha256,
                script.len() as u64,
            )
            .map_err(crate::error::app_error)?;
        tools.insert(
            "authenticode".into(),
            SoftwareMaterial {
                path: path.to_str().ok_or(Error::Configuration)?.into(),
                artifact: reference,
            },
        );
    }
    if matches!(
        action.behavior,
        wire::SoftwareTaskBehavior::Msi(_) | wire::SoftwareTaskBehavior::Pkg(_)
    ) {
        let name = if matches!(action.behavior, wire::SoftwareTaskBehavior::Msi(_)) {
            "package.msi"
        } else {
            "package.pkg"
        };
        let keys = std::iter::once(action.behavior.installer())
            .chain(commands.removal_artifact)
            .collect::<std::collections::BTreeSet<_>>();
        for key in keys {
            let (path, reference) = compiler.files.get(key).cloned().ok_or(Error::Untrusted)?;
            let exported = compiler
                .root
                .native_export(
                    &path,
                    &reference.sha256,
                    name,
                    helper.as_ref().map(|h| h.context().subject.as_str()),
                )
                .map_err(crate::error::app_error)?;
            compiler.files.insert(key.into(), (exported, reference));
        }
    }
    let primary = compiler
        .files
        .get(action.behavior.installer())
        .ok_or(Error::Untrusted)?
        .1
        .clone();
    let format = match &action.behavior {
        wire::SoftwareTaskBehavior::Bundle(_) => SoftwareFormat::Bundle {
            manifest: bundle.ok_or(Error::Protocol)?,
            limits: BundleLimits {
                archive_bytes: 4 * 1024 * 1024 * 1024,
                files: 4096,
                file_bytes: 1024 * 1024 * 1024,
                expanded_bytes: 8 * 1024 * 1024 * 1024,
                depth: 32,
            },
        },
        wire::SoftwareTaskBehavior::Msi(_) => SoftwareFormat::Msi {},
        wire::SoftwareTaskBehavior::Pkg(_) => SoftwareFormat::Pkg {},
        wire::SoftwareTaskBehavior::Winget(_) => {
            let wire::SoftwareTaskExport::Winget { binding, .. } = &step.export else {
                return Err(Error::Protocol);
            };
            SoftwareFormat::Winget {
                export_identity: id(hex(&binding.publication))?,
            }
        }
        wire::SoftwareTaskBehavior::Brew(_) => {
            let wire::SoftwareTaskExport::Brew { binding, .. } = &step.export else {
                return Err(Error::Protocol);
            };
            SoftwareFormat::Homebrew {
                export_identity: id(hex(&binding.publication))?,
            }
        }
        wire::SoftwareTaskBehavior::Exe(exe) => SoftwareFormat::Exe {
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
    };
    let source_command = if intent == SoftwareOperation::Uninstall {
        commands.uninstall.ok_or(Error::Unsupported)?
    } else {
        commands.install
    };
    let (run_as, session, work_root, _) = identity(config, helper.as_ref(), source_command)?;
    let request = WorkerRequest {
        intent,
        export: step.export.clone(),
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
    let build = |compiler: &mut Compiler<'_>, operation, invocation| {
        let mut request = request.clone();
        request.operation = operation;
        command(compiler, request, invocation, &image)
    };
    let (install, install_source) = build(&mut compiler, O::Install, commands.install)?;
    // Only InPlace freezes an upgrade invocation. Never publish an unjournaled input.
    let update = if commands.upgrade_policy == wire::SoftwareTaskUpgrade::InPlace {
        Some(build(
            &mut compiler,
            O::Upgrade,
            commands.upgrade.unwrap_or(commands.install),
        )?)
    } else {
        None
    };
    let removal = commands
        .uninstall
        .map(|c| build(&mut compiler, O::Uninstall, c))
        .transpose()?;
    let uninstall = removal.as_ref().map(|(c, _)| c.clone());
    let mut auxiliary = BTreeMap::new();
    let mut mutations = BTreeMap::new();
    if matches!(
        format,
        SoftwareFormat::DmgApp { .. }
            | SoftwareFormat::DmgPkg { .. }
            | SoftwareFormat::Winget { .. }
            | SoftwareFormat::Homebrew { .. }
    ) {
        let phases = match intent {
            SoftwareOperation::Detect => Vec::new(),
            _ if matches!(
                format,
                SoftwareFormat::Winget { .. } | SoftwareFormat::Homebrew { .. }
            ) =>
            {
                vec![
                    (SoftwarePhase::Prepare, O::Prepare),
                    (SoftwarePhase::Cleanup, O::Cleanup),
                ]
            }
            SoftwareOperation::Uninstall if matches!(format, SoftwareFormat::DmgApp { .. }) => {
                vec![(SoftwarePhase::Cleanup, O::Cleanup)]
            }
            SoftwareOperation::Uninstall => Vec::new(),
            SoftwareOperation::Install => {
                let mut phases = vec![(SoftwarePhase::Attach, O::Attach)];
                if matches!(format, SoftwareFormat::DmgApp { .. }) {
                    phases.push((SoftwarePhase::Stage, O::Stage));
                }
                phases.push((SoftwarePhase::Cleanup, O::Cleanup));
                phases
            }
        };
        for (phase, operation) in phases {
            let (mut command, source) = build(&mut compiler, operation, source_command)?;
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
            if let Some((_, source)) = &update {
                mutations.insert(SoftwarePhase::Upgrade, Box::new(source.clone()));
            }
            if commands.upgrade_policy == wire::SoftwareTaskUpgrade::UninstallThenInstall {
                let c = commands.uninstall.ok_or(Error::Unsupported)?;
                let (removal, source) = build(&mut compiler, O::RemovePrevious, c)?;
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
    let (detection, detector_source) = match commands.detection {
        Some(wire::SoftwareTaskDetection::MsiProduct {
            product_code,
            version,
        }) => (
            SoftwareDetector::MsiProduct {
                product_code: product_code.clone(),
                version: package(version)?,
            },
            None,
        ),
        Some(wire::SoftwareTaskDetection::PkgReceipt { receipt, version }) => (
            SoftwareDetector::PkgReceipt {
                receipt: receipt.clone(),
                version: package(version)?,
            },
            None,
        ),
        Some(wire::SoftwareTaskDetection::Script { command: script }) => {
            let (mut detector, source) = build(&mut compiler, O::Detect, &script.invocation)?;
            detector.exit_codes = SoftwareExitCodes {
                success: [0].into(),
                reboot: Default::default(),
            };
            (
                SoftwareDetector::Script {
                    invocation: Box::new(detector),
                },
                Some(Box::new(source)),
            )
        }
        _ => {
            let (mut detector, source) = build(&mut compiler, O::Detect, commands.install)?;
            detector.exit_codes = SoftwareExitCodes {
                success: [0].into(),
                reboot: Default::default(),
            };
            (
                SoftwareDetector::Script {
                    invocation: Box::new(detector),
                },
                Some(Box::new(source)),
            )
        }
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
                    invocation: Box::new(update.ok_or(Error::Protocol)?.0),
                },
                wire::SoftwareTaskUpgrade::UninstallThenInstall => {
                    SoftwareUpgrade::UninstallThenInstall {}
                }
                wire::SoftwareTaskUpgrade::Deny => SoftwareUpgrade::Deny {},
            },
            allow_downgrade: action.downgrade == wire::SoftwareTaskDowngrade::Allow,
            allow_reboot: action.reboot == wire::SoftwareTaskReboot::Report,
        },
        SoftwareStepArtifacts {
            mutations,
            detection: detector_source,
            files: files
                .into_iter()
                .map(|m| (PathBuf::from(m.path), m.artifact))
                .collect(),
        },
    ))
}

#[cfg(all(test, target_os = "macos"))]
mod gc_tests {
    fn inventory_files(
        root: &mut execution_runner::staging::MaterialRoot,
    ) -> Result<std::collections::BTreeSet<std::path::PathBuf>, execution_app::Error> {
        use execution_runner::staging::ReclaimPhase;
        let mut pass = root.begin_reclamation();
        while pass.phase() == ReclaimPhase::Inventory {
            root.reclaim_step(&mut pass)?;
        }
        Ok(pass.files().cloned().collect())
    }
    fn finish_reclamation(
        root: &mut execution_runner::staging::MaterialRoot,
        candidates: &std::collections::BTreeMap<std::path::PathBuf, execution_contract::Digest>,
        retained: &std::collections::BTreeSet<std::path::PathBuf>,
    ) -> Result<execution_runner::staging::Reclaimed, execution_app::Error> {
        use execution_runner::staging::ReclaimPhase;
        let mut pass = root.begin_reclamation();
        while pass.phase() == ReclaimPhase::Inventory {
            root.reclaim_step(&mut pass)?;
        }
        root.prove_reclamation(&mut pass, candidates, retained)?;
        while pass.phase() != ReclaimPhase::Complete {
            root.reclaim_step(&mut pass)?;
        }
        pass.into_result()
    }
    use super::*;
    use crate::{backend::test_support::local, protocol_test_support as protocol};
    use agent_client::OpenMode;
    use execution_app::{AppConfig, ExecutionApp, RequestContext};
    use std::collections::BTreeSet;

    #[tokio::test]
    async fn native_brew_compiler_rejects_absent_authenticated_helper_before_publishing_recipes() {
        let _budget = crate::scenario_budget::enter();
        let server = protocol::Server::tls().await;
        server
            .data
            .lock()
            .unwrap()
            .software_native(&server.url, true);
        let credential = server
            .data
            .lock()
            .unwrap()
            .software_source
            .as_ref()
            .unwrap()
            .credential
            .clone()
            .unwrap();
        server.secrets.0.lock().unwrap().insert(
            agent_client::native_source::credential_key(&credential).unwrap(),
            protocol::software_native::TOKEN.into(),
        );
        let cache = protocol::Root::new();
        let mut client = server.client(&cache, OpenMode::Create);
        server.register(&mut client).await;
        let offer = client.claim().await.unwrap().offer.unwrap();
        let materials = client.prepare(&offer).await.unwrap();
        let wire::TaskPayload::Software(payload) = offer.payload() else {
            unreachable!()
        };
        let db = local::Database::new();
        let _journal = execution_sqlite::Store::initialize_test(
            &db.path,
            local::plan().spec().request.authority.clone(),
            plan::storage_limits(),
        )
        .unwrap();
        let root_path = db.path.parent().unwrap().join("materials");
        platform_private_storage::directory(&root_path).unwrap();
        let mut root = execution_runner::staging::MaterialRoot::open_fixture(
            &root_path,
            &Digest::new("a".repeat(64)).unwrap(),
            &db.path,
        )
        .unwrap();
        let config = ExecutionConfig {
            #[cfg(test)]
            worker_image_fixture: None,
            local_permissions: crate::local_permissions::Config {
                policy_file: "/unavailable/local-permissions.json".into(),
            },
            work_root: db.path.parent().unwrap().into(),
            material_root: root_path,
            interpreters: vec![],
            managers: vec![],
            processes: 1,
        };
        let binding = execution_app::ServiceBinding {
            authority: local::plan().spec().request.authority.clone(),
            device: DeviceId::new("device-1").unwrap(),
        };
        let before = inventory_files(&mut root).unwrap();
        assert!(matches!(
            crate::backend::software::compile(
                &offer,
                &materials,
                payload,
                &binding,
                &local::plan().spec().request.actor,
                (&config, &mut root),
                None
            ),
            Err(Error::Identity)
        ));
        assert_eq!(inventory_files(&mut root).unwrap(), before);
        assert!(!server.data.lock().unwrap().started);
    }

    #[tokio::test]
    async fn native_compiler_reclaims_all_frozen_inputs_across_intents_and_upgrade_strategies() {
        let _budget = crate::scenario_budget::enter();
        let db = local::Database::new();
        let root_path = db.path.parent().unwrap().join("materials");
        platform_private_storage::directory(&root_path).unwrap();
        let journal = execution_sqlite::Store::initialize_test(
            &db.path,
            local::plan().spec().request.authority.clone(),
            plan::storage_limits(),
        )
        .unwrap();
        let mut root = execution_runner::staging::MaterialRoot::open_fixture(
            &root_path,
            &Digest::new("a".repeat(64)).unwrap(),
            &db.path,
        )
        .unwrap();
        let config = ExecutionConfig {
            #[cfg(test)]
            worker_image_fixture: None,
            local_permissions: crate::local_permissions::Config {
                policy_file: "/unavailable/local-permissions.json".into(),
            },
            work_root: db.path.parent().unwrap().into(),
            material_root: root_path,
            interpreters: vec![],
            managers: vec![],
            processes: 1,
        };
        let mut app = None;
        let mut journal = Some(journal);
        let consumer = id("native-gc-results").unwrap();
        // Real native input compilation, material publication and SQLite; no native worker
        // is dispatched. /bin/sh is a protected image fixture, not a worker execution proof.
        for (index, (intent, policy)) in [
            (SoftwareOperation::Detect, wire::SoftwareTaskUpgrade::Deny),
            (SoftwareOperation::Detect, wire::SoftwareTaskUpgrade::Deny),
            (
                SoftwareOperation::Uninstall,
                wire::SoftwareTaskUpgrade::Deny,
            ),
            (
                SoftwareOperation::Install,
                wire::SoftwareTaskUpgrade::UninstallThenInstall,
            ),
            (
                SoftwareOperation::Detect,
                wire::SoftwareTaskUpgrade::InPlace,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let server = protocol::Server::new().await;
            let cache = protocol::Root::new();
            let mut client = server.client(&cache, OpenMode::Create);
            server.register(&mut client).await;
            {
                let mut data = server.data.lock().unwrap();
                data.software(1, false);
                let wire::TaskPayload::Software(mut payload) =
                    data.offer.as_ref().unwrap().payload.clone()
                else {
                    panic!("software")
                };
                let wire::SoftwareTaskBehavior::Pkg(pkg) = &payload.steps[0].action.behavior else {
                    panic!("pkg")
                };
                payload.steps[0].action.behavior =
                    wire::SoftwareTaskBehavior::Dmg(wire::SoftwareTaskDmg {
                        image: "package".into(),
                        volume: "GC fixture".into(),
                        scope: wire::SoftwareTaskScope::System,
                        invocation: pkg.install.clone(),
                        upgrade: policy,
                        payload: wire::SoftwareTaskDmgPayload::AppCopy {
                            application: wire::SoftwareTaskMacApplication {
                                path: "Example.app".into(),
                                bundle_id: "example.gc".into(),
                                version: "1.0".into(),
                                target_name: "Example.app".into(),
                            },
                            uninstall: true,
                        },
                    });
                payload.intent = match intent {
                    SoftwareOperation::Detect => wire::SoftwareTaskIntent::Detect,
                    SoftwareOperation::Uninstall => wire::SoftwareTaskIntent::Uninstall,
                    SoftwareOperation::Install => wire::SoftwareTaskIntent::Install,
                };
                use sha2::{Digest as _, Sha256};
                payload.definition_digest =
                    Sha256::digest(serde_json::to_vec(&payload.steps).unwrap()).into();
                data.offer = Some(data.signed(wire::TaskPayload::Software(payload)));
            }
            let offer = client.claim().await.unwrap().offer.unwrap();
            let materials = client.prepare(&offer).await.unwrap();
            let wire::TaskPayload::Software(payload) = offer.payload() else {
                panic!("software")
            };
            let (step, _) = compile(
                (&payload.steps[0], 0),
                payload,
                &materials,
                (&config, &mut root),
                None,
                intent,
                (
                    &package("aarch64").unwrap(),
                    pin(Path::new("/bin/sh")).unwrap(),
                ),
            )
            .unwrap();
            let mut spec = local::plan().spec().clone();
            spec.request.request_id = RequestId::new(format!("native-gc-{index}")).unwrap();
            spec.request.target.platform = Platform::Macos;
            spec.request.target.scope = TargetScope::Device {};
            spec.request.operation.action = id(match intent {
                SoftwareOperation::Detect => "software.detect",
                SoftwareOperation::Install => "software.install",
                SoftwareOperation::Uninstall => "software.uninstall",
            })
            .unwrap();
            spec.request.parameters.clear();
            spec.run_as = RunAs::System {
                platform: Platform::Macos,
            };
            spec.session_requirement = SessionRequirement::NotRequired {};
            spec.launch = step.install.launch.clone();
            spec.launch.interpreter.profile = reference("native-software-sequence", "1").unwrap();
            spec.execution = ExecutionSpec::SoftwareProgram {
                program: Box::new(SoftwareProgram {
                    definition_digest: Digest::new(hex(&payload.definition_digest)).unwrap(),
                    intent,
                    steps: vec![step],
                }),
            };
            let input = FrozenExecution::freeze(spec, &plan::storage_limits().input).unwrap();
            if app.is_none() {
                let mut host = local::TestHost::new();
                host.template = input.clone();
                host.state.lock().unwrap().allow = false;
                let runner = execution_app::DeterministicTestRunner::new(
                    id("no-native-dispatch").unwrap(),
                    execution_app::TestScenario::Wait,
                    8,
                )
                .unwrap();
                app = Some(
                    ExecutionApp::new(
                        journal.take().unwrap(),
                        host,
                        runner,
                        AppConfig::test_defaults(),
                    )
                    .unwrap(),
                );
            }
            let app = app.as_mut().unwrap();
            let caller = RequestContext {
                actor: input.spec().request.actor.clone(),
            };
            let _ = app.request_execution(&caller, &input); // Registers before capability/admission checks.
            let status = app
                .cancel(&caller, &input.spec().request.request_id)
                .unwrap();
            assert!(crate::backend::settled(&status));
            assert_eq!(status.attempts, 0);
            let original = app
                .service_frozen_input(&input.spec().request.request_id)
                .unwrap();
            assert_eq!(original.digest(), input.digest());
            let refs = crate::recovery::references(&original, &config).unwrap();
            let mut existing = inventory_files(&mut root).unwrap();
            existing.remove(&config.material_root.join(".owner"));
            assert!(
                existing.iter().all(|path| refs.contains_key(path)),
                "every publication must have original journal proof"
            );
            let inputs: Vec<_> = crate::recovery::software_invocations(
                &original.spec().execution.software_program().unwrap().steps[0],
            )
            .collect();
            let has_upgrade = inputs.iter().any(|c| {
                let path = config
                    .material_root
                    .join("system")
                    .join(c.launch.artifact.sha256.as_str());
                serde_json::from_slice::<WorkerRequest>(&std::fs::read(path).unwrap())
                    .is_ok_and(|r| r.operation == O::Upgrade)
            });
            assert_eq!(has_upgrade, policy == wire::SoftwareTaskUpgrade::InPlace);
            let pending = app
                .service_delivery(&input.spec().request.request_id, &consumer, 64)
                .unwrap();
            assert!(!pending.is_empty());
            assert_eq!(
                finish_reclamation(
                    &mut root,
                    &refs,
                    &refs.keys().cloned().collect::<BTreeSet<_>>()
                )
                .unwrap()
                .files,
                0
            );
            for item in pending {
                app.service_confirm(
                    &input.spec().request.request_id,
                    &consumer,
                    &item.receipt.event_id,
                )
                .unwrap();
            }
            assert!(app
                .service_delivery(&input.spec().request.request_id, &consumer, 1)
                .unwrap()
                .is_empty());
            finish_reclamation(&mut root, &refs, &BTreeSet::new()).unwrap();
            assert_eq!(
                inventory_files(&mut root).unwrap(),
                BTreeSet::from([config.material_root.join(".owner")])
            );
            assert_eq!(
                finish_reclamation(&mut root, &refs, &BTreeSet::new())
                    .unwrap()
                    .files,
                0
            );
        }
    }
}

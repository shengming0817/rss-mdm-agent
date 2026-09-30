//! Deterministic V4 compiler. The backend has already selected sources and ordered dependencies.
use crate::{
    plan::{self, hex, id, reference},
    ExecutionConfig,
};
use agent_client::{wire, Error, Materials, Offer};
use execution_contract::*;
use execution_runner::{Artifacts, SoftwareStepArtifacts};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

fn package(value: &str) -> Result<PackageValue, Error> {
    PackageValue::new(value).map_err(|_| Error::Protocol)
}
fn artifact(bytes: &[u8; 32]) -> Result<ExactArtifactRef, Error> {
    Ok(ExactArtifactRef {
        resource: reference("backend-material", hex(bytes))?,
        sha256: Digest::new(hex(bytes)).map_err(|_| Error::Protocol)?,
    })
}
struct Compiler<'a> {
    config: &'a ExecutionConfig,
    helper: Option<Arc<execution_runner::helper::Connection>>,
    files: BTreeMap<String, (PathBuf, ExactArtifactRef)>,
    bundle_root: Option<PathBuf>,
}
impl Compiler<'_> {
    fn command(
        &self,
        command: &wire::SoftwareTaskCommand,
        adapter: SoftwareKind,
        payload: &(PathBuf, ExactArtifactRef),
        operation: SoftwareOperation,
        version: &str,
        package: &str,
    ) -> Result<
        (
            SoftwareInvocation,
            Artifacts,
            Vec<(PathBuf, ExactArtifactRef)>,
        ),
        Error,
    > {
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
        let native = matches!(
            command.executor,
            wire::SoftwareTaskExecutor::Msi
                | wire::SoftwareTaskExecutor::PackageInstaller
                | wire::SoftwareTaskExecutor::Winget
                | wire::SoftwareTaskExecutor::Brew
        );
        let profile = match command.executor {
            wire::SoftwareTaskExecutor::PowerShell7
            | wire::SoftwareTaskExecutor::Msi
            | wire::SoftwareTaskExecutor::Winget => wire::ExecutorProfile::PowerShell7,
            wire::SoftwareTaskExecutor::PosixSh
            | wire::SoftwareTaskExecutor::PackageInstaller
            | wire::SoftwareTaskExecutor::Brew => wire::ExecutorProfile::PosixSh,
            wire::SoftwareTaskExecutor::Bash => wire::ExecutorProfile::Bash,
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
        let mut files = vec![payload.clone()];
        let mut arguments = Vec::new();
        let (content, content_ref) = if native {
            if command.entry.is_some() {
                return Err(Error::Protocol);
            }
            let manager = self
                .config
                .managers
                .iter()
                .find(|m| m.executor == command.executor)
                .ok_or(Error::Unsupported)?;
            manager
                .image
                .verify(&manager.image.path)
                .map_err(|_| Error::Untrusted)?;
            let manager_ref = ExactArtifactRef {
                resource: reference("native-manager", &manager.image.sha256)?,
                sha256: Digest::new(&manager.image.sha256).map_err(|_| Error::Configuration)?,
            };
            files.push((manager.image.path.clone(), manager_ref));
            let wrapper = wrapper(adapter, operation)?;
            use sha2::{Digest as _, Sha256};
            let digest: [u8; 32] = Sha256::digest(wrapper).into();
            let reference = artifact(&digest)?;
            let path = execution_runner::staging::publish(
                &self.config.material_root,
                delegate.as_ref().map(|h| h.context().subject.as_str()),
                std::io::Cursor::new(wrapper),
                &reference.sha256,
                wrapper.len() as u64,
            )?;
            arguments.extend([
                manager
                    .image
                    .path
                    .to_str()
                    .ok_or(Error::Configuration)?
                    .to_owned(),
                payload.0.to_str().ok_or(Error::Configuration)?.to_owned(),
                package.to_owned(),
                version.to_owned(),
            ]);
            (path, reference)
        } else {
            let key = command.entry.as_ref().ok_or(Error::Protocol)?;
            let (path, reference) = self.files.get(key).cloned().ok_or(Error::Untrusted)?;
            if self.bundle_root.is_some() {
                (path, reference)
            } else {
                let length = std::fs::metadata(&path)?.len();
                let path = execution_runner::staging::publish(
                    &self.config.material_root,
                    delegate.as_ref().map(|h| h.context().subject.as_str()),
                    std::fs::File::open(path)?,
                    &reference.sha256,
                    length,
                )?;
                (path, reference)
            }
        };
        arguments.extend(command.arguments.clone());
        let (profile_name, prefix) = match profile {
            wire::ExecutorProfile::PowerShell7 => (
                "native-pwsh7-file",
                vec!["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
            ),
            wire::ExecutorProfile::PosixSh => ("native-posix-sh-file", vec![]),
            wire::ExecutorProfile::Bash => ("native-bash-file", vec!["--noprofile", "--norc"]),
            _ => return Err(Error::Unsupported),
        };
        let argv = prefix
            .into_iter()
            .map(|s| LaunchArg::Literal { value: s.into() })
            .chain(Some(LaunchArg::ArtifactPath {}))
            .chain(
                arguments
                    .into_iter()
                    .map(|value| LaunchArg::Literal { value }),
            )
            .collect();
        let invocation = SoftwareInvocation {
            run_as,
            session_requirement: session,
            timeout_ms: u64::from(command.timeout_seconds) * 1000,
            output_bytes: u64::from(command.output_bytes),
            launch: LaunchSpec {
                artifact: content_ref,
                interpreter: InterpreterRef {
                    profile: reference(profile_name, "1")?,
                    artifact: ExactArtifactRef {
                        resource: reference(profile_name, &interpreter.image.sha256)?,
                        sha256: Digest::new(&interpreter.image.sha256)
                            .map_err(|_| Error::Configuration)?,
                    },
                },
                argv,
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output: OutputSpec {
                    format: OutputFormat::Text {},
                    stdout: TextEncoding::Utf8,
                    stderr: TextEncoding::Utf8,
                },
                cwd: self
                    .bundle_root
                    .as_ref()
                    .unwrap_or(&work_root)
                    .to_str()
                    .ok_or(Error::Configuration)?
                    .into(),
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
            files,
        ))
    }
}
pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    payload: &wire::SoftwareTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    config: &ExecutionConfig,
    helper: Option<Arc<execution_runner::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let wire::TaskPayload::Software(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if &compared != original || payload.device_id != binding.device.as_str() {
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
    let mut timeout = 0u64;
    let mut output = 0u64;
    for (index, step) in payload.steps.iter().enumerate() {
        let action = &step.action;
        let adapter = match action.format {
            wire::SoftwareTaskFormat::Msi => SoftwareKind::Msi,
            wire::SoftwareTaskFormat::Pkg => SoftwareKind::Pkg,
            wire::SoftwareTaskFormat::Winget => SoftwareKind::Winget,
            wire::SoftwareTaskFormat::Brew => SoftwareKind::Homebrew,
            wire::SoftwareTaskFormat::Bundle if platform == Platform::Macos => {
                SoftwareKind::MacosBundle
            }
            wire::SoftwareTaskFormat::Bundle => SoftwareKind::WindowsBundle,
        };
        let mut compiler = Compiler {
            config,
            helper: helper.clone(),
            files: BTreeMap::new(),
            bundle_root: None,
        };
        for declared in &step.artifacts {
            let key = format!("{index}/{}", declared.key);
            let file = materials
                .files()
                .iter()
                .find(|f| f.key() == key)
                .ok_or(Error::Untrusted)?;
            let reference = artifact(&declared.sha256)?;
            let path = execution_runner::staging::publish(
                &config.material_root,
                helper.as_ref().map(|h| h.context().subject.as_str()),
                file.reader()?,
                &reference.sha256,
                declared.length,
            )?;
            compiler
                .files
                .insert(declared.key.clone(), (path, reference));
        }
        let mut primary = compiler
            .files
            .get(&action.primary)
            .cloned()
            .ok_or(Error::Untrusted)?;
        if matches!(adapter, SoftwareKind::Homebrew | SoftwareKind::Winget) {
            let name = if adapter == SoftwareKind::Homebrew {
                let leaf = action.package.rsplit('/').next().ok_or(Error::Protocol)?;
                if !leaf
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"@+_.-".contains(&b))
                {
                    return Err(Error::Protocol);
                }
                format!("{leaf}.rb")
            } else {
                "manifest.yaml".into()
            };
            primary.0 = execution_runner::staging::native_export(
                &primary.0,
                &primary.1.sha256,
                &name,
                helper.as_ref().map(|h| h.context().subject.as_str()),
            )?;
        }
        let bundle = action.bundle.as_ref().map(|b| BundleManifest {
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
            let members = execution_runner::staging::bundle(
                &config.material_root,
                helper.as_ref().map(|h| h.context().subject.as_str()),
                &primary.0,
                &primary.1.sha256,
                manifest,
            )?;
            compiler.bundle_root = members
                .get(if platform == Platform::Macos {
                    "install.sh"
                } else {
                    "install.ps1"
                })
                .and_then(|p| p.parent())
                .map(PathBuf::from);
            if compiler.bundle_root.is_none() {
                return Err(Error::Untrusted);
            }
            for (name, path) in members {
                let declared = manifest.entries.get(&name).ok_or(Error::Untrusted)?;
                compiler
                    .files
                    .insert(name, (path, artifact(&declared.sha256)?));
            }
        }
        let (install, install_source, mut files) = compiler.command(
            &action.install,
            adapter,
            &primary,
            SoftwareOperation::Install,
            &action.version,
            &action.package,
        )?;
        let removal = action
            .uninstall
            .as_ref()
            .map(|c| {
                compiler.command(
                    c,
                    adapter,
                    &primary,
                    SoftwareOperation::Uninstall,
                    &action.version,
                    &action.package,
                )
            })
            .transpose()?;
        let uninstall = removal.as_ref().map(|(command, _, _)| command.clone());
        let (mutation, mutation_budget) = match intent {
            SoftwareOperation::Install => (Some(Box::new(install_source)), Some(&install)),
            SoftwareOperation::Uninstall => (
                removal.map(|(_, source, extra)| {
                    files.extend(extra);
                    Box::new(source)
                }),
                uninstall.as_ref(),
            ),
            SoftwareOperation::Detect => (None, None),
        };
        if let Some(command) = mutation_budget {
            timeout = timeout.saturating_add(command.timeout_ms);
            output = output.saturating_add(command.output_bytes);
        }
        let (detection, detector_source) = match &action.detect {
            wire::SoftwareTaskDetection::MsiProduct {
                product_code,
                version,
            } => (
                SoftwareDetector::MsiProduct {
                    product_code: product_code.clone(),
                    version: package(version)?,
                },
                None,
            ),
            wire::SoftwareTaskDetection::PkgReceipt { receipt, version } => (
                SoftwareDetector::PkgReceipt {
                    receipt: receipt.clone(),
                    version: package(version)?,
                },
                None,
            ),
            wire::SoftwareTaskDetection::Script { command } => {
                let (invocation, source, extra) = compiler.command(
                    command,
                    adapter,
                    &primary,
                    SoftwareOperation::Detect,
                    &action.version,
                    &action.package,
                )?;
                timeout = timeout.saturating_add(invocation.timeout_ms);
                output = output.saturating_add(invocation.output_bytes);
                files.extend(extra);
                (
                    SoftwareDetector::Script {
                        invocation: Box::new(invocation),
                    },
                    Some(Box::new(source)),
                )
            }
        };
        files.extend(compiler.files.values().cloned());
        steps.push(SoftwareProgramStep {
            adapter,
            package: package(&action.package)?,
            version: package(&action.version)?,
            architecture: architecture.clone(),
            payload: primary.1,
            export_identity: step.export_identity.as_ref().map(id).transpose()?,
            install,
            uninstall,
            detection,
            existing: match action.ownership {
                wire::SoftwareTaskOwnership::ManagedOnly => ExistingSoftware::ManagedOnly,
                wire::SoftwareTaskOwnership::AllowUserExisting => {
                    ExistingSoftware::AllowUserExisting
                }
            },
            allow_downgrade: action.downgrade == wire::SoftwareTaskDowngrade::Allow,
            allow_reboot: action.reboot == wire::SoftwareTaskReboot::Report,
            bundle_limits: bundle.as_ref().map(|_| BundleLimits {
                archive_bytes: 4 * 1024 * 1024 * 1024,
                files: 4096,
                file_bytes: 1024 * 1024 * 1024,
                expanded_bytes: 8 * 1024 * 1024 * 1024,
                depth: 32,
            }),
            bundle,
        });
        sources.push(SoftwareStepArtifacts {
            mutation,
            detection: detector_source,
            files,
        });
    }
    timeout = timeout.clamp(1000, 86_400_000);
    output = output.clamp(1024, 1_048_576);
    let mut controller = steps.first().ok_or(Error::Protocol)?.install.launch.clone();
    controller.interpreter.profile = reference("native-software-sequence", "1")?;
    let program = SoftwareProgram {
        definition_digest: Digest::new(hex(&payload.definition_digest))
            .map_err(|_| Error::Protocol)?,
        intent,
        steps,
    };
    let input = ExecutionInput {
        schema_version: V5,
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
fn wrapper(adapter: SoftwareKind, operation: SoftwareOperation) -> Result<&'static [u8], Error> {
    Ok(match (adapter, operation) {
        (SoftwareKind::Pkg, SoftwareOperation::Install) => b"#!/bin/sh\nset -eu\nmanager=$1; payload=$2; shift 4\nexec \"$manager\" -pkg \"$payload\" -target / \"$@\"\n",
        (SoftwareKind::Msi, SoftwareOperation::Install) => b"param($Manager,$Payload,$Package,$Version)\n$ErrorActionPreference='Stop'\n& $Manager /i $Payload /qn /norestart @args\nexit $LASTEXITCODE\n",
        (SoftwareKind::Msi, SoftwareOperation::Uninstall) => b"param($Manager,$Payload,$Package,$Version)\n$ErrorActionPreference='Stop'\n& $Manager /x $Payload /qn /norestart @args\nexit $LASTEXITCODE\n",
        (SoftwareKind::Winget, SoftwareOperation::Install) => b"param($Manager,$Payload,$Package,$Version)\n$ErrorActionPreference='Stop'\n& $Manager install --manifest $Payload --version $Version --exact --skip-dependencies --silent --disable-interactivity @args\nexit $LASTEXITCODE\n",
        (SoftwareKind::Winget, SoftwareOperation::Uninstall) => b"param($Manager,$Payload,$Package,$Version)\n$ErrorActionPreference='Stop'\n& $Manager uninstall --manifest $Payload --version $Version --exact --silent --disable-interactivity @args\nexit $LASTEXITCODE\n",
        (SoftwareKind::Homebrew, SoftwareOperation::Install) => b"#!/bin/sh\nset -eu\nmanager=$1; payload=$2; shift 4\nexport HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_INSTALLED_DEPENDENTS_CHECK=1 HOMEBREW_NO_INSTALL_UPGRADE=1 HOMEBREW_NO_ANALYTICS=1\nexec \"$manager\" install --formula --ignore-dependencies \"$payload\" \"$@\"\n",
        (SoftwareKind::Homebrew, SoftwareOperation::Uninstall) => b"#!/bin/sh\nset -eu\nmanager=$1; payload=$2; shift 4\nexport HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_ANALYTICS=1\nexec \"$manager\" uninstall --formula \"$payload\" \"$@\"\n",
        _ => return Err(Error::Unsupported),
    })
}

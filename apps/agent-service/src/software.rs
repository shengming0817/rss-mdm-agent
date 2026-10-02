//! Deterministic V6 compiler. The backend has already selected sources and ordered dependencies.
use crate::{
    plan::{self, hex, id, reference},
    ExecutionConfig, SoftwareManagerKind,
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
#[derive(Clone, Copy)]
enum CommandKind {
    Native(SoftwareManagerKind),
    Script(wire::SoftwareTaskInterpreter),
}
impl CommandKind {
    fn script(
        script: &wire::SoftwareTaskScript,
    ) -> (Self, Option<&str>, &wire::SoftwareTaskInvocation) {
        (
            Self::Script(script.interpreter),
            Some(&script.entry),
            &script.invocation,
        )
    }
}
type CompiledCommand = (
    SoftwareInvocation,
    Artifacts,
    Vec<(PathBuf, ExactArtifactRef)>,
);
struct Compiler<'a> {
    config: &'a ExecutionConfig,
    helper: Option<Arc<execution_runner::helper::Connection>>,
    files: BTreeMap<String, (PathBuf, ExactArtifactRef)>,
    bundle_root: Option<PathBuf>,
}
impl Compiler<'_> {
    fn command(
        &self,
        (kind, entry, command): (CommandKind, Option<&str>, &wire::SoftwareTaskInvocation),
        adapter: SoftwareKind,
        payload: &(PathBuf, ExactArtifactRef),
        operation: SoftwareOperation,
        version: &str,
        package: &str,
    ) -> Result<CompiledCommand, Error> {
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
        let profile = match kind {
            CommandKind::Native(SoftwareManagerKind::Msi | SoftwareManagerKind::Winget)
            | CommandKind::Script(wire::SoftwareTaskInterpreter::PowerShell7) => {
                wire::ExecutorProfile::PowerShell7
            }
            CommandKind::Native(
                SoftwareManagerKind::PackageInstaller | SoftwareManagerKind::Brew,
            )
            | CommandKind::Script(wire::SoftwareTaskInterpreter::PosixSh) => {
                wire::ExecutorProfile::PosixSh
            }
            CommandKind::Script(wire::SoftwareTaskInterpreter::Bash) => wire::ExecutorProfile::Bash,
        };
        let expected_reboot = if adapter == SoftwareKind::Msi {
            [3010, 1641].into_iter().collect()
        } else {
            Default::default()
        };
        if command.exit_codes.success != [0].into_iter().collect()
            || (!command.exit_codes.reboot.is_empty()
                && command.exit_codes.reboot != expected_reboot)
        {
            return Err(Error::Unsupported);
        }
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
        let (content, content_ref) = if let CommandKind::Native(executor) = kind {
            if entry.is_some() {
                return Err(Error::Protocol);
            }
            let manager = self
                .config
                .managers
                .iter()
                .find(|m| m.executor == executor)
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
            let key = entry.ok_or(Error::Protocol)?;
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
            exit_codes: SoftwareExitCodes {
                success: command.exit_codes.success.clone(),
                reboot: command.exit_codes.reboot.clone(),
            },
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
        if !action.signatures.is_empty() {
            return Err(Error::Unsupported);
        }
        if !matches!(step.export, wire::SoftwareTaskExport::Direct) {
            return Err(Error::Unsupported);
        }
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

        let commands = agent_client::software_commands(action)?;
        let (adapter, native) = match &action.behavior {
            wire::SoftwareTaskBehavior::Msi(n) => {
                (SoftwareKind::Msi, Some((SoftwareManagerKind::Msi, n)))
            }
            wire::SoftwareTaskBehavior::Pkg(n) => (
                SoftwareKind::Pkg,
                Some((SoftwareManagerKind::PackageInstaller, n)),
            ),
            wire::SoftwareTaskBehavior::Winget(n) => {
                (SoftwareKind::Winget, Some((SoftwareManagerKind::Winget, n)))
            }
            wire::SoftwareTaskBehavior::Brew(n) => {
                (SoftwareKind::Homebrew, Some((SoftwareManagerKind::Brew, n)))
            }
            wire::SoftwareTaskBehavior::Bundle(_) => (
                if platform == Platform::Macos {
                    SoftwareKind::MacosBundle
                } else {
                    SoftwareKind::WindowsBundle
                },
                None,
            ),
            _ => return Err(Error::Unsupported),
        };
        if native.is_some_and(|(_, n)| {
            n.upgrade != wire::SoftwareTaskUpgrade::InPlace || n.upgrade_invocation != n.install
        }) {
            return Err(Error::Unsupported);
        }
        let install_command = if let Some(script) = commands.install_script {
            CommandKind::script(script)
        } else {
            (
                CommandKind::Native(native.ok_or(Error::Unsupported)?.0),
                None,
                commands.install,
            )
        };
        let mut compiler = Compiler {
            config,
            helper: helper.clone(),
            files: BTreeMap::new(),
            bundle_root: None,
        };
        for declared in &step.artifacts {
            let key = &declared.key;
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
            compiler.files.insert(
                declared
                    .key
                    .strip_prefix(&format!("{index}/"))
                    .ok_or(Error::Protocol)?
                    .to_owned(),
                (path, reference),
            );
        }
        let mut primary = compiler
            .files
            .get(action.behavior.installer())
            .cloned()
            .ok_or(Error::Untrusted)?;
        if let Some(name) = native_export_name(adapter, &action.package)? {
            primary.0 = execution_runner::staging::native_export(
                &primary.0,
                &primary.1.sha256,
                &name,
                helper.as_ref().map(|h| h.context().subject.as_str()),
            )?;
        }
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
            let members = execution_runner::staging::bundle(
                &config.material_root,
                helper.as_ref().map(|h| h.context().subject.as_str()),
                &primary.0,
                &primary.1.sha256,
                manifest,
            )?;
            compiler.bundle_root = Some(bundle_directory(
                &members,
                &commands.install_script.ok_or(Error::Protocol)?.entry,
            )?);
            for (name, path) in members {
                let declared = manifest.entries.get(&name).ok_or(Error::Untrusted)?;
                compiler
                    .files
                    .insert(name, (path, artifact(&declared.sha256)?));
            }
        }
        let (install, install_source, mut files) = compiler.command(
            install_command,
            adapter,
            &primary,
            SoftwareOperation::Install,
            &action.version,
            &action.package,
        )?;
        let removal = if let Some(invocation) = commands.uninstall {
            let command = if let Some(script) = commands.uninstall_script {
                CommandKind::script(script)
            } else {
                (
                    CommandKind::Native(native.ok_or(Error::Unsupported)?.0),
                    None,
                    invocation,
                )
            };
            let payload = if let Some(key) = commands.removal_artifact {
                compiler.files.get(key).ok_or(Error::Untrusted)?
            } else {
                &primary
            };
            Some(compiler.command(
                command,
                adapter,
                payload,
                SoftwareOperation::Uninstall,
                &action.version,
                &action.package,
            )?)
        } else {
            None
        };
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
        let (detection, detector_source) = match commands.detection {
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
            wire::SoftwareTaskDetection::Script { command: script } => {
                let (invocation, source, extra) = compiler.command(
                    CommandKind::script(script),
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
            _ => return Err(Error::Unsupported),
        };
        files.extend(compiler.files.values().cloned());
        steps.push(SoftwareProgramStep {
            format: match adapter {
                SoftwareKind::Msi => SoftwareFormat::Msi {},
                SoftwareKind::Pkg => SoftwareFormat::Pkg {},
                SoftwareKind::WindowsBundle | SoftwareKind::MacosBundle => SoftwareFormat::Bundle {
                    manifest: bundle.clone().ok_or(Error::Protocol)?,
                    limits: BundleLimits {
                        archive_bytes: 4 * 1024 * 1024 * 1024,
                        files: 4096,
                        file_bytes: 1024 * 1024 * 1024,
                        expanded_bytes: 8 * 1024 * 1024 * 1024,
                        depth: 32,
                    },
                },
                _ => return Err(Error::Unsupported),
            },
            package: package(&action.package)?,
            version: package(&action.version)?,
            architecture: architecture.clone(),
            payload: primary.1,
            signatures: Vec::new(),
            upgrade: SoftwareUpgrade::InPlace {
                invocation: Box::new(install.clone()),
            },
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
        schema_version: V6,
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

fn bundle_directory(members: &BTreeMap<String, PathBuf>, entry: &str) -> Result<PathBuf, Error> {
    let path = std::path::Path::new(entry);
    let depth = path.components().count();
    if depth == 0
        || !path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
    {
        return Err(Error::Protocol);
    }
    let target = members.get(entry).ok_or(Error::Untrusted)?;
    let root = target.ancestors().nth(depth).ok_or(Error::Untrusted)?;
    if root.join(path) != *target {
        return Err(Error::Untrusted);
    }
    Ok(root.to_path_buf())
}
// Native package managers require these filenames; the name never comes from a wire path.
pub(crate) fn native_export_name(
    adapter: SoftwareKind,
    package: &str,
) -> Result<Option<String>, Error> {
    Ok(Some(match adapter {
        SoftwareKind::Pkg => "package.pkg".into(),
        SoftwareKind::Msi => "package.msi".into(),
        SoftwareKind::Winget => "manifest.yaml".into(),
        SoftwareKind::Homebrew => {
            let leaf = package.rsplit('/').next().ok_or(Error::Protocol)?;
            if leaf.is_empty()
                || !leaf
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"@+_.-".contains(&b))
            {
                return Err(Error::Protocol);
            }
            format!("{leaf}.rb")
        }
        _ => return Ok(None),
    }))
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn review_regression_declared_bundle_entry_determines_its_root() {
        let root = std::env::temp_dir().join("bundle-directory-proof");
        for entry in ["setup.sh", "scripts/nested/setup.sh"] {
            let members = [(entry.to_owned(), root.join(entry))].into();
            assert_eq!(bundle_directory(&members, entry).unwrap(), root);
        }
    }
}

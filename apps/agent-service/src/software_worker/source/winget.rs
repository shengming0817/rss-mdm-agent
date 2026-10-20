//! Native WinGet consumes one verified publication snapshot, never a fresh source resolution.
//! ref: microsoft/winget-cli {Argument.cpp,Commands/InstallCommand.cpp,Commands/UpgradeCommand.cpp,
//! Commands/UninstallCommand.cpp,Rest/Schema/1_0/Json/ManifestDeserializer_1_0.cpp}.
use super::*;
use std::io::Read;

fn manifest(request: &WorkerRequest) -> Result<Vec<u8>, Error> {
    let wire::SoftwareTaskExport::Winget { binding, .. } = &request.export else {
        return Err(Error::Protocol);
    };
    if !matches!(
        request.action.behavior,
        wire::SoftwareTaskBehavior::Winget(_)
    ) {
        return Err(Error::Protocol);
    }
    let mut bytes = Vec::new();
    fs::File::open(request.material("native-source")?)?
        .take(agent_client::native_source::MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    agent_client::native_source::winget_manifest(&bytes, binding, &request.action)
}
fn path(request: &WorkerRequest) -> Result<String, Error> {
    request
        .resource_root
        .join("manifest.yaml")
        .to_str()
        .map(String::from)
        .ok_or(Error::Configuration)
}
fn arguments(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
) -> Result<Vec<String>, Error> {
    let wire::SoftwareTaskBehavior::Winget(native) = &request.action.behavior else {
        return Err(Error::Protocol);
    };
    let command = match request.operation {
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious => {
            &native
                .uninstall
                .as_ref()
                .ok_or(Error::Unsupported)?
                .invocation
        }
        WorkerOperation::Upgrade => &native.upgrade_invocation,
        _ => &native.install,
    };
    if !command.arguments.is_empty() {
        return Err(Error::Unsupported);
    }
    let Some(before) = before else {
        return Err(Error::Denied);
    };
    if matches!(before, SoftwareState::Unknown { .. }) {
        return Err(Error::Denied);
    }
    let removal = matches!(
        request.operation,
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
    );
    let mut args = vec![
        if removal {
            "uninstall"
        } else if request.operation == WorkerOperation::Upgrade {
            "upgrade"
        } else {
            "install"
        }
        .into(),
        "--manifest".into(),
        path(request)?,
        "--scope".into(),
        if native.scope == wire::SoftwareTaskScope::System {
            "machine"
        } else {
            "user"
        }
        .into(),
        "--silent".into(),
        "--disable-interactivity".into(),
    ];
    if removal {
        let version = if request.operation == WorkerOperation::RemovePrevious {
            let SoftwareState::Present { version } = before else {
                return Err(Error::Denied);
            };
            version.as_str()
        } else {
            &request.action.version
        };
        args.extend(["--version".into(), version.into()]);
    } else {
        // --manifest cannot be combined with search selectors. Exact identity/version and hashes
        // come exclusively from this verified file; no source query may replace them.
        args.extend([
            "--skip-dependencies".into(),
            "--architecture".into(),
            match request.architecture {
                wire::TaskArchitecture::Aarch64 => "arm64",
                wire::TaskArchitecture::X86_64 => "x64",
            }
            .into(),
        ]);
    }
    Ok(args)
}
pub(super) fn execute(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
    _: &mut State,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    let bytes = manifest(request)?;
    let directory = platform_private_storage::PrivateDirectory::open(&request.resource_root)?;
    let name = Path::new("manifest.yaml");
    if request.operation == WorkerOperation::Prepare {
        match directory.read(name, agent_client::native_source::MAX_DOCUMENT_BYTES) {
            Ok(existing) if existing == bytes => (),
            Ok(_) => return Err(Error::Untrusted),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                use std::io::Write;
                let mut file = directory.create_file(name)?;
                file.write_all(&bytes)?;
                file.sync_all()?;
            }
            Err(error) => return Err(error.into()),
        }
        tool(
            request,
            "manager",
            vec![
                "validate".into(),
                "--manifest".into(),
                path(request)?,
                "--disable-interactivity".into(),
            ],
            &environment(request)?,
        )?;
        return Ok(result(0, None, String::new()));
    }
    if directory.read(name, agent_client::native_source::MAX_DOCUMENT_BYTES)? != bytes {
        return Err(Error::Untrusted);
    }
    // Native policy gates LocalManifestFiles. No settings/policy/--ignore-security-hash mutation.
    let (code, stdout, stderr) = run_tool_environment(
        request,
        &request.tool("manager")?,
        &arguments(request, before)?,
        Some(&environment(request)?),
    )?;
    Ok(result(
        code,
        None,
        format!(
            "{}{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        ),
    ))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn native_file_input_stays_frozen_when_the_remote_source_changes() {
        use crate::protocol_test_support::{Root, Server};
        let root = Root::new();
        let mut request = super::super::tests::fixture(&root);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let server = runtime.block_on(Server::tls());
        server
            .data
            .lock()
            .unwrap()
            .software_native(&server.url, false);
        let (action, export, document) = {
            let data = server.data.lock().unwrap();
            let wire::TaskPayload::Software(spec) = &data.offer.as_ref().unwrap().payload else {
                unreachable!()
            };
            (
                spec.steps[0].action.clone(),
                spec.steps[0].export.clone(),
                data.software_source.as_ref().unwrap().document.clone(),
            )
        };
        request.action = action;
        request.export = export;
        let file = request.materials.get_mut("native-source").unwrap();
        fs::write(&file.path, serde_json::to_vec(&document).unwrap()).unwrap();
        file.artifact.sha256 = Digest::new(format!(
            "{:x}",
            Sha256::digest(fs::read(&file.path).unwrap())
        ))
        .unwrap();
        let manager = request.tools.get_mut("manager").unwrap();
        fs::write(&manager.path,"#!/bin/sh\ncase \"$1\" in validate) test -f \"$3\" ;; install) /bin/cat \"$3\" ;; *) exit 91 ;; esac\n").unwrap();
        manager.artifact.sha256 = Digest::new(format!(
            "{:x}",
            Sha256::digest(fs::read(&manager.path).unwrap())
        ))
        .unwrap();
        super::super::execute(&request, Some(&SoftwareState::Absent {})).unwrap();
        server
            .data
            .lock()
            .unwrap()
            .software_source
            .as_mut()
            .unwrap()
            .document["Versions"][0]["Installers"][0]["InstallerUrl"] =
            "https://other.example.test/evil.msi".into();
        request.operation = WorkerOperation::Install;
        let (code, result) =
            super::super::execute(&request, Some(&SoftwareState::Absent {})).unwrap();
        assert_eq!(code, 0);
        assert!(result.detected.is_none());
        let consumed: serde_json::Value = serde_json::from_str(&result.diagnostics).unwrap();
        assert_eq!(
            consumed["Installers"],
            document["Versions"][0]["Installers"]
        );
        assert!(server
            .data
            .lock()
            .unwrap()
            .software_source
            .as_ref()
            .unwrap()
            .requests
            .is_empty());
        fs::write(path(&request).unwrap(), b"changed").unwrap();
        assert!(super::super::execute(&request, Some(&SoftwareState::Absent {})).is_err());
        request.operation = WorkerOperation::Cleanup;
        super::super::execute(&request, None).unwrap();
        assert!(!request.resource_root.exists());
    }
    #[test]
    fn sole_manifest_input_cannot_be_overridden_by_search_or_native_flags() {
        let root = crate::protocol_test_support::Root::new();
        let mut request = super::super::tests::fixture(&root);
        let wire::SoftwareTaskExport::Brew { binding, .. } = request.export else {
            unreachable!()
        };
        request.export = wire::SoftwareTaskExport::Winget {
            binding,
            uri: "https://source.example.test/frozen/".into(),
            identifier: "rss-frozen".into(),
        };
        let wire::SoftwareTaskBehavior::Brew(mut native) = request.action.behavior else {
            unreachable!()
        };
        native.uninstall = Some(wire::SoftwareTaskRemoval {
            installer: native.installer.clone(),
            invocation: native.install.clone(),
        });
        request.action.behavior = wire::SoftwareTaskBehavior::Winget(native);
        let before = SoftwareState::Present {
            version: PackageValue::new("0.9").unwrap(),
        };
        request.operation = WorkerOperation::Upgrade;
        let args = arguments(&request, Some(&before)).unwrap();
        assert_eq!(args[0], "upgrade");
        assert!(args
            .windows(2)
            .any(|p| p == ["--manifest", path(&request).unwrap().as_str()]));
        assert!(!args.iter().any(
            |v| ["--id", "--source", "--version", "--ignore-security-hash"].contains(&v.as_str())
        ));
        assert!(args.iter().any(|v| v == "--skip-dependencies"));
        request.operation = WorkerOperation::RemovePrevious;
        let args = arguments(&request, Some(&before)).unwrap();
        assert_eq!(args[0], "uninstall");
        assert!(args.windows(2).any(|p| p == ["--version", "0.9"]));
        assert!(!args.iter().any(|v| v == "--skip-dependencies"));
        assert!(arguments(&request, None).is_err());
        request.operation = WorkerOperation::Install;
        let wire::SoftwareTaskBehavior::Winget(native) = &mut request.action.behavior else {
            unreachable!()
        };
        native.install.arguments.push("--source=other".into());
        assert!(matches!(
            arguments(&request, Some(&SoftwareState::Absent {})),
            Err(Error::Unsupported)
        ));
    }
}

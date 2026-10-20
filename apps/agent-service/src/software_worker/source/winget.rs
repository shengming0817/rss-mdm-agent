//! Explicit frozen REST source; no default-source search, settings reset or manifest fallback.
//! ref: microsoft/winget-cli SourceFlow.cpp, GroupPolicy.cpp and AppInstallerErrors.h.
use super::*;

fn authority(request: &WorkerRequest) -> Result<(&wire::SoftwareExportBinding, &str, &str), Error> {
    let wire::SoftwareTaskExport::Winget {
        binding,
        uri,
        identifier,
    } = &request.export
    else {
        return Err(Error::Protocol);
    };
    if !matches!(
        request.action.behavior,
        wire::SoftwareTaskBehavior::Winget(_)
    ) {
        return Err(Error::Protocol);
    }
    Ok((binding, uri, identifier))
}
fn present(request: &WorkerRequest) -> Result<bool, Error> {
    let (_, uri, name) = authority(request)?;
    let (code, stdout, _) = run_tool_environment(
        request,
        &request.tool("manager")?,
        &[
            "source".into(),
            "export".into(),
            "--name".into(),
            name.into(),
            "--disable-interactivity".into(),
        ],
        Some(&environment(request)?),
    )?;
    // The documented HRESULT is structured failure, independent of translated console messages.
    if code as u32 == 0x8A150012 {
        return Ok(false);
    }
    if code != 0 {
        return Err(Error::Unavailable);
    }
    let value: serde_json::Value = serde_json::from_slice(&stdout).map_err(|_| Error::Protocol)?;
    if value["Name"].as_str() != Some(name)
        || value["Arg"].as_str() != Some(uri)
        || value["Identifier"].as_str() != Some(name)
        || value["Type"].as_str() != Some("Microsoft.Rest")
    {
        return Err(Error::Untrusted);
    }
    Ok(true)
}
fn arguments(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
) -> Result<Vec<String>, Error> {
    let (_, _, name) = authority(request)?;
    let wire::SoftwareTaskBehavior::Winget(native) = &request.action.behavior else {
        return Err(Error::Protocol);
    };
    let command = match request.operation {
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious => native
            .uninstall
            .as_ref()
            .ok_or(Error::Unsupported)?
            .invocation
            .clone(),
        WorkerOperation::Upgrade => native.upgrade_invocation.clone(),
        _ => native.install.clone(),
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
    let version = if request.operation == WorkerOperation::RemovePrevious {
        let SoftwareState::Present { version } = before else {
            return Err(Error::Denied);
        };
        version.as_str()
    } else {
        &request.action.version
    };
    let mut args = vec![
        if removal {
            "uninstall"
        } else if request.operation == WorkerOperation::Upgrade {
            "upgrade"
        } else {
            "install"
        }
        .into(),
        "--id".into(),
        request.action.package.clone(),
        "--version".into(),
        version.into(),
        "--exact".into(),
        "--source".into(),
        name.into(),
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
    if !removal {
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
    record: &mut State,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    let (binding, uri, name) = authority(request)?;
    if request.operation == WorkerOperation::Prepare {
        let bytes = fs::read(request.material("native-source")?)?;
        agent_client::native_source::verify_winget(&bytes, binding, &request.action)?;
        if !present(request)? {
            record.created = true;
            save(request, record)?;
            tool(
                request,
                "manager",
                vec![
                    "source".into(),
                    "add".into(),
                    "--name".into(),
                    name.into(),
                    "--arg".into(),
                    uri.into(),
                    "--type".into(),
                    "Microsoft.Rest".into(),
                    "--explicit".into(),
                    "--disable-interactivity".into(),
                ],
                &environment(request)?,
            )?;
            if !present(request)? {
                return Err(Error::Untrusted);
            }
        }
        return Ok(result(0, None, String::new()));
    }
    if !present(request)? {
        return Err(Error::Untrusted);
    }
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
pub(super) fn cleanup(request: &WorkerRequest, record: &State) -> Result<(), Error> {
    if !record.created || !present(request)? {
        return Ok(());
    }
    let (_, _, name) = authority(request)?;
    tool(
        request,
        "manager",
        vec![
            "source".into(),
            "remove".into(),
            "--name".into(),
            name.into(),
            "--disable-interactivity".into(),
        ],
        &environment(request)?,
    )?;
    if present(request)? {
        return Err(Error::Unavailable);
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn selectors_and_removal_version_are_frozen_without_native_flag_overrides() {
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
        assert!(args.windows(2).any(|p| p == ["--source", "rss-frozen"]));
        assert!(args.windows(2).any(|p| p == ["--version", "1.0"]));
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

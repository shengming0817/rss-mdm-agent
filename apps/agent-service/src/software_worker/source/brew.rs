//! Frozen Tap checkout and bottle/cask consumption under the authenticated non-root login.
//! ref: Homebrew/brew Library/Homebrew/{cmd/--cache.rb,cmd/install.rb,tap.rb}.
use super::*;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;

fn authority(
    request: &WorkerRequest,
) -> Result<(&wire::SoftwareExportBinding, &str, &str, &str), Error> {
    let wire::SoftwareTaskExport::Brew {
        binding,
        uri,
        commit,
        tap,
        ..
    } = &request.export
    else {
        return Err(Error::Protocol);
    };
    if unsafe { libc::geteuid() } == 0 || !matches!(request.run_as, RunAs::User { .. }) {
        return Err(Error::Denied);
    }
    let mut segments = tap.split('/');
    let (Some(user), Some(name), None) = (segments.next(), segments.next(), segments.next()) else {
        return Err(Error::Protocol);
    };
    if [user, name, &request.action.package].iter().any(|v| {
        v.is_empty()
            || v.starts_with('.')
            || !v
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"@+_.-".contains(&b))
    }) {
        return Err(Error::Protocol);
    }
    Ok((binding, uri, commit, tap))
}
fn env(request: &WorkerRequest) -> Result<BTreeMap<String, String>, Error> {
    let mut env = environment(request)?;
    // Avoid the developer command's persistent `homebrew.devcmdrun` configuration write.
    env.insert("HOMEBREW_DEVELOPER".into(), "1".into());
    for key in [
        "HOMEBREW_NO_AUTO_UPDATE",
        "HOMEBREW_NO_INSTALL_FROM_API",
        "HOMEBREW_NO_ANALYTICS",
        "HOMEBREW_NO_INSTALL_CLEANUP",
        "HOMEBREW_NO_INSTALLED_DEPENDENTS_CHECK",
        "HOMEBREW_NO_ENV_HINTS",
        "HOMEBREW_NO_COLOR",
    ] {
        env.insert(key.into(), "1".into());
    }
    env.insert(
        "HOMEBREW_CACHE".into(),
        request
            .resource_root
            .join("cache")
            .to_str()
            .ok_or(Error::Configuration)?
            .into(),
    );
    env.insert(
        "HOMEBREW_TEMP".into(),
        request
            .resource_root
            .join("temp")
            .to_str()
            .ok_or(Error::Configuration)?
            .into(),
    );
    Ok(env)
}
fn git(request: &WorkerRequest, directory: &Path, args: &[&str]) -> Result<Vec<u8>, Error> {
    let mut arguments = vec![
        "-c".into(),
        "core.hooksPath=/dev/null".into(),
        "-c".into(),
        "protocol.allow=never".into(),
        "-C".into(),
        directory.to_str().ok_or(Error::Configuration)?.into(),
    ];
    arguments.extend(args.iter().map(|s| (*s).into()));
    tool(request, "git", arguments, &environment(request)?)
}
fn location(request: &WorkerRequest, tap: &str) -> Result<PathBuf, Error> {
    let repository = PathBuf::from(text(tool(
        request,
        "manager",
        vec!["--repository".into()],
        &env(request)?,
    )?)?);
    if !repository.is_absolute()
        || repository
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(Error::Untrusted);
    }
    let (user, name) = tap.split_once('/').ok_or(Error::Protocol)?;
    let parent = repository.join("Library/Taps").join(user);
    // Each namespace is controlled by the actual Homebrew login. Do not traverse links.
    for path in [
        repository.clone(),
        repository.join("Library"),
        repository.join("Library/Taps"),
    ] {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::Untrusted);
        }
    }
    if parent.try_exists()? {
        let metadata = fs::symlink_metadata(&parent)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::Untrusted);
        }
    }
    Ok(parent.join(format!("homebrew-{name}")))
}
fn verify_tap(request: &WorkerRequest, path: &Path, uri: &str, commit: &str) -> Result<(), Error> {
    if fs::symlink_metadata(path)?.file_type().is_symlink()
        || fs::symlink_metadata(path.join(".git"))?
            .file_type()
            .is_symlink()
    {
        return Err(Error::Untrusted);
    }
    if text(git(request, path, &["rev-parse", "HEAD"])?)? != commit
        || text(git(
            request,
            path,
            &[
                "config",
                "--local",
                "--no-includes",
                "--get",
                "remote.origin.url",
            ],
        )?)? != uri
    {
        return Err(Error::Untrusted);
    }
    let mut documents = Vec::new();
    let mut total = 0usize;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        if name != "Formula" && name != "Casks" || !entry.file_type()?.is_dir() {
            return Err(Error::Untrusted);
        }
        for document in fs::read_dir(entry.path())? {
            let document = document?;
            let metadata = document.metadata()?;
            if !document.file_type()?.is_file() || documents.len() >= 256 {
                return Err(Error::Untrusted);
            }
            total = total
                .checked_add(usize::try_from(metadata.len()).map_err(|_| Error::Capacity)?)
                .ok_or(Error::Capacity)?;
            if total > agent_client::native_source::MAX_DOCUMENT_BYTES {
                return Err(Error::Capacity);
            }
            let leaf = document
                .file_name()
                .into_string()
                .map_err(|_| Error::Protocol)?;
            let mut bytes = Vec::new();
            fs::File::open(document.path())?
                .take(metadata.len() + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 != metadata.len() {
                return Err(Error::Untrusted);
            }
            documents.push((
                format!("{}/{leaf}", name.to_str().ok_or(Error::Protocol)?),
                bytes,
            ));
        }
    }
    let (binding, ..) = authority(request)?;
    if wire::brew_documents_digest(documents.iter().map(|(p, b)| (p.as_str(), b.as_slice())))?
        != binding.document_sha256
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}
fn prepare(request: &WorkerRequest, record: &mut State) -> Result<(), Error> {
    let (binding, uri, commit, tap) = authority(request)?;
    let path = location(request, tap)?;
    if path.try_exists()? {
        // Reuse only an exact externally owned checkout; it is never removed by this task.
        verify_tap(request, &path, uri, commit)?;
        if record.path.as_ref().is_some_and(|old| old != &path) {
            return Err(Error::Untrusted);
        }
        record.path = Some(path);
        save(request, record)?;
        return Ok(());
    }
    record.path = Some(path.clone());
    record.created = true;
    let parent = path.parent().ok_or(Error::Untrusted)?;
    record.parent_created = !parent.try_exists()?;
    save(request, record)?; // Ownership intent precedes every directory or Git write.
    if record.parent_created {
        platform_private_storage::directory(parent)?;
    }
    // Checkout can stop after any file write. Keep it inside the owned attempt until fully
    // verified, then publish atomically without replacing an existing tap (renameatx_np EXCL).
    let staged = request.resource_root.join("tap");
    platform_private_storage::directory(&staged)?;
    let repository = staged.join(".git");
    platform_private_storage::directory(&repository)?;
    let mut pack = Vec::new();
    fs::File::open(request.material("native-source")?)?
        .take(agent_client::native_source::MAX_PACK_BYTES as u64 + 1)
        .read_to_end(&mut pack)?;
    request.native_pending.store(true, Ordering::Release);
    let verified = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::Unavailable)?
        .block_on(agent_client::native_source::import_brew_pack(
            &repository,
            &pack,
            commit,
            &binding.document_sha256,
            Some((
                &request.native_output,
                request.output_bytes.saturating_sub(1024),
            )),
        ));
    request.native_pending.store(false, Ordering::Release);
    verified?;
    git(request, &repository, &["config", "core.bare", "false"])?;
    git(request, &repository, &["config", "remote.origin.url", uri])?;
    let arguments = vec![
        "--git-dir".into(),
        repository.to_str().ok_or(Error::Configuration)?.into(),
        "--work-tree".into(),
        staged.to_str().ok_or(Error::Configuration)?.into(),
        "checkout".into(),
        "--detach".into(),
        commit.into(),
    ];
    tool(request, "git", arguments, &environment(request)?)?;
    verify_tap(request, &staged, uri, commit)?;
    let metadata = fs::symlink_metadata(&staged)?;
    record.created_directory = Some((metadata.dev(), metadata.ino()));
    save(request, record)?; // Rename preserves this identity even if the worker dies just after publication.
    super::super::macos::rename(&staged, &path, false)?;
    Ok(())
}
fn arguments(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
    tap: &str,
    cask: bool,
) -> Result<Vec<String>, Error> {
    let commands = crate::backend::commands::software_commands(&request.action)?;
    let invocation = match request.operation {
        WorkerOperation::Upgrade => commands.upgrade.ok_or(Error::Unsupported)?,
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious => {
            commands.uninstall.ok_or(Error::Unsupported)?
        }
        _ => commands.install,
    };
    // Native source/coordinate/dependency controls are generated here. Literal trailing native
    // flags would override those controls; this adapter accepts only product environment inputs.
    if !invocation.arguments.is_empty() {
        return Err(Error::Unsupported);
    }
    if matches!(before, Some(SoftwareState::Unknown { .. })) || before.is_none() {
        return Err(Error::Denied);
    }
    let removal = matches!(
        request.operation,
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
    );
    let mut args = vec![
        if removal { "uninstall" } else { "install" }.into(),
        if cask { "--cask" } else { "--formula" }.into(),
    ];
    if !removal {
        if cask {
            args.extend(["--require-sha".into(), "--skip-cask-deps".into()]);
        } else {
            args.extend(["--force-bottle".into(), "--ignore-dependencies".into()]);
        }
    }
    args.push(format!("{tap}/{}", request.action.package));
    Ok(args)
}
fn cache(
    request: &WorkerRequest,
    binding: &wire::SoftwareExportBinding,
    tap: &str,
    cask: bool,
) -> Result<(), Error> {
    for path in [
        request.resource_root.join("cache"),
        request.resource_root.join("cache/downloads"),
        request.resource_root.join("temp"),
    ] {
        platform_private_storage::directory(&path)?;
    }
    let qualified = format!("{tap}/{}", request.action.package);
    let info = tool(
        request,
        "manager",
        vec![
            "info".into(),
            "--json=v2".into(),
            if cask { "--cask" } else { "--formula" }.into(),
            qualified.clone(),
        ],
        &env(request)?,
    )?;
    let info: serde_json::Value = serde_json::from_slice(&info).map_err(|_| Error::Protocol)?;
    let entries = info[if cask { "casks" } else { "formulae" }]
        .as_array()
        .ok_or(Error::Protocol)?;
    if entries.len() != 1
        || entries[0]["tap"].as_str() != Some(tap)
        || (if cask {
            &entries[0]["version"]
        } else {
            &entries[0]["versions"]["stable"]
        })
        .as_str()
            != Some(&request.action.version)
    {
        return Err(Error::Untrusted);
    }
    let args = vec![
        "--cache".into(),
        if cask { "--cask" } else { "--formula" }.into(),
    ];
    let args = args
        .into_iter()
        .chain((!cask).then(|| "--force-bottle".into()))
        .chain([qualified])
        .collect();
    let cache = PathBuf::from(text(tool(request, "manager", args, &env(request)?)?)?);
    if cache.parent() != Some(request.resource_root.join("cache/downloads").as_path()) {
        return Err(Error::Untrusted);
    }
    let leaf = cache
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(Error::Untrusted)?;
    let artifact = binding
        .artifacts
        .iter()
        .find(|a| leaf.starts_with(&format!("{:x}--", Sha256::digest(a.url.as_bytes()))))
        .ok_or(Error::Untrusted)?;
    let hash = crate::backend::plan::hex(&artifact.sha256);
    let material = request
        .materials
        .values()
        .find(|m| m.artifact.sha256.as_str() == hash)
        .ok_or(Error::Untrusted)?;
    let mut input = fs::File::open(&material.path)?;
    if input.metadata()?.len() != artifact.length {
        return Err(Error::Untrusted);
    }
    if cache.try_exists()? {
        execution_runner::staging::verify_staged(
            &cache,
            &material.artifact.sha256,
            &request.resource_root,
            &request.run_as,
            &request.session,
        )
        .map_err(crate::error::app_error)?;
    } else {
        let mut output = platform_private_storage::create_new(&cache)?;
        std::io::copy(&mut input, &mut output)?;
        output.flush()?;
        output.sync_all()?;
        execution_runner::staging::verify_staged(
            &cache,
            &material.artifact.sha256,
            &request.resource_root,
            &request.run_as,
            &request.session,
        )
        .map_err(crate::error::app_error)?;
    }
    Ok(())
}
pub(super) fn execute(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
    record: &mut State,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    let (binding, uri, commit, tap) = authority(request)?;
    if request.operation == WorkerOperation::Prepare {
        if let (
            SoftwareOperation::Install,
            wire::SoftwareTaskDowngrade::Deny,
            Some(SoftwareState::Present { version }),
        ) = (request.intent, request.action.downgrade, before)
        {
            // ref: Homebrew/brew Library/Homebrew/{dev-cmd/ruby.rb,version.rb}.
            tool(request, "manager", vec!["ruby".into(), "-e".into(),
                "require 'version'; exit(Version.new(ARGV.fetch(0)) <= Version.new(ARGV.fetch(1)) ? 0 : 1)".into(),
                "--".into(), version.as_str().into(), request.action.version.clone()], &env(request)?)?;
        }
        prepare(request, record)?;
        return Ok(result(0, None, String::new()));
    }
    let path = record.path.as_ref().ok_or(Error::Untrusted)?;
    if *path != location(request, tap)? {
        return Err(Error::Untrusted);
    }
    verify_tap(request, path, uri, commit)?;
    let formula = path
        .join("Formula")
        .join(format!("{}.rb", request.action.package));
    let cask = path
        .join("Casks")
        .join(format!("{}.rb", request.action.package));
    if formula.is_file() == cask.is_file() {
        return Err(Error::Untrusted);
    }
    let cask = cask.is_file();
    let args = arguments(request, before, tap, cask)?;
    if !matches!(
        request.operation,
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
    ) {
        cache(request, binding, tap, cask)?;
    }
    let (code, stdout, stderr) = run_tool_environment(
        request,
        &request.tool("manager")?,
        &args,
        Some(&env(request)?),
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
    let (_, uri, commit, tap) = authority(request)?;
    if !record.created {
        return Ok(());
    }
    let expected = location(request, tap)?;
    if record.path.as_ref() != Some(&expected) {
        return Err(Error::Untrusted);
    }
    // Only a complete verified checkout can be published here; partial checkout stays inside
    // resource_root and is removed by the existing attempt cleanup after original activity ends.
    if expected.try_exists()? {
        platform_private_storage::PrivateDirectory::open(&expected)?;
        let metadata = fs::symlink_metadata(&expected)?;
        if record.created_directory != Some((metadata.dev(), metadata.ino())) {
            return Err(Error::Untrusted);
        }
        verify_tap(request, &expected, uri, commit)?;
        fs::remove_dir_all(&expected)?;
    }
    if record.parent_created {
        let parent = expected.parent().ok_or(Error::Untrusted)?;
        if parent.try_exists()? && fs::read_dir(parent)?.next().is_none() {
            fs::remove_dir(parent)?;
        }
    }
    Ok(())
}

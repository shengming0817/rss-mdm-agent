//! Windows execution files, process jobs and profile validation.
mod files;
mod process;
use execution_app::Error;
use execution_contract::*;
pub(crate) use execution_ipc::windows_identity::{identity, wide};
use execution_ipc::windows_identity::{nonce, own, raw, security, sid, token_identity, Local};
pub(crate) use files::*;
pub(crate) use process::{spawn, Owner};
use script_plan::ScriptProfile;
use std::{
    os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, OwnedHandle},
    },
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    System::Threading::*,
};
pub(crate) fn profile(profile: &VersionedRef) -> Result<(), Error> {
    if profile.revision.as_str() == "1"
        && (matches!(
            ScriptProfile::from_reference(profile),
            Ok(ScriptProfile::PowerShell7)
        ) || ["native-osquery-template", "native-software-worker"]
            .contains(&profile.id.as_str()))
    {
        Ok(())
    } else {
        Err(Error::Unsupported)
    }
}

pub(crate) fn encoding(_: ArtifactEncoding) -> Result<(), Error> {
    Ok(())
}
pub(crate) fn arguments(
    profile: &VersionedRef,
    args: &[String],
    script: &std::path::Path,
) -> Result<(), Error> {
    if profile.id.as_str() == crate::osquery::PROFILE {
        return if crate::osquery::prefix_matches(args) {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    let path = script.to_str().ok_or(Error::InvalidInput)?;
    if profile.id.as_str() == "native-software-worker" {
        return if args == ["--software-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    self::profile(profile)?;
    let convention = ScriptProfile::from_reference(profile).map_err(|_| Error::Unsupported)?;
    if convention.matches_materialized_file_argv(args, path) {
        Ok(())
    } else {
        Err(Error::Denied)
    }
}

//! Windows execution files, process jobs and profile validation.
mod files;
mod process;
use execution_app::Error;
use execution_contract::*;
use execution_ipc::windows_identity::*;
pub(crate) use files::*;
pub(crate) use process::{spawn, Owner};
use std::{
    ffi::{c_void, OsStr},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, Cryptography::*, *},
    System::{RemoteDesktop::*, Threading::*},
};
pub(crate) fn profile(profile: &VersionedRef) -> Result<(), Error> {
    if profile.revision.as_str() == "1"
        && [
            "native-pwsh7-file",
            "native-osquery-template",
            "native-software-worker",
        ]
        .contains(&profile.id.as_str())
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
    let prefix: &[&str] = match profile.id.as_str() {
        "native-pwsh7-file" => &["-NoLogo", "-NoProfile", "-NonInteractive", "-File", path],
        _ => return Err(Error::Unsupported),
    };
    if args.len() < prefix.len() || !args.iter().zip(prefix).all(|(a, b)| a == b) {
        return Err(Error::Denied);
    }
    Ok(())
}

//! Windows execution files, process jobs and profile validation.
mod files;
mod process;
pub(crate) use crate::windows_profile::{arguments, profile};
use execution_app::Error;
use execution_contract::*;
pub(crate) use execution_ipc::windows_identity::{identity, wide};
use execution_ipc::windows_identity::{nonce, own, raw, security, sid, token_identity, Local};
pub(crate) use files::*;
pub(crate) use process::{spawn, Owner};
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
pub(crate) fn encoding(_: ArtifactEncoding) -> Result<(), Error> {
    Ok(())
}

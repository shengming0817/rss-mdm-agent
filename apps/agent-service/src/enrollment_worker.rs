//! Fixed native MDM entry worker. It never registers an Agent or grants enterprise execution.
//! ref: Microsoft Windows MDM enrollment URI; Apple profiles(1), open(1).
use agent_client::{Error, NativeEnrollmentMaterial};
use execution_contract::{CollectionCompletion, Digest, RunAs, SessionRequirement};
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{io::Write, path::Path};
pub(crate) const TIMEOUT_MS: u64 = 600_000;
pub(crate) const OUTPUT_BYTES: u64 = 4096;
pub(crate) const PROFILE: &str = "native-enrollment-worker";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub task: w::EnrollmentTaskSpec,
    pub material: Option<NativeEnrollmentMaterial>,
    pub profile: Option<(std::path::PathBuf, Digest)>,
    pub image_sha256: Digest,
    pub run_as: RunAs,
    pub session: SessionRequirement,
}
/// Run only a staged input already admitted by the system service and journal.
pub fn run(path: &Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 768 * 1024)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    if input.task.permit != w::TaskPermit::Start {
        return Err(Error::Denied);
    }
    input.task.signing_bytes("worker-validation")?;
    let now = i64::try_from(crate::wall()? / 1000).map_err(|_| Error::Clock)?;
    let material = input.material.as_ref().ok_or(Error::Identity)?;
    material.validate_for(&input.task, now)?;
    let image = std::env::current_exe()?;
    let _lease = execution_runner::staging::verify_retained(&image, &input.image_sha256)
        .map_err(crate::error::app_error)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)?;
    // Emit closure only after the fixed OS launcher has returned and its child was reaped.
    // The enrollment UI is an OS handoff; opening it is not proof of MDM registration.
    let result = open(&input, path)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)?;
    let output = serde_json::to_vec(&result).map_err(|_| Error::Protocol)?;
    let completion = CollectionCompletion {
        native_output_bytes: 0,
        input_sha256: format!("{:x}", Sha256::digest(&bytes)),
        output_sha256: format!("{:x}", Sha256::digest(&output)),
        closed: true,
    };
    std::io::stdout().write_all(&output)?;
    std::io::stderr().write_all(&serde_json::to_vec(&completion).map_err(|_| Error::Protocol)?)?;
    Ok(0)
}
#[cfg(target_os = "macos")]
fn open(input: &Input, _: &Path) -> Result<w::EnrollmentEntryOutcome, Error> {
    if !matches!(input.task.entry, w::EnrollmentEntry::Macos { .. }) {
        return Err(Error::Unsupported);
    }
    let (profile, digest) = input.profile.as_ref().ok_or(Error::Identity)?;
    let _lease = execution_runner::staging::verify_retained(profile, digest)
        .map_err(crate::error::app_error)?;
    let result = std::process::Command::new("/usr/bin/open")
        .arg("-W")
        .arg(profile)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(if result.success() {
        w::EnrollmentEntryOutcome::Opened
    } else {
        w::EnrollmentEntryOutcome::Failed
    })
}
#[cfg(windows)]
fn open(input: &Input, _: &Path) -> Result<w::EnrollmentEntryOutcome, Error> {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::{
            Shell::ShellExecuteW,
            WindowsAndMessaging::{MessageBoxW, IDOK, MB_OKCANCEL, SW_SHOWNORMAL},
        },
    };
    let w::EnrollmentEntry::Windows { server } = &input.task.entry else {
        return Err(Error::Unsupported);
    };
    let material = input.material.as_ref().ok_or(Error::Identity)?;
    let username = material
        .receipt
        .username
        .as_deref()
        .ok_or(Error::Protocol)?;
    let mut url = url::Url::parse("ms-device-enrollment:?").map_err(|_| Error::Protocol)?;
    url.query_pairs_mut()
        .append_pair("mode", "mdm")
        .append_pair("username", username)
        .append_pair("servername", server);
    let wide = |value: &str| value.encode_utf16().chain([0]).collect::<Vec<_>>();
    let prompt=zeroize::Zeroizing::new(wide(&format!("请在 Windows 设备管理注册界面输入以下一次性凭据。系统仍会要求确认。\n用户名：{username}\n密码：{}",material.password.expose())));
    if unsafe {
        MessageBoxW(
            std::ptr::null_mut() as HWND,
            prompt.as_ptr(),
            wide("注册 MDM").as_ptr(),
            MB_OKCANCEL,
        )
    } != IDOK
    {
        return Ok(w::EnrollmentEntryOutcome::UserRequired);
    }
    if crate::wall()? / 1000 >= material.receipt.expires_at as u64 {
        return Err(Error::Expired);
    }
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide(url.as_str()).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    Ok(if result as isize > 32 {
        w::EnrollmentEntryOutcome::Opened
    } else {
        w::EnrollmentEntryOutcome::Failed
    })
}
#[cfg(not(any(windows, target_os = "macos")))]
fn open(_: &Input, _: &Path) -> Result<w::EnrollmentEntryOutcome, Error> {
    Err(Error::Unsupported)
}

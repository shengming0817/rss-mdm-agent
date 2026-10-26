//! Stable account identity from the local directory, never supplied by a task.
use agent_client::Error;
pub(crate) fn generated_uid(subject: &str) -> Result<Option<String>, Error> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn rss_account_generated_uid(uid: u32, output: *mut u8) -> i32;
        }
        let uid: u32 = subject.parse().map_err(|_| Error::Identity)?;
        if uid == 0 || uid.to_string() != subject {
            return Err(Error::Identity);
        }
        let mut bytes = [0u8; 36];
        // SAFETY: native function writes at most 36 bytes; no borrowed native objects escape.
        if unsafe { rss_account_generated_uid(uid, bytes.as_mut_ptr()) } != 0 {
            return Err(Error::Identity);
        }
        let value = std::str::from_utf8(&bytes)
            .map_err(|_| Error::Identity)?
            .to_ascii_lowercase();
        if !rss_mdm_agent_wire::runtime::generated_uid_valid(&value) {
            return Err(Error::Identity);
        }
        Ok(Some(value))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = subject;
        Ok(None)
    }
}

/// Windows authority is native evidence, never a renderer claim.
pub(crate) fn windows_authority(
    subject: &str,
) -> Result<Option<rss_mdm_agent_wire::runtime::AccessWindowsAuthority>, Error> {
    #[cfg(windows)]
    {
        device_access::windows::account::inspect(subject)
            .map(|a| Some(a.source))
            .map_err(|_| Error::Identity)
    }
    #[cfg(not(windows))]
    {
        let _ = subject;
        Ok(None)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalOwner {
    incarnation: String,
}
fn incarnation(subject: &str) -> Result<String, Error> {
    #[cfg(target_os = "macos")]
    {
        generated_uid(subject)?.ok_or(Error::Identity)
    }
    #[cfg(windows)]
    {
        Ok(subject.to_owned())
    } // The authenticated IPC subject is the native token's full SID.
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = subject;
        Err(Error::Unsupported)
    }
}
pub(crate) fn local_owner(
    initiator: &execution_contract::Initiator,
) -> Result<execution_contract::InputValue, Error> {
    use execution_contract::{Initiator, InputValue};
    let (Initiator::Human { os_session } | Initiator::Ai { os_session, .. }) = initiator else {
        return Err(Error::Denied);
    };
    Ok(InputValue::Literal {
        value: serde_json::to_value(LocalOwner {
            incarnation: incarnation(os_session.account.subject.as_str())?,
        })
        .map_err(|_| Error::Protocol)?,
    })
}
pub(crate) fn verify_local_owner(
    plan: &execution_contract::FrozenExecution,
    binding: &execution_app::ServiceBinding,
    subject: &str,
) -> Result<(), Error> {
    use execution_contract::{Initiator, InputValue};
    let p = plan.spec();
    let (Initiator::Human { os_session } | Initiator::Ai { os_session, .. }) = &p.request.initiator
    else {
        return Err(Error::Denied);
    };
    if p.request.authority != binding.authority
        || p.request.target.device != binding.device
        || os_session.device != binding.device
        || os_session.account.subject.as_str() != subject
        || os_session.account.platform != crate::backend::plan::platform()?
    {
        return Err(Error::Denied);
    }
    let Some(InputValue::Literal { value }) = p.request.parameters.get("local-owner") else {
        return Err(Error::Denied);
    };
    let owner: LocalOwner = serde_json::from_value(value.clone()).map_err(|_| Error::Denied)?;
    if owner.incarnation != incarnation(subject)? {
        return Err(Error::Identity);
    }
    Ok(())
}

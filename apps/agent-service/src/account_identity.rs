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

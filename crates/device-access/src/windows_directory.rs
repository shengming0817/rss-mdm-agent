//! Native account provenance. ref: Microsoft Windows-classic-samples CredentialProvider.
//! No directory credentials or cloud token material is read or retained here.
use super::*;
use std::mem::{size_of, size_of_val};
use uuid::Uuid;
use windows::Win32::{Security::Authentication::Identity::*, System::RemoteDesktop::*};

struct Policy(LSA_HANDLE);
impl Policy {
    fn open(access: u32) -> Result<Self, F> {
        let attributes = LSA_OBJECT_ATTRIBUTES {
            Length: size_of::<LSA_OBJECT_ATTRIBUTES>() as u32,
            ..Default::default()
        };
        let mut handle = LSA_HANDLE::default();
        if unsafe { LsaOpenPolicy(None, &attributes, access, &mut handle) }.0 < 0 {
            return Err(F::NativeFailure);
        }
        Ok(Self(handle))
    }
}
impl Drop for Policy {
    fn drop(&mut self) {
        unsafe {
            let _ = LsaClose(self.0);
        }
    }
}
struct Memory(*mut std::ffi::c_void);
impl Drop for Memory {
    fn drop(&mut self) {
        unsafe {
            let _ = LsaFreeMemory(Some(self.0));
        }
    }
}
fn policy_info(class: POLICY_INFORMATION_CLASS) -> Result<Memory, F> {
    let policy = Policy::open(POLICY_VIEW_LOCAL_INFORMATION as u32)?;
    let mut data = Memory(std::ptr::null_mut());
    if unsafe { LsaQueryInformationPolicy(policy.0, class, &mut data.0) }.0 < 0 || data.0.is_null()
    {
        return Err(F::NativeFailure);
    }
    Ok(data)
}
pub(super) fn machine_sid() -> Result<String, F> {
    let data = policy_info(PolicyAccountDomainInformation)?;
    string_sid(unsafe { (*data.0.cast::<POLICY_ACCOUNT_DOMAIN_INFO>()).DomainSid })
}
fn domain_sid() -> Result<Option<String>, F> {
    let data = policy_info(PolicyDnsDomainInformation)?;
    let sid = unsafe { (*data.0.cast::<POLICY_DNS_DOMAIN_INFO>()).Sid };
    if sid.0.is_null() {
        Ok(None)
    } else {
        string_sid(sid).map(Some)
    }
}
fn domain_joined() -> Result<bool, F> {
    let mut name = PWSTR::null();
    let mut status = NETSETUP_JOIN_STATUS::default();
    let result = unsafe { NetGetJoinInformation(PCWSTR::null(), &mut name, &mut status) };
    if !name.is_null() {
        unsafe {
            let _ = NetApiBufferFree(Some(name.0.cast()));
        }
    }
    if result != 0 {
        return Err(F::NativeFailure);
    }
    Ok(status == NetSetupDomainName)
}
struct Aad(*mut DSREG_JOIN_INFO);
impl Drop for Aad {
    fn drop(&mut self) {
        unsafe {
            NetFreeAadJoinInformation(Some(self.0));
        }
    }
}
pub(super) fn current_cloud_join() -> Result<String, F> {
    cloud_join().map(|(_, join)| join)
}
fn cloud_join() -> Result<(Uuid, String), F> {
    if domain_joined()? {
        return Err(F::Unsupported);
    }
    let info =
        Aad(unsafe { NetGetAadJoinInformation(PCWSTR::null()) }.map_err(|_| F::NativeFailure)?);
    if info.0.is_null() {
        return Err(F::AccountChanged);
    }
    let info = unsafe { &*info.0 };
    if info.joinType != DSREG_DEVICE_JOIN {
        return Err(F::Unsupported);
    }
    let tenant =
        Uuid::parse_str(&unsafe { text(info.pszTenantId.0)? }).map_err(|_| F::AccountChanged)?;
    let device =
        Uuid::parse_str(&unsafe { text(info.pszDeviceId.0)? }).map_err(|_| F::AccountChanged)?;
    if tenant.is_nil() || device.is_nil() {
        return Err(F::AccountChanged);
    }
    // Device identity changes on leave/rejoin; certificate renewal is not a new account identity.
    Ok((tenant, format!("entra:{tenant}:{device}")))
}
unsafe fn counted(value: &LSA_UNICODE_STRING) -> Result<String, F> {
    if value.Length % 2 != 0 || value.Length > 1024 || (value.Length > 0 && value.Buffer.is_null())
    {
        return Err(F::AccountChanged);
    }
    if value.Length == 0 {
        return Ok(String::new());
    }
    String::from_utf16(unsafe {
        std::slice::from_raw_parts(value.Buffer.0, value.Length as usize / 2)
    })
    .map_err(|_| F::AccountChanged)
}
/// A real console logon token, never a renderer-supplied name or a SYSTEM process token.
fn cloud_session(sid: &str) -> Result<(String, i64, Vec<String>), F> {
    let session = unsafe { WTSGetActiveConsoleSessionId() };
    if session == u32::MAX {
        return Err(F::NeedsUser);
    }
    let mut token = HANDLE::default();
    unsafe { WTSQueryUserToken(session, &mut token) }.map_err(|_| F::NeedsUser)?;
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let token = Token(token);
    let mut statistics = TOKEN_STATISTICS::default();
    let mut returned = 0;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenStatistics,
            Some((&mut statistics as *mut TOKEN_STATISTICS).cast()),
            size_of::<TOKEN_STATISTICS>() as u32,
            &mut returned,
        )
    }
    .map_err(|_| F::NativeFailure)?;
    let mut user = [0usize; 128];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(user.as_mut_ptr().cast()),
            size_of_val(&user) as u32,
            &mut returned,
        )
    }
    .map_err(|_| F::NativeFailure)?;
    if returned as usize > size_of_val(&user)
        || string_sid(unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid })? != sid
    {
        return Err(F::AccountChanged);
    }
    let mut groups = vec![0usize; 8192];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenGroups,
            Some(groups.as_mut_ptr().cast()),
            (groups.len() * size_of::<usize>()) as u32,
            &mut returned,
        )
    }
    .map_err(|_| F::NativeFailure)?;
    let token_groups = unsafe { &*groups.as_ptr().cast::<TOKEN_GROUPS>() };
    let count = token_groups.GroupCount as usize;
    if returned as usize > groups.len() * size_of::<usize>()
        || count > 1024
        || std::mem::offset_of!(TOKEN_GROUPS, Groups) + count * size_of::<SID_AND_ATTRIBUTES>()
            > returned as usize
    {
        return Err(F::NativeFailure);
    }
    let groups = unsafe { std::slice::from_raw_parts(token_groups.Groups.as_ptr(), count) }
        .iter()
        .map(|group| string_sid(group.Sid))
        .collect::<Result<Vec<_>, _>>()?;
    let mut data = std::ptr::null_mut();
    if unsafe { LsaGetLogonSessionData(&statistics.AuthenticationId, &mut data) }.0 < 0
        || data.is_null()
    {
        return Err(F::NeedsUser);
    }
    struct Logon(*mut SECURITY_LOGON_SESSION_DATA);
    impl Drop for Logon {
        fn drop(&mut self) {
            unsafe {
                let _ = LsaFreeReturnBuffer(self.0.cast());
            }
        }
    }
    let _allocation = Logon(data);
    let data = unsafe { &*data };
    if (data.Size as usize) < size_of::<SECURITY_LOGON_SESSION_DATA>() {
        return Err(F::NativeFailure);
    }
    if data.Session != session
        || ![2, 11].contains(&data.LogonType)
        || string_sid(data.Sid)? != sid
        || !unsafe { counted(&data.AuthenticationPackage)? }.eq_ignore_ascii_case("CloudAP")
        || !unsafe { counted(&data.LogonDomain)? }.eq_ignore_ascii_case("AzureAD")
    {
        return Err(F::AccountChanged);
    }
    let user = unsafe { counted(&data.Upn)? };
    if user.is_empty() || user.contains('\\') || user.chars().any(char::is_control) {
        return Err(F::AccountChanged);
    }
    let millis = data
        .LogonTime
        .checked_div(10_000)
        .and_then(|t| t.checked_sub(11_644_473_600_000))
        .ok_or(F::NativeFailure)?;
    Ok((format!("AzureAD\\{user}"), millis, groups))
}
pub(super) fn inspect(sid: &str) -> Result<Option<Account>, F> {
    if let Some(object_id) = crate::entra_object_id(sid) {
        let (tenant_id, join) = cloud_join()?;
        let (login, time, groups) = cloud_session(sid)?;
        // Native session observation is distinct from persistent role mutation readback.
        let role = if groups.iter().any(|group| group == "S-1-5-32-544") {
            Role::Administrator
        } else {
            Role::Standard
        };
        if cloud_join()? != (tenant_id, join.clone()) {
            return Err(F::AccountChanged);
        }
        let (domain, name) = login.split_once('\\').ok_or(F::AccountChanged)?;
        return Ok(Some(Account {
            domain: domain.into(),
            name: name.into(),
            source: Source::Entra {
                tenant_id,
                object_id,
            },
            join,
            logon_millis: Some(time),
            role,
            enabled: true,
        }));
    }
    let domain = sid
        .rsplit_once('-')
        .map(|(d, _)| d)
        .ok_or(F::AccountChanged)?;
    if domain == machine_sid()? {
        return Ok(None);
    }
    if !domain_joined()? || domain_sid()?.as_deref() != Some(domain) {
        return Err(F::AccountChanged);
    }
    let (name_domain, name) = lookup(&Sid::parse(sid)?)?;
    let role = super::effective_role(&format!("{name_domain}\\{name}"))?;
    Ok(Some(Account {
        domain: name_domain,
        name,
        source: Source::ActiveDirectory {
            domain_sid: domain.into(),
        },
        join: format!("ad:{domain}"),
        logon_millis: None,
        role,
        enabled: true,
    }))
}
/// Revalidate a previously established login without reaching a domain controller or IdP.
pub(super) fn bound(authority: &crate::Authority) -> Result<(), F> {
    let join = validate_source(&authority.sid, &authority.source)?;
    if join != authority.join {
        return Err(F::AccountChanged);
    }
    Ok(())
}
pub(super) fn validate_source(sid: &str, source: &Source) -> Result<String, F> {
    let join = match source {
        Source::Local => {
            let machine = machine_sid()?;
            if sid.rsplit_once('-').map(|(domain, _)| domain) != Some(machine.as_str()) {
                return Err(F::AccountChanged);
            }
            machine
        }
        Source::ActiveDirectory {
            domain_sid: expected,
        } => {
            if !domain_joined()?
                || domain_sid()?.as_ref() != Some(expected)
                || sid.rsplit_once('-').map(|(d, _)| d) != Some(expected.as_str())
            {
                return Err(F::AccountChanged);
            }
            format!("ad:{expected}")
        }
        Source::Entra {
            tenant_id,
            object_id,
        } => {
            let (tenant, join) = cloud_join()?;
            if &tenant != tenant_id || crate::entra_object_id(sid).as_ref() != Some(object_id) {
                return Err(F::AccountChanged);
            }
            join
        }
    };
    Ok(join)
}
pub(super) fn deny_interactive(sid: &str) -> Result<(), F> {
    let policy = Policy::open((POLICY_LOOKUP_NAMES | POLICY_CREATE_ACCOUNT) as u32)?;
    let sid = Sid::parse(sid)?;
    let mut name = wide("SeDenyInteractiveLogonRight");
    let right = LSA_UNICODE_STRING {
        Length: ((name.len() - 1) * 2) as u16,
        MaximumLength: (name.len() * 2) as u16,
        Buffer: PWSTR(name.as_mut_ptr()),
    };
    if unsafe { LsaAddAccountRights(policy.0, sid.0, &[right]) }.0 < 0 {
        return Err(F::NativeFailure);
    }
    let mut rights = std::ptr::null_mut();
    let mut count = 0;
    if unsafe { LsaEnumerateAccountRights(policy.0, sid.0, &mut rights, &mut count) }.0 < 0
        || rights.is_null()
        || count > 1024
    {
        return Err(F::NativeFailure);
    }
    let _allocation = Memory(rights.cast());
    for value in unsafe { std::slice::from_raw_parts(rights, count as usize) } {
        if unsafe { counted(value)? } == "SeDenyInteractiveLogonRight" {
            return Ok(());
        }
    }
    Err(F::NativeFailure)
}

/// Read current local assignments, not the immutable Administrators bit in a logon token.
fn cloud_role(sid: &str, groups: &[String]) -> Result<Role, F> {
    let members = administrators()?;
    if members.iter().any(|member| member == sid) {
        return Ok(Role::Administrator);
    }
    for member in members {
        if crate::entra_object_id(&member).is_some() {
            if groups.contains(&member) {
                return Ok(Role::Administrator);
            }
        } else if inspect_local(&member).is_err() {
            // Unresolved/nested local or directory groups cannot be safely demoted as a user.
            return Err(F::Unsupported);
        }
    }
    Ok(Role::Standard)
}
pub(super) fn configured_cloud_role(sid: &str) -> Result<Role, F> {
    cloud_join()?;
    if administrators()?.iter().any(|member| member == sid) {
        return Ok(Role::Administrator);
    }
    let (_, _, groups) = cloud_session(sid)?;
    cloud_role(sid, &groups)
}

pub(super) fn has_deny_interactive(sid: &str) -> Result<bool, F> {
    let policy = Policy::open(POLICY_LOOKUP_NAMES as u32)?;
    let sid = Sid::parse(sid)?;
    let mut rights = std::ptr::null_mut();
    let mut count = 0;
    let status = unsafe { LsaEnumerateAccountRights(policy.0, sid.0, &mut rights, &mut count) };
    if status.0 as u32 == 0xc0000034 {
        return Ok(false);
    }
    if status.0 < 0 || count > 1024 || (count > 0 && rights.is_null()) {
        return Err(F::NativeFailure);
    }
    if rights.is_null() {
        return Ok(false);
    }
    let _memory = Memory(rights.cast());
    for value in unsafe { std::slice::from_raw_parts(rights, count as usize) } {
        if unsafe { counted(value)? } == "SeDenyInteractiveLogonRight" {
            return Ok(true);
        }
    }
    Ok(false)
}

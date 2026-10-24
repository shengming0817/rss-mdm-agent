//! Shared local SAM identity and role checks; no RSS authorization is inferred here.
use rss_mdm_agent_wire::runtime::{AccessFailure as F, AccessRole as Role};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        NetworkManagement::NetManagement::*,
        Security::{Authorization::*, *},
    },
};
use zeroize::Zeroizing;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
struct Buffer(*mut u8);
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            let _ = NetApiBufferFree(Some(self.0.cast()));
        }
    }
}
struct Sid(PSID);
impl Sid {
    fn parse(value: &str) -> Result<Self, F> {
        let mut sid = PSID::default();
        unsafe { ConvertStringSidToSidW(PCWSTR(wide(value).as_ptr()), &mut sid) }
            .map_err(|_| F::AccountChanged)?;
        Ok(Self(sid))
    }
}
impl Drop for Sid {
    fn drop(&mut self) {
        unsafe {
            LocalFree(Some(HLOCAL(self.0 .0)));
        }
    }
}
unsafe fn text(ptr: *const u16) -> Result<String, F> {
    if ptr.is_null() {
        return Err(F::NativeFailure);
    }
    let mut length = 0;
    while length < 256 && unsafe { *ptr.add(length) } != 0 {
        length += 1;
    }
    if length == 256 {
        return Err(F::NativeFailure);
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(ptr, length) })
        .map_err(|_| F::NativeFailure)
}
fn string_sid(sid: PSID) -> Result<String, F> {
    let mut value = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut value) }.map_err(|_| F::NativeFailure)?;
    let result = unsafe { text(value.0) };
    unsafe {
        LocalFree(Some(HLOCAL(value.0.cast())));
    }
    result
}
fn lookup(sid: &Sid) -> Result<(String, String), F> {
    let mut name = [0u16; 256];
    let mut domain = [0u16; 256];
    let (mut nc, mut dc) = (256, 256);
    let mut kind = SID_NAME_USE::default();
    unsafe {
        LookupAccountSidW(
            PCWSTR::null(),
            sid.0,
            Some(PWSTR(name.as_mut_ptr())),
            &mut nc,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut dc,
            &mut kind,
        )
    }
    .map_err(|_| F::AccountChanged)?;
    Ok((unsafe { text(domain.as_ptr())? }, unsafe {
        text(name.as_ptr())?
    }))
}
pub struct Account {
    pub domain: String,
    pub name: String,
    pub role: Role,
    pub enabled: bool,
}
pub fn inspect(expected: &str) -> Result<Account, F> {
    rss_mdm_agent_wire::runtime::AccessAccount::Windows {
        sid: expected.into(),
    }
    .validate()
    .map_err(|_| F::AccountChanged)?;
    let (domain, name) = lookup(&Sid::parse(expected)?)?;
    let mut buffer = Buffer(std::ptr::null_mut());
    let status = unsafe {
        NetUserGetInfo(
            PCWSTR::null(),
            PCWSTR(wide(&name).as_ptr()),
            23,
            &mut buffer.0,
        )
    };
    if status != 0 || buffer.0.is_null() {
        return Err(F::AccountChanged);
    }
    let info = unsafe { &*buffer.0.cast::<USER_INFO_23>() };
    if string_sid(info.usri23_user_sid)? != expected {
        return Err(F::AccountChanged);
    }
    let mut internet = Buffer(std::ptr::null_mut());
    if unsafe {
        NetUserGetInfo(
            PCWSTR::null(),
            PCWSTR(wide(&name).as_ptr()),
            24,
            &mut internet.0,
        )
    } != 0
        || internet.0.is_null()
        || unsafe {
            (*internet.0.cast::<USER_INFO_24>())
                .usri24_internet_identity
                .as_bool()
        }
    {
        return Err(F::AccountChanged);
    }
    // UF_NORMAL_ACCOUNT excludes trust, machine, and temporary duplicate accounts.
    if info.usri23_flags.0 & UF_NORMAL_ACCOUNT == 0 {
        return Err(F::AccountChanged);
    }
    let role = if administrators()?.iter().any(|sid| sid == expected) {
        Role::Administrator
    } else {
        Role::Standard
    };
    Ok(Account {
        domain,
        name,
        role,
        enabled: info.usri23_flags.0 & UF_ACCOUNTDISABLE.0 == 0,
    })
}
fn administrators() -> Result<Vec<String>, F> {
    let (_, group) = lookup(&Sid::parse("S-1-5-32-544")?)?;
    let mut members = vec![];
    let mut resume = 0;
    for _ in 0..128 {
        let mut buffer = Buffer(std::ptr::null_mut());
        let (mut count, mut total) = (0, 0);
        let status = unsafe {
            NetLocalGroupGetMembers(
                PCWSTR::null(),
                PCWSTR(wide(&group).as_ptr()),
                0,
                &mut buffer.0,
                65536,
                &mut count,
                &mut total,
                Some(&mut resume),
            )
        };
        if ![0, 234].contains(&status) || count > 65536 || (count != 0 && buffer.0.is_null()) {
            return Err(F::NativeFailure);
        }
        for index in 0..count as usize {
            members.push(string_sid(unsafe {
                (*buffer.0.cast::<LOCALGROUP_MEMBERS_INFO_0>().add(index)).lgrmi0_sid
            })?);
        }
        if status == 0 {
            return Ok(members);
        }
        if count == 0 {
            return Err(F::NativeFailure);
        }
    }
    Err(F::NativeFailure)
}
pub fn set_role(sid: &str, role: Role) -> Result<(), F> {
    let current = inspect(sid)?;
    if current.role == role {
        return Ok(());
    }
    if role == Role::Standard {
        // Built-in recovery administrator and the last enabled local administrator are protected.
        if sid.ends_with("-500") {
            return Err(F::RecoveryAccountProtected);
        }
        let mut recovery = false;
        for other in administrators()?.into_iter().filter(|s| s != sid) {
            if inspect(&other).is_ok_and(|account| account.enabled) {
                recovery = true;
                break;
            }
        }
        if !recovery {
            return Err(F::RecoveryAccountProtected);
        }
    }
    let native = Sid::parse(sid)?;
    let member = LOCALGROUP_MEMBERS_INFO_0 {
        lgrmi0_sid: native.0,
    };
    let (_, group) = lookup(&Sid::parse("S-1-5-32-544")?)?;
    let status = unsafe {
        if role == Role::Administrator {
            NetLocalGroupAddMembers(
                PCWSTR::null(),
                PCWSTR(wide(&group).as_ptr()),
                0,
                (&member as *const LOCALGROUP_MEMBERS_INFO_0).cast(),
                1,
            )
        } else {
            NetLocalGroupDelMembers(
                PCWSTR::null(),
                PCWSTR(wide(&group).as_ptr()),
                0,
                (&member as *const LOCALGROUP_MEMBERS_INFO_0).cast(),
                1,
            )
        }
    };
    if status != 0 || inspect(sid)?.role != role {
        return Err(F::NativeFailure);
    }
    Ok(())
}
pub fn create(name: &str, password: &str) -> Result<String, F> {
    if name.is_empty()
        || name.len() > 20
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || v == b'_' || v == b'-')
        || password.is_empty()
        || password.encode_utf16().count() > 256
        || password.contains('\0')
    {
        return Err(F::NativeFailure);
    }
    let mut name = wide(name);
    let mut password = Zeroizing::new(wide(password));
    let info = USER_INFO_1 {
        usri1_name: PWSTR(name.as_mut_ptr()),
        usri1_password: PWSTR(password.as_mut_ptr()),
        usri1_priv: USER_PRIV_USER,
        usri1_flags: USER_ACCOUNT_FLAGS(UF_NORMAL_ACCOUNT | UF_SCRIPT.0 | UF_ACCOUNTDISABLE.0),
        ..Default::default()
    };
    let status = unsafe {
        NetUserAdd(
            PCWSTR::null(),
            1,
            (&info as *const USER_INFO_1).cast(),
            None,
        )
    };
    if status == NERR_UserExists {
        return Err(F::NameConflict);
    }
    if [
        ERROR_PASSWORD_RESTRICTION.0,
        NERR_PasswordTooShort,
        NERR_PasswordTooLong,
        NERR_PasswordNotComplexEnough,
        NERR_PasswordHistConflict,
        NERR_PasswordTooRecent,
    ]
    .contains(&status)
    {
        // These statuses explicitly reject NetUserAdd; no account was created.
        return Err(F::NeedsUser);
    }
    if status != 0 {
        return Err(F::NativeFailure);
    }
    let mut buffer = Buffer(std::ptr::null_mut());
    if unsafe { NetUserGetInfo(PCWSTR::null(), PCWSTR(name.as_ptr()), 23, &mut buffer.0) } != 0
        || buffer.0.is_null()
    {
        return Err(F::NativeFailure);
    }
    let sid = string_sid(unsafe { (*buffer.0.cast::<USER_INFO_23>()).usri23_user_sid })?;
    // Add ordinary Users membership by invariant built-in SID, never localized spelling.
    let (_, group) = lookup(&Sid::parse("S-1-5-32-545")?)?;
    let native = Sid::parse(&sid)?;
    let member = LOCALGROUP_MEMBERS_INFO_0 {
        lgrmi0_sid: native.0,
    };
    let status = unsafe {
        NetLocalGroupAddMembers(
            PCWSTR::null(),
            PCWSTR(wide(&group).as_ptr()),
            0,
            (&member as *const LOCALGROUP_MEMBERS_INFO_0).cast(),
            1,
        )
    };
    if ![0, 1378].contains(&status) {
        return Err(F::NativeFailure);
    }
    Ok(sid)
}
pub fn enable(sid: &str) -> Result<(), F> {
    let account = inspect(sid)?;
    let mut buffer = Buffer(std::ptr::null_mut());
    let name = wide(&account.name);
    if unsafe { NetUserGetInfo(PCWSTR::null(), PCWSTR(name.as_ptr()), 1, &mut buffer.0) } != 0
        || buffer.0.is_null()
    {
        return Err(F::NativeFailure);
    }
    let flags = unsafe { (*buffer.0.cast::<USER_INFO_1>()).usri1_flags };
    let info = USER_INFO_1008 {
        usri1008_flags: USER_ACCOUNT_FLAGS(flags.0 & !UF_ACCOUNTDISABLE.0),
    };
    if unsafe {
        NetUserSetInfo(
            PCWSTR::null(),
            PCWSTR(name.as_ptr()),
            1008,
            (&info as *const USER_INFO_1008).cast(),
            None,
        )
    } != 0
        || !inspect(sid)?.enabled
    {
        return Err(F::NativeFailure);
    }
    Ok(())
}

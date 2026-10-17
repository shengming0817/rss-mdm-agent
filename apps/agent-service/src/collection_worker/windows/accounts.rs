//! Local SAM, direct group membership, WTS and device join observations.
//! ref: microsoft/windows-rs 0.61.3 Win32/NetworkManagement/NetManagement.
use super::*;
use ::windows::{
    core::{BOOL, PCWSTR, PWSTR},
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        NetworkManagement::NetManagement::*,
        Security::{Authorization::*, *},
        System::RemoteDesktop::*,
    },
};
struct Buffer(*mut u8);
impl Drop for Buffer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = NetApiBufferFree(Some(self.0.cast()));
            }
        }
    }
}
/// OS-owned terminated strings; bounds limit untrusted directory metadata.
pub(super) unsafe fn text(ptr: *const u16) -> Result<String, Status> {
    if ptr.is_null() {
        return Err(Status::Missing);
    }
    let mut n = 0;
    while n < 4096 && unsafe { *ptr.add(n) } != 0 {
        n += 1;
    }
    if n == 4096 {
        return Err(Status::Truncated);
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(ptr, n) }).map_err(|_| Status::Invalid)
}
pub(super) fn sid(value: PSID) -> Result<String, Status> {
    unsafe {
        let mut ptr = PWSTR::null();
        ConvertSidToStringSidW(value, &mut ptr).map_err(error)?;
        let result = text(ptr.0);
        LocalFree(Some(HLOCAL(ptr.0.cast())));
        result
    }
}
fn code(value: u32) -> Result<(), Status> {
    match value {
        0 => Ok(()),
        5 => Err(Status::Denied),
        50 => Err(Status::Unsupported),
        _ => Err(Status::Failed),
    }
}
fn selected(selection: &w::AccountSelection, sid: &str) -> bool {
    match selection {
        w::AccountSelection::Local => true,
        w::AccountSelection::Selected { identities } => identities.contains(sid),
    }
}
fn account_rows(
    selection: &w::AccountSelection,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    let mut seen = std::collections::BTreeSet::new();
    let mut resume = 0;
    loop {
        budget.check()?;
        let mut page = Buffer(std::ptr::null_mut());
        let (mut count, mut total) = (0, 0);
        let prior = resume;
        let result = unsafe {
            NetUserEnum(
                PCWSTR::null(),
                0,
                NET_USER_ENUM_FILTER_FLAGS(0),
                &mut page.0,
                65536,
                &mut count,
                &mut total,
                Some(&mut resume),
            )
        };
        if result != 234 {
            code(result)?;
        }
        if count > 65536 || (count > 0 && page.0.is_null()) {
            return Err(Status::Invalid);
        }
        for index in 0..count as usize {
            budget.check()?;
            unsafe {
                let name = (*(page.0.cast::<USER_INFO_0>().add(index))).usri0_name;
                let mut detail = Buffer(std::ptr::null_mut());
                code(NetUserGetInfo(
                    PCWSTR::null(),
                    PCWSTR(name.0),
                    23,
                    &mut detail.0,
                ))?;
                if detail.0.is_null() {
                    return Err(Status::Invalid);
                }
                let info = &*detail.0.cast::<USER_INFO_23>();
                let id = sid(info.usri23_user_sid)?;
                if selected(selection, &id) {
                    seen.insert(id.clone());
                    rows.push(Row::from([
                        ("id".into(), id.into()),
                        ("name".into(), text(info.usri23_name.0)?.into()),
                        ("account_type".into(), "local".into()),
                        (
                            "enabled".into(),
                            if info.usri23_flags.0 & UF_ACCOUNTDISABLE.0 != 0 {
                                "disabled"
                            } else {
                                "enabled"
                            }
                            .into(),
                        ),
                    ]))?;
                }
            }
        }
        if result == 0 {
            break;
        }
        if resume == prior || count == 0 {
            return Err(Status::Partial);
        }
    }
    for (id, name) in [
        ("S-1-5-18", "SYSTEM"),
        ("S-1-5-19", "LOCAL SERVICE"),
        ("S-1-5-20", "NETWORK SERVICE"),
    ] {
        if selected(selection, id) {
            seen.insert(id.to_owned());
            rows.push(Row::from([
                ("id".into(), id.into()),
                ("name".into(), name.into()),
                ("account_type".into(), "system".into()),
                ("enabled".into(), "unknown".into()),
            ]))?;
        }
    }
    match selection {
        // SAM enumeration does not prove the universe of virtual/service principals.
        w::AccountSelection::Local => Err(Status::Partial),
        w::AccountSelection::Selected { identities } if identities != &seen => Err(Status::Partial),
        _ => Ok(()),
    }
}
fn administrators(
    selection: &w::AccountSelection,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    unsafe {
        let mut admin = PSID::default();
        ConvertStringSidToSidW(::windows::core::w!("S-1-5-32-544"), &mut admin).map_err(error)?;
        let mut name = [0u16; 256];
        let mut domain = [0u16; 256];
        let (mut nc, mut dc) = (256, 256);
        let mut kind = SID_NAME_USE::default();
        let result = LookupAccountSidW(
            PCWSTR::null(),
            admin,
            Some(PWSTR(name.as_mut_ptr())),
            &mut nc,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut dc,
            &mut kind,
        );
        LocalFree(Some(HLOCAL(admin.0)));
        result.map_err(error)?;
        let mut resume = 0;
        loop {
            budget.check()?;
            let mut page = Buffer(std::ptr::null_mut());
            let (mut count, mut total) = (0, 0);
            let prior = resume;
            let result = NetLocalGroupGetMembers(
                PCWSTR::null(),
                PCWSTR(name.as_ptr()),
                0,
                &mut page.0,
                65536,
                &mut count,
                &mut total,
                Some(&mut resume),
            );
            if result != 234 {
                code(result)?;
            }
            if count > 65536 || (count > 0 && page.0.is_null()) {
                return Err(Status::Invalid);
            }
            for index in 0..count as usize {
                budget.check()?;
                let member = (*(page.0.cast::<LOCALGROUP_MEMBERS_INFO_0>().add(index))).lgrmi0_sid;
                let id = sid(member)?;
                if !selected(selection, &id) {
                    continue;
                }
                let mut n = [0u16; 256];
                let mut d = [0u16; 256];
                let (mut nl, mut dl) = (256, 256);
                let mut kind = SID_NAME_USE::default();
                let resolved = LookupAccountSidW(
                    PCWSTR::null(),
                    member,
                    Some(PWSTR(n.as_mut_ptr())),
                    &mut nl,
                    Some(PWSTR(d.as_mut_ptr())),
                    &mut dl,
                    &mut kind,
                )
                .is_ok();
                rows.push(Row::from([
                    ("id".into(), id.clone().into()),
                    ("member".into(), id.into()),
                    (
                        "member_type".into(),
                        if resolved {
                            format!("sid_type_{}", kind.0)
                        } else {
                            "unknown".into()
                        }
                        .into(),
                    ),
                    (
                        "resolved".into(),
                        if resolved { "resolved" } else { "unresolved" }.into(),
                    ),
                ]))?;
            }
            if result == 0 {
                break;
            }
            if resume == prior || count == 0 {
                return Err(Status::Partial);
            }
        }
    }
    Ok(())
}
fn sessions(
    selection: &w::AccountSelection,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    struct Wts(*mut std::ffi::c_void);
    impl Drop for Wts {
        fn drop(&mut self) {
            unsafe {
                WTSFreeMemory(self.0);
            }
        }
    }
    unsafe fn field(session: u32, class: WTS_INFO_CLASS) -> Result<String, Status> {
        let mut ptr = PWSTR::null();
        let mut bytes = 0;
        unsafe {
            WTSQuerySessionInformationW(None, session, class, &mut ptr, &mut bytes)
                .map_err(error)?;
        }
        let _owned = Wts(ptr.0.cast());
        if bytes > 8192 {
            return Err(Status::Truncated);
        }
        unsafe { text(ptr.0) }
    }
    unsafe {
        let mut ptr = std::ptr::null_mut();
        let mut count = 0;
        WTSEnumerateSessionsW(None, 0, 1, &mut ptr, &mut count).map_err(error)?;
        let _owned = Wts(ptr.cast());
        if count > 65536 {
            return Err(Status::Truncated);
        }
        let mut partial = false;
        for i in 0..count as usize {
            budget.check()?;
            let session = &*ptr.add(i);
            let user = field(session.SessionId, WTSUserName)?;
            if user.is_empty() {
                continue;
            }
            let domain = field(session.SessionId, WTSDomainName)?;
            let name: Vec<u16> = format!("{domain}\\{user}")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let mut raw = [0u64; 9];
            let mut size = 68;
            let mut d = [0u16; 256];
            let mut dl = 256;
            let mut kind = SID_NAME_USE::default();
            if LookupAccountNameW(
                PCWSTR::null(),
                PCWSTR(name.as_ptr()),
                Some(PSID(raw.as_mut_ptr().cast())),
                &mut size,
                Some(PWSTR(d.as_mut_ptr())),
                &mut dl,
                &mut kind,
            )
            .is_err()
            {
                partial = true;
                continue;
            }
            let id = sid(PSID(raw.as_mut_ptr().cast()))?;
            if selected(selection, &id) {
                rows.push(Row::from([
                    ("id".into(), session.SessionId.to_string().into()),
                    ("owner".into(), id.into()),
                    ("state".into(), format!("wts_{}", session.State.0).into()),
                ]))?;
            }
        }
        if partial {
            return Err(Status::Partial);
        }
    }
    Ok(())
}
pub(super) fn read(
    query: &w::NativeSystemQuery,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    match query {
        w::NativeSystemQuery::WindowsAccounts { section, selection } => match section {
            w::AccountSection::Accounts => account_rows(selection, rows, budget),
            w::AccountSection::Administrators => administrators(selection, rows, budget),
            w::AccountSection::Sessions => sessions(selection, rows, budget),
        },
        w::NativeSystemQuery::WindowsJoin => unsafe {
            let mut name = PWSTR::null();
            let mut state = NETSETUP_JOIN_STATUS::default();
            let ad = NetGetJoinInformation(PCWSTR::null(), &mut name, &mut state);
            let _name = Buffer(name.0.cast());
            let ad = if ad != 0 {
                "unknown"
            } else if state == NetSetupDomainName {
                "joined"
            } else {
                "not_joined"
            };
            budget.check()?;
            let entra = match NetGetAadJoinInformation(PCWSTR::null()) {
                Ok(info) if info.is_null() => "not_joined",
                Ok(info) => {
                    let joined = (*info).joinType == DSREG_DEVICE_JOIN;
                    NetFreeAadJoinInformation(Some(info));
                    if joined {
                        "joined"
                    } else {
                        "not_joined"
                    }
                }
                Err(_) => "unknown",
            };
            let mut registered = BOOL(0);
            let mdm = if ::windows::Win32::Management::MobileDeviceManagementRegistration::IsDeviceRegisteredWithManagement(&mut registered, None).is_err() { "unknown" } else if registered.as_bool() { "joined" } else { "not_joined" };
            rows.push(Row::from([
                ("ad".into(), ad.into()),
                ("entra".into(), entra.into()),
                ("mdm".into(), mdm.into()),
            ]))
        },
        _ => Err(Status::Invalid),
    }
}

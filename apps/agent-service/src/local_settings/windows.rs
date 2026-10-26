//! Fixed Windows settings APIs, always using the frozen account/session.
//! ref: microsoft/windows-rs@0.61.2 Win32/{Networking/WinInet,Devices/Display,System/Power}.
use super::*;
use serde_json::{json, Value};
use windows_sys::Win32::{
    Foundation::GlobalFree,
    System::{Power::*, Registry::*},
    UI::Shell::ShellExecuteW,
};
mod audio;
mod display;
mod energy_mode;
mod proxy;
pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
pub(super) unsafe fn text(pointer: *const u16) -> Result<String, Error> {
    if pointer.is_null() {
        return Ok(String::new());
    }
    let mut len = 0;
    while len < 32768 && unsafe { *pointer.add(len) } != 0 {
        len += 1;
    }
    if len == 32768 {
        return Err(Error::Capacity);
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, len) })
        .map_err(|_| Error::Protocol)
}
pub(super) fn field(key: &str, value: Option<Value>, reason: Option<&str>) -> SettingsField {
    SettingsField {
        key: key.into(),
        writable: false,
        quality: if value.is_some() {
            DataQuality::Complete
        } else {
            DataQuality::Unknown
        },
        value,
        reason: reason.map(str::to_owned),
    }
}
pub(super) fn target(
    id: &str,
    name: &str,
    mut fields: Vec<SettingsField>,
    writable: bool,
    reason: Option<&str>,
) -> SettingsTarget {
    for field in &mut fields {
        field.writable = writable
            && field.value.is_some()
            && matches!(
                field.key.as_str(),
                "http"
                    | "https"
                    | "socks"
                    | "bypass"
                    | "mode"
                    | "origin"
                    | "displayOffMinutes"
                    | "sleepMinutes"
            );
    }
    SettingsTarget {
        id: id.into(),
        name: name.into(),
        fields,
        options: vec![],
        reason: reason.map(str::to_owned),
    }
}
pub(super) fn get<'a>(t: &'a SettingsTarget, key: &str) -> Option<&'a Value> {
    t.fields.iter().find(|f| f.key == key)?.value.as_ref()
}
/// Read only the relevant policy value; absence is distinct from an unreadable value.
pub(super) fn policy_dword(root: HKEY, path: &str, name: &str) -> Result<Option<u32>, Error> {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let code = unsafe {
        RegGetValueW(
            root,
            wide(path).as_ptr(),
            wide(name).as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    match code {
        0 if size == 4 => Ok(Some(value)),
        2 | 3 => Ok(None),
        _ => Err(Error::Denied),
    }
}
fn setting_writable(
    ac: bool,
    setting: &windows_sys::core::GUID,
    active: &windows_sys::core::GUID,
) -> bool {
    unsafe {
        PowerSettingAccessCheckEx(
            if ac {
                ACCESS_AC_POWER_SETTING_INDEX
            } else {
                ACCESS_DC_POWER_SETTING_INDEX
            },
            setting,
            KEY_WRITE,
        ) == 0
            && PowerSettingAccessCheckEx(ACCESS_ACTIVE_SCHEME, std::ptr::null(), KEY_WRITE) == 0
            && PowerSettingAccessCheckEx(ACCESS_SCHEME, active, KEY_WRITE) == 0
    }
}
pub(super) fn read(section: SettingsSection) -> Result<Vec<SettingsTarget>, Error> {
    match section {
        SettingsSection::Network => proxy::read(),
        SettingsSection::Display => display::read(),
        SettingsSection::Power | SettingsSection::Lock => power(),
        SettingsSection::Sound => audio::read(),
        _ => Err(Error::Unsupported),
    }
}
pub(super) fn open(section: SettingsSection) -> Result<(), Error> {
    let page = match section {
        SettingsSection::Network => "ms-settings:network-proxy",
        SettingsSection::Display => "ms-settings:display",
        SettingsSection::Sound => "ms-settings:sound",
        SettingsSection::Power | SettingsSection::Lock => "ms-settings:powersleep",
        SettingsSection::Storage => "ms-settings:storagesense",
        SettingsSection::Security => "ms-settings:windowsdefender",
        SettingsSection::About => "ms-settings:about",
    };
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide(page).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        return Err(Error::Unavailable);
    }
    Ok(())
}
pub(super) fn apply(patch: &SettingsPatch, before: &SettingsSnapshot) -> Result<(), Error> {
    match patch {
        SettingsPatch::Proxy { .. } => proxy::apply(patch, before),
        SettingsPatch::Display { .. } => display::apply(patch, before, false),
        SettingsPatch::Power { .. } => power_apply(patch, before),
    }
}
pub(super) fn finish_display(
    patch: &SettingsPatch,
    before: &SettingsSnapshot,
    keep: bool,
) -> Result<(), Error> {
    display::finish(patch, before, keep)
}
fn scheme() -> Result<windows_sys::core::GUID, Error> {
    let mut pointer = std::ptr::null_mut();
    if unsafe { PowerGetActiveScheme(std::ptr::null_mut(), &mut pointer) } != 0 || pointer.is_null()
    {
        return Err(Error::Unavailable);
    }
    let guid = unsafe { *pointer };
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(pointer.cast());
    }
    Ok(guid)
}
fn power() -> Result<Vec<SettingsTarget>, Error> {
    use windows_sys::Win32::System::SystemServices::{
        GUID_SLEEP_SUBGROUP, GUID_STANDBY_TIMEOUT, GUID_VIDEO_POWERDOWN_TIMEOUT,
        GUID_VIDEO_SUBGROUP,
    };
    let scheme = scheme()?;
    let mut targets = vec![];
    for (ac, id, name) in [(true, "ac", "外接电源"), (false, "dc", "电池供电")] {
        let mut fields = vec![field(
            "scheme",
            Some(json!(format!(
                "{:08x}-{:04x}-{:04x}-{:02x?}",
                scheme.data1, scheme.data2, scheme.data3, scheme.data4
            ))),
            None,
        )];
        for (key, group, setting) in [
            (
                "displayOffMinutes",
                GUID_VIDEO_SUBGROUP,
                GUID_VIDEO_POWERDOWN_TIMEOUT,
            ),
            ("sleepMinutes", GUID_SLEEP_SUBGROUP, GUID_STANDBY_TIMEOUT),
        ] {
            let mut value = 0;
            let code = unsafe {
                if ac {
                    PowerReadACValueIndex(
                        std::ptr::null_mut(),
                        &scheme,
                        &group,
                        &setting,
                        &mut value,
                    )
                } else {
                    PowerReadDCValueIndex(
                        std::ptr::null_mut(),
                        &scheme,
                        &group,
                        &setting,
                        &mut value,
                    )
                }
            };
            fields.push(field(
                key,
                (code == 0 && value % 60 == 0).then(|| json!(value / 60)),
                (code != 0 || value % 60 != 0).then_some("无法取得精确分钟值，保留系统设置"),
            ));
        }
        let mode = energy_mode::read(ac).ok();
        fields.push(field(
            "mode",
            mode.as_ref().map(|m| json!(m)),
            mode.is_none()
                .then_some("当前平台未提供可可靠编辑的电源模式"),
        ));
        fields.push(field(
            "effectiveMode",
            None,
            Some("用户选择可能被系统、硬件或组织策略覆盖"),
        ));
        let mut t = target(id, name, fields, true, None);
        for field in &mut t.fields {
            let setting = match field.key.as_str() {
                "displayOffMinutes" => Some(GUID_VIDEO_POWERDOWN_TIMEOUT),
                "sleepMinutes" => Some(GUID_STANDBY_TIMEOUT),
                _ => None,
            };
            if let Some(setting) = setting {
                field.writable &= setting_writable(ac, &setting, &scheme);
                if !field.writable && field.reason.is_none() {
                    field.reason = Some("当前电源设置受策略或原生权限限制".into());
                }
            }
        }
        if mode.is_some() {
            t.options = vec![
                SettingsOption {
                    id: "efficiency".into(),
                    label: "最佳能效".into(),
                },
                SettingsOption {
                    id: "balanced".into(),
                    label: "平衡".into(),
                },
                SettingsOption {
                    id: "performance".into(),
                    label: "最佳性能".into(),
                },
            ];
        }
        targets.push(t);
    }
    let mut status = SYSTEM_POWER_STATUS::default();
    let fields = if unsafe { GetSystemPowerStatus(&mut status) } != 0 {
        vec![
            field(
                "source",
                match status.ACLineStatus {
                    0 => Some(json!("电池供电")),
                    1 => Some(json!("外接电源")),
                    _ => None,
                },
                Some("供电来源未知"),
            ),
            field(
                "batteryPresent",
                (status.BatteryFlag != 255).then(|| json!(status.BatteryFlag & 128 == 0)),
                Some("电池是否存在尚未确认"),
            ),
        ]
    } else {
        vec![field("source", None, Some("无法读取供电状态"))]
    };
    targets.push(target("power-source", "供电状态", fields, false, None));
    targets.push(target(
        "login",
        "返回电脑时",
        vec![field(
            "requireLogin",
            None,
            Some("登录要求需要在系统与组织策略中确认"),
        )],
        false,
        Some("请通过系统设置核对登录要求"),
    ));
    Ok(targets)
}
fn power_apply(patch: &SettingsPatch, before: &SettingsSnapshot) -> Result<(), Error> {
    use windows_sys::Win32::System::SystemServices::{
        GUID_SLEEP_SUBGROUP, GUID_STANDBY_TIMEOUT, GUID_VIDEO_POWERDOWN_TIMEOUT,
        GUID_VIDEO_SUBGROUP,
    };
    let SettingsPatch::Power {
        target,
        display_off_minutes,
        sleep_minutes,
        mode,
    } = patch
    else {
        return Err(Error::Unsupported);
    };
    check_change(patch, before, &power()?)?;
    let scheme = scheme()?;
    for (value, group, setting) in [
        (
            display_off_minutes,
            GUID_VIDEO_SUBGROUP,
            GUID_VIDEO_POWERDOWN_TIMEOUT,
        ),
        (sleep_minutes, GUID_SLEEP_SUBGROUP, GUID_STANDBY_TIMEOUT),
    ] {
        if let Some(minutes) = value {
            if !setting_writable(target == "ac", &setting, &scheme) {
                return Err(Error::Denied);
            }
            let code = unsafe {
                if target == "ac" {
                    PowerWriteACValueIndex(
                        std::ptr::null_mut(),
                        &scheme,
                        &group,
                        &setting,
                        u32::from(*minutes) * 60,
                    )
                } else {
                    PowerWriteDCValueIndex(
                        std::ptr::null_mut(),
                        &scheme,
                        &group,
                        &setting,
                        u32::from(*minutes) * 60,
                    )
                }
            };
            if code != 0 {
                return Err(Error::Denied);
            }
        }
    }
    if let Some(mode) = mode {
        energy_mode::set(target == "ac", mode)?;
    }
    if (display_off_minutes.is_some() || sleep_minutes.is_some())
        && unsafe { PowerSetActiveScheme(std::ptr::null_mut(), &scheme) } != 0
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}

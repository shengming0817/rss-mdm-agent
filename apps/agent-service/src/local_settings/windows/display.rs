//! CCD native mode/layout changes; no registry DPI mutation or topology replacement.
//! ref: microsoft/windows-rs@0.61.2 Win32/Devices/Display (QueryDisplayConfig/SetDisplayConfig).
use super::*;
use windows_sys::Win32::{Devices::Display::*, Graphics::Gdi::*};
struct Topology {
    paths: Vec<DISPLAYCONFIG_PATH_INFO>,
    modes: Vec<DISPLAYCONFIG_MODE_INFO>,
}
fn topology() -> Result<Topology, Error> {
    for _ in 0..3 {
        let (mut paths, mut modes) = (0, 0);
        if unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths, &mut modes) }
            != 0
        {
            return Err(Error::Unavailable);
        }
        if paths > 64 || modes > 256 {
            return Err(Error::Capacity);
        }
        let mut result = Topology {
            paths: vec![DISPLAYCONFIG_PATH_INFO::default(); paths as usize],
            modes: vec![DISPLAYCONFIG_MODE_INFO::default(); modes as usize],
        };
        let code = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths,
                result.paths.as_mut_ptr(),
                &mut modes,
                result.modes.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        if code == 122 {
            continue;
        }
        if code != 0 {
            return Err(Error::Unavailable);
        }
        result.paths.truncate(paths as usize);
        result.modes.truncate(modes as usize);
        return Ok(result);
    }
    Err(Error::Conflict)
}
fn source(path: &DISPLAYCONFIG_PATH_INFO) -> Result<DISPLAYCONFIG_SOURCE_DEVICE_NAME, Error> {
    let mut value = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            adapterId: path.sourceInfo.adapterId,
            id: path.sourceInfo.id,
        },
        ..Default::default()
    };
    if unsafe { DisplayConfigGetDeviceInfo(&mut value.header) } != 0 {
        return Err(Error::Unavailable);
    }
    Ok(value)
}
fn monitor(path: &DISPLAYCONFIG_PATH_INFO) -> Result<(String, String), Error> {
    let mut value = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        ..Default::default()
    };
    if unsafe { DisplayConfigGetDeviceInfo(&mut value.header) } != 0 {
        return Err(Error::Unavailable);
    }
    let id = String::from_utf16_lossy(
        &value.monitorDevicePath[..value
            .monitorDevicePath
            .iter()
            .position(|v| *v == 0)
            .ok_or(Error::Protocol)?],
    );
    let name = String::from_utf16_lossy(
        &value.monitorFriendlyDeviceName[..value
            .monitorFriendlyDeviceName
            .iter()
            .position(|v| *v == 0)
            .ok_or(Error::Protocol)?],
    );
    if id.is_empty() {
        return Err(Error::Unavailable);
    }
    Ok((id, name))
}
fn mode(device: &[u16; 32], index: u32) -> Option<DEVMODEW> {
    let mut mode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    (unsafe { EnumDisplaySettingsExW(device.as_ptr(), index, &mut mode, 0) } != 0).then_some(mode)
}
fn mode_id(mode: &DEVMODEW) -> String {
    format!(
        "{}x{}@{}",
        mode.dmPelsWidth, mode.dmPelsHeight, mode.dmDisplayFrequency
    )
}
pub(super) fn read() -> Result<Vec<SettingsTarget>, Error> {
    let topology = topology()?;
    let mut result = vec![];
    let managed = policy_present(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Policies\Microsoft\Windows\Control Panel\Desktop",
    );
    for path in &topology.paths {
        let (id, name) = monitor(path)?;
        let source = source(path)?;
        let current =
            mode(&source.viewGdiDeviceName, ENUM_CURRENT_SETTINGS).ok_or(Error::Unavailable)?;
        let position = unsafe { current.Anonymous1.Anonymous2.dmPosition };
        let cloned = topology
            .paths
            .iter()
            .filter(|p| {
                p.sourceInfo.adapterId.LowPart == path.sourceInfo.adapterId.LowPart
                    && p.sourceInfo.adapterId.HighPart == path.sourceInfo.adapterId.HighPart
                    && p.sourceInfo.id == path.sourceInfo.id
            })
            .count()
            > 1;
        let mut target = target(
            &id,
            &name,
            vec![
                field("mode", Some(json!(mode_id(&current))), None),
                field("width", Some(json!(current.dmPelsWidth)), None),
                field("height", Some(json!(current.dmPelsHeight)), None),
                field("origin", Some(json!({"x":position.x,"y":position.y})), None),
                field(
                    "scaling",
                    None,
                    Some("系统未提供可靠的公开每显示器 DPI 写入 API，请打开系统显示设置"),
                ),
            ],
            !managed && !cloned,
            if managed {
                Some("由组织显示策略管理")
            } else if cloned {
                Some("复制显示拓扑需要通过系统设置修改")
            } else {
                None
            },
        );
        let mut seen = std::collections::BTreeSet::new();
        for index in 0..4096 {
            let Some(mode) = mode(&source.viewGdiDeviceName, index) else {
                break;
            };
            let id = mode_id(&mode);
            if seen.insert(id.clone()) {
                if target.options.len() >= 256 {
                    return Err(Error::Capacity);
                }
                target.options.push(SettingsOption {
                    id,
                    label: format!(
                        "{} × {} · {} Hz",
                        mode.dmPelsWidth, mode.dmPelsHeight, mode.dmDisplayFrequency
                    ),
                });
            }
        }
        result.push(target);
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}
pub(super) fn apply(
    patch: &SettingsPatch,
    before: &SettingsSnapshot,
    permanent: bool,
) -> Result<(), Error> {
    let SettingsPatch::Display {
        target,
        mode: selection,
        origin,
    } = patch
    else {
        return Err(Error::Unsupported);
    };
    if !permanent && read()? != before.targets {
        return Err(Error::Conflict);
    }
    if !before.targets.iter().any(|t| t.id == *target && t.writable) {
        return Err(Error::Denied);
    }
    if policy_present(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Policies\Microsoft\Windows\Control Panel\Desktop",
    ) {
        return Err(Error::Denied);
    }
    let mut topology = topology()?;
    let mut found = false;
    for path in &mut topology.paths {
        if monitor(path)?.0 != *target {
            continue;
        }
        found = true;
        let source = source(path)?;
        let index = unsafe { path.sourceInfo.Anonymous.modeInfoIdx } as usize;
        let slot = topology
            .modes
            .get_mut(index)
            .filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE)
            .ok_or(Error::Unsupported)?;
        let mut current = unsafe { slot.Anonymous.sourceMode };
        if let Some(id) = selection {
            let mut selected = None;
            for index in 0..4096 {
                let Some(mode) = mode(&source.viewGdiDeviceName, index) else {
                    break;
                };
                if mode_id(&mode) == *id {
                    selected = Some(mode);
                    break;
                }
            }
            let selected = selected.ok_or(Error::Conflict)?;
            current.width = selected.dmPelsWidth;
            current.height = selected.dmPelsHeight;
            path.targetInfo.Anonymous.modeInfoIdx = u32::MAX;
            path.targetInfo.refreshRate = DISPLAYCONFIG_RATIONAL {
                Numerator: selected.dmDisplayFrequency,
                Denominator: 1,
            };
        }
        if let Some(origin) = origin {
            current.position.x = origin.x;
            current.position.y = origin.y;
        }
        slot.Anonymous.sourceMode = current;
    }
    if !found {
        return Err(Error::Conflict);
    }
    let base = SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES;
    if unsafe {
        SetDisplayConfig(
            topology.paths.len() as u32,
            topology.paths.as_ptr(),
            topology.modes.len() as u32,
            topology.modes.as_ptr(),
            base | SDC_VALIDATE,
        )
    } != 0
    {
        return Err(Error::Unsupported);
    }
    if unsafe {
        SetDisplayConfig(
            topology.paths.len() as u32,
            topology.paths.as_ptr(),
            topology.modes.len() as u32,
            topology.modes.as_ptr(),
            base | SDC_APPLY | if permanent { SDC_SAVE_TO_DATABASE } else { 0 },
        )
    } != 0
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}
pub(super) fn finish(
    patch: &SettingsPatch,
    before: &SettingsSnapshot,
    keep: bool,
) -> Result<(), Error> {
    if keep {
        return apply(patch, before, true);
    }
    let SettingsPatch::Display { target, .. } = patch else {
        return Err(Error::Unsupported);
    };
    let original = before
        .targets
        .iter()
        .find(|t| t.id == *target)
        .ok_or(Error::Conflict)?;
    let mode = get(original, "mode")
        .and_then(Value::as_str)
        .ok_or(Error::Protocol)?
        .into();
    let origin: DisplayOrigin =
        serde_json::from_value(get(original, "origin").ok_or(Error::Protocol)?.clone())
            .map_err(|_| Error::Protocol)?;
    // The worker already compares the complete active preview before entering this operation.
    let current = SettingsSnapshot {
        targets: read()?,
        ..before.clone()
    };
    apply(
        &SettingsPatch::Display {
            target: target.clone(),
            mode: Some(mode),
            origin: Some(origin),
        },
        &current,
        false,
    )
}

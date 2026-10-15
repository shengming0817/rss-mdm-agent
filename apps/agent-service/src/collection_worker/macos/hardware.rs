use super::*;
use serde_json::json;
pub(super) fn collect(
    c: &Context<'_>,
    section: w::MacosHardwareSection,
    rows: &mut Rows<'_>,
) -> Result<()> {
    use w::MacosHardwareSection::*;
    match section {
        Identity | Firmware => {
            let value = c.command_plist(
                "system_profiler",
                &["-xml", "SPHardwareDataType", "-detailLevel", "basic"],
            )?;
            let blocks = value.as_array().ok_or(Status::Invalid)?;
            let hardware = blocks
                .iter()
                .filter_map(plist::Value::as_dictionary)
                .find(|d| {
                    d.get("_dataType").and_then(plist::Value::as_string)
                        == Some("SPHardwareDataType")
                })
                .ok_or(Status::Missing)?;
            let items = array(hardware, "_items")?;
            if items.len() != 1 {
                return Err(Status::Invalid.into());
            }
            let d = object(&items[0])?;
            let r = if section == Identity {
                row(&[
                    ("hardware_uuid", text(d, "platform_UUID")?.into()),
                    ("serial", text(d, "serial_number")?.into()),
                    ("model", text(d, "machine_model")?.into()),
                ])
            } else {
                row(&[("version", text(d, "boot_rom_version")?.into())])
            };
            rows.push(r)?;
        }
        Os => {
            let value = c.plist(Path::new(
                "/System/Library/CoreServices/SystemVersion.plist",
            ))?;
            let d = object(&value)?;
            // hw.optional.arm64 reports the hardware even when the agent runs under Rosetta.
            let arch = if sys_number("hw.optional.arm64").unwrap_or(0) == 1 {
                "aarch64"
            } else {
                "x86_64"
            };
            rows.push(row(&[
                ("name", text(d, "ProductName")?.into()),
                ("version", text(d, "ProductVersion")?.into()),
                ("build", text(d, "ProductBuildVersion")?.into()),
                ("architecture", arch.into()),
            ]))?;
        }
        Cpu => {
            rows.push(row(&[
                ("model", sys_text("machdep.cpu.brand_string")?.into()),
                ("physical_cores", json!(sys_number("hw.physicalcpu")?)),
                ("logical_cores", json!(sys_number("hw.logicalcpu")?)),
            ]))?;
        }
        Memory => {
            rows.push(row(&[(
                "total_bytes",
                sys_number("hw.memsize")?.to_string().into(),
            )]))?;
        }
        Disks => {
            let value = c.command_plist("diskutil", &["list", "-plist", "physical"])?;
            for disk in array(object(&value)?, "AllDisksAndPartitions")? {
                c.check()?;
                let d = object(disk)?;
                let device = text(d, "DeviceIdentifier")?;
                let info = c.command_plist("diskutil", &["info", "-plist", device])?;
                let info = object(&info)?;
                // Prefer a persistent disk UUID. A device node alone is only a current coordinate.
                let id = text(info, "DiskUUID").or_else(|_| text(info, "DeviceTreePath"))?;
                rows.push(row(&[
                    ("id", id.into()),
                    ("model", text(info, "MediaName")?.into()),
                    ("size_bytes", uint(info, "TotalSize")?.to_string().into()),
                ]))?;
            }
        }
        ApfsContainers => {
            let value = c.command_plist("diskutil", &["apfs", "list", "-plist"])?;
            containers(&value, rows)?;
        }
        Volumes => {
            let value = c.command_plist("diskutil", &["list", "-plist"])?;
            let root = c.command_plist("diskutil", &["info", "-plist", "/"])?;
            let apfs = c.command_plist("diskutil", &["apfs", "list", "-plist"])?;
            let mut volumes = BTreeMap::new();
            for container in array(object(&apfs)?, "Containers")? {
                let container = object(container)?;
                for volume in array(container, "Volumes")? {
                    let volume = object(volume)?;
                    if volumes
                        .insert(text(volume, "DeviceIdentifier")?, (container, volume))
                        .is_some()
                    {
                        return Err(Status::Invalid.into());
                    }
                }
            }
            for device in array(object(&value)?, "AllDisks")? {
                c.check()?;
                let device = device.as_string().ok_or(Status::Invalid)?;
                let value = c.command_plist("diskutil", &["info", "-plist", device])?;
                if let Some(r) = volume_row(
                    object(&value)?,
                    volumes.get(device).copied(),
                    object(&root)?,
                )? {
                    rows.push(r)?;
                }
            }
        }
        Battery => {
            let value = c.command_plist("ioreg", &["-a", "-r", "-c", "AppleSmartBattery"])?;
            let batteries = value.as_array().ok_or(Status::Invalid)?;
            if batteries.is_empty() {
                return Err(Status::Missing.into());
            }
            if batteries.len() != 1 {
                return Err(Status::Unsupported.into());
            }
            let d = object(&batteries[0])?;
            let current = uint(d, "CurrentCapacity")?;
            let maximum = uint(d, "MaxCapacity")?;
            if maximum == 0 || current > maximum {
                return Err(Status::Invalid.into());
            }
            rows.push(row(&[(
                "charge_percent",
                json!(current as f64 * 100.0 / maximum as f64),
            )]))?;
        }
        Network => network(c, rows)?,
    }
    Ok(())
}
// diskutil list includes mounted snapshots. Only the APFS container manifest owns
// logical-volume identities; boot snapshots are related through the volume group.
pub(super) fn volume_row(
    d: &plist::Dictionary,
    apfs: Option<(&plist::Dictionary, &plist::Dictionary)>,
    root: &plist::Dictionary,
) -> Result<Option<Row>> {
    if d.get("APFSSnapshot").and_then(plist::Value::as_boolean) == Some(true) {
        return Ok(None);
    }
    let Some(filesystem) = d.get("FilesystemType").and_then(plist::Value::as_string) else {
        return Ok(None);
    };
    let (id, system) = if filesystem == "apfs" {
        let (_, v) = apfs.ok_or(Status::Invalid)?;
        let system_role = array(v, "Roles")?
            .iter()
            .any(|r| r.as_string() == Some("System"));
        let group = d.get("APFSVolumeGroupID").and_then(plist::Value::as_string);
        let root_group = root
            .get("APFSVolumeGroupID")
            .and_then(plist::Value::as_string);
        let id = text(v, "APFSVolumeUUID")?;
        (
            id,
            system_role
                && (group.is_some() && group == root_group
                    || Some(id) == root.get("VolumeUUID").and_then(plist::Value::as_string)),
        )
    } else {
        let id = text(d, "VolumeUUID")?;
        (
            id,
            Some(id) == root.get("VolumeUUID").and_then(plist::Value::as_string),
        )
    };
    let mut r = row(&[
        ("id", id.into()),
        ("name", text(d, "VolumeName")?.into()),
        ("filesystem", filesystem.into()),
        ("size_bytes", uint(d, "TotalSize")?.to_string().into()),
        ("system_volume", json!(system)),
    ]);
    if let Some(free) = d
        .get("FreeSpace")
        .and_then(plist::Value::as_unsigned_integer)
    {
        r.insert("free_bytes".into(), free.to_string().into());
    }
    if let Some((container, _)) = apfs {
        r.insert(
            "container_id".into(),
            text(container, "APFSContainerUUID")?.into(),
        );
    }
    Ok(Some(r))
}
pub(super) fn containers(value: &plist::Value, rows: &mut Rows<'_>) -> Result<()> {
    for value in array(object(value)?, "Containers")? {
        let d = object(value)?;
        let size = uint(d, "CapacityCeiling")?;
        let free = uint(d, "CapacityFree")?;
        if free > size {
            return Err(Status::Invalid.into());
        }
        rows.push(row(&[
            ("id", text(d, "APFSContainerUUID")?.into()),
            ("size_bytes", size.to_string().into()),
            ("free_bytes", free.to_string().into()),
        ]))?;
    }
    Ok(())
}
fn sys(name: &str) -> Result<Vec<u8>> {
    let name = CString::new(name).map_err(|_| Status::Invalid)?;
    let mut buffer = vec![0u8; 4096];
    let mut size = buffer.len();
    // SAFETY: fixed read-only sysctl name, bounded writable buffer, no replacement value.
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(Status::Missing.into());
    }
    buffer.truncate(size);
    Ok(buffer)
}
fn sys_text(name: &str) -> Result<String> {
    String::from_utf8(sys(name)?)
        .map(|s| s.trim_end_matches('\0').into())
        .map_err(|_| Status::Invalid.into())
}
fn sys_number(name: &str) -> Result<u64> {
    let b = sys(name)?;
    match b.len() {
        4 => Ok(u32::from_ne_bytes(b.try_into().unwrap()).into()),
        8 => Ok(u64::from_ne_bytes(b.try_into().unwrap())),
        _ => Err(Status::Invalid.into()),
    }
}
fn network(c: &Context<'_>, rows: &mut Rows<'_>) -> Result<()> {
    struct Interfaces(*mut libc::ifaddrs);
    impl Drop for Interfaces {
        fn drop(&mut self) {
            unsafe { libc::freeifaddrs(self.0) };
        }
    }
    let mut first = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut first) } != 0 {
        return Err(Status::Denied.into());
    }
    let _guard = Interfaces(first);
    let mut cursor = first;
    let mut interfaces: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
    while !cursor.is_null() {
        c.check()?;
        let entry = unsafe { &*cursor };
        let name = unsafe { std::ffi::CStr::from_ptr(entry.ifa_name) }
            .to_str()
            .map_err(|_| Status::Invalid)?
            .to_owned();
        let values = interfaces.entry(name).or_default();
        if !entry.ifa_addr.is_null() {
            let family = unsafe { (*entry.ifa_addr).sa_family as i32 };
            let address = match family {
                libc::AF_INET => {
                    let a = unsafe { &*entry.ifa_addr.cast::<libc::sockaddr_in>() };
                    Some(std::net::Ipv4Addr::from(a.sin_addr.s_addr.to_ne_bytes()).to_string())
                }
                libc::AF_INET6 => {
                    let a = unsafe { &*entry.ifa_addr.cast::<libc::sockaddr_in6>() };
                    Some(std::net::Ipv6Addr::from(a.sin6_addr.s6_addr).to_string())
                }
                _ => None,
            };
            if let Some(a) = address {
                values.insert(a);
            }
        }
        if values.len() > 64 || interfaces.len() > 4096 {
            return Err(Status::Truncated.into());
        }
        cursor = entry.ifa_next;
    }
    for (name, addresses) in interfaces {
        rows.push(row(&[
            ("id", name.clone().into()),
            ("name", name.into()),
            ("addresses", json!(addresses)),
        ]))?;
    }
    Ok(())
}

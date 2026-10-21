//! Fixed native Windows readings; no WMI scripts or fabricated provider success.
use super::*;
use windows_sys::Win32::{
    Foundation::FILETIME,
    NetworkManagement::IpHelper::*,
    Storage::FileSystem::*,
    System::{
        SystemInformation::*,
        Threading::{
            GetActiveProcessorCount, GetActiveProcessorGroupCount, GetSystemTimes,
            ALL_PROCESSOR_GROUPS,
        },
    },
};
fn ticks(t: FILETIME) -> u64 {
    u64::from(t.dwHighDateTime) << 32 | u64::from(t.dwLowDateTime)
}
pub(super) fn boot() -> Result<String, Error> {
    // Windows reader uses a process epoch: a service restart deliberately establishes
    // a new counter origin instead of inferring boot identity from a movable UTC clock.
    static EPOCH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    Ok(EPOCH
        .get_or_init(|| uuid::Uuid::new_v4().to_string())
        .clone())
}
pub(super) fn cpu() -> Result<Cpu, Error> {
    // SAFETY: these APIs retain no pointers; three initialized FILETIME outputs stay
    // writable for the call and are read only after GetSystemTimes succeeds.
    unsafe {
        // GetSystemTimes covers one processor group; never mislabel a partial group as device-wide.
        if GetActiveProcessorGroupCount() != 1 {
            return Err(Error::Unsupported);
        }
        let processors = GetActiveProcessorCount(ALL_PROCESSOR_GROUPS);
        if processors == 0 {
            return Err(Error::Unavailable);
        }
        let mut idle = std::mem::zeroed();
        let mut kernel = std::mem::zeroed();
        let mut user = std::mem::zeroed();
        if GetSystemTimes(&mut idle, &mut kernel, &mut user) == 0 {
            return Err(Error::Unavailable);
        }
        let total = ticks(kernel)
            .checked_add(ticks(user))
            .ok_or(Error::Capacity)?;
        let busy = total.checked_sub(ticks(idle)).ok_or(Error::Protocol)?;
        Ok(Cpu {
            boot: boot()?,
            processors,
            counters: vec![ticks(idle), ticks(kernel), ticks(user)],
            busy,
            total,
        })
    }
}
pub(super) fn memory() -> Result<(u64, u64), Error> {
    // SAFETY: MEMORYSTATUSEX is a plain C structure, zero-initialized with its
    // required byte length. The API writes within it and retains no reference.
    unsafe {
        let mut memory: MEMORYSTATUSEX = std::mem::zeroed();
        memory.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut memory) == 0 {
            return Err(Error::Unavailable);
        }
        if memory.ullTotalPhys == 0 || memory.ullAvailPhys > memory.ullTotalPhys {
            return Err(Error::Protocol);
        }
        Ok((memory.ullTotalPhys, memory.ullAvailPhys))
    }
}
pub(super) fn interfaces() -> Result<BTreeMap<String, Traffic>, Error> {
    // SAFETY: successful GetIfTable2 supplies one initialized, aligned OS allocation
    // containing NumEntries rows. The Table guard owns it throughout slice iteration;
    // no row reference escapes, and the guard frees it exactly once on every exit.
    unsafe {
        let mut table = std::ptr::null_mut();
        if GetIfTable2(&mut table) != 0 || table.is_null() {
            return Err(Error::Unavailable);
        }
        struct Table(*mut MIB_IF_TABLE2);
        impl Drop for Table {
            fn drop(&mut self) {
                // SAFETY: uniquely owns the successful GetIfTable2 allocation.
                unsafe {
                    FreeMibTable(self.0.cast());
                }
            }
        }
        let _table = Table(table);
        let n = (*table).NumEntries as usize;
        if n > 128 {
            return Err(Error::Capacity);
        }
        let mut interfaces = BTreeMap::new();
        for row in std::slice::from_raw_parts((*table).Table.as_ptr(), n) {
            let g = row.InterfaceGuid;
            let key = format!(
                "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                g.data1,
                g.data2,
                g.data3,
                g.data4[0],
                g.data4[1],
                g.data4[2],
                g.data4[3],
                g.data4[4],
                g.data4[5],
                g.data4[6],
                g.data4[7]
            );
            if g.data1 == 0 && g.data2 == 0 && g.data3 == 0 && g.data4 == [0; 8] {
                return Err(Error::Protocol);
            }
            if interfaces
                .insert(
                    key,
                    Traffic {
                        received_bytes: row.InOctets,
                        sent_bytes: row.OutOctets,
                    },
                )
                .is_some()
            {
                return Err(Error::Protocol);
            }
        }
        Ok(interfaces)
    }
}
pub(super) fn volumes() -> Result<BTreeMap<String, Space>, Error> {
    // SAFETY: the initialized UTF-16 buffer remains writable at the supplied length.
    // Successful enumeration returns a terminated volume name used synchronously;
    // numeric outputs are initialized and no API retains these stack pointers.
    // The valid enumeration handle is uniquely owned by the Volumes guard.
    unsafe {
        let mut volumes = BTreeMap::new();
        let mut volume = [0u16; 1024];
        let handle = FindFirstVolumeW(volume.as_mut_ptr(), volume.len() as u32);
        if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return Err(Error::Unavailable);
        }
        struct Volumes(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for Volumes {
            fn drop(&mut self) {
                // SAFETY: closes the successful FindFirstVolumeW handle once.
                unsafe {
                    FindVolumeClose(self.0);
                }
            }
        }
        let _volumes = Volumes(handle);
        loop {
            {
                let mut available = 0;
                let mut total = 0;
                let mut free = 0;
                if GetDiskFreeSpaceExW(volume.as_ptr(), &mut available, &mut total, &mut free) == 0
                {
                    return Err(Error::Unavailable);
                }
                let end = volume.iter().position(|v| *v == 0).ok_or(Error::Protocol)?;
                let key = String::from_utf16(&volume[..end]).map_err(|_| Error::Protocol)?;
                if free > total {
                    return Err(Error::Protocol);
                }
                if volumes
                    .insert(
                        key,
                        Space {
                            total_bytes: total,
                            free_bytes: free,
                        },
                    )
                    .is_some()
                {
                    return Err(Error::Protocol);
                }
                if volumes.len() > 128 {
                    return Err(Error::Capacity);
                }
            }
            if FindNextVolumeW(handle, volume.as_mut_ptr(), volume.len() as u32) == 0 {
                if windows_sys::Win32::Foundation::GetLastError()
                    != windows_sys::Win32::Foundation::ERROR_NO_MORE_FILES
                {
                    return Err(Error::Unavailable);
                }
                break;
            }
        }
        Ok(volumes)
    }
}

//! Fixed native Windows readings; no WMI scripts or fabricated provider success.
use super::*;
use windows_sys::Win32::{
    Foundation::FILETIME,
    NetworkManagement::IpHelper::*,
    Storage::FileSystem::*,
    System::{SystemInformation::*, Threading::GetSystemTimes},
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
pub(super) fn read() -> Result<Snapshot, Error> {
    unsafe {
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
        let mut memory: MEMORYSTATUSEX = std::mem::zeroed();
        memory.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut memory) == 0 {
            return Err(Error::Unavailable);
        }
        let mut table = std::ptr::null_mut();
        if GetIfTable2(&mut table) != 0 || table.is_null() {
            return Err(Error::Unavailable);
        }
        struct Table(*mut MIB_IF_TABLE2);
        impl Drop for Table {
            fn drop(&mut self) {
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
                "{:08x}-{:04x}-{:04x}-{:02x?}",
                g.data1, g.data2, g.data3, g.data4
            );
            interfaces.insert(
                key,
                Traffic {
                    received_bytes: row.InOctets,
                    sent_bytes: row.OutOctets,
                },
            );
        }
        let mut volumes = BTreeMap::new();
        let mut volume = [0u16; 1024];
        let handle = FindFirstVolumeW(volume.as_mut_ptr(), volume.len() as u32);
        if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return Err(Error::Unavailable);
        }
        struct Volumes(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for Volumes {
            fn drop(&mut self) {
                unsafe {
                    FindVolumeClose(self.0);
                }
            }
        }
        let _volumes = Volumes(handle);
        loop {
            if GetDriveTypeW(volume.as_ptr()) == DRIVE_FIXED {
                let mut available = 0;
                let mut total = 0;
                let mut free = 0;
                if GetDiskFreeSpaceExW(volume.as_ptr(), &mut available, &mut total, &mut free) == 0
                {
                    return Err(Error::Unavailable);
                }
                let end = volume.iter().position(|v| *v == 0).ok_or(Error::Protocol)?;
                let key = String::from_utf16(&volume[..end]).map_err(|_| Error::Protocol)?;
                volumes.insert(
                    key,
                    Space {
                        total_bytes: total,
                        free_bytes: free,
                    },
                );
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
        Ok(Snapshot {
            boot: boot()?,
            cpu_busy: busy,
            cpu_total: total,
            memory_total: memory.ullTotalPhys,
            memory_available: memory.ullAvailPhys,
            volumes,
            interfaces,
        })
    }
}

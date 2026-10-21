//! Bounded native Mach/sysctl reads; failed APIs never turn into successful zeroes.
//! ref: apple-oss-distributions/xnu bsd/sys/mount.h (statfs filesystem identity).
use super::*;
use std::{
    ffi::CStr,
    mem::{size_of, zeroed},
};
unsafe extern "C" {
    fn mach_port_deallocate(
        task: libc::mach_port_t,
        port: libc::mach_port_t,
    ) -> libc::kern_return_t;
}
fn sysctl<T: Copy>(name: &CStr) -> Result<T, Error> {
    // SAFETY: the caller fixes the OS key and matching plain C output layout.
    unsafe {
        let mut value: T = zeroed();
        let mut len = size_of::<T>();
        if libc::sysctlbyname(
            name.as_ptr(),
            (&mut value as *mut T).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
            || len != size_of::<T>()
        {
            return Err(Error::Unavailable);
        }
        Ok(value)
    }
}
pub(super) fn boot() -> Result<String, Error> {
    let value: libc::timeval = sysctl(c"kern.boottime")?;
    Ok(format!("{}:{}", value.tv_sec, value.tv_usec))
}
#[allow(deprecated)]
pub(super) fn cpu() -> Result<Cpu, Error> {
    // SAFETY: fixed Mach ABI and owned host port, released on every exit.
    unsafe {
        let host = libc::mach_host_self();
        struct Host(libc::mach_port_t);
        impl Drop for Host {
            fn drop(&mut self) {
                unsafe {
                    mach_port_deallocate(libc::mach_task_self(), self.0);
                }
            }
        }
        let _host = Host(host);
        let mut cpu: libc::host_cpu_load_info_data_t = zeroed();
        let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
        if libc::host_statistics(
            host,
            libc::HOST_CPU_LOAD_INFO,
            (&mut cpu as *mut libc::host_cpu_load_info_data_t).cast(),
            &mut count,
        ) != libc::KERN_SUCCESS
            || count != libc::HOST_CPU_LOAD_INFO_COUNT
        {
            return Err(Error::Unavailable);
        }
        let total = cpu.cpu_ticks.iter().map(|v| u64::from(*v)).sum::<u64>();
        let busy = total
            .checked_sub(u64::from(cpu.cpu_ticks[libc::CPU_STATE_IDLE as usize]))
            .ok_or(Error::Protocol)?;
        Ok(Cpu {
            boot: boot()?,
            processors: sysctl(c"hw.logicalcpu")?,
            counters: cpu.cpu_ticks.iter().map(|v| u64::from(*v)).collect(),
            busy,
            total,
        })
    }
}
#[allow(deprecated)]
pub(super) fn memory() -> Result<(u64, u64), Error> {
    // SAFETY: fixed Mach ABI and owned host port, released on every exit.
    unsafe {
        let host = libc::mach_host_self();
        struct Host(libc::mach_port_t);
        impl Drop for Host {
            fn drop(&mut self) {
                unsafe {
                    mach_port_deallocate(libc::mach_task_self(), self.0);
                }
            }
        }
        let _host = Host(host);
        let mut vm: libc::vm_statistics64_data_t = zeroed();
        let mut count = libc::HOST_VM_INFO64_COUNT;
        if libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            (&mut vm as *mut libc::vm_statistics64_data_t).cast(),
            &mut count,
        ) != libc::KERN_SUCCESS
            // Kernels return the supported ABI prefix, not necessarily the newest libc layout.
            || count < ((std::mem::offset_of!(libc::vm_statistics64_data_t, inactive_count) + size_of::<u32>()) / size_of::<libc::integer_t>()) as u32
        {
            return Err(Error::Unavailable);
        }
        let memory_total: u64 = sysctl(c"hw.memsize")?;
        let page = libc::sysconf(libc::_SC_PAGESIZE);
        if page <= 0 {
            return Err(Error::Unavailable);
        }
        let memory_available = (u64::from(vm.free_count) + u64::from(vm.inactive_count))
            .checked_mul(page as u64)
            .filter(|v| *v <= memory_total)
            .ok_or(Error::Protocol)?;
        if memory_total == 0 {
            return Err(Error::Protocol);
        }
        Ok((memory_total, memory_available))
    }
}
pub(super) fn volumes() -> Result<BTreeMap<String, Space>, Error> {
    let mut result = BTreeMap::new();
    let boot = boot()?;
    // SAFETY: getfsstat writes only into our bounded owned array; concurrent readers share no buffer.
    unsafe {
        let capacity = libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT);
        if capacity <= 0 || capacity > 4096 {
            return Err(Error::Unavailable);
        }
        let slots = (capacity as usize + 16).min(4097);
        let mut mounts: Vec<libc::statfs> = vec![zeroed(); slots];
        let count = libc::getfsstat(
            mounts.as_mut_ptr(),
            (mounts.len() * size_of::<libc::statfs>()) as i32,
            libc::MNT_NOWAIT,
        );
        if count <= 0 || count as usize >= mounts.len() {
            return Err(Error::Unavailable);
        }
        for m in &mounts[..count as usize] {
            // Prefer the native volume UUID. Pseudo filesystems need not expose one;
            // their fallback is scoped to this boot and native mount-source domain.
            // f_fsid is stable only for the mounted instance, never a permanent volume ID.
            let mut attributes: libc::attrlist = zeroed();
            attributes.bitmapcount = libc::ATTR_BIT_MAP_COUNT as u16;
            attributes.volattr = libc::ATTR_VOL_UUID | libc::ATTR_VOL_INFO;
            let mut uuid_bytes = [0u8; 20];
            let persistent = if libc::getattrlist(
                m.f_mntonname.as_ptr(),
                &mut attributes,
                uuid_bytes.as_mut_ptr().cast(),
                uuid_bytes.len(),
                0,
            ) == 0
                && u32::from_ne_bytes(uuid_bytes[..4].try_into().unwrap()) == 20
            {
                uuid::Uuid::from_slice(&uuid_bytes[4..])
                    .ok()
                    .filter(|id| !id.is_nil())
            } else {
                None
            };
            let key = if let Some(id) = persistent {
                format!("volume:{id}")
            } else {
                let fsid = std::slice::from_raw_parts(
                    std::ptr::addr_of!(m.f_fsid).cast::<u8>(),
                    size_of::<libc::fsid_t>(),
                );
                if fsid.iter().all(|byte| *byte == 0) {
                    return Err(Error::Protocol);
                }
                let source = CStr::from_ptr(m.f_mntfromname.as_ptr())
                    .to_str()
                    .map_err(|_| Error::Protocol)?;
                if source.is_empty() || source.len() > 256 || source.chars().any(char::is_control) {
                    return Err(Error::Protocol);
                }
                let id = fsid
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                format!("boot:{boot}:fsid:{id}:source:{source}")
            };
            let total = m
                .f_blocks
                .checked_mul(u64::from(m.f_bsize))
                .ok_or(Error::Capacity)?;
            let free = m
                .f_bfree
                .checked_mul(u64::from(m.f_bsize))
                .filter(|v| *v <= total)
                .ok_or(Error::Protocol)?;
            if result
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
            if result.len() > 128 {
                return Err(Error::Capacity);
            }
        }
    }
    Ok(result)
}
pub(super) fn interfaces() -> Result<BTreeMap<String, Traffic>, Error> {
    let mut mib = [libc::CTL_NET, libc::PF_ROUTE, 0, 0, libc::NET_RT_IFLIST2, 0];
    let mut size = 0usize;
    // SAFETY: a bounded sysctl byte buffer is parsed using checked message lengths and unaligned copies.
    unsafe {
        if libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
            || size > 1024 * 1024
        {
            return Err(Error::Unavailable);
        }
        let mut bytes = vec![0u8; size];
        if libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            bytes.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
            || size > bytes.len()
        {
            return Err(Error::Unavailable);
        }
        let mut result = BTreeMap::new();
        let mut offset = 0;
        while offset < size {
            if size - offset < 4 {
                return Err(Error::Protocol);
            }
            let len = u16::from_ne_bytes([bytes[offset], bytes[offset + 1]]) as usize;
            if len < 4 || len > size - offset {
                return Err(Error::Protocol);
            }
            if i32::from(bytes[offset + 3]) == libc::RTM_IFINFO2 {
                if len < size_of::<libc::if_msghdr2>() {
                    return Err(Error::Protocol);
                }
                let m =
                    std::ptr::read_unaligned(bytes.as_ptr().add(offset).cast::<libc::if_msghdr2>());
                let mut name = [0i8; libc::IF_NAMESIZE];
                if libc::if_indextoname(u32::from(m.ifm_index), name.as_mut_ptr()).is_null() {
                    return Err(Error::Unavailable);
                }
                let name = CStr::from_ptr(name.as_ptr())
                    .to_str()
                    .map_err(|_| Error::Protocol)?;
                if name.is_empty() {
                    return Err(Error::Protocol);
                }
                if result
                    .insert(
                        format!("{}:{name}", m.ifm_index),
                        Traffic {
                            received_bytes: m.ifm_data.ifi_ibytes,
                            sent_bytes: m.ifm_data.ifi_obytes,
                        },
                    )
                    .is_some()
                {
                    return Err(Error::Protocol);
                }
                if result.len() > 128 {
                    return Err(Error::Capacity);
                }
            }
            offset += len;
        }
        Ok(result)
    }
}

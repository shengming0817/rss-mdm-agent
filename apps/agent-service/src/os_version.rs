//! Native product OS version; no shell, environment value or platform-name inference.
use std::io;

/// Read the current numeric operating-system product version.
pub fn current() -> io::Result<[u16; 4]> {
    platform_version()
}
#[cfg(target_os = "macos")]
fn platform_version() -> io::Result<[u16; 4]> {
    // ref: Apple xnu-11215.81.4 bsd/kern/kern_sysctl.c, kern.osproductversion.
    let mut buffer = [0u8; 64];
    let mut length = buffer.len();
    // SAFETY: a fixed NUL-terminated name and an in-bounds writable output buffer;
    // the null new value makes this a read-only sysctl query.
    let status = unsafe {
        libc::sysctlbyname(
            c"kern.osproductversion".as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    if length == 0 || length > buffer.len() || buffer[length - 1] != 0 {
        return Err(io::Error::other("invalid OS version"));
    }
    parse(std::str::from_utf8(&buffer[..length - 1]).map_err(io::Error::other)?)
}
#[cfg(windows)]
fn platform_version() -> io::Result<[u16; 4]> {
    // ref: Microsoft RtlGetVersion / RTL_OSVERSIONINFOW ABI.
    #[repr(C)]
    struct Version {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version: *mut Version) -> i32;
    }
    let mut version = Version {
        size: std::mem::size_of::<Version>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        service_pack: [0; 128],
    };
    // SAFETY: the documented size and C layout describe the writable structure exactly.
    if unsafe { RtlGetVersion(&mut version) } != 0 {
        return Err(io::Error::other("OS version unavailable"));
    }
    Ok([
        version.major.try_into().map_err(io::Error::other)?,
        version.minor.try_into().map_err(io::Error::other)?,
        version.build.try_into().map_err(io::Error::other)?,
        0,
    ])
}
#[cfg(not(any(target_os = "macos", windows)))]
fn platform_version() -> io::Result<[u16; 4]> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "unsupported managed OS",
    ))
}
#[cfg(any(target_os = "macos", test))]
fn parse(text: &str) -> io::Result<[u16; 4]> {
    let mut result = [0; 4];
    let parts: Vec<_> = text.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return Err(io::Error::other("invalid OS version"));
    }
    for (slot, part) in result.iter_mut().zip(parts) {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(io::Error::other("invalid OS version"));
        }
        *slot = part.parse().map_err(io::Error::other)?;
    }
    if result == [0; 4] {
        return Err(io::Error::other("invalid OS version"));
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    #[test]
    fn version_is_numeric_and_bounded() {
        assert_eq!(super::parse("14.7.1").unwrap(), [14, 7, 1, 0]);
        for bad in ["", "0", "14.-1", "14.1.2.3.4", "14.65536", "14.1\n"] {
            assert!(super::parse(bad).is_err());
        }
    }
    #[test]
    #[cfg(any(target_os = "macos", windows))]
    fn native_product_version_is_available() {
        assert_ne!(super::current().unwrap(), [0; 4]);
    }
}

// ref: apple-oss-distributions/configd@585b7f2fca293f4642d21d15c5daf187f63c4796
// SystemConfiguration.fproj/SCDynamicStoreCopySpecific.h;
// apple-oss-distributions/Libc net/FreeBSD/getifaddrs.c (owned interface list).
use std::{
    ffi::{c_void, CStr},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ptr,
};
#[link(name = "SystemConfiguration", kind = "framework")]
unsafe extern "C" {
    fn SCDynamicStoreCopyComputerName(store: *const c_void, encoding: *mut u32) -> *const c_void;
    fn SCDynamicStoreCopyConsoleUser(
        store: *const c_void,
        uid: *mut u32,
        gid: *mut u32,
    ) -> *const c_void;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringGetCString(value: *const c_void, buffer: *mut i8, size: isize, encoding: u32) -> u8;
    fn CFRelease(value: *const c_void);
}
unsafe fn copied_string(value: *const c_void) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let mut bytes = [0i8; 1024];
    // SAFETY: a Copy-rule CFString is owned here and is released exactly once.
    let ok =
        unsafe { CFStringGetCString(value, bytes.as_mut_ptr(), bytes.len() as isize, 0x08000100) };
    unsafe { CFRelease(value) };
    (ok != 0)
        .then(|| {
            unsafe { CStr::from_ptr(bytes.as_ptr()) }
                .to_str()
                .ok()
                .map(str::to_owned)
        })
        .flatten()
}
fn interfaces() -> Option<Vec<IpAddr>> {
    struct List(*mut libc::ifaddrs);
    impl Drop for List {
        fn drop(&mut self) {
            unsafe { libc::freeifaddrs(self.0) };
        }
    }
    let mut first = ptr::null_mut();
    // SAFETY: getifaddrs owns the list until freeifaddrs; addresses are inspected by family.
    if unsafe { libc::getifaddrs(&mut first) } != 0 {
        return None;
    }
    let list = List(first);
    let mut next = list.0;
    let mut result = Vec::new();
    let mut count = 0;
    while !next.is_null() {
        count += 1;
        if count > 1024 {
            return None;
        }
        let item = unsafe { &*next };
        next = item.ifa_next;
        if item.ifa_addr.is_null()
            || item.ifa_flags & libc::IFF_UP as u32 == 0
            || item.ifa_flags & libc::IFF_LOOPBACK as u32 != 0
        {
            continue;
        }
        let family = unsafe { (*item.ifa_addr).sa_family } as i32;
        if family == libc::AF_INET {
            let address = unsafe { &*item.ifa_addr.cast::<libc::sockaddr_in>() };
            result.push(IpAddr::V4(Ipv4Addr::from(
                address.sin_addr.s_addr.to_ne_bytes(),
            )));
        } else if family == libc::AF_INET6 {
            let address = unsafe { &*item.ifa_addr.cast::<libc::sockaddr_in6>() };
            result.push(IpAddr::V6(Ipv6Addr::from(address.sin6_addr.s6_addr)));
        }
    }
    Some(result)
}
pub(super) fn read(subject: &str) -> (Option<String>, Option<Vec<IpAddr>>, Option<String>) {
    let name =
        unsafe { copied_string(SCDynamicStoreCopyComputerName(ptr::null(), ptr::null_mut())) };
    let mut uid = u32::MAX;
    let mut gid = 0;
    let user = unsafe {
        copied_string(SCDynamicStoreCopyConsoleUser(
            ptr::null(),
            &mut uid,
            &mut gid,
        ))
    };
    let user = if uid > 0 && uid != u32::MAX && uid.to_string() == subject {
        user
    } else {
        None
    };
    (name, interfaces(), user)
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_reads_have_bounded_shapes_and_never_return_a_different_login() {
        let (name, ips, user) = super::read("0");
        assert!(name.is_some_and(|value| !value.is_empty() && value.len() < 1024));
        assert!(ips.is_some());
        assert!(user.is_none());
        let (name, ips, user) = super::read(&unsafe { libc::geteuid() }.to_string());
        assert!(super::super::text(name).is_some());
        assert!(super::super::addresses(ips).is_some_and(|values| values.len() <= 16));
        assert!(user.is_none_or(|value| super::super::text(Some(value)).is_some()));
    }
}

// ref: windows-sys 0.61.2 Win32/{NetworkManagement/IpHelper,System/RemoteDesktop,System/SystemInformation}.
use std::{
    mem::size_of,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA, NO_ERROR},
    NetworkManagement::{IpHelper::*, Ndis::IfOperStatusUp},
    Networking::WinSock::{
        IpDadStatePreferred, AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
    },
    System::{
        RemoteDesktop::*,
        SystemInformation::{ComputerNamePhysicalDnsHostname, GetComputerNameExW},
    },
};
fn name() -> Option<String> {
    let mut text = [0u16; 257];
    let mut length = text.len() as u32;
    // SAFETY: fixed writable output; success length excludes terminator.
    if unsafe {
        GetComputerNameExW(
            ComputerNamePhysicalDnsHostname,
            text.as_mut_ptr(),
            &mut length,
        )
    } == 0
        || length as usize >= text.len()
    {
        return None;
    }
    String::from_utf16(&text[..length as usize]).ok()
}
fn session_field(session: u32, field: WTS_INFO_CLASS) -> Option<String> {
    struct Buffer(*mut u16);
    impl Drop for Buffer {
        fn drop(&mut self) {
            unsafe { WTSFreeMemory(self.0.cast()) };
        }
    }
    let mut value = ptr::null_mut();
    let mut bytes = 0;
    // SAFETY: WTS owns the output allocation; drop frees it on every successful path.
    if unsafe {
        WTSQuerySessionInformationW(ptr::null_mut(), session, field, &mut value, &mut bytes)
    } == 0
    {
        return None;
    }
    let owned = Buffer(value);
    if owned.0.is_null() || bytes < 2 || bytes > 2048 || bytes % 2 != 0 {
        return None;
    }
    let units = unsafe { std::slice::from_raw_parts(owned.0, bytes as usize / 2) };
    let end = units.iter().position(|c| *c == 0)?;
    String::from_utf16(&units[..end])
        .ok()
        .filter(|v| !v.is_empty())
}
fn interfaces() -> Option<Vec<IpAddr>> {
    let mut length = 16_384u32;
    for _ in 0..3 {
        if length > 1_048_576 {
            return None;
        }
        // u64 allocation provides the required native union/pointer alignment.
        let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
        let head = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        let code = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC as u32,
                GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                ptr::null(),
                head,
                &mut length,
            )
        };
        if code == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if code == ERROR_NO_DATA {
            return Some(vec![]);
        }
        if code != NO_ERROR {
            return None;
        }
        let mut ips = Vec::new();
        let mut adapter = head;
        let mut visited = 0;
        // All nodes and socket addresses are owned by buffer until this iteration ends.
        while !adapter.is_null() {
            visited += 1;
            if visited > 1024 {
                return None;
            }
            let item = unsafe { &*adapter };
            adapter = item.Next;
            if item.OperStatus != IfOperStatusUp || item.IfType == 24 {
                continue;
            }
            let mut unicast = item.FirstUnicastAddress;
            let mut addresses = 0;
            while !unicast.is_null() {
                addresses += 1;
                if addresses > 1024 {
                    return None;
                }
                let item = unsafe { &*unicast };
                unicast = item.Next;
                let addr = item.Address.lpSockaddr;
                if addr.is_null()
                    || item.Address.iSockaddrLength < 2
                    || item.DadState != IpDadStatePreferred
                {
                    continue;
                }
                let family = unsafe { (*addr).sa_family };
                if family == AF_INET
                    && item.Address.iSockaddrLength as usize >= size_of::<SOCKADDR_IN>()
                {
                    let addr = unsafe { &*addr.cast::<SOCKADDR_IN>() };
                    ips.push(IpAddr::V4(Ipv4Addr::from(
                        unsafe { addr.sin_addr.S_un.S_addr }.to_ne_bytes(),
                    )));
                } else if family == AF_INET6
                    && item.Address.iSockaddrLength as usize >= size_of::<SOCKADDR_IN6>()
                {
                    let addr = unsafe { &*addr.cast::<SOCKADDR_IN6>() };
                    ips.push(IpAddr::V6(Ipv6Addr::from(unsafe { addr.sin6_addr.u.Byte })));
                }
            }
        }
        return Some(ips);
    }
    None
}
pub(super) fn read(session: u32) -> (Option<String>, Option<Vec<IpAddr>>, Option<String>) {
    let user = if session == 0 {
        None
    } else {
        session_field(session, WTSUserName).map(|user| {
            session_field(session, WTSDomainName)
                .map_or_else(|| user.clone(), |domain| format!("{domain}\\{user}"))
        })
    };
    (name(), interfaces(), user)
}

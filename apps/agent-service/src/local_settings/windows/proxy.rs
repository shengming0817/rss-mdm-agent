//! WinINet current-user connection options; WinHTTP is an independent read-only target.
use super::*;
use windows_sys::Win32::Networking::{WinHttp::*, WinInet::*};
struct Options {
    flags: u32,
    server: String,
    bypass: String,
}
fn options() -> Result<Options, Error> {
    let mut options = [INTERNET_PER_CONN_OPTIONW::default(); 3];
    for (v, key) in options.iter_mut().zip([
        INTERNET_PER_CONN_FLAGS,
        INTERNET_PER_CONN_PROXY_SERVER,
        INTERNET_PER_CONN_PROXY_BYPASS,
    ]) {
        v.dwOption = key;
    }
    let mut list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        pszConnection: std::ptr::null_mut(),
        dwOptionCount: 3,
        dwOptionError: 0,
        pOptions: options.as_mut_ptr(),
    };
    let mut size = list.dwSize;
    let code = unsafe {
        InternetQueryOptionW(
            std::ptr::null(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            (&mut list as *mut INTERNET_PER_CONN_OPTION_LISTW).cast(),
            &mut size,
        )
    };
    if code == 0 {
        return Err(Error::Unavailable);
    }
    let server = unsafe { text(options[1].Value.pszValue) };
    let bypass = unsafe { text(options[2].Value.pszValue) };
    unsafe {
        GlobalFree(options[1].Value.pszValue.cast());
        GlobalFree(options[2].Value.pszValue.cast());
    }
    Ok(Options {
        flags: unsafe { options[0].Value.dwValue },
        server: server?,
        bypass: bypass?,
    })
}
fn parse(
    server: &str,
    enabled: bool,
) -> Result<std::collections::BTreeMap<String, ProxyEndpoint>, Error> {
    let mut result = std::collections::BTreeMap::new();
    for entry in server.split(';').filter(|s| !s.is_empty()) {
        let (key, endpoint) = entry.split_once('=').unwrap_or(("all", entry));
        let (host, port) = endpoint.rsplit_once(':').ok_or(Error::Unsupported)?;
        let port: u16 = port.parse().map_err(|_| Error::Unsupported)?;
        if !matches!(key, "http" | "https" | "socks" | "ftp" | "all")
            || host.is_empty()
            || result
                .insert(
                    key.into(),
                    ProxyEndpoint {
                        enabled,
                        host: host.into(),
                        port,
                    },
                )
                .is_some()
        {
            return Err(Error::Unsupported);
        }
    }
    Ok(result)
}
pub(super) fn read() -> Result<Vec<SettingsTarget>, Error> {
    let options = options()?;
    let parsed = parse(&options.server, options.flags & PROXY_TYPE_PROXY != 0);
    let managed = policy_present(
        HKEY_CURRENT_USER,
        r"Software\Policies\Microsoft\Internet Explorer\Control Panel",
    ) || policy_present(
        HKEY_LOCAL_MACHINE,
        r"Software\Policies\Microsoft\Windows\CurrentVersion\Internet Settings",
    );
    let mut fields = vec![];
    for protocol in ["http", "https", "socks"] {
        let value = parsed.as_ref().ok().map(|p| {
            p.get(protocol)
                .or_else(|| p.get("all"))
                .cloned()
                .unwrap_or(ProxyEndpoint {
                    enabled: false,
                    host: String::new(),
                    port: 0,
                })
        });
        fields.push(field(
            protocol,
            value.map(|p| json!(p)),
            parsed
                .is_err()
                .then_some("原代理格式无法无损编辑，请使用系统设置"),
        ));
    }
    fields.extend([
        field(
            "bypass",
            Some(json!(options
                .bypass
                .split(';')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>())),
            None,
        ),
        field(
            "pacEnabled",
            Some(json!(options.flags & PROXY_TYPE_AUTO_PROXY_URL != 0)),
            None,
        ),
        field(
            "wpadEnabled",
            Some(json!(options.flags & PROXY_TYPE_AUTO_DETECT != 0)),
            None,
        ),
    ]);
    let mut result = vec![target(
        "current-user",
        "当前用户代理",
        fields,
        !managed && parsed.is_ok(),
        managed.then_some("由组织代理策略管理"),
    )];
    let mut proxy = WINHTTP_PROXY_INFO::default();
    let fields = if unsafe { WinHttpGetDefaultProxyConfiguration(&mut proxy) } != 0 {
        let server = unsafe { text(proxy.lpszProxy) };
        let bypass = unsafe { text(proxy.lpszProxyBypass) };
        unsafe {
            GlobalFree(proxy.lpszProxy.cast());
            GlobalFree(proxy.lpszProxyBypass.cast());
        }
        vec![
            field("proxy", server.ok().map(|s| json!(s)), None),
            field("bypass", bypass.ok().map(|s| json!(s)), None),
        ]
    } else {
        vec![field("proxy", None, Some("无法读取 WinHTTP 系统代理"))]
    };
    result.push(target(
        "winhttp",
        "系统服务代理（WinHTTP）",
        fields,
        false,
        Some("系统服务代理独立于当前用户代理，仅提供读取"),
    ));
    Ok(result)
}
pub(super) fn apply(patch: &SettingsPatch, before: &SettingsSnapshot) -> Result<(), Error> {
    let SettingsPatch::Proxy {
        target,
        http,
        https,
        socks,
        bypass,
    } = patch
    else {
        return Err(Error::Unsupported);
    };
    if target != "current-user" || read()? != before.targets {
        return Err(Error::Conflict);
    }
    if !before.targets.iter().any(|t| t.id == *target && t.writable) {
        return Err(Error::Denied);
    }
    let current = options()?;
    // WinINet has one manual enable flag. Preserve dormant protocol settings rather than
    // silently enabling or discarding unedited entries when an individual endpoint changes.
    if current.flags & PROXY_TYPE_PROXY == 0
        && !current.server.is_empty()
        && [http, https, socks]
            .iter()
            .any(|p| p.as_ref().is_some_and(|p| p.enabled))
    {
        return Err(Error::Unsupported);
    }
    let endpoints_changed = [http, https, socks].iter().any(|p| p.is_some());
    let mut servers = parse(&current.server, current.flags & PROXY_TYPE_PROXY != 0)?;
    if let Some(all) = servers.remove("all") {
        for protocol in ["http", "https", "socks", "ftp"] {
            servers
                .entry(protocol.into())
                .or_insert_with(|| all.clone());
        }
    }
    for (protocol, endpoint) in [("http", http), ("https", https), ("socks", socks)] {
        if let Some(endpoint) = endpoint {
            if endpoint.enabled {
                servers.insert(protocol.into(), endpoint.clone());
            } else {
                servers.remove(protocol);
            }
        }
    }
    let server = if !endpoints_changed {
        current.server.clone()
    } else {
        servers
            .iter()
            .filter(|(_, p)| p.enabled)
            .map(|(key, p)| format!("{key}={}:{}", p.host, p.port))
            .collect::<Vec<_>>()
            .join(";")
    };
    let bypass = bypass
        .as_ref()
        .map(|v| v.join(";"))
        .unwrap_or(current.bypass);
    // Preserve all PAC/WPAD flags, and do not write automatic URL or discovery options.
    let flags = if !endpoints_changed {
        current.flags
    } else {
        (current.flags & !PROXY_TYPE_PROXY)
            | PROXY_TYPE_DIRECT
            | if server.is_empty() {
                0
            } else {
                PROXY_TYPE_PROXY
            }
    };
    let mut server = wide(&server);
    let mut bypass = wide(&bypass);
    let mut values = [INTERNET_PER_CONN_OPTIONW::default(); 3];
    values[0].dwOption = INTERNET_PER_CONN_FLAGS;
    values[0].Value.dwValue = flags;
    values[1].dwOption = INTERNET_PER_CONN_PROXY_SERVER;
    values[1].Value.pszValue = server.as_mut_ptr();
    values[2].dwOption = INTERNET_PER_CONN_PROXY_BYPASS;
    values[2].Value.pszValue = bypass.as_mut_ptr();
    let list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        pszConnection: std::ptr::null_mut(),
        dwOptionCount: 3,
        dwOptionError: 0,
        pOptions: values.as_mut_ptr(),
    };
    if unsafe {
        InternetSetOptionW(
            std::ptr::null(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            (&list as *const INTERNET_PER_CONN_OPTION_LISTW).cast(),
            list.dwSize,
        )
    } == 0
    {
        return Err(Error::Denied);
    }
    if unsafe {
        InternetSetOptionW(
            std::ptr::null(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null(),
            0,
        )
    } == 0
        || unsafe {
            InternetSetOptionW(
                std::ptr::null(),
                INTERNET_OPTION_REFRESH,
                std::ptr::null(),
                0,
            )
        } == 0
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}

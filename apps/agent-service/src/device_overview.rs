//! Local presentation facts owned by the system service, scoped to the authenticated OS peer.
use execution_ipc::host::{DeviceOverview, Reply};
use std::net::IpAddr;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

fn text(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty() && v.len() <= 256 && !v.chars().any(char::is_control))
}
fn addresses(values: Option<Vec<IpAddr>>) -> Option<Vec<String>> {
    values.map(|mut values| {
        values.retain(|ip| match ip {
            IpAddr::V4(ip) => {
                !ip.is_loopback()
                    && !ip.is_unspecified()
                    && !ip.is_multicast()
                    && !ip.is_broadcast()
                    && !ip.is_link_local()
            }
            IpAddr::V6(ip) => {
                !ip.is_loopback()
                    && !ip.is_unspecified()
                    && !ip.is_multicast()
                    && !ip.is_unicast_link_local()
            }
        });
        values.sort();
        values.dedup();
        values
            .into_iter()
            .take(16)
            .map(|ip| ip.to_string())
            .collect()
    })
}
pub(crate) fn read(subject: &str, session: u32) -> Reply {
    #[cfg(target_os = "macos")]
    let (name, ips, user) = {
        let _ = session;
        macos::read(subject)
    };
    #[cfg(windows)]
    let (name, ips, user) = {
        let _ = subject;
        windows::read(session)
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let (name, ips, user) = {
        let _ = (subject, session);
        (None, None, None)
    };
    Reply::DeviceOverview {
        value: DeviceOverview {
            attributes: execution_ipc::host::DeviceAttributes {
                computer_name: text(name),
                ip_addresses: addresses(ips),
                login_user: text(user),
                extra: Default::default(),
            },
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_offline_vs_failed_and_bounds_values() {
        assert_eq!(addresses(None), None);
        assert_eq!(addresses(Some(vec![])), Some(vec![]));
        let values = [
            "127.0.0.1",
            "0.0.0.0",
            "169.254.1.2",
            "224.0.0.1",
            "::1",
            "fe80::1",
            "192.0.2.4",
            "192.0.2.4",
            "2001:db8::1",
        ];
        assert_eq!(
            addresses(Some(values.iter().map(|s| s.parse().unwrap()).collect())),
            Some(vec!["192.0.2.4".into(), "2001:db8::1".into()])
        );
        assert_eq!(text(Some("bad\nname".into())), None);
        assert_eq!(text(Some("x".repeat(257))), None);
        assert_eq!(text(Some("Studio Mac".into())), Some("Studio Mac".into()));
    }
}

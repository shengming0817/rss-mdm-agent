//! Native value projections, independent of the platform API and asset mappings.
use serde_json::Value;
pub(super) fn bitlocker(key: &str, value: u64) -> Option<Value> {
    Some(match key {
        "conversion" => [
            "decrypted",
            "encrypted",
            "encrypting",
            "decrypting",
            "encryption_paused",
            "decryption_paused",
        ]
        .get(usize::try_from(value).ok()?)?
        .to_string()
        .into(),
        "protection" => match value {
            0 => "unprotected",
            1 => "protected",
            _ => "unknown",
        }
        .into(),
        "lock" => match value {
            0 => "unlocked",
            1 => "locked",
            _ => return None,
        }
        .into(),
        "algorithm" => format!("fve_{value}").into(),
        "encrypted_percent" if value <= 100 => value.into(),
        _ => return None,
    })
}
pub(super) fn enabled(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None => "unknown",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encryption_and_protection_are_independent() {
        assert_eq!(bitlocker("conversion", 1), Some("encrypted".into()));
        assert_eq!(bitlocker("protection", 0), Some("unprotected".into()));
        assert_eq!(bitlocker("lock", 2), None);
        assert_eq!(bitlocker("encrypted_percent", 101), None);
        assert_eq!(enabled(None), "unknown");
    }
}

/// Only unambiguous SCM executable syntax; never emulate CreateProcess search rules.
pub(super) fn service_image(command: &str) -> Option<&str> {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        let (path, args) = rest.split_once('"')?;
        if path.is_empty() || (!args.is_empty() && !args.starts_with(char::is_whitespace)) {
            return None;
        }
        Some(path)
    } else if !command.contains(char::is_whitespace)
        && command.to_ascii_lowercase().ends_with(".exe")
    {
        Some(command)
    } else {
        None
    }
}
/// WUA ssDefault: managed server, otherwise first registered non-managed service in API order.
/// ref: Microsoft ServerSelection enumeration; AU-default is a different property.
pub(super) fn update_source(
    selection: i32,
    explicit: Option<&str>,
    services: &[(String, bool, bool)],
) -> Option<String> {
    match selection {
        0 | 1 => {
            let mut managed = services.iter().filter(|(_, managed, _)| *managed);
            if let Some(first) = managed.next() {
                if managed.next().is_some() {
                    return None;
                }
                return Some(first.0.clone());
            }
            if selection == 0 {
                services
                    .iter()
                    .find(|(_, managed, registered)| !managed && *registered)
                    .map(|v| v.0.clone())
            } else {
                None
            }
        }
        2 => Some("windows-update".into()),
        3 => explicit
            .filter(|id| services.iter().any(|v| v.0 == *id))
            .map(str::to_owned),
        _ => None,
    }
}
/// Historical selection is not the machine's current configuration. Managed history only
/// proves the selected category; it cannot identify an old WSUS endpoint. Results stay Partial.
pub(super) fn history_source(selection: i32, explicit: Option<&str>) -> Option<String> {
    match selection {
        1 => Some("managed-server".into()),
        2 => Some("windows-update".into()),
        3 => uuid::Uuid::parse_str(explicit?.trim_matches(['{', '}']))
            .ok()
            .filter(|id| !id.is_nil())
            .map(|id| id.to_string()),
        _ => None,
    }
}
#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn service_paths_never_guess_unquoted_whitespace_resolution() {
        assert_eq!(
            service_image(r#""C:\Program Files\Vendor\App.exe" --service"#),
            Some(r"C:\Program Files\Vendor\App.exe")
        );
        assert_eq!(
            service_image(r"C:\Vendor\App.exe"),
            Some(r"C:\Vendor\App.exe")
        );
        assert_eq!(service_image(r"C:\Program Files\Vendor\App.exe"), None);
        assert_eq!(service_image(r#""C:\Vendor\App.exe"evil"#), None);
        assert_eq!(service_image("app.exe --run"), None);
    }
    #[test]
    fn managed_search_and_history_do_not_borrow_au_default_identity() {
        let services = vec![
            ("au-service".into(), false, true),
            ("wsus".into(), true, false),
        ];
        assert_eq!(update_source(0, None, &services).as_deref(), Some("wsus"));
        assert_eq!(
            update_source(0, None, &services[..1]).as_deref(),
            Some("au-service")
        );
        assert_eq!(update_source(3, Some("unknown"), &services), None);
        assert_eq!(history_source(1, None).as_deref(), Some("managed-server"));
        assert_eq!(history_source(2, None).as_deref(), Some("windows-update"));
        assert_eq!(history_source(3, Some("bad")), None);
        assert_eq!(
            history_source(3, Some("{aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa}")).as_deref(),
            Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
        );
    }
}

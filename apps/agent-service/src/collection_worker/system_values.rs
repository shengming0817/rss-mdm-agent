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

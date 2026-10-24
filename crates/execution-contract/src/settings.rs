//! Finite local settings requests. No executable rule, path, URL or command is accepted.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
/// Product setting page and fixed native destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SettingsSection {
    /// Connections and per-service proxies.
    Network,
    /// Connected displays and their supported modes.
    Display,
    /// Current audio input/output.
    Sound,
    /// AC/DC energy preferences.
    Power,
    /// Idle display, sleep and authentication.
    Lock,
    /// Mounted system volume capacity.
    Storage,
    /// Device protection observations.
    Security,
    /// OS and hardware specifications.
    About,
}
/// One actual value or an explicit unavailable observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsField {
    /// Code-owned field identifier, used only for presentation.
    pub key: String,
    /// Observed value. Missing never means disabled, zero or absent.
    pub value: Option<serde_json::Value>,
    /// Completeness of this individual observation.
    pub quality: DataQuality,
    /// Accurate human-readable limitation when needed.
    pub reason: Option<String>,
}
/// One value accepted by the OS for the exact target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsOption {
    /// Opaque native identifier, re-resolved and validated before mutation.
    pub id: String,
    /// Display label, without authority.
    pub label: String,
}
/// An observed display, network service, power source or current device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsTarget {
    /// Stable native target identifier, never a path selector.
    pub id: String,
    /// Human-readable target name.
    pub name: String,
    /// Individual observations.
    pub fields: Vec<SettingsField>,
    /// Supported native display modes or energy modes.
    pub options: Vec<SettingsOption>,
    /// Whether current policy, privilege and native support permit edits.
    pub writable: bool,
    /// Why writes are unavailable; reads remain independent.
    pub reason: Option<String>,
}
/// Timestamped observation, independent of submission and process exit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSnapshot {
    /// Actual section read.
    pub section: SettingsSection,
    /// Digest over native values, including target identity and capabilities.
    pub revision: Digest,
    /// Actual observation time, in UTC milliseconds.
    pub collected_at_unix_ms: u64,
    /// Native observation source.
    pub source: String,
    /// Bounded targets and per-field quality.
    pub targets: Vec<SettingsTarget>,
}
/// One explicit manual proxy endpoint. Automatic configuration is preserved independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxyEndpoint {
    /// Whether this manual protocol proxy is enabled.
    pub enabled: bool,
    /// Hostname or IP only, without scheme, credentials or path.
    pub host: String,
    /// Native TCP port; zero allowed only when disabled.
    pub port: u16,
}
/// Closed settings edits; omitted properties are preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SettingsPatch {
    /// Manual protocol endpoints and optional bypass list for an observed network service.
    Proxy {
        /// Snapshot-selected native service identity.
        target: String,
        /// HTTP endpoint, if changed.
        http: Option<ProxyEndpoint>,
        /// HTTPS endpoint, if changed.
        https: Option<ProxyEndpoint>,
        /// SOCKS endpoint, if changed.
        socks: Option<ProxyEndpoint>,
        /// Optional exact replacement of manual bypass entries.
        bypass: Option<Vec<String>>,
    },
    /// Native display mode/layout, previewed for fifteen seconds before keeping.
    Display {
        /// Snapshot-selected stable display identity.
        target: String,
        /// Native mode from the observed options; includes supported HiDPI modes.
        mode: Option<String>,
        /// Desktop origin, if changed.
        origin: Option<DisplayOrigin>,
    },
    /// Energy preferences for one actual power source.
    Power {
        /// AC or DC target returned by the native reader.
        target: String,
        /// Minutes before display sleep; zero means never.
        display_off_minutes: Option<u16>,
        /// Minutes before system sleep; zero means never.
        sleep_minutes: Option<u16>,
        /// Supported native energy mode, if changed.
        mode: Option<String>,
    },
}
/// Exact native desktop coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplayOrigin {
    /// Horizontal desktop coordinate.
    pub x: i32,
    /// Vertical desktop coordinate.
    pub y: i32,
}
impl SettingsPatch {
    /// Setting family that owns this patch.
    pub fn section(&self) -> SettingsSection {
        match self {
            Self::Proxy { .. } => SettingsSection::Network,
            Self::Display { .. } => SettingsSection::Display,
            Self::Power { .. } => SettingsSection::Power,
        }
    }
    /// Enforce product bounds independently from OS validation.
    pub fn valid(&self) -> bool {
        fn text(s: &str) -> bool {
            !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
        }
        fn endpoint(p: &ProxyEndpoint) -> bool {
            (p.host.is_empty() && !p.enabled
                || text(&p.host)
                    && !p
                        .host
                        .chars()
                        .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | '@' | '?' | '#')))
                && (!p.enabled || p.port != 0)
        }
        match self {
            Self::Proxy {
                target,
                http,
                https,
                socks,
                bypass,
            } => {
                text(target)
                    && (http.is_some() || https.is_some() || socks.is_some() || bypass.is_some())
                    && [http, https, socks]
                        .iter()
                        .all(|p| p.as_ref().is_none_or(endpoint))
                    && bypass.as_ref().is_none_or(|a| {
                        a.len() <= 128
                            && a.iter()
                                .all(|s| text(s) && !s.contains(';') && !s.contains(','))
                    })
            }
            Self::Display {
                target,
                mode,
                origin,
            } => {
                text(target)
                    && (mode.is_some() || origin.is_some())
                    && mode.as_ref().is_none_or(|s| text(s))
                    && origin
                        .as_ref()
                        .is_none_or(|p| p.x.abs_diff(0) <= 32768 && p.y.abs_diff(0) <= 32768)
            }
            Self::Power {
                target,
                display_off_minutes,
                sleep_minutes,
                mode,
            } => {
                matches!(target.as_str(), "ac" | "dc")
                    && (display_off_minutes.is_some() || sleep_minutes.is_some() || mode.is_some())
                    && display_off_minutes.is_none_or(|n| n <= 1440)
                    && sleep_minutes.is_none_or(|n| n <= 1440)
                    && mode.as_ref().is_none_or(|s| text(s))
            }
        }
    }
}
/// Exact effect observation, never inferred from exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SettingsEffect {
    /// Request retained; no evidence yet.
    Pending,
    /// Requested values independently read back.
    Applied,
    /// Temporary display values await the original user's explicit keep action.
    Preview,
    /// Original values independently read back after restoration.
    Restored,
    /// External changes or a readback differ from the requested values.
    Conflict,
    /// Native operation failed without proven effect.
    Failed,
    /// Effect cannot be established, including interrupted mutation.
    Unknown,
    /// OS accepted the fixed native settings opening request.
    OpenRequested,
    /// Read completed; completeness is recorded per field.
    Observed,
    /// Cancellation was recorded before any physical attempt began.
    Cancelled,
}
/// Finite desktop settings request using the common journal and native session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SettingsOperation {
    /// Read a retained observation without dispatching another scan.
    Snapshot {
        /// Requested page.
        section: SettingsSection,
    },
    /// Explicit new native observation.
    Refresh {
        /// Stable preallocated UUID.
        request: RequestId,
        /// Requested page.
        section: SettingsSection,
    },
    /// Explicit human edit bound to a recent independently observed revision.
    Change {
        /// Stable preallocated UUID.
        request: RequestId,
        /// Original snapshot revision.
        expected: Digest,
        /// Closed requested values.
        patch: SettingsPatch,
    },
    /// Open only this page's fixed OS destination through the user worker.
    OpenNative {
        /// Stable preallocated UUID.
        request: RequestId,
        /// Fixed destination.
        section: SettingsSection,
    },
    /// Recover the original task after response loss.
    Status {
        /// Original UUID.
        request: RequestId,
    },
    /// Request cancellation; restoration remains a separate fact.
    Cancel {
        /// Original UUID.
        request: RequestId,
    },
    /// Keep the exact preview, before its independent fifteen-second deadline.
    KeepDisplay {
        /// Original UUID.
        request: RequestId,
        /// Complete frozen execution digest.
        digest: Digest,
    },
}
/// Read-only projection of the original settings operation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsResponse {
    /// Current protected delegation permits edits of this section; native target checks still apply.
    pub write_allowed: bool,
    /// Request that produced this observation, absent for retained reads.
    pub request: Option<RequestId>,
    /// Complete immutable execution digest, when retained.
    pub digest: Option<Digest>,
    /// Physical lifecycle ended; effect is independently reported.
    pub finished: bool,
    /// Observed effect of this exact request.
    pub effect: SettingsEffect,
    /// Last valid native observation.
    pub snapshot: Option<SettingsSnapshot>,
    /// Deadline for keeping a display preview, in UTC milliseconds.
    pub keep_before_unix_ms: Option<u64>,
    /// Accurate bounded explanation, without raw process output.
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_intents_reject_executable_content_and_bound_native_values() {
        let request = "00000000-0000-4000-8000-000000000001";
        let refresh = serde_json::json!({"kind":"refresh","request":request,"section":"display"});
        assert!(serde_json::from_value::<SettingsOperation>(refresh.clone()).is_ok());
        for (key, value) in [
            ("command", serde_json::json!("/bin/sh")),
            ("url", serde_json::json!("file:///tmp/input")),
            ("rules", serde_json::json!([])),
        ] {
            let mut invalid = refresh.clone();
            invalid[key] = value;
            assert!(serde_json::from_value::<SettingsOperation>(invalid).is_err());
        }
        let valid = ProxyEndpoint {
            enabled: true,
            host: "proxy.example.test".into(),
            port: 8080,
        };
        let patch = |http| SettingsPatch::Proxy {
            target: "service-id".into(),
            http: Some(http),
            https: None,
            socks: None,
            bypass: None,
        };
        assert!(patch(valid.clone()).valid());
        for host in [
            "https://proxy.test",
            "user:secret@proxy.test",
            "a\nb",
            "/tmp/proxy",
        ] {
            assert!(!patch(ProxyEndpoint {
                host: host.into(),
                ..valid.clone()
            })
            .valid());
        }
        assert!(!SettingsPatch::Display {
            target: "display".into(),
            mode: None,
            origin: Some(DisplayOrigin { x: i32::MIN, y: 0 })
        }
        .valid());
        assert!(!SettingsPatch::Power {
            target: "all".into(),
            display_off_minutes: Some(15),
            sleep_minutes: None,
            mode: None
        }
        .valid());
    }
}

//! Shared external addresses. An address never carries execution authority.
// ref: tauri-apps/plugins-workspace plugins/deep-link/src/lib.rs@385f60c07710310f169c1cf0617baae03e258038
use crate::{Id, QueryRef, RequestId};
use rss_mdm_agent_wire::runtime::{CapabilitySelector, PolicyAlias, PolicyEventName};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum complete external URI size, before decoding.
pub const ENTRY_URI_BYTES: usize = 4096;
/// Stable page nodes. Titles and implementation paths are deliberately absent.
pub const PAGE_NODES: &[&str] = &[
    "home",
    "assistant",
    "tasks",
    "tasks.active",
    "tasks.history",
    "tasks.management",
    "software",
    "software.installed",
    "software.updates",
    "software.enterprise",
    "software.operations",
    "tools",
    "tools.network",
    "tools.space",
    "tools.power",
    "tools.security",
    "tools.diagnostics",
    "tools.recent",
    "tools.enterprise",
    "settings",
    "settings.account",
    "settings.registration",
    "settings.connection",
    "settings.personal-vault",
    "settings.diagnostics",
    "settings.about",
    "system",
    "system.overview",
    "system.network",
    "system.display",
    "system.sound",
    "system.power",
    "system.lock",
    "system.storage",
    "system.security",
    "system.about",
];

/// Explicit address kinds; display names are not exact selectors.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum EntrySelector {
    /// One packaged local tool; its existing owner supplies schema and qualification.
    LocalTool {
        /// Stable tool name.
        name: LocalTool,
    },
    /// Stable page or page section.
    Node {
        /// Explicit owner-registered ID.
        id: String,
    },
    /// Server-owned ID, tool name or unique alias.
    Capability {
        /// Canonical producer selector.
        selector: CapabilitySelector,
    },
    /// Explicit one-to-many event, never an alias lookup.
    Event {
        /// Producer-normalized name.
        name: PolicyEventName,
    },
    /// Original preparation or execution.
    Request {
        /// Original business request.
        request: RequestId,
    },
    /// Exact periodic collection session.
    Sampling {
        /// Original definition and revision.
        query: QueryRef,
    },
    /// Original source-qualified management fact.
    Activity {
        /// Producer fact identity.
        id: String,
    },
    /// Existing producer batch.
    Batch {
        /// Canonical batch UUID.
        id: String,
    },
    /// Original remote execution and attempt.
    Task {
        /// Backend task UUID.
        task: String,
        /// Exact attempt UUID.
        attempt: String,
    },
}
/// Stable names for actual existing local operation owners.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
pub enum LocalTool {
    /// Existing finite DNS/TCP diagnostic.
    #[serde(rename = "network.check")]
    Network,
    /// Existing local settings owner.
    #[serde(rename = "system.settings")]
    Settings,
    /// Existing software preparation/submission owner.
    #[serde(rename = "software.manage")]
    Software,
}
impl LocalTool {
    /// Canonical public tool name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Network => "network.check",
            Self::Settings => "system.settings",
            Self::Software => "software.manage",
        }
    }
    /// Stable UI owner node.
    pub fn node(self) -> &'static str {
        match self {
            Self::Network => "tools.network",
            Self::Settings => "system.overview",
            Self::Software => "software.installed",
        }
    }
    /// Schema generated directly from the original operation type, never a copied catalog.
    pub fn schema(self) -> serde_json::Value {
        match self {
            Self::Network => schemars::schema_for!(crate::NetworkCheckOperation),
            Self::Settings => schemars::schema_for!(crate::SettingsOperation),
            Self::Software => schemars::schema_for!(crate::LocalSoftwareOperation),
        }
        .to_value()
    }
}
/// A direct owner schema and stable navigation reference; discovery grants no permission.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalToolDescriptor {
    /// Stable original tool.
    pub name: LocalTool,
    /// Original Rust-generated operation schema.
    pub input_schema: serde_json::Value,
    /// Stable original page node.
    pub node: String,
}
/// Enterprise coordinates are compared with the active registration, never used to switch it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryNamespace {
    /// Product instance identity, not a server URL.
    pub instance: String,
    /// Exact tenant UUID.
    pub tenant: String,
    /// Exact device identity.
    pub device: String,
    /// Exact registration UUID.
    pub registration: String,
    /// Exact registration generation.
    pub generation: u64,
}
/// The external link selects a view or prepares an original user interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EntryAction {
    /// Read/open only.
    View,
    /// Open the exact input summary; a real user must submit it.
    Invoke,
}
/// One bounded navigation intent; neither origin nor confirmation is accepted here.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryIntent {
    /// Requested navigation behavior.
    pub action: EntryAction,
    /// Shared exact selector.
    pub selector: EntrySelector,
    /// Mandatory for enterprise links; absent for purely local page nodes.
    pub namespace: Option<EntryNamespace>,
    /// Exact owner definition version when supplied.
    pub version: Option<String>,
    /// Exact owner configuration revision when supplied.
    pub revision: Option<u64>,
    /// Owner-qualified node inside the exact attempt.
    pub step: Option<String>,
    /// Non-secret suggestions; the current schema is checked before display/use.
    pub parameters: BTreeMap<String, serde_json::Value>,
}
/// Value-free parser error; never log an untrusted URI or parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid external address")]
pub struct EntryError;

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
        && value
            .bytes()
            .any(|b| (b'1'..=b'9').contains(&b) || (b'a'..=b'f').contains(&b))
}
fn decode(value: &str) -> Result<String, EntryError> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let pair = bytes.get(i + 1..i + 3).ok_or(EntryError)?;
            let h = (pair[0] as char).to_digit(16).ok_or(EntryError)?;
            let l = (pair[1] as char).to_digit(16).ok_or(EntryError)?;
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let out = String::from_utf8(out).map_err(|_| EntryError)?;
    if out.chars().any(char::is_control) {
        return Err(EntryError);
    }
    Ok(out)
}
impl EntrySelector {
    /// Parse one exact CLI selector using the same producer name rules as links.
    pub fn parse(kind: &str, value: &str) -> Result<Self, EntryError> {
        if !bounded(value) {
            return Err(EntryError);
        }
        Ok(match kind {
            "node" if PAGE_NODES.contains(&value) => Self::Node { id: value.into() },
            "id" if canonical_uuid(value) => Self::Capability {
                selector: serde_json::from_value(serde_json::json!({"kind":"id","id":value}))
                    .map_err(|_| EntryError)?,
            },
            "tool" if ["network.check", "system.settings", "software.manage"].contains(&value) => {
                Self::LocalTool {
                    name: match value {
                        "network.check" => LocalTool::Network,
                        "system.settings" => LocalTool::Settings,
                        _ => LocalTool::Software,
                    },
                }
            }
            "tool" => {
                let selector = CapabilitySelector::ToolName { name: value.into() };
                selector.policy_id().map_err(|_| EntryError)?;
                Self::Capability { selector }
            }
            "alias" => Self::Capability {
                selector: CapabilitySelector::Alias {
                    alias: PolicyAlias::parse(value).map_err(|_| EntryError)?,
                },
            },
            "event" => Self::Event {
                name: PolicyEventName::parse(value).map_err(|_| EntryError)?,
            },
            "request" => Self::Request {
                request: RequestId::new(value).map_err(|_| EntryError)?,
            },
            "activity" => Self::Activity { id: value.into() },
            "batch" if canonical_uuid(value) => Self::Batch { id: value.into() },
            _ => return Err(EntryError),
        })
    }
}
impl EntryIntent {
    /// Validate direct DTOs as strictly as external URI input.
    pub fn validate(&self) -> Result<(), EntryError> {
        let local = matches!(self.selector, EntrySelector::LocalTool { .. })
            || matches!(&self.selector, EntrySelector::Node { id } if PAGE_NODES.contains(&id.as_str()) && !id.ends_with(".enterprise") && id != "tasks.management");
        if matches!(&self.selector, EntrySelector::Node { id } if !PAGE_NODES.contains(&id.as_str()))
        {
            return Err(EntryError);
        }
        if !local
            && !matches!(
                self.selector,
                EntrySelector::Request { .. } | EntrySelector::Sampling { .. }
            )
            && self.namespace.is_none()
        {
            return Err(EntryError);
        }
        if let Some(n) = &self.namespace {
            if !bounded(&n.instance)
                || n.instance.contains(['/', ':', '\\', '@', '?', '#'])
                || !bounded(&n.device)
                || !canonical_uuid(&n.tenant)
                || !canonical_uuid(&n.registration)
                || n.generation == 0
            {
                return Err(EntryError);
            }
        }
        if (self.version.is_some() || self.revision.is_some())
            && !matches!(self.selector, EntrySelector::Capability { .. })
        {
            return Err(EntryError);
        }
        if matches!(self.selector, EntrySelector::Event { .. }) && !self.parameters.is_empty() {
            return Err(EntryError);
        }
        if self.version.as_ref().is_some_and(|v| !bounded(v))
            || self.step.as_ref().is_some_and(|v| !bounded(v))
        {
            return Err(EntryError);
        }
        match &self.selector {
            EntrySelector::Capability { selector } => {
                selector.policy_id().map_err(|_| EntryError)?;
            }
            EntrySelector::Task { task, attempt }
                if !canonical_uuid(task) || !canonical_uuid(attempt) =>
            {
                return Err(EntryError)
            }
            EntrySelector::Activity { id } if !bounded(id) => return Err(EntryError),
            EntrySelector::Batch { id } if !canonical_uuid(id) => return Err(EntryError),
            _ => {}
        }
        if self.step.is_some()
            && !matches!(
                self.selector,
                EntrySelector::Task { .. }
                    | EntrySelector::Request { .. }
                    | EntrySelector::Activity { .. }
            )
        {
            return Err(EntryError);
        }
        if self.action == EntryAction::Invoke
            && !matches!(
                self.selector,
                EntrySelector::Capability { .. }
                    | EntrySelector::Event { .. }
                    | EntrySelector::LocalTool { .. }
            )
        {
            return Err(EntryError);
        }
        if self.action == EntryAction::View && !self.parameters.is_empty() {
            return Err(EntryError);
        }
        if serde_json::to_vec(&self.parameters)
            .map_err(|_| EntryError)?
            .len()
            > 2048
        {
            return Err(EntryError);
        }
        Ok(())
    }
    /// Parse only the current rssmdm URI grammar. URL normalization is not authorization.
    pub fn parse(uri: &str) -> Result<Self, EntryError> {
        if uri.len() > ENTRY_URI_BYTES
            || !uri.starts_with("rssmdm://")
            || uri.contains(['\\', '#'])
            || uri.chars().any(char::is_control)
        {
            return Err(EntryError);
        }
        let rest = uri.strip_prefix("rssmdm://").ok_or(EntryError)?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let segments: Vec<_> = path.split('/').collect();
        if segments.len() != 4 || segments[1] != "v1" {
            return Err(EntryError);
        }
        let action = match segments[0] {
            "view" => EntryAction::View,
            "invoke" => EntryAction::Invoke,
            _ => return Err(EntryError),
        };
        let mut fields = BTreeMap::new();
        if !query.is_empty() {
            for pair in query.split('&') {
                let (key, value) = pair.split_once('=').ok_or(EntryError)?;
                if ![
                    "instance",
                    "tenant",
                    "device",
                    "registration",
                    "generation",
                    "version",
                    "revision",
                    "step",
                    "parameters",
                    "attempt",
                    "queryRevision",
                ]
                .contains(&key)
                    || fields.insert(key, decode(value)?).is_some()
                {
                    return Err(EntryError);
                }
            }
        }
        let value = decode(segments[3])?;
        let selector = match segments[2] {
            "task" => EntrySelector::Task {
                task: value,
                attempt: fields.remove("attempt").ok_or(EntryError)?,
            },
            "sampling" => EntrySelector::Sampling {
                query: QueryRef {
                    id: Id::new(value).map_err(|_| EntryError)?,
                    revision: crate::Digest::new(fields.remove("queryRevision").ok_or(EntryError)?)
                        .map_err(|_| EntryError)?,
                },
            },
            kind => EntrySelector::parse(kind, &value)?,
        };
        if fields.contains_key("attempt") || fields.contains_key("queryRevision") {
            return Err(EntryError);
        }
        let namespace = if ["instance", "tenant", "device", "registration", "generation"]
            .iter()
            .any(|k| fields.contains_key(k))
        {
            Some(EntryNamespace {
                instance: fields.remove("instance").ok_or(EntryError)?,
                tenant: fields.remove("tenant").ok_or(EntryError)?,
                device: fields.remove("device").ok_or(EntryError)?,
                registration: fields.remove("registration").ok_or(EntryError)?,
                generation: fields
                    .remove("generation")
                    .ok_or(EntryError)?
                    .parse()
                    .map_err(|_| EntryError)?,
            })
        } else {
            None
        };
        let out = Self {
            action,
            selector,
            namespace,
            version: fields.remove("version"),
            revision: fields
                .remove("revision")
                .map(|v| v.parse().map_err(|_| EntryError))
                .transpose()?,
            step: fields.remove("step"),
            parameters: fields
                .remove("parameters")
                .map(|v| serde_json::from_str(&v).map_err(|_| EntryError))
                .transpose()?
                .unwrap_or_default(),
        };
        out.validate()?;
        Ok(out)
    }
    /// Produce the sole canonical encoding for copy-link and CLI open.
    pub fn uri(&self) -> Result<String, EntryError> {
        self.validate()?;
        let (kind, value) = match &self.selector {
            EntrySelector::LocalTool { name } => ("tool", name.name().into()),
            EntrySelector::Node { id } => ("node", id.clone()),
            EntrySelector::Capability {
                selector: CapabilitySelector::Id { id },
            } => ("id", id.to_string()),
            EntrySelector::Capability {
                selector: CapabilitySelector::ToolName { name },
            } => ("tool", name.clone()),
            EntrySelector::Capability {
                selector: CapabilitySelector::Alias { alias },
            } => ("alias", alias.as_str().into()),
            EntrySelector::Event { name } => ("event", name.as_str().into()),
            EntrySelector::Request { request } => ("request", request.as_str().into()),
            EntrySelector::Activity { id } => ("activity", id.clone()),
            EntrySelector::Batch { id } => ("batch", id.clone()),
            EntrySelector::Task { task, .. } => ("task", task.clone()),
            EntrySelector::Sampling { query } => ("sampling", query.id.as_str().into()),
        };
        let action = match self.action {
            EntryAction::View => "view",
            EntryAction::Invoke => "invoke",
        };
        let mut uri = url::Url::parse(&format!("rssmdm://{action}/v1/")).map_err(|_| EntryError)?;
        uri.path_segments_mut()
            .map_err(|_| EntryError)?
            .pop_if_empty()
            .push(kind)
            .push(&value);
        let mut fields = BTreeMap::new();
        if let Some(n) = &self.namespace {
            fields.extend([
                ("instance", n.instance.clone()),
                ("tenant", n.tenant.clone()),
                ("device", n.device.clone()),
                ("registration", n.registration.clone()),
                ("generation", n.generation.to_string()),
            ]);
        }
        if let Some(v) = &self.version {
            fields.insert("version", v.clone());
        }
        if let Some(v) = self.revision {
            fields.insert("revision", v.to_string());
        }
        if let Some(v) = &self.step {
            fields.insert("step", v.clone());
        }
        if !self.parameters.is_empty() {
            fields.insert(
                "parameters",
                serde_json::to_string(&self.parameters).map_err(|_| EntryError)?,
            );
        }
        match &self.selector {
            EntrySelector::Task { attempt, .. } => {
                fields.insert("attempt", attempt.clone());
            }
            EntrySelector::Sampling { query } => {
                fields.insert("queryRevision", query.revision.as_str().into());
            }
            _ => {}
        }
        if !fields.is_empty() {
            uri.query_pairs_mut().extend_pairs(fields);
        }
        // Form serialization uses '+' for spaces; URI parsing never treats literal '+' as space.
        let uri = uri.to_string().replace('+', "%20");
        if uri.len() > ENTRY_URI_BYTES {
            return Err(EntryError);
        }
        Ok(uri)
    }
}

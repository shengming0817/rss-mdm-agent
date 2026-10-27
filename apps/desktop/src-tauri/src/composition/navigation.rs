//! Bounded native navigation queue. Business identity and execution stay with the service.
use execution_contract::{EntryAction, EntryIntent, EntrySelector, LocalTool};
use execution_ipc::{entry::EntryOperation, host::Reply};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub id: String,
    pub intent: EntryIntent,
}
/// Display-only preparation. Request identities and confirmation never cross from a URI.
#[derive(Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LocalPreparation {
    Network {
        target: Option<execution_contract::NetworkTarget>,
    },
    Settings {
        section: execution_contract::SettingsSection,
        action: SettingsEntryAction,
        patch: Option<execution_contract::SettingsPatch>,
        expected: Option<execution_contract::Digest>,
    },
    Software {
        software: Option<execution_contract::SoftwareRef>,
        action: Option<execution_contract::LocalSoftwareAction>,
    },
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SettingsEntryAction {
    Snapshot,
    Refresh,
    Change,
    OpenNative,
}

// Decode the original operation schemas; the placeholder is never returned or submitted.
// ref: serde-rs/json src/value/mod.rs@v1.0.149 (from_value)
fn local_preparation(intent: &EntryIntent) -> Result<LocalPreparation, ()> {
    use execution_contract::{LocalSoftwareOperation, NetworkCheckOperation, SettingsOperation};
    let EntrySelector::LocalTool { name } = intent.selector else {
        return Err(());
    };
    if intent.action != EntryAction::Invoke {
        return Err(());
    }
    if intent.parameters.is_empty() {
        return Ok(match name {
            LocalTool::Network => LocalPreparation::Network { target: None },
            LocalTool::Settings => LocalPreparation::Settings {
                section: execution_contract::SettingsSection::Power,
                action: SettingsEntryAction::Snapshot,
                patch: None,
                expected: None,
            },
            LocalTool::Software => LocalPreparation::Software {
                software: None,
                action: None,
            },
        });
    }
    let mut value = serde_json::to_value(&intent.parameters).map_err(|_| ())?;
    let fields = if matches!(name, LocalTool::Software) {
        value
            .get_mut("input")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or(())?
    } else {
        value.as_object_mut().ok_or(())?
    };
    if fields.contains_key("request") {
        return Err(());
    }
    fields.insert("request".into(), serde_json::json!("navigation-draft"));
    Ok(match name {
        LocalTool::Network => {
            match serde_json::from_value::<NetworkCheckOperation>(value).map_err(|_| ())? {
                NetworkCheckOperation::Run { target, .. }
                    if !target.host.is_empty()
                        && target.host.len() <= 253
                        && !target.host.chars().any(|c| {
                            c.is_control() || c.is_whitespace() || "/@?#%;\\".contains(c)
                        })
                        && target.port != 0 =>
                {
                    LocalPreparation::Network {
                        target: Some(target),
                    }
                }
                _ => return Err(()),
            }
        }
        LocalTool::Settings => {
            let (section, action, patch, expected) =
                match serde_json::from_value::<SettingsOperation>(value).map_err(|_| ())? {
                    SettingsOperation::Change {
                        patch, expected, ..
                    } if patch.valid() => (
                        patch.section(),
                        SettingsEntryAction::Change,
                        Some(patch),
                        Some(expected),
                    ),
                    SettingsOperation::Refresh { section, .. } => {
                        (section, SettingsEntryAction::Refresh, None, None)
                    }
                    SettingsOperation::OpenNative { section, .. } => {
                        (section, SettingsEntryAction::OpenNative, None, None)
                    }
                    _ => return Err(()),
                };
            LocalPreparation::Settings {
                section,
                action,
                patch,
                expected,
            }
        }
        LocalTool::Software => {
            match serde_json::from_value::<LocalSoftwareOperation>(value).map_err(|_| ())? {
                LocalSoftwareOperation::Prepare(choice) => LocalPreparation::Software {
                    software: Some(choice.software),
                    action: Some(choice.action),
                },
                _ => return Err(()),
            }
        }
    })
}
struct Pending {
    delivery: Delivery,
    accepted: Instant,
    canonical: String,
}
#[derive(Default)]
pub struct Navigation {
    queue: Mutex<VecDeque<Pending>>,
}
impl Navigation {
    pub fn receive(&self, uri: &str) -> bool {
        let Ok(intent) = EntryIntent::parse(uri) else {
            return false;
        };
        let Ok(canonical) = intent.uri() else {
            return false;
        };
        let Ok(mut queue) = self.queue.lock() else {
            return false;
        };
        queue.retain(|v| v.accepted.elapsed() < Duration::from_secs(300));
        if queue
            .iter()
            .any(|v| v.canonical == canonical && v.accepted.elapsed() < Duration::from_secs(2))
        {
            return true;
        }
        if queue.len() >= 16 {
            return false;
        }
        queue.push_back(Pending {
            delivery: Delivery {
                id: uuid::Uuid::new_v4().to_string(),
                intent,
            },
            accepted: Instant::now(),
            canonical,
        });
        true
    }
}
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NavigationOperation {
    Next,
    Ack {
        id: String,
    },
    Clear,
    Format {
        intent: EntryIntent,
    },
    Link {
        selector: EntrySelector,
        version: Option<String>,
        revision: Option<u64>,
        step: Option<String>,
    },
    Resolve {
        intent: EntryIntent,
    },
    Submit {
        intent: EntryIntent,
        request: execution_contract::RequestId,
    },
    Activities {
        batch: Option<String>,
        related: Option<String>,
        after: Option<String>,
        limit: u16,
    },
    Nodes,
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum NavigationResponse {
    Pending(Option<Delivery>),
    Done,
    Uri(String),
    Nodes(Vec<String>),
    Policy(Box<rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem>),
    Task(execution_app::tasks::TaskRef),
    Activities(rss_mdm_agent_wire::activities::ManagementActivityPage),
    Event(execution_contract::EntryIntent),
    LocalTool(LocalPreparation),
    Request(Box<execution_contract::BackendRequest>),
}
fn denied() -> super::super::self_service::ServiceError {
    super::super::self_service::error(
        "navigation_denied",
        "目标不存在、版本已变化或当前账户无权访问",
    )
}
pub async fn dispatch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    generation: Option<String>,
    input: NavigationOperation,
) -> super::super::self_service::Result<NavigationResponse> {
    let state = app.state::<Navigation>();
    match input {
        NavigationOperation::Next => {
            let mut queue = state.queue.lock().map_err(|_| denied())?;
            queue.retain(|v| v.accepted.elapsed() < Duration::from_secs(300));
            return Ok(NavigationResponse::Pending(
                queue.front().map(|v| v.delivery.clone()),
            ));
        }
        NavigationOperation::Ack { id } => {
            let mut queue = state.queue.lock().map_err(|_| denied())?;
            if queue.front().is_some_and(|v| v.delivery.id == id) {
                queue.pop_front();
            }
            return Ok(NavigationResponse::Done);
        }
        NavigationOperation::Clear => {
            state.queue.lock().map_err(|_| denied())?.clear();
            return Ok(NavigationResponse::Done);
        }
        NavigationOperation::Format { intent } => {
            return Ok(NavigationResponse::Uri(intent.uri().map_err(|_| denied())?))
        }
        NavigationOperation::Nodes => {
            return Ok(NavigationResponse::Nodes(
                execution_contract::PAGE_NODES
                    .iter()
                    .map(|v| (*v).into())
                    .collect(),
            ))
        }
        NavigationOperation::Link {
            ref selector,
            version: None,
            revision: None,
            step: None,
        } if matches!(selector, EntrySelector::Node { .. }) => {
            let intent = EntryIntent {
                action: EntryAction::View,
                selector: selector.clone(),
                namespace: None,
                version: None,
                revision: None,
                step: None,
                parameters: Default::default(),
            };
            if let Ok(uri) = intent.uri() {
                return Ok(NavigationResponse::Uri(uri));
            }
        }
        NavigationOperation::Resolve { ref intent }
            if matches!(
                intent.selector,
                EntrySelector::Node { .. } | EntrySelector::LocalTool { .. }
            ) && intent.namespace.is_none() =>
        {
            intent.validate().map_err(|_| denied())?;
            if matches!(intent.selector, EntrySelector::LocalTool { .. })
                && intent.action == EntryAction::Invoke
            {
                return Ok(NavigationResponse::LocalTool(
                    local_preparation(intent).map_err(|_| denied())?,
                ));
            }
            return Ok(NavigationResponse::Event(intent.clone()));
        }
        _ => {}
    }
    let runtime = app
        .try_state::<super::runtime::DesktopRuntime>()
        .ok_or_else(denied)?;
    let generation = generation.ok_or_else(denied)?;
    let execution = runtime.execution_for(&generation).await?;
    let attach = execution.entry(EntryOperation::Attach).await?;
    let Reply::EntryContext { value: context } = attach else {
        return Err(denied());
    };
    runtime.current(&generation)?;
    let reply = match input {
        NavigationOperation::Link {
            selector,
            version,
            revision,
            step,
        } => {
            let mut intent = EntryIntent {
                action: EntryAction::View,
                selector,
                namespace: context.namespace,
                version,
                revision,
                step,
                parameters: Default::default(),
            };
            if matches!(intent.selector, EntrySelector::Capability { .. }) {
                let Reply::PolicyResolved { value } = execution
                    .entry(EntryOperation::Preview {
                        intent: intent.clone(),
                    })
                    .await?
                else {
                    return Err(denied());
                };
                runtime.current(&generation)?;
                intent.version = Some(value.policy_version.to_string());
                intent.revision = Some(value.policy_revision);
            }
            return Ok(NavigationResponse::Uri(intent.uri().map_err(|_| denied())?));
        }
        NavigationOperation::Resolve { intent } => {
            intent.validate().map_err(|_| denied())?;
            if intent.namespace.is_some() && intent.namespace != context.namespace {
                return Err(denied());
            }
            match &intent.selector {
                EntrySelector::Node { .. }
                | EntrySelector::Event { .. }
                | EntrySelector::LocalTool { .. } => {
                    if matches!(intent.selector, EntrySelector::LocalTool { .. })
                        && intent.action == EntryAction::Invoke
                    {
                        return Ok(NavigationResponse::LocalTool(
                            local_preparation(&intent).map_err(|_| denied())?,
                        ));
                    }
                    return Ok(NavigationResponse::Event(intent));
                }
                _ => {}
            }
            if matches!(intent.selector, EntrySelector::Capability { .. }) {
                execution.entry(EntryOperation::Preview { intent }).await?
            } else {
                execution
                    .entry(EntryOperation::Resolve {
                        selector: intent.selector,
                    })
                    .await?
            }
        }
        NavigationOperation::Submit { intent, request } => {
            intent.validate().map_err(|_| denied())?;
            if intent.namespace != context.namespace || intent.action != EntryAction::Invoke {
                return Err(denied());
            }
            execution
                .entry(EntryOperation::Invoke { intent, request })
                .await?
        }
        NavigationOperation::Activities {
            batch,
            related,
            after,
            limit,
        } => {
            execution
                .entry(EntryOperation::Activities {
                    batch,
                    related,
                    after,
                    limit,
                })
                .await?
        }
        _ => return Err(denied()),
    };
    runtime.current(&generation)?;
    Ok(match reply {
        Reply::PolicyResolved { value } => NavigationResponse::Policy(value),
        Reply::PolicyRequest { value } => NavigationResponse::Request(value),
        Reply::Task {
            value: execution_app::tasks::TaskResponse::Detail(value),
        } => NavigationResponse::Task(value.reference),
        Reply::Activities { value } => NavigationResponse::Activities(value),
        _ => return Err(denied()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepared(tool: &str, parameters: serde_json::Value) -> Result<LocalPreparation, ()> {
        let mut intent = EntryIntent::parse(&format!("rssmdm://invoke/v1/tool/{tool}")).unwrap();
        intent.parameters = serde_json::from_value(parameters).unwrap();
        intent.validate().map_err(|_| ())?;
        local_preparation(&intent)
    }
    #[test]
    fn local_suggestions_decode_original_schemas_without_request_or_confirmation_authority() {
        for (tool, input, kind) in [
            (
                "network.check",
                serde_json::json!({"kind":"run","target":{"host":"example.test","port":443}}),
                "network",
            ),
            (
                "system.settings",
                serde_json::json!({"kind":"change","expected":"ab".repeat(32),"patch":{"kind":"power","target":"ac","displayOffMinutes":10}}),
                "settings",
            ),
            (
                "software.manage",
                serde_json::json!({"kind":"prepare","input":{"software":{"query":{"id":"local-software","revision":"ab".repeat(32)},"installation":"notes"},"action":"uninstall"}}),
                "software",
            ),
        ] {
            let value = serde_json::to_value(prepared(tool, input).unwrap()).unwrap();
            assert_eq!(value["kind"], kind);
            assert!(!value.to_string().contains("navigation-draft"));
        }
        for tool in ["network.check", "system.settings", "software.manage"] {
            assert!(prepared(tool, serde_json::json!({})).is_ok());
        }
        for (tool, input) in [
            (
                "network.check",
                serde_json::json!({"kind":"run","request":"borrowed","target":{"host":"example.test","port":443}}),
            ),
            (
                "network.check",
                serde_json::json!({"kind":"run","target":{"host":"https://secret@example.test","port":443}}),
            ),
            (
                "network.check",
                serde_json::json!({"kind":"run","token":"secret","target":{"host":"example.test","port":443}}),
            ),
            (
                "system.settings",
                serde_json::json!({"kind":"keepDisplay","digest":"ab".repeat(32)}),
            ),
            (
                "system.settings",
                serde_json::json!({"kind":"change","expected":"ab".repeat(32),"patch":{"kind":"power","target":"ac","displayOffMinutes":2000}}),
            ),
            (
                "software.manage",
                serde_json::json!({"kind":"submit","input":{"digest":"ab".repeat(32)}}),
            ),
        ] {
            assert!(prepared(tool, input).is_err());
        }
    }
    #[test]
    fn navigation_is_bounded_and_never_transforms_a_delivery_into_a_submission() {
        let state = Navigation::default();
        let uri = "rssmdm://view/v1/node/home";
        assert!(state.receive(uri));
        assert!(state.receive(uri));
        assert_eq!(state.queue.lock().unwrap().len(), 1);
        assert!(!state.receive("rssmdm://invoke/v1/node/home"));
        for node in execution_contract::PAGE_NODES
            .iter()
            .filter(|n| !n.ends_with(".enterprise") && **n != "tasks.management")
            .skip(1)
            .take(15)
        {
            assert!(state.receive(&format!("rssmdm://view/v1/node/{node}")));
        }
        assert!(!state.receive("rssmdm://view/v1/node/settings.about"));
        assert!(state
            .queue
            .lock()
            .unwrap()
            .iter()
            .all(|v| v.delivery.intent.action == EntryAction::View));
    }
}

//! Finite CLI/native entry operations; business facts remain with their original owners.
use execution_contract::{EntryIntent, EntrySelector, NativeContextRef, QueryRef, RequestId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A caller cannot select provenance, an endpoint, OS subject or employee credentials.
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum EntryOperation {
    /// Enumerate actual local operation owners and the current remote directory page.
    Discover {
        /// Existing remote Policy cursor.
        after: Option<String>,
    },
    /// Existing data/query owner; native ingress independently restricts machine scope.
    Data {
        /// Original bounded data operation.
        input: execution_contract::DataOperation,
    },
    /// Attach to this exact OS login without replacing its current product identity.
    Attach,
    /// Resolve the exact current authorized target, without execution.
    Resolve {
        /// Exact typed address.
        selector: EntrySelector,
    },
    /// Validate an external intent's namespace, exact version and non-secret parameters.
    Preview {
        /// External intent, never confirmation.
        intent: EntryIntent,
    },
    /// Submit a frozen selection after explicit CLI invocation or native UI submission.
    Invoke {
        /// The exact displayed intent.
        intent: EntryIntent,
        /// Original stable request ID.
        request: RequestId,
    },
    /// Read already-consumed producer facts using the existing activity HTTP owner.
    Activities {
        /// Exact existing batch, when filtering a batch.
        batch: Option<String>,
        /// Exact activity whose related history is being read.
        related: Option<String>,
        /// Producer cursor for the same filter.
        after: Option<String>,
        /// Requested size, 1..=64.
        limit: u16,
    },
    /// Explicitly wake the existing check-in owner.
    Checkin,
    /// Advance the current login's existing management synchronization once.
    Sync,
    /// Refresh one existing authorized definition using the original query owner.
    Refresh {
        /// Stable request.
        request: RequestId,
        /// Exact original installed definition.
        query: QueryRef,
    },
}
/// Explicit administrator-owned machine CLI action, never an enterprise permission.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MachineAction {
    /// Safe service diagnostics.
    Status,
    /// Wake the original check-in drive.
    Checkin,
    /// Existing device-authorized activity synchronization.
    Sync,
    /// Device-scoped retained data and local definition discovery.
    ReadData,
    /// Existing packaged system-native queries only.
    Query,
}
/// Installed CLI image and closed unattended deployment grants.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliPolicy {
    /// Protected installed executable identity.
    pub image: installation_security::Artifact,
    /// Empty denies unattended calls. Interactive OS qualification remains independent.
    pub machine_actions: Vec<MachineAction>,
}
impl CliPolicy {
    /// Closed ingress allowlist. A machine call cannot submit a Human Policy or confirmation.
    pub fn permits(&self, request: &crate::host::Request) -> bool {
        use crate::host::Request;
        use execution_contract::DataOperation;
        let action = match request {
            Request::ServiceStatus {} => MachineAction::Status,
            Request::Entry {
                input: EntryOperation::Attach,
            } => return !self.machine_actions.is_empty(),
            Request::Entry {
                input: EntryOperation::Checkin,
            } => MachineAction::Checkin,
            Request::Entry {
                input: EntryOperation::Sync,
            } => MachineAction::Sync,
            Request::Entry {
                input:
                    EntryOperation::Data {
                        input: DataOperation::Submit(_),
                    }
                    | EntryOperation::Refresh { .. },
            } => MachineAction::Query,
            Request::Entry {
                input: EntryOperation::Data { .. },
            } => MachineAction::ReadData,
            _ => return false,
        };
        self.machine_actions.contains(&action)
    }
}
/// Safe attachment; no employee credential leaves the service.
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryContext {
    /// Current owner-issued native reference, absent for a machine caller.
    pub context: Option<NativeContextRef>,
    /// Current real registration coordinates; absence never fabricates an enterprise identity.
    pub namespace: Option<execution_contract::EntryNamespace>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Request;
    #[test]
    fn machine_grants_never_admit_human_invocations_or_arbitrary_data() {
        let artifact: installation_security::Artifact = serde_json::from_value(
            serde_json::json!({"path":"/protected/rssmdm","sha256":"0".repeat(64)}),
        )
        .unwrap();
        let policy = CliPolicy {
            image: artifact,
            machine_actions: vec![
                MachineAction::Status,
                MachineAction::Checkin,
                MachineAction::Sync,
                MachineAction::ReadData,
                MachineAction::Query,
            ],
        };
        assert!(policy.permits(&Request::ServiceStatus {}));
        assert!(!policy.permits(&Request::Offers {}));
        assert!(!policy.permits(&Request::Entry {
            input: EntryOperation::Resolve {
                selector: execution_contract::EntrySelector::Node { id: "home".into() }
            }
        }));
        assert!(!CliPolicy {
            machine_actions: vec![],
            ..policy
        }
        .permits(&Request::Entry {
            input: EntryOperation::Attach
        }));
    }
}

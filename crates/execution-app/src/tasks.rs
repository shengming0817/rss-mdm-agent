//! Unified, authorized projections of original owners; no task execution state is stored here.
use execution_contract::{Id, RequestId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Exact original owner identity, scoped by the authenticated native connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum TaskRef {
    /// Original execution or pre-execution request.
    Request {
        /// Original immutable request identity.
        request: RequestId,
    },
    /// Original periodic collection definition/session, without generating tasks per sample.
    Sampling {
        /// Exact original versioned collection definition.
        query: execution_contract::QueryRef,
    },
    /// Source-qualified server activity identity.
    Activity {
        /// Original producer ID, never a display name.
        id: String,
    },
}
/// Presentation source; never an authorization grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TaskSource {
    /// Authenticated human action.
    Human,
    /// Model-proposed action.
    Ai,
    /// Managed background activity.
    Policy,
}
/// Filter family, preserving each owner's facts in the detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TaskKind {
    /// Policy execution.
    Policy,
    /// Explicit query.
    Query,
    /// Software operation.
    Software,
    /// Management fact.
    Management,
    /// Periodic collection session.
    Sampling,
}
/// Coarse list partition only; detail facts retain their original states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TaskView {
    /// Active original requests.
    Active,
    /// All retained original requests.
    #[default]
    History,
    /// Authorized remote management facts.
    Management,
}
/// Bounded server-filtered task selection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskQuery {
    /// Visible partition.
    pub view: TaskView,
    /// Initiating source.
    pub source: Option<TaskSource>,
    /// Capability family.
    pub task_kind: Option<TaskKind>,
    /// Exact owner's serialized phase.
    pub status: Option<String>,
    /// Inclusive UTC lower time bound in milliseconds.
    pub from: Option<u64>,
    /// Exclusive UTC upper time bound in milliseconds.
    pub until: Option<u64>,
    /// Exact subject, always additionally checked against native authorization.
    pub subject: Option<String>,
    /// Exact Policy ID.
    pub policy: Option<String>,
    /// Opaque continuation bound to this filter and native scope.
    pub cursor: Option<String>,
    /// Requested bounded item count, 1..=64.
    pub limit: u16,
}
/// Original owner facts without privileged process output.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum TaskFacts {
    /// Safe execution journal facts.
    Execution(Box<crate::ExecutionTaskDetails>),
    /// Original preparation; no invented execution attempt.
    Preparation(Box<execution_contract::BackendRequest>),
    /// Existing periodic collection latest projection, distinct from pending-upload batches.
    Sampling {
        /// Trusted query definition.
        definition: Box<execution_contract::QueryDescriptor>,
        /// Already collected bounded facts and completeness.
        latest: Box<execution_contract::CollectedData>,
    },
    /// Device-authorized, scrubbed remote activity.
    Management(Box<rss_mdm_agent_wire::activities::ManagementActivity>),
}
/// Available owner operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TaskAction {
    /// Explicit independently admitted new request for a supported known failure.
    Retry,
    /// Request cancellation, independently from termination.
    Cancel,
    /// Pause this consumer's interruptible content preparation.
    Pause,
    /// Resume this consumer after renewed authorization.
    Resume,
}
/// Original child node identity, qualified by attempt and owner key.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskNode {
    /// Stable owner-qualified node ID.
    pub id: String,
    /// Parent owner node when present.
    pub parent: Option<String>,
    /// Safe stage name.
    pub stage: String,
    /// Actual observed bytes; not durable execution evidence.
    pub bytes: Option<u64>,
    /// Known signed or observed total; absent means indeterminate.
    pub total: Option<u64>,
    /// Original attempt identity.
    pub attempt: Option<String>,
    /// Node-specific control support explanation.
    pub reason: Option<String>,
    /// Real process exit for this physical invocation, independently from its effect.
    pub exit_code: Option<i32>,
    /// Independent detector state from the original software checkpoint.
    pub detected: Option<execution_contract::SoftwareState>,
    /// Confirmed invocation quiescence; absent does not mean terminated.
    pub quiescent: Option<bool>,
}
pub use execution_contract::TaskRevision;
/// One authorized original task, never a second execution snapshot.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskDetail {
    /// Original owner reference.
    pub reference: TaskRef,
    /// Current service/native identity stamp, required by controls.
    pub scope: String,
    /// Owner control revision; byte observations do not change it.
    pub revision: TaskRevision,
    /// Trusted recording time when retained by the owner.
    pub recorded_at: Option<u64>,
    /// Initiating source.
    pub source: TaskSource,
    /// Capability family.
    pub task_kind: TaskKind,
    /// Owner phase for filtering and display.
    pub stage: String,
    /// Current coarse activity, not effect proof.
    pub active: bool,
    /// Safe owner facts.
    pub facts: TaskFacts,
    /// Actual child observations.
    pub nodes: Vec<TaskNode>,
    /// Operations currently supported and authorized.
    pub allowed_actions: Vec<TaskAction>,
    /// Explanation for unsupported owner controls.
    pub control_reason: Option<String>,
    /// Independent durable upload receipt; absence means unknown/not applicable.
    pub upload_acknowledged: Option<bool>,
    /// Independent collection result validation, absent when unavailable/not applicable.
    pub collection_quality: Option<execution_contract::DataQuality>,
    /// Enumeration completeness never follows from process exit zero.
    pub collection_complete: Option<bool>,
    /// Independently observed local software state from the original fixed-worker result.
    pub software_state: Option<execution_contract::SoftwareState>,
    /// Original exact Policy ID/version, when the Policy owner retained one.
    pub policy: Option<execution_contract::VersionedRef>,
    /// Original Policy configuration revision.
    pub policy_revision: Option<u64>,
    /// Original request linked by an independently admitted retry.
    pub retry_of: Option<RequestId>,
    /// Independently authorized related producer identities.
    pub related: Vec<TaskRef>,
    /// Original signed-offer coordinates when retained by the business preparation owner.
    pub selection: Option<execution_contract::BackendSelection>,
    /// Explicit association gaps, including unconsumed related continuation.
    pub missing: Vec<String>,
    /// Parameter names and value categories only; never the original values.
    pub parameter_summary: std::collections::BTreeMap<String, String>,
    /// A dedicated trusted-human confirmation is currently relevant; no AI control variant exists.
    pub human_confirmation: bool,
}
/// Authorized list page with explicit coverage.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPage {
    /// Actual original tasks in this page.
    pub items: Vec<TaskDetail>,
    /// Opaque continuation; empty items can still have a continuation.
    pub next: Option<String>,
    /// Native identity stamp.
    pub scope: String,
    /// UTC observation time in milliseconds.
    pub as_of: u64,
    /// Last successful remote sync, absent before first sync.
    pub last_sync: Option<i64>,
    /// Most recent attempted remote read succeeded; cached completeness is not connectivity.
    pub remote_available: bool,
    /// Actual remote coverage, absent before first sync.
    pub coverage: Vec<rss_mdm_agent_wire::activities::ActivityCoverage>,
    /// Earliest retained remote fact time in seconds.
    pub retained_from: Option<i64>,
    /// Bounded sources are not necessarily a complete history.
    pub complete: bool,
}
/// One explicit control; retain its identity after a lost reply.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskControl {
    /// Original task.
    pub reference: TaskRef,
    /// Unique logical operation identity.
    pub id: Id,
    /// Exact observed native scope.
    pub scope: String,
    /// Owner CAS revision.
    pub expected_revision: TaskRevision,
    /// Explicit owner action.
    pub action: TaskAction,
}
/// Read the original owner's control receipt without resubmitting.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskControlQuery {
    /// Original task.
    pub reference: TaskRef,
    /// Original control identity.
    pub id: Id,
    /// Original native scope.
    pub scope: String,
}
/// Durable control disposition, independently from task termination/effect.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ControlDisposition {
    /// Committed by original owner.
    Confirmed,
    /// No receipt currently proves acceptance.
    Unknown,
    /// Observed owner revision is stale.
    Stale,
    /// Current authority or stage rejects this action.
    Denied,
}
/// Safe control receipt and separately queried current task facts.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskControlReceipt {
    /// Original control identity.
    pub id: Id,
    /// Owner-confirmed disposition.
    pub disposition: ControlDisposition,
    /// New original request only for a confirmed explicit retry.
    pub created: Option<TaskRef>,
}
/// Four operations shared by native and AI task consumers.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "input",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum TaskOperation {
    /// Bounded list.
    List(TaskQuery),
    /// Exact original detail.
    Detail(TaskRef),
    /// Explicit CAS control.
    Control(TaskControl),
    /// Read original receipt.
    ControlStatus(TaskControlQuery),
}
/// Closed task port responses.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum TaskResponse {
    /// List projection.
    Page(TaskPage),
    /// Detail projection.
    Detail(Box<TaskDetail>),
    /// Original owner's receipt.
    Control(TaskControlReceipt),
}

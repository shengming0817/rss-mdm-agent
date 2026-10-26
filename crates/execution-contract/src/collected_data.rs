//! Local collected facts and explicit queries. Descriptors never grant execution permission.
use crate::{Digest, Id, RequestId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Exact service-owned query definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryRef {
    /// Stable resource identity.
    pub id: Id,
    /// Digest of the complete current definition.
    pub revision: Digest,
}
/// Physical source, distinct from caller identity and execution permission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum QueryKind {
    /// Published native collection.
    Native,
    /// Employee-authorized script or osquery Policy.
    Policy,
    /// Signed periodic observation.
    Telemetry,
}
/// Provenance classification only; it never grants execution or local read permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum QueryOrigin {
    /// An explicitly selected, packaged local capability including its frozen shared baseline.
    Local,
    /// A verified enterprise task or configuration, subject to local read authorization.
    Enterprise,
}
/// Visibility of a local result, independent of the service's execution identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DataScope {
    /// Product-authorized device facts.
    Device,
    /// Facts private to one authenticated OS account.
    User {
        /// Exact OS account identity.
        subject: Id,
    },
}
/// Safe query discovery; an unavailable query can still have retained data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryDescriptor {
    /// Exact definition.
    pub reference: QueryRef,
    /// Display text; never an executable selector.
    pub title: String,
    /// Source family.
    pub kind: QueryKind,
    /// Frozen provenance, independent of the physical source and caller permissions.
    pub origin: QueryOrigin,
    /// Required local read boundary.
    pub scope: DataScope,
    /// Stable service-owned projection domains, independent of display title or producer labels.
    pub domains: Vec<String>,
    /// Complete safe caller input schema; native recipes have no parameters.
    pub input_schema: Value,
    /// Exact original published Policy, when applicable.
    pub policy: Option<PublishedQuery>,
    /// Whether a new invocation is currently available.
    pub available: bool,
    /// Closed unavailability reason.
    pub reason: Option<DataQuality>,
}
/// Original Policy coordinates for presentation and existing confirmation integration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishedQuery {
    /// Current server Policy identity.
    pub policy_id: Id,
    /// Current configuration revision.
    pub policy_revision: u64,
    /// Immutable execution version.
    pub policy_version: Id,
    /// Safe administrator description, never executable input.
    pub description: String,
}
/// Bounded query directory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryDirectory {
    /// Explicitly discover a page of currently authorized Policies.
    pub refresh_policies: bool,
    /// Opaque Policy discovery cursor; null starts the first page.
    pub policy_cursor: Option<String>,
    /// Exclusive stable query cursor.
    pub after: Option<Id>,
    /// Requested page size, from one through 128.
    pub limit: u16,
}
/// Directory page, scoped by the authenticated service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryPage {
    /// Next Policy discovery page; separate from local definition pagination.
    pub policy_next: Option<String>,
    /// Visible queries.
    pub items: Vec<QueryDescriptor>,
    /// Exclusive continuation.
    pub next: Option<Id>,
}
/// Observation quality; absence and a complete empty collection are different.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DataQuality {
    /// Complete validated observation.
    Complete,
    /// The collector reported absence for this source; distinct from never collected.
    Missing,
    /// Some data or sources are missing.
    Partial,
    /// A declared output budget was reached.
    Truncated,
    /// Collection failed.
    Failed,
    /// No observation has been collected.
    NotCollected,
    /// The definition cannot be recovered.
    DefinitionUnavailable,
    /// This collector or mapping is unsupported.
    Unsupported,
    /// New collection permission is absent.
    Denied,
    /// A configuration or result reference expired.
    Expired,
    /// An existing execution outcome is unknown.
    Unknown,
}
/// Select an existing snapshot without starting collection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DataSelection {
    /// Latest retained observation for an exact query.
    Query {
        /// Current query identity.
        query: QueryRef,
    },
    /// Result of one original request.
    Request {
        /// Stable request identity.
        request: RequestId,
    },
}
/// A cursor is only a position; every read rechecks caller authorization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataCursor {
    /// Exact snapshot digest, preventing pages from mixing observations.
    pub snapshot: Digest,
    /// Exclusive row position.
    pub offset: u32,
}
/// Bounded read of already collected data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataRead {
    /// Query or original request.
    pub selection: DataSelection,
    /// Immutable snapshot continuation.
    pub after: Option<DataCursor>,
    /// One through 128 rows.
    pub limit: u16,
    /// Optional consumer freshness requirement in seconds; never triggers collection.
    pub max_age_seconds: Option<u32>,
}
/// Authorized, bounded observation projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollectedData {
    /// Source definition.
    pub query: QueryRef,
    /// Required local read boundary.
    pub scope: DataScope,
    /// Original business request, if the source has one.
    pub request: Option<RequestId>,
    /// UTC collection time in seconds, absent for unobserved values.
    pub collected_at: Option<u64>,
    /// Actual evidence quality.
    pub quality: DataQuality,
    /// Whether the original enumeration was complete.
    pub enumeration_complete: bool,
    /// Allowlisted local field projection.
    pub fields: BTreeMap<String, Value>,
    /// Structured rows; no raw process streams.
    pub rows: Vec<Value>,
    /// Whether the consumer's freshness requirement was exceeded.
    pub stale: bool,
    /// Continuation for this exact snapshot.
    pub next: Option<DataCursor>,
}
/// Explicit new query; OS identity, target and origin come from authenticated IPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuerySubmit {
    /// Stable, caller-owned operation UUID.
    pub request: RequestId,
    /// Exact discovered query definition.
    pub query: QueryRef,
    /// Only parameters allowed by the published definition.
    pub parameters: BTreeMap<String, Value>,
}
/// Original query lookup or cancellation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryRequest {
    /// Original request UUID.
    pub request: RequestId,
}
/// Query state is a projection of the original execution, not another lifecycle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum QueryPhase {
    /// Submission accepted.
    Accepted,
    /// Existing business confirmation is required.
    AwaitingConfirmation,
    /// Physical collection is running.
    Running,
    /// A terminal, validated result is available.
    Completed,
    /// Execution failed.
    Failed,
    /// Cancellation was observed.
    Cancelled,
    /// Physical outcome is not known.
    Unknown,
}
/// Status of the same original request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryStatus {
    /// Original request.
    pub request: RequestId,
    /// Exact query.
    pub query: QueryRef,
    /// Actual state projection.
    pub phase: QueryPhase,
    /// Cancellation request; this does not prove termination.
    pub cancel_requested: bool,
    /// Existing Policy preparation/confirmation, when applicable.
    pub preparation: Option<QueryPreparationRef>,
}
/// Reference to the existing Policy preparation; use its existing confirmation UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryPreparationRef {
    /// Original Policy request; never a new approval identity.
    pub request: RequestId,
    /// Exact preparation revision.
    pub revision: u64,
}
/// Five operations shared by Native and AI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "input",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum DataOperation {
    /// Discover current queries.
    Directory(QueryDirectory),
    /// Read retained data.
    Read(DataRead),
    /// Explicitly submit one query.
    Submit(QuerySubmit),
    /// Look up the original request.
    Status(QueryRequest),
    /// Request business cancellation.
    Cancel(QueryRequest),
}
/// Typed result of the shared data boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum DataResponse {
    /// Directory page.
    Directory(QueryPage),
    /// Existing result page.
    Read(CollectedData),
    /// Original request state.
    Status(QueryStatus),
}

/// Closed data access errors; never contain process output or credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DataError {
    /// Invalid bounded input.
    InvalidInput,
    /// Caller is not authorized.
    Denied,
    /// No record in the authorized namespace.
    NotFound,
    /// Original identity names different content or snapshot.
    Conflict,
    /// Required authority or reference expired.
    Expired,
    /// Collector or mapping is unsupported.
    Unsupported,
    /// The same query already has an unsettled request in this login.
    InProgress,
    /// Bounded storage or response capacity reached.
    Capacity,
    /// Local dependency unavailable; query the original ID after an ambiguous submission.
    Unavailable,
}

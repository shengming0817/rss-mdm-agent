use crate::{
    ActorId, DeviceId, Digest, EnvironmentKey, Id, NetworkDestination, PlanId, RequestId, V1,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Exact resource/configuration reference. Resolution and authenticity belong to the owner.
pub struct VersionedRef {
    /// Opaque reference identity; the revision must be supplied separately.
    pub id: Id,
    /// Exact immutable revision reference; does not resolve or follow a moving alias.
    pub revision: Id,
}

/// Separate namespaces: none of these serializable references authenticates its issuer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Authority {
    /// Local authority namespace, established and verified by a later trusted host.
    Local {
        /// Authority reference in this variant namespace; no proof of issuer authenticity.
        id: Id,
    },
    /// Enterprise authority with an explicit tenant; verified by the product identity owner.
    Enterprise {
        /// Authority reference in this variant namespace; no proof of issuer authenticity.
        id: Id,
        /// Explicit enterprise tenant reference; local/test authority never fabricates a tenant.
        tenant: Id,
    },
    /// Explicit test-only authority; cannot issue production or real-platform evidence.
    Test {
        /// Authority reference in this variant namespace; no proof of issuer authenticity.
        id: Id,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
/// Request origin and account provenance; never grants the product actor additional authority.
pub enum Initiator {
    /// Human origin with explicit OS login provenance.
    Human {
        /// Originating OS account/session reference, separate from requested run-as identity.
        os_session: OsSessionRef,
    },
    /// AI origin with OS provenance and the exact product connection configuration.
    Ai {
        /// Provider namespace, independent of product authentication.
        provider: Id,
        /// Originating OS account/session reference, separate from requested run-as identity.
        os_session: OsSessionRef,
        /// Exact connection configuration used at initiation; no external account identity.
        config: VersionedRef,
        /// Originating AI conversation reference in the provider namespace.
        conversation: Id,
        /// Originating tool-call reference; cannot act as a product approval.
        tool_call: Id,
    },
    /// Policy origin without an interactive OS/provider login; authority still comes from the actor context.
    Policy {
        /// Exact policy revision associated with this request or audit decision.
        policy: VersionedRef,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Target OS semantics for validation; an enum value is not a platform support claim.
pub enum Platform {
    /// Windows target/account namespace.
    Windows,
    /// macOS target/account namespace.
    Macos,
    /// Linux host/account design namespace; not a claim of MDM-managed Linux support.
    Linux,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Opaque account reference in an OS namespace, independent of product or provider identity.
pub struct OsAccountRef {
    /// OS namespace of this reference or target; does not assert platform support.
    pub platform: Platform,
    /// Opaque stable OS account reference, such as a SID/UID reference, not an authentication credential.
    pub subject: Id,
}
/// Claimed OS login provenance at the request origin, separate from target and run-as identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OsSessionRef {
    /// Origin device reference; it does not establish registration or target authority.
    pub device: DeviceId,
    /// Login account at the origin; never implicitly equated to the product actor.
    pub account: OsAccountRef,
    /// Origin OS login/session reference, including an explicit test reference in fixtures.
    pub session: Id,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
/// Device-wide or explicit-user scope of the requested operation.
pub enum TargetScope {
    /// Operation targets the device as a whole.
    Device {},
    /// Operation targets an explicit OS user on the device.
    User {
        /// Explicit account reference; never implicitly mapped to a product actor by name or email.
        account: OsAccountRef,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Explicit device, platform and scope bound into the plan digest.
pub struct Target {
    /// Explicit device reference in the containing authority/context; not registration evidence.
    pub device: DeviceId,
    /// OS namespace of this reference or target; does not assert platform support.
    pub platform: Platform,
    /// Device-wide or explicit-user scope as defined by this type.
    pub scope: TargetScope,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
/// Requested execution identity; authorization and actual identity switching are external.
pub enum RunAs {
    /// Run under the explicitly referenced target OS account.
    User {
        /// Explicit account reference; never implicitly mapped to a product actor by name or email.
        account: OsAccountRef,
    },
    /// Request the target platform system identity; this declaration does not grant elevation.
    System {
        /// OS namespace of this reference or target; does not assert platform support.
        platform: Platform,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Action and exact resource identity submitted to the authorization owner.
pub struct Operation {
    /// Stable operation identifier owned by the catalog/authorization policy.
    pub action: Id,
    /// Exact resource ID and revision; no latest-version lookup is performed here.
    pub resource: VersionedRef,
}

/// Literal values are untrusted data. Long-lived credentials MUST use a versioned secret reference.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum InputValue {
    /// Untrusted JSON parameter data; environment literals are additionally restricted to NUL-free strings.
    Literal {
        #[serde(deserialize_with = "crate::validation::unique_json")]
        /// Untrusted literal data; long-lived credentials must be passed as controlled references.
        value: serde_json::Value,
    },
    /// Exact secret reference resolved under owner authorization; never embeds credential bytes.
    Secret {
        /// Exact versioned reference. Authenticity and access are checked by its owner.
        reference: VersionedRef,
    },
}
impl fmt::Debug for InputValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InputValue([redacted])")
    }
}

/// Intent data. The authority/actor are claims to be authenticated by the host, never credentials.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionRequest {
    /// Required current V1 discriminator; absent or unsupported versions are rejected.
    pub schema_version: V1,
    /// Stable local request correlation identity.
    pub request_id: RequestId,
    /// Claimed authority namespace; a deserialized reference is not an authenticated issuer.
    pub authority: Authority,
    /// Claimed product actor reference; OS/provider login does not establish this identity.
    pub actor: ActorId,
    /// Origin and account provenance only; authority must be independently verified.
    pub initiator: Initiator,
    /// Exact delegation reference when acting through delegation; absence grants no authority.
    pub delegation: Option<VersionedRef>,
    /// Explicit device/platform/user target; it is independent from the originating account.
    pub target: Target,
    /// Action and exact resource reference whose permission must be decided by the owner.
    pub operation: Operation,
    #[serde(deserialize_with = "crate::validation::unique_map")]
    /// Untrusted parameter values or versioned secret references; the catalog owns parameter semantics.
    pub parameters: BTreeMap<String, InputValue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Resource revision and exact SHA-256 content identity; does not itself verify downloaded bytes.
pub struct ExactArtifactRef {
    /// Exact resource ID and revision; no latest-version lookup is performed here.
    pub resource: VersionedRef,
    /// Expected content SHA-256. The adapter must verify the actual bytes before use.
    pub sha256: Digest,
}

/// Exact launch description only. No PATH lookup, download or spawning is performed by this crate.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchSpec {
    /// Exact script/installer/test artifact chosen for this plan.
    pub artifact: ExactArtifactRef,
    /// Exact interpreter artifact; the runner must not substitute PATH or a newer version.
    pub interpreter: crate::InterpreterRef,
    /// Ordered literal arguments and exactly one verified artifact-path slot; never a shell string.
    pub argv: Vec<crate::LaunchArg>,
    /// Expected original artifact bytes; no transcoding or script wrapping is permitted.
    pub artifact_encoding: crate::ArtifactEncoding,
    /// Explicit noninteractive input; never inherited from the host.
    pub stdin: crate::StandardInput,
    /// Explicit capture and decoding requirements for both output streams.
    pub output: crate::OutputSpec,
    /// Absolute working-directory declaration; platform adapters must resolve and verify real paths.
    pub cwd: String,
    #[serde(deserialize_with = "crate::validation::unique_map")]
    /// Clear inherited environment first, then set only these values. Windows names are normalized after collision rejection.
    pub env: BTreeMap<EnvironmentKey, InputValue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
/// Required network restrictions. Unknown or unenforceable restrictions must be rejected by the runner.
pub enum NetworkAccess {
    /// Network access must be denied by the runner.
    Denied {},
    /// Only the explicitly enumerated endpoints/protocols are permitted.
    Allowlist {
        /// Nonempty exact endpoint list; duplicate normalized destinations are rejected.
        destinations: Vec<NetworkDestination>,
    },
}
/// Required restrictions are declarations; platform enforcement is owned by the runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Constraints {
    /// Required network destinations/protocol restrictions, not proof that isolation is available.
    pub network: NetworkAccess,
    /// Absolute read-access declarations; actual resolution/enforcement belongs to the runner.
    pub read_paths: Vec<String>,
    /// Absolute write-access declarations; no filesystem access is performed during validation.
    pub write_paths: Vec<String>,
    /// Whether child processes are permitted by the plan; the runner must enforce this value.
    pub allow_child_processes: bool,
    /// Whether sandbox enforcement is mandatory; missing support must not silently downgrade it.
    pub require_sandbox: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Cumulative allowances for the entire plan, never replenished by retries.
pub struct ExecutionBudget {
    /// Total elapsed wall time from the first attempt start, including retries/backoff/waits.
    /// Pre-execution approval waits are excluded; retries never replenish this budget.
    pub total_timeout_ms: u64,
    /// Aggregate stdout/stderr/result bytes across all attempts, including discarded output.
    pub total_output_bytes: u64,
    /// Maximum attempts for the whole plan; retries never multiply time/output budgets.
    pub max_attempts: u32,
}
/// UTC Unix milliseconds. The host separately validates current time and revocation freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidityWindow {
    /// Earliest validity instant as UTC Unix milliseconds; no system clock is consulted here.
    pub not_before_unix_ms: u64,
    /// Exclusive validity deadline as UTC Unix milliseconds, strictly after the start.
    pub expires_at_unix_ms: u64,
}

/// Required target user-session availability, independent of the originating login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SessionRequirement {
    /// No active desktop login is required; the explicit run-as identity still applies.
    NotRequired {},
    /// Require an active session for this account on the plan's target device.
    ActiveUser {
        /// Exact account whose session is required, not inferred from the initiator.
        account: OsAccountRef,
    },
}

/// The sole canonical execution description, including its original request.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanSpec {
    /// Required current V1 discriminator; absent or unsupported versions are rejected.
    pub schema_version: V1,
    /// Immutable local plan identity, also bound into its digest.
    pub plan_id: PlanId,
    /// Original operation intent, retained once as part of the canonical plan.
    pub request: ExecutionRequest,
    /// Resolved launch description, including exact artifacts and ordered process inputs.
    pub launch: LaunchSpec,
    /// Requested target execution identity, separate from actor and originating login.
    pub run_as: RunAs,
    /// Mandatory session requirement bound into the plan digest; never inferred or defaulted.
    pub session_requirement: SessionRequirement,
    /// Mandatory execution restrictions bound into the digest.
    pub constraints: Constraints,
    /// Plan-wide resource allowances checked against separately supplied host limits.
    pub budget: ExecutionBudget,
    /// Bounded validity window; current time and revocation freshness are checked externally.
    pub validity: ValidityWindow,
    /// Exact policy revision associated with this request or audit decision.
    pub policy: VersionedRef,
}
macro_rules! redacted_debug {
    ($($t:ty),*) => { $(impl fmt::Debug for $t {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($t), "([redacted])")) }
    })* };
}
redacted_debug!(ExecutionRequest, LaunchSpec, PlanSpec);

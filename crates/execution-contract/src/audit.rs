use crate::{
    ActorId, AttemptId, Authority, Digest, EventId, Id, Initiator, Operation, PlanId, RequestId,
    Target, VersionedRef, V1,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Recorded observation category. None of these references verifies a real-world effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceKind {
    TestResult,
    ProcessExited,
    StateObserved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRef {
    pub reference: VersionedRef,
    pub kind: EvidenceKind,
    pub runner: Id,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Proposed,
    Denied,
    ApprovalRequired,
    Admitted,
    Observed,
}
/// Audit data only, never an authorization or trusted evidence constructor.
/// Contains references and a reason code, not parameters, launch arguments or raw output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuditEvent {
    pub schema_version: V1,
    pub event_id: EventId,
    pub authority: Authority,
    pub request_id: RequestId,
    pub plan_id: PlanId,
    pub plan_digest: Digest,
    pub attempt_id: Option<AttemptId>,
    pub actor: ActorId,
    pub initiator: Initiator,
    pub approver: Option<ActorId>,
    pub operation: Operation,
    pub target: Target,
    pub decision: Decision,
    pub reason: Id,
    pub delegation: Option<VersionedRef>,
    pub policy: VersionedRef,
    pub approval: Option<VersionedRef>,
    pub occurred_at_unix_ms: u64,
    pub evidence: Vec<EvidenceRef>,
}

/// Decode bounded audit data. A successful decode is not evidence verification.
pub fn decode_audit(
    bytes: &[u8],
    limits: &crate::PlanLimits,
) -> Result<AuditEvent, crate::ContractError> {
    let value = crate::validation::decode_value(bytes, limits)?;
    let event: AuditEvent =
        serde_json::from_value(value).map_err(|_| crate::ContractError::Encoding)?;
    if matches!(event.authority, Authority::Test { .. })
        && event
            .evidence
            .iter()
            .any(|e| e.kind != EvidenceKind::TestResult)
    {
        return Err(crate::ContractError::Context);
    }
    Ok(event)
}

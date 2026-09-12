use crate::{
    ActorId, AttemptId, Authority, ContractError, Digest, EventId, Id, Initiator, Operation,
    PlanId, PlanLimits, RequestId, Target, VersionedRef, V1,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

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

/// Nonempty evidence references. References still require verification by their owner.
/// INVARIANT: AUDIT-OBSERVATION-CORRELATION-01 — an observation cannot omit its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct EvidenceRefs(#[schemars(length(min = 1))] Vec<EvidenceRef>);
impl EvidenceRefs {
    pub fn new(evidence: Vec<EvidenceRef>) -> Result<Self, ContractError> {
        if evidence.is_empty() {
            return Err(ContractError::Value);
        }
        Ok(Self(evidence))
    }
    pub fn as_slice(&self) -> &[EvidenceRef] {
        &self.0
    }
}
impl<'de> Deserialize<'de> for EvidenceRefs {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(Vec::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
/// INVARIANT: AUDIT-OBSERVATION-CORRELATION-01 — every observation is tied to one attempt.
/// Earlier admission decisions do not invent an execution attempt or evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Decision {
    Proposed {},
    Denied {},
    ApprovalRequired {},
    Admitted {},
    Observed {
        attempt_id: AttemptId,
        evidence: EvidenceRefs,
    },
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
}
impl AuditEvent {
    /// Validate constructed audit DTOs; not a trusted audit receipt or signature check.
    pub fn validate(&self, limits: &PlanLimits) -> Result<(), ContractError> {
        limits.validate()?;
        if let Decision::Observed { evidence, .. } = &self.decision {
            if evidence.as_slice().len() > limits.max_collection_items {
                return Err(ContractError::Limit);
            }
            if matches!(self.authority, Authority::Test { .. })
                && evidence
                    .as_slice()
                    .iter()
                    .any(|e| e.kind != EvidenceKind::TestResult)
            {
                return Err(ContractError::Context);
            }
        }
        let value = serde_json::to_value(self).map_err(|_| ContractError::Encoding)?;
        crate::validation::check_json(&value, limits)?;
        if serde_json::to_vec(&value)
            .map_err(|_| ContractError::Encoding)?
            .len()
            > limits.max_input_bytes
        {
            return Err(ContractError::Limit);
        }
        Ok(())
    }
}
/// Decode bounded audit data. A successful decode is not evidence verification.
pub fn decode_audit(bytes: &[u8], limits: &PlanLimits) -> Result<AuditEvent, ContractError> {
    let value = crate::validation::decode_value(bytes, limits)?;
    let event: AuditEvent = crate::validation::typed_value(value)?;
    event.validate(limits)?;
    Ok(event)
}

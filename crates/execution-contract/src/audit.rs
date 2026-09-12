use crate::{
    ActorId, AttemptId, Authority, ContractError, Digest, EventId, Id, Initiator, Operation,
    PlanId, PlanLimits, RequestId, Target, VersionedRef, V1,
};
use crate::{ErrorKind, Field, Rule};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// Recorded observation category. None of these references verifies a real-world effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceKind {
    /// Fixture/test result only; cannot assert a real process or device effect.
    TestResult,
    /// Reference to a process-exit observation; does not prove the target state converged.
    ProcessExited,
    /// Reference to an independently observed target state; still requires evidence verification.
    StateObserved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Versioned observation reference and runner provenance; the referenced fact remains unverified.
pub struct EvidenceRef {
    /// Exact versioned reference. Authenticity and access are checked by its owner.
    pub reference: VersionedRef,
    /// Recorded observation category; a category label does not prove an external fact.
    pub kind: EvidenceKind,
    /// Runner reference whose evidence is being recorded, including an explicit fixture runner in tests.
    pub runner: Id,
}

/// Nonempty evidence references. References still require verification by their owner.
/// INVARIANT: AUDIT-OBSERVATION-CORRELATION-01 — an observation cannot omit its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct EvidenceRefs(#[schemars(length(min = 1))] Vec<EvidenceRef>);
impl EvidenceRefs {
    /// Validate and construct this value; failures identify a static field and rule without returning input.
    pub fn new(evidence: Vec<EvidenceRef>) -> Result<Self, ContractError> {
        if evidence.is_empty() {
            return Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::AuditEvidence,
                Rule::Empty,
            ));
        }
        Ok(Self(evidence))
    }
    /// Borrow the nonempty evidence references without verifying their external facts.
    pub fn as_slice(&self) -> &[EvidenceRef] {
        &self.0
    }
}
impl<'de> Deserialize<'de> for EvidenceRefs {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(Vec::deserialize(d)?).map_err(|error| serde::de::Error::custom(error.for_serde()))
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
    /// Request or plan proposal, before execution admission.
    Proposed {},
    /// Permission/admission denied; no execution proof is implied.
    Denied {},
    /// A separate approval is needed before admission.
    ApprovalRequired {},
    /// Recorded admission decision, not an executable permit.
    Admitted {},
    /// Observation with a mandatory attempt identity and nonempty evidence references.
    Observed {
        /// Concrete execution attempt associated with this observation.
        attempt_id: AttemptId,
        /// Nonempty observation references tied to the same concrete attempt; still require verification.
        evidence: EvidenceRefs,
    },
}
/// Audit data only, never an authorization or trusted evidence constructor.
/// Contains references and a reason code, not parameters, launch arguments or raw output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuditEvent {
    /// Required current V1 discriminator; absent or unsupported versions are rejected.
    pub schema_version: V1,
    /// Stable audit event identity used for correlation and deduplication by the journal.
    pub event_id: EventId,
    /// Claimed authority namespace; a deserialized reference is not an authenticated issuer.
    pub authority: Authority,
    /// Stable local request correlation identity.
    pub request_id: RequestId,
    /// Immutable local plan identity, also bound into its digest.
    pub plan_id: PlanId,
    /// Digest of the exact frozen plan observed by this record, not a signature or permit.
    pub plan_digest: Digest,
    /// Claimed product actor reference; OS/provider login does not establish this identity.
    pub actor: ActorId,
    /// Origin and account provenance only; authority must be independently verified.
    pub initiator: Initiator,
    /// Referenced approving product actor, if applicable; not a trusted approval by itself.
    pub approver: Option<ActorId>,
    /// Action and exact resource reference whose permission must be decided by the owner.
    pub operation: Operation,
    /// Explicit device/platform/user target; it is independent from the originating account.
    pub target: Target,
    /// Recorded decision/observation shape; recording Admitted cannot issue a permit.
    pub decision: Decision,
    /// Safe reason-code identifier; do not put user text, payload or credentials here.
    pub reason: Id,
    /// Exact delegation reference when acting through delegation; absence grants no authority.
    pub delegation: Option<VersionedRef>,
    /// Exact policy revision associated with this request or audit decision.
    pub policy: VersionedRef,
    /// Exact approval-record reference, if applicable; validity/consumption are checked externally.
    pub approval: Option<VersionedRef>,
    /// Recorded event instant as UTC Unix milliseconds; supplied by the owner clock.
    pub occurred_at_unix_ms: u64,
}
impl AuditEvent {
    /// Validate constructed audit DTOs; not a trusted audit receipt or signature check.
    pub fn validate(&self, limits: &PlanLimits) -> Result<(), ContractError> {
        limits.validate()?;
        if let Decision::Observed { evidence, .. } = &self.decision {
            if evidence.as_slice().len() > limits.max_collection_items {
                return Err(ContractError::new(
                    ErrorKind::LimitExceeded,
                    Field::AuditEvidence,
                    Rule::CollectionLimit,
                ));
            }
            if matches!(self.authority, Authority::Test { .. })
                && evidence
                    .as_slice()
                    .iter()
                    .any(|e| e.kind != EvidenceKind::TestResult)
            {
                return Err(ContractError::new(
                    ErrorKind::InconsistentContext,
                    Field::AuditEvidence,
                    Rule::TestEvidence,
                ));
            }
        }
        let value = serde_json::to_value(self)
            .map_err(|_| ContractError::new(ErrorKind::Encoding, Field::Document, Rule::Syntax))?;
        crate::validation::check_json(&value, limits)?;
        if serde_json::to_vec(&value)
            .map_err(|_| ContractError::new(ErrorKind::Encoding, Field::Document, Rule::Syntax))?
            .len()
            > limits.max_input_bytes
        {
            return Err(ContractError::new(
                ErrorKind::LimitExceeded,
                Field::InputBytes,
                Rule::ByteLimit,
            ));
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

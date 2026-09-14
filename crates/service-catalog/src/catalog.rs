use crate::{
    bounded, parameters, ArgumentRule, CatalogError, CatalogItem, CatalogLimits, CatalogRef,
    CatalogSnapshot, DefinitionRule, Limit, OperationVariant, ParameterLimits, ParameterProjection,
    PublicationState, SelectionRef,
};
use execution_contract::{Digest, Id, InputValue, Target};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Validated immutable directory. Freezing binds content but does not authenticate the publisher.
/// ```compile_fail
/// let _: service_catalog::FrozenCatalog = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct FrozenCatalog {
    snapshot: CatalogSnapshot,
    reference: CatalogRef,
}
/// Strict decode and semantic validation, followed by canonical freezing.
/// Returns safe static errors; never resolves resources, reads a clock or authorizes execution.
pub fn decode_catalog(bytes: &[u8], limits: &CatalogLimits) -> Result<FrozenCatalog, CatalogError> {
    let value = bounded::decode(bytes, limits).map_err(|error| match error {
        CatalogError::InvalidArguments(ArgumentRule::RoundedNumber) => CatalogError::InvalidShape,
        other => other,
    })?;
    if value.get("schemaVersion").and_then(|v| v.as_u64()) != Some(1) {
        return Err(CatalogError::UnsupportedVersion);
    }
    let mut snapshot: CatalogSnapshot =
        serde_json::from_value(value).map_err(|_| CatalogError::InvalidShape)?;
    validate_snapshot(&snapshot)?;
    // Collection order is not directory identity; parameter maps are already ordered.
    snapshot.items.sort_by(|a, b| a.id.cmp(&b.id));
    for item in &mut snapshot.items {
        item.operations.sort_by(|a, b| a.id.cmp(&b.id));
        for operation in &mut item.operations {
            operation.requirements.capabilities.sort();
            operation.requirements.evidence.sort();
        }
    }
    let canonical =
        serde_json_canonicalizer::to_vec(&snapshot).map_err(|_| CatalogError::Encoding)?;
    if canonical.len() > limits.max_bytes {
        return Err(CatalogError::LimitExceeded(Limit::Bytes));
    }
    let digest = Sha256::new()
        .chain_update(b"rss-mdm-agent/service-catalog/v1\0")
        .chain_update(&canonical)
        .finalize();
    let digest = Digest::new(
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    )
    .map_err(|_| CatalogError::Encoding)?;
    let reference = CatalogRef {
        authority: snapshot.authority.clone(),
        identity: snapshot.identity.clone(),
        digest,
    };
    Ok(FrozenCatalog {
        snapshot,
        reference,
    })
}
impl FrozenCatalog {
    /// Freeze caller-constructed DTOs through the same bounded decoder; no unchecked constructor.
    pub fn freeze(snapshot: CatalogSnapshot, limits: &CatalogLimits) -> Result<Self, CatalogError> {
        limits.validate()?;
        decode_catalog(&bounded::encode(&snapshot, limits.max_bytes)?, limits)
    }
    /// Immutable validated data; cloning a DTO never creates a validated selection.
    pub fn snapshot(&self) -> &CatalogSnapshot {
        &self.snapshot
    }
    /// Complete exact content identity, including namespace.
    pub fn reference(&self) -> CatalogRef {
        self.reference.clone()
    }
    fn find(
        &self,
        item: &Id,
        variant: &Id,
    ) -> Result<(&CatalogItem, &OperationVariant), CatalogError> {
        let item = self
            .snapshot
            .items
            .iter()
            .find(|v| &v.id == item)
            .ok_or(CatalogError::NotFound)?;
        let variant = item
            .operations
            .iter()
            .find(|v| &v.id == variant)
            .ok_or(CatalogError::NotFound)?;
        Ok((item, variant))
    }
    /// One shared projection for human forms and AI tools; no capability or permission filtering.
    pub fn projection(
        &self,
        item: &Id,
        variant: &Id,
        limits: &ParameterLimits,
    ) -> Result<ParameterProjection, CatalogError> {
        parameters::projection(&self.find(item, variant)?.1.parameters, limits)
    }
    /// Select exact immutable content and normalize parameters. Expired/withdrawn content remains
    /// inspectable; call availability for a display explanation. No result is an execution permit.
    pub fn select(
        &self,
        bytes: &[u8],
        limits: &CatalogLimits,
        parameter_limits: &ParameterLimits,
    ) -> Result<SelectedOperation, CatalogError> {
        let input: SelectionInput = serde_json::from_value(bounded::decode(bytes, limits)?)
            .map_err(|_| CatalogError::InvalidShape)?;
        if input.catalog != self.reference {
            return Err(CatalogError::ReferenceMismatch);
        }
        let (item, operation) = self.find(&input.item_id, &input.variant_id)?;
        let projection = parameters::projection(&operation.parameters, parameter_limits)?;
        let arguments = bounded::encode(&input.arguments, parameter_limits.max_bytes)?;
        let parameters = projection.validate(&arguments)?;
        Ok(SelectedOperation {
            reference: SelectionRef {
                catalog: input.catalog,
                item_id: input.item_id,
                variant_id: input.variant_id,
                arguments_digest: {
                    let bytes = serde_json_canonicalizer::to_vec(&parameters)
                        .map_err(|_| CatalogError::Encoding)?;
                    let digest = Sha256::new()
                        .chain_update(b"rss-mdm-agent/catalog-arguments/v1\0")
                        .chain_update(bytes)
                        .finalize();
                    Digest::new(format!("{digest:x}")).map_err(|_| CatalogError::Encoding)?
                },
            },
            operation: operation.clone(),
            parameters,
            state: item.state,
            expires_at: self.snapshot.expires_at_unix_ms,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionInput {
    catalog: CatalogRef,
    item_id: Id,
    variant_id: Id,
    arguments: serde_json::Value,
}
fn unique<'a>(values: impl Iterator<Item = &'a Id>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|v| seen.insert(v))
}
fn validate_snapshot(s: &CatalogSnapshot) -> Result<(), CatalogError> {
    if s.expires_at_unix_ms == 0 || s.expires_at_unix_ms > parameters::MAX_INTEGER as u64 {
        return Err(CatalogError::InvalidDefinition(DefinitionRule::TimeWindow));
    }
    if !unique(s.items.iter().map(|v| &v.id)) {
        return Err(CatalogError::InvalidDefinition(
            DefinitionRule::DuplicateItem,
        ));
    }
    for item in &s.items {
        if item.name.trim().is_empty() || item.category.trim().is_empty() {
            return Err(CatalogError::InvalidDefinition(DefinitionRule::Display));
        }
        if item.operations.is_empty() || !unique(item.operations.iter().map(|v| &v.id)) {
            return Err(CatalogError::InvalidDefinition(DefinitionRule::Operations));
        }
        for op in &item.operations {
            parameters::validate_definitions(&op.parameters)?;
            if op.requirements.evidence.is_empty()
                || !unique(op.requirements.capabilities.iter())
                || !unique(op.requirements.evidence.iter())
            {
                return Err(CatalogError::InvalidDefinition(
                    DefinitionRule::Requirements,
                ));
            }
        }
    }
    Ok(())
}
/// Frozen choice preserving all declarations. Its fields cannot be mutated or deserialized.
/// ```compile_fail
/// let _: service_catalog::SelectedOperation = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone)]
pub struct SelectedOperation {
    reference: SelectionRef,
    operation: OperationVariant,
    parameters: BTreeMap<String, InputValue>,
    state: PublicationState,
    expires_at: u64,
}
impl fmt::Debug for SelectedOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SelectedOperation([redacted])")
    }
}
/// Explanation of the recorded directory state, not resource freshness or permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogAvailability {
    /// Snapshot records a listed item and the supplied time is before expiry. Not executable authority.
    Listed,
    /// Snapshot explicitly records withdrawal.
    Withdrawn,
    /// Supplied UTC time is at or past exclusive expiry.
    Expired,
}
impl SelectedOperation {
    /// Exact snapshot/item/variant locator, suitable for binding later owner assessments.
    pub fn reference(&self) -> &SelectionRef {
        &self.reference
    }
    /// Complete action, resource version digest, selector, schema and requirements; none are dropped.
    pub fn operation(&self) -> &OperationVariant {
        &self.operation
    }
    /// Shared normalized values, with secrets represented only by versioned references.
    pub fn parameters(&self) -> &BTreeMap<String, InputValue> {
        &self.parameters
    }
    /// Pure recorded-state explanation. Host supplies trusted UTC Unix milliseconds and checks clock freshness.
    /// Withdrawal takes precedence over expiry; no local clock or cached execution grant is consulted.
    pub fn availability(&self, now_unix_ms: u64) -> CatalogAvailability {
        if self.state == PublicationState::Withdrawn {
            CatalogAvailability::Withdrawn
        } else if now_unix_ms >= self.expires_at {
            CatalogAvailability::Expired
        } else {
            CatalogAvailability::Listed
        }
    }
    /// Check association/freshness of an owner-provided display annotation. Does not verify its issuer,
    /// match capabilities, resolve artifacts or grant permission; missing/expired annotations stay unknown.
    pub fn display_status(
        &self,
        target: &Target,
        now_unix_ms: u64,
        assessment: Option<&ExternalAssessment>,
    ) -> Result<DisplayStatus, CatalogError> {
        let Some(a) = assessment else {
            return Ok(DisplayStatus::default());
        };
        if &a.selection != self.reference() || &a.target != target {
            return Err(CatalogError::ReferenceMismatch);
        }
        if a.checked_at_unix_ms >= a.expires_at_unix_ms
            || a.expires_at_unix_ms > parameters::MAX_INTEGER as u64
        {
            return Err(CatalogError::InvalidDefinition(DefinitionRule::TimeWindow));
        }
        if now_unix_ms < a.checked_at_unix_ms || now_unix_ms >= a.expires_at_unix_ms {
            return Ok(DisplayStatus::default());
        }
        Ok(a.display)
    }
}
/// Independent display axes from the capability/authorization owner, never execution authority.
/// Hosts must authenticate the source and reauthorize at execution, including for Allowed values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplayStatus {
    /// Whether this exact choice should be shown to this target.
    pub visibility: DisplayDecision,
    /// Whether this target can request this exact choice.
    pub requestability: DisplayDecision,
    /// Whether owner preflight currently reports this exact choice as executable.
    pub executability: DisplayDecision,
}
/// Closed owner-provided display decision for one axis; never a permission.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayDecision {
    /// No fresh owner explanation; says nothing about execution readiness.
    #[default]
    Unknown,
    /// Owner reports this display axis as allowed; execution still requires reauthorization.
    Allowed,
    /// Owner reports missing capability; catalog does not compute the match.
    MissingCapability,
    /// Owner reports an unsupported target.
    UnsupportedTarget,
    /// Owner reports an unresolved exact resource.
    UnresolvedResource,
    /// Owner reports another execution blocker without exposing raw details.
    Blocked,
}
/// Display annotation bound to a complete selection and explicit target. Deserialization is not trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalAssessment {
    /// Exact catalog namespace/version/digest/item/operation and normalized arguments digest.
    pub selection: SelectionRef,
    /// Explicit device/platform/user target, not authentication evidence.
    pub target: Target,
    /// Earliest applicability time, UTC Unix milliseconds.
    pub checked_at_unix_ms: u64,
    /// Exclusive annotation expiry, UTC Unix milliseconds.
    pub expires_at_unix_ms: u64,
    /// Independent visible/requestable/executable display decisions, never permissions.
    pub display: DisplayStatus,
}

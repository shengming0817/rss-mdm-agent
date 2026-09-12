use crate::{
    validation::{check_json, decode_value, validate_plan},
    ContractError, Digest, PlanLimits, PlanSpec,
};
use sha2::{Digest as _, Sha256};
use std::fmt;

const DIGEST_DOMAIN: &[u8] = b"rss-mdm-agent/execution-plan/v1\0";

/// Immutable, validated plan data. Freezing does not authenticate or authorize it.
///
/// INVARIANT: EXECUTION-FROZEN-PLAN-01 — fields are private and there is no Deserialize.
/// ```compile_fail
/// use execution_contract::FrozenPlan;
/// let _: FrozenPlan = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// use execution_contract::{FrozenPlan, PlanSpec, Digest};
/// fn forge(spec: PlanSpec, digest: Digest) -> FrozenPlan { FrozenPlan { spec, digest } }
/// ```
#[derive(Clone)]
pub struct FrozenPlan {
    spec: PlanSpec,
    digest: Digest,
}
impl FrozenPlan {
    pub fn freeze(spec: PlanSpec, limits: &PlanLimits) -> Result<Self, ContractError> {
        validate_plan(&spec, limits)?;
        // Bound caller-constructed dynamic values before recursive serialization.
        for value in spec
            .request
            .parameters
            .values()
            .chain(spec.launch.env.values())
        {
            if let crate::InputValue::Literal { value } = value {
                check_json(value, limits)?;
            }
        }
        let value = serde_json::to_value(&spec).map_err(|_| ContractError::Encoding)?;
        check_json(&value, limits)?;
        let bytes =
            serde_json_canonicalizer::to_vec(&value).map_err(|_| ContractError::Encoding)?;
        if bytes.len() > limits.max_input_bytes {
            return Err(ContractError::Limit);
        }
        let digest = Digest::from_bytes(
            &Sha256::new()
                .chain_update(DIGEST_DOMAIN)
                .chain_update(&bytes)
                .finalize()
                .into(),
        );
        // Consumers see the same normalized values whose bytes were hashed.
        let spec = serde_json::from_slice(&bytes).map_err(|_| ContractError::Encoding)?;
        Ok(Self { spec, digest })
    }
    pub fn spec(&self) -> &PlanSpec {
        &self.spec
    }
    pub fn digest(&self) -> &Digest {
        &self.digest
    }
    /// Compare a claimed stored digest only after rebuilding through freeze.
    pub fn matches_digest(&self, expected: &Digest) -> bool {
        &self.digest == expected
    }
}
impl fmt::Debug for FrozenPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrozenPlan")
            .field("plan_id", &self.spec.plan_id)
            .finish_non_exhaustive()
    }
}
/// Bounded strict decoding plus semantic validation. The result is still untrusted plan data.
pub fn decode_plan(bytes: &[u8], limits: &PlanLimits) -> Result<PlanSpec, ContractError> {
    let value = decode_value(bytes, limits)?;
    let spec: PlanSpec = crate::validation::typed_value(value)?;
    validate_plan(&spec, limits)?;
    Ok(spec)
}

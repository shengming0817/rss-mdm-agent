use crate::{
    validation::{check_json, decode_value, validate_plan},
    ContractError, Digest, ExecutionInput, ExecutionLimits,
};
use crate::{ErrorKind, Field, Rule};
use sha2::{Digest as _, Sha256};
use std::fmt;

const DIGEST_DOMAIN: &[u8] = b"rss-mdm-agent/execution-input/v5\0";

/// Immutable, validated plan data. Freezing does not authenticate or authorize it.
///
/// INVARIANT: EXECUTION-FROZEN-PLAN-01 — fields are private and there is no Deserialize.
/// ```compile_fail
/// use execution_contract::FrozenExecution;
/// let _: FrozenExecution = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// use execution_contract::{FrozenExecution, ExecutionInput, Digest};
/// fn forge(spec: ExecutionInput, digest: Digest) -> FrozenExecution { FrozenExecution { spec, digest } }
/// ```
#[derive(Clone)]
pub struct FrozenExecution {
    spec: ExecutionInput,
    digest: Digest,
}
impl FrozenExecution {
    /// Validate and normalize the complete plan, then bind its canonical V5 bytes to SHA-256.
    /// Returns a structured configuration, value, context, budget or encoding diagnostic; grants no authority.
    pub fn freeze(
        mut spec: ExecutionInput,
        limits: &ExecutionLimits,
    ) -> Result<Self, ContractError> {
        validate_plan(&spec, limits)?;
        // Validation rejects collisions before canonical collection can overwrite anything.
        spec.launch.env = std::mem::take(&mut spec.launch.env)
            .into_iter()
            .map(|(key, value)| (key.canonical_for(spec.request.target.platform), value))
            .collect();
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
        let value = serde_json::to_value(&spec)
            .map_err(|_| ContractError::new(ErrorKind::Encoding, Field::Document, Rule::Syntax))?;
        check_json(&value, limits)?;
        let bytes = serde_json_canonicalizer::to_vec(&value)
            .map_err(|_| ContractError::new(ErrorKind::Encoding, Field::Document, Rule::Syntax))?;
        if bytes.len() > limits.max_input_bytes {
            return Err(ContractError::new(
                ErrorKind::LimitExceeded,
                Field::InputBytes,
                Rule::ByteLimit,
            ));
        }
        let digest = Digest::from_bytes(
            &Sha256::new()
                .chain_update(DIGEST_DOMAIN)
                .chain_update(&bytes)
                .finalize()
                .into(),
        );
        // Consumers see the same normalized values whose bytes were hashed.
        let spec = serde_json::from_slice(&bytes)
            .map_err(|_| ContractError::new(ErrorKind::Encoding, Field::Document, Rule::Syntax))?;
        Ok(Self { spec, digest })
    }
    /// Borrow the immutable normalized plan. Mutating a clone requires a new freeze and digest.
    pub fn spec(&self) -> &ExecutionInput {
        &self.spec
    }
    /// Borrow the derived plan digest; this value is neither a signature nor approval.
    pub fn digest(&self) -> &Digest {
        &self.digest
    }
    /// Compare a claimed stored digest only after rebuilding through freeze.
    pub fn matches_digest(&self, expected: &Digest) -> bool {
        &self.digest == expected
    }
}
impl fmt::Debug for FrozenExecution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrozenExecution")
            .field("request_id", &self.spec.request.request_id)
            .finish_non_exhaustive()
    }
}
/// Bounded strict decoding plus semantic validation. The result is still untrusted plan data.
pub fn decode_execution(
    bytes: &[u8],
    limits: &ExecutionLimits,
) -> Result<ExecutionInput, ContractError> {
    let value = decode_value(bytes, limits)?;
    let spec: ExecutionInput = crate::validation::typed_value(value)?;
    validate_plan(&spec, limits)?;
    Ok(spec)
}

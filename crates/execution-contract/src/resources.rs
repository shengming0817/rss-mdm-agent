//! Additional exact resource claims, frozen with the execution and held in the same journal.
use crate::*;
/// Bounded resource keys augment native software manager exclusion. They grant no execution rights.
impl FrozenExecution {
    /// Physical resources retained until the whole attempt is independently quiescent.
    pub fn exclusive_resource_keys(&self) -> Result<Vec<String>, ContractError> {
        let mut keys = self
            .spec()
            .execution
            .software_program()
            .map(|p| p.lock_keys())
            .unwrap_or_default();
        if let Some(value) = self.spec().request.parameters.get("exclusive-resources") {
            let invalid =
                || ContractError::new(ErrorKind::InvalidValue, Field::Document, Rule::Syntax);
            let InputValue::Literal { value } = value else {
                return Err(invalid());
            };
            let values = value
                .as_array()
                .filter(|a| a.len() <= 8)
                .ok_or_else(invalid)?;
            for value in values {
                let key = value
                    .as_str()
                    .filter(|s| {
                        !s.is_empty()
                            && s.len() <= 128
                            && s.bytes().all(|b| {
                                b.is_ascii_lowercase()
                                    || b.is_ascii_digit()
                                    || matches!(b, b'-' | b':' | b'.')
                            })
                    })
                    .ok_or_else(invalid)?;
                if !keys.iter().any(|v| v == key) {
                    keys.push(key.into());
                }
            }
        }
        keys.sort();
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> ExecutionInput {
        serde_json::from_slice(include_bytes!("../tests/fixtures/plan.json")).unwrap()
    }
    #[test]
    fn claims_are_bounded_and_bound_to_the_complete_plan_digest() {
        let limits = ExecutionLimits {
            max_input_bytes: 65536,
            max_depth: 32,
            max_nodes: 8192,
            max_string_bytes: 8192,
            max_collection_items: 256,
            max_timeout_ms: 60000,
            max_output_bytes: 65536,
            max_stdin_bytes: 65536,
            max_attempts: 3,
        };
        let original = FrozenExecution::freeze(spec(), &limits).unwrap();
        let mut claimed = spec();
        claimed.request.parameters.insert(
            "exclusive-resources".into(),
            InputValue::Literal {
                value: serde_json::json!(["settings:display", "settings:display"]),
            },
        );
        let frozen = FrozenExecution::freeze(claimed.clone(), &limits).unwrap();
        assert_ne!(original.digest(), frozen.digest());
        assert_eq!(
            frozen.exclusive_resource_keys().unwrap(),
            vec!["settings:display"]
        );
        for value in [
            serde_json::json!("settings:display"),
            serde_json::json!(["../resource"]),
            serde_json::json!(vec!["resource"; 9]),
        ] {
            claimed
                .request
                .parameters
                .insert("exclusive-resources".into(), InputValue::Literal { value });
            assert!(FrozenExecution::freeze(claimed.clone(), &limits).is_err());
        }
    }
}

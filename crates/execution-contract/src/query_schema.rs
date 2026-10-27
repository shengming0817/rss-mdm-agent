//! Shared safe invocation interface, consumed by Native and MCP projections.
use serde_json::Value;
/// A published parameter schema cannot be represented by the shared caller contract.
#[derive(Debug, thiserror::Error)]
#[error("unsupported query parameter schema")]
pub struct QuerySchemaError;
/// Project only caller-owned Input parameters; retain server-owned full validation at invocation.
/// ref: existing Policy MCP parameter projection (same JSON safe integer intersection).
pub fn query_input_schema(
    item: &rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem,
) -> Result<Value, QuerySchemaError> {
    let mut schema = item.capability.input_schema.clone();
    project_parameter_schema(&mut schema)?;
    Ok(schema)
}
// The JS transport accepts only exactly representable integers. Project that intersection;
// the service still validates resolved parameters against the untouched Resource schema.
fn project_parameter_schema(schema: &mut serde_json::Value) -> Result<(), QuerySchemaError> {
    const SAFE: i64 = crate::MAX_SAFE_INTEGER as i64;
    if schema.is_boolean() {
        return Ok(());
    }
    let object = schema.as_object_mut().ok_or(QuerySchemaError)?;
    if object.get("type").and_then(serde_json::Value::as_str) == Some("integer") {
        for (keyword, edge, lower) in [("minimum", -SAFE, true), ("maximum", SAFE, false)] {
            let bound = object.get(keyword).and_then(serde_json::Value::as_f64);
            if bound.is_some_and(|value| {
                if lower {
                    value > SAFE as f64
                } else {
                    value < -SAFE as f64
                }
            }) {
                return Err(QuerySchemaError);
            }
            if bound.is_none_or(|value| {
                if lower {
                    value < edge as f64
                } else {
                    value > edge as f64
                }
            }) {
                object.insert(keyword.into(), edge.into());
            }
        }
    }
    for key in [
        "properties",
        "$defs",
        "definitions",
        "patternProperties",
        "dependentSchemas",
    ] {
        if let Some(children) = object.get_mut(key).and_then(Value::as_object_mut) {
            for child in children.values_mut() {
                project_parameter_schema(child)?;
            }
        }
    }
    for key in [
        "items",
        "additionalProperties",
        "contains",
        "not",
        "if",
        "then",
        "else",
        "propertyNames",
    ] {
        if let Some(child) = object.get_mut(key) {
            project_parameter_schema(child)?;
        }
    }
    for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(children) = object.get_mut(key).and_then(Value::as_array_mut) {
            for child in children {
                project_parameter_schema(child)?;
            }
        }
    }
    fn transport_safe(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Number(number) => number
                .as_f64()
                .is_some_and(|n| n.is_finite() && (n.fract() != 0.0 || n.abs() <= SAFE as f64)),
            serde_json::Value::Array(values) => values.iter().all(transport_safe),
            serde_json::Value::Object(values) => values.values().all(transport_safe),
            _ => true,
        }
    }
    if !transport_safe(schema) {
        return Err(QuerySchemaError);
    }
    Ok(())
}

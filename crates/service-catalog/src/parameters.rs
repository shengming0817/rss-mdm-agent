use crate::{bounded, CatalogError};
use execution_contract::{Id, InputValue, VersionedRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) const MAX_INTEGER: i64 = 9_007_199_254_740_991;

/// A field's display metadata and single source of validation semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parameter {
    /// Nonempty plain-text field label.
    pub title: String,
    /// Plain-text field help, never model instructions.
    pub description: String,
    /// Required fields cannot have defaults.
    pub required: bool,
    /// Closed first-version rules; adding a rule requires updating every projection/validator.
    pub rule: ParameterRule,
}
/// Supported value grammar. Secret references are the only permitted object-valued parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParameterRule {
    /// Unicode scalar length, with independent host UTF-8 byte limits.
    String {
        /// Inclusive minimum character count.
        min_length: u32,
        /// Inclusive maximum character count.
        max_length: u32,
        /// Optional nonempty unique allowed values, all within the length constraints.
        choices: Option<Vec<String>>,
        /// Valid default for an optional field only.
        default: Option<String>,
    },
    /// Mathematically integral JSON number within the JS safe integer range; no string conversion.
    Integer {
        /// Inclusive safe integer minimum.
        minimum: i64,
        /// Inclusive safe integer maximum.
        maximum: i64,
        /// Optional nonempty unique allowed safe integers.
        choices: Option<Vec<i64>>,
        /// Valid default for an optional field only.
        default: Option<i64>,
    },
    /// A JSON boolean, never a truthy string or number.
    Boolean {
        /// Valid default for an optional field only.
        default: Option<bool>,
    },
    /// Exact opaque secret reference; no literal/default/enum or material resolution.
    SecretReference {},
}
/// Shared host parameter budgets. Directory content cannot increase them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterLimits {
    /// Maximum compact JSON argument bytes, before and after defaults/normalization.
    pub max_bytes: usize,
    /// Maximum UTF-8 bytes of each parameter key, string or secret reference coordinate.
    pub max_string_bytes: usize,
    /// Maximum declared and supplied parameter count.
    pub max_parameters: usize,
}
impl ParameterLimits {
    fn validate(&self) -> Result<(), CatalogError> {
        if [self.max_bytes, self.max_string_bytes, self.max_parameters].contains(&0) {
            Err(CatalogError::InvalidLimits)
        } else {
            Ok(())
        }
    }
}
/// One projection for both renderers. The schema is structural; the shared runtime enforces budgets.
#[derive(Debug, Clone)]
pub struct ParameterProjection {
    fields: BTreeMap<Id, Parameter>,
    schema: Value,
    limits: ParameterLimits,
}
impl ParameterProjection {
    /// Single field declaration source for form widgets and safe labels/help.
    pub fn fields(&self) -> &BTreeMap<Id, Parameter> {
        &self.fields
    }
    /// Same Draft 2020-12 schema for human and AI arguments; consumers must not weaken it.
    pub fn input_schema(&self) -> &Value {
        &self.schema
    }
    /// Shared budget selection; JSON Schema does not express aggregate UTF-8 byte budgets.
    pub fn limits(&self) -> ParameterLimits {
        self.limits
    }
    /// Decode and normalize either human or AI arguments. Returns data, not execution permission.
    pub fn validate(&self, bytes: &[u8]) -> Result<BTreeMap<String, InputValue>, CatalogError> {
        let budget = bounded::CatalogLimits {
            max_bytes: self.limits.max_bytes,
            max_depth: 3,
            max_nodes: self
                .limits
                .max_parameters
                .saturating_mul(3)
                .saturating_add(1),
            max_string_bytes: self.limits.max_string_bytes,
            max_collection_items: self.limits.max_parameters.max(2),
        };
        normalize(&self.fields, bounded::decode(bytes, &budget)?, &self.limits)
    }
}
fn integer(value: &Value) -> Option<i64> {
    if let Some(i) = value.as_i64() {
        return (i.unsigned_abs() <= MAX_INTEGER as u64).then_some(i);
    }
    let f = value.as_f64()?;
    (f.is_finite() && f.fract() == 0.0 && f.abs() <= MAX_INTEGER as f64).then_some(f as i64)
}
impl ParameterRule {
    fn default_value(&self) -> Option<Value> {
        match self {
            Self::String { default, .. } => default.as_ref().map(|v| json!(v)),
            Self::Integer { default, .. } => default.map(|v| json!(v)),
            Self::Boolean { default } => default.map(|v| json!(v)),
            Self::SecretReference {} => None,
        }
    }
    fn accepts(&self, value: &Value) -> bool {
        match self {
            Self::String {
                min_length,
                max_length,
                choices,
                ..
            } => value.as_str().is_some_and(|s| {
                let len = s.chars().count();
                len >= *min_length as usize
                    && len <= *max_length as usize
                    && choices.as_ref().is_none_or(|v| v.iter().any(|x| x == s))
            }),
            Self::Integer {
                minimum,
                maximum,
                choices,
                ..
            } => integer(value).is_some_and(|v| {
                v >= *minimum && v <= *maximum && choices.as_ref().is_none_or(|xs| xs.contains(&v))
            }),
            Self::Boolean { .. } => value.is_boolean(),
            Self::SecretReference {} => {
                serde_json::from_value::<VersionedRef>(value.clone()).is_ok()
            }
        }
    }
}
pub(crate) fn validate_definitions(fields: &BTreeMap<Id, Parameter>) -> Result<(), CatalogError> {
    for field in fields.values() {
        if field.title.trim().is_empty() {
            return Err(CatalogError::InvalidDefinition);
        }
        let valid = match &field.rule {
            ParameterRule::String {
                min_length,
                max_length,
                choices,
                ..
            } => {
                min_length <= max_length
                    && choices.as_ref().is_none_or(|xs| {
                        !xs.is_empty()
                            && xs.iter().collect::<BTreeSet<_>>().len() == xs.len()
                            && xs.iter().all(|v| field.rule.accepts(&json!(v)))
                    })
            }
            ParameterRule::Integer {
                minimum,
                maximum,
                choices,
                ..
            } => {
                minimum <= maximum
                    && *minimum >= -MAX_INTEGER
                    && *maximum <= MAX_INTEGER
                    && choices.as_ref().is_none_or(|xs| {
                        !xs.is_empty()
                            && xs.iter().collect::<BTreeSet<_>>().len() == xs.len()
                            && xs.iter().all(|v| field.rule.accepts(&json!(v)))
                    })
            }
            _ => true,
        };
        if !valid
            || field
                .rule
                .default_value()
                .is_some_and(|v| field.required || !field.rule.accepts(&v))
        {
            return Err(CatalogError::InvalidDefinition);
        }
    }
    Ok(())
}
pub(crate) fn projection(
    fields: &BTreeMap<Id, Parameter>,
    limits: &ParameterLimits,
) -> Result<ParameterProjection, CatalogError> {
    limits.validate()?;
    if fields.len() > limits.max_parameters
        || fields
            .keys()
            .any(|k| k.as_str().len() > limits.max_string_bytes)
    {
        return Err(CatalogError::LimitExceeded);
    }
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (key, field) in fields {
        let mut schema = match &field.rule {
            ParameterRule::String {
                min_length,
                max_length,
                choices,
                ..
            } => {
                let mut v = json!({"type":"string","minLength":min_length,"maxLength":max_length});
                if let Some(xs) = choices {
                    v["enum"] = json!(xs);
                }
                v
            }
            ParameterRule::Integer {
                minimum,
                maximum,
                choices,
                ..
            } => {
                let mut v = json!({"type":"integer","minimum":minimum,"maximum":maximum});
                if let Some(xs) = choices {
                    v["enum"] = json!(xs);
                }
                v
            }
            ParameterRule::Boolean { .. } => json!({"type":"boolean"}),
            ParameterRule::SecretReference {} => serde_json::to_value(
                schemars::generate::SchemaSettings::draft2020_12()
                    .with(|settings| settings.inline_subschemas = true)
                    .into_generator()
                    .into_root_schema_for::<VersionedRef>(),
            )
            .map_err(|_| CatalogError::Encoding)?,
        };
        schema["title"] = json!(field.title);
        schema["description"] = json!(field.description);
        if let Some(v) = field.rule.default_value() {
            schema["default"] = v;
        }
        if matches!(field.rule, ParameterRule::SecretReference {}) {
            schema["writeOnly"] = json!(true);
        }
        if field.required {
            required.push(key.as_str());
        }
        properties.insert(key.as_str().to_owned(), schema);
    }
    Ok(ParameterProjection {
        fields: fields.clone(),
        schema: json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":properties,"required":required,"additionalProperties":false}),
        limits: *limits,
    })
}
pub(crate) fn normalize(
    fields: &BTreeMap<Id, Parameter>,
    value: Value,
    limits: &ParameterLimits,
) -> Result<BTreeMap<String, InputValue>, CatalogError> {
    limits.validate()?;
    if fields.len() > limits.max_parameters {
        return Err(CatalogError::LimitExceeded);
    }
    let mut args = value
        .as_object()
        .cloned()
        .ok_or(CatalogError::InvalidArguments)?;
    if args.len() > limits.max_parameters {
        return Err(CatalogError::LimitExceeded);
    }
    bounded::encode(&args, limits.max_bytes)?;
    for (key, value) in &args {
        if !fields.keys().any(|k| k.as_str() == key) {
            return Err(CatalogError::InvalidArguments);
        }
        check_strings(key, value, limits)?;
    }
    let mut result = BTreeMap::new();
    for (key, field) in fields {
        let value = args
            .get(key.as_str())
            .cloned()
            .or_else(|| field.rule.default_value());
        let Some(mut value) = value else {
            if field.required {
                return Err(CatalogError::InvalidArguments);
            }
            continue;
        };
        if !field.rule.accepts(&value) {
            return Err(CatalogError::InvalidArguments);
        }
        if matches!(field.rule, ParameterRule::Integer { .. }) {
            value = json!(integer(&value).ok_or(CatalogError::InvalidArguments)?);
        }
        check_strings(key.as_str(), &value, limits)?;
        args.insert(key.as_str().to_owned(), value.clone());
        let input = if matches!(field.rule, ParameterRule::SecretReference {}) {
            InputValue::Secret {
                reference: serde_json::from_value(value)
                    .map_err(|_| CatalogError::InvalidArguments)?,
            }
        } else {
            InputValue::Literal { value }
        };
        result.insert(key.as_str().to_owned(), input);
    }
    bounded::encode(&args, limits.max_bytes)?;
    Ok(result)
}
fn check_strings(key: &str, value: &Value, limits: &ParameterLimits) -> Result<(), CatalogError> {
    if key.len() > limits.max_string_bytes
        || value
            .as_str()
            .is_some_and(|s| s.len() > limits.max_string_bytes)
    {
        return Err(CatalogError::LimitExceeded);
    }
    if let Some(object) = value.as_object() {
        for (k, v) in object {
            if k.len() > limits.max_string_bytes
                || v.as_str()
                    .is_some_and(|s| s.len() > limits.max_string_bytes)
            {
                return Err(CatalogError::LimitExceeded);
            }
        }
    }
    Ok(())
}

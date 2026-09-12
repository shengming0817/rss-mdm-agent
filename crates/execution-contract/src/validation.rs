use crate::{ContractError, NetworkAccess, PlanSpec, RunAs, TargetScope};
use serde::{
    de::{MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Value;
use std::fmt;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
/// Caller-supplied bounds, independent from the untrusted plan. No unlimited/default policy.
#[derive(Debug, Clone, Copy)]
pub struct PlanLimits {
    pub max_input_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_string_bytes: usize,
    pub max_collection_items: usize,
    pub max_timeout_ms: u64,
    pub max_output_bytes: u64,
    pub max_attempts: u32,
}
impl PlanLimits {
    pub(crate) fn validate(&self) -> Result<(), ContractError> {
        if [
            self.max_input_bytes,
            self.max_depth,
            self.max_nodes,
            self.max_string_bytes,
            self.max_collection_items,
        ]
        .contains(&0)
            || self.max_depth > 64
            || self.max_timeout_ms == 0
            || self.max_output_bytes == 0
            || self.max_attempts == 0
        {
            return Err(ContractError::Limit);
        }
        Ok(())
    }
}

/// Check value size and number semantics before JCS can round unsafe integer values.
pub(crate) fn check_json(value: &Value, limits: &PlanLimits) -> Result<(), ContractError> {
    let mut remaining = limits.max_nodes;
    fn walk(
        v: &Value,
        l: &PlanLimits,
        depth: usize,
        left: &mut usize,
    ) -> Result<(), ContractError> {
        if depth > l.max_depth || *left == 0 {
            return Err(ContractError::Limit);
        }
        *left -= 1;
        match v {
            Value::String(s) if s.len() > l.max_string_bytes => return Err(ContractError::Limit),
            Value::Number(n) => {
                let safe = if let Some(n) = n.as_u64() {
                    n <= MAX_SAFE_INTEGER
                } else if let Some(n) = n.as_i64() {
                    n.unsigned_abs() <= MAX_SAFE_INTEGER
                } else {
                    n.as_f64().is_some_and(|n| {
                        n.is_finite() && (n.fract() != 0.0 || n.abs() <= MAX_SAFE_INTEGER as f64)
                    })
                };
                if !safe {
                    return Err(ContractError::Value);
                }
            }
            Value::Array(items) => {
                if items.len() > l.max_collection_items {
                    return Err(ContractError::Limit);
                }
                for item in items {
                    walk(item, l, depth + 1, left)?;
                }
            }
            Value::Object(items) => {
                if items.len() > l.max_collection_items {
                    return Err(ContractError::Limit);
                }
                for (key, item) in items {
                    if key.is_empty() || key.len() > l.max_string_bytes || key.contains('\0') {
                        return Err(ContractError::Value);
                    }
                    walk(item, l, depth + 1, left)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(value, limits, 1, &mut remaining)
}

// serde_json supplies the parser and its recursion guard. This visitor only closes its
// last-key-wins map behavior, including inside arbitrary parameter values.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("unique JSON data")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| UniqueValue(Value::Number(n)))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(UniqueValue(v)) = a.next_element()? {
                    out.push(v);
                }
                Ok(UniqueValue(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(key) = a.next_key::<String>()? {
                    if out.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate key"));
                    }
                    let UniqueValue(v) = a.next_value()?;
                    out.insert(key, v);
                }
                Ok(UniqueValue(Value::Object(out)))
            }
        }
        d.deserialize_any(UniqueVisitor)
    }
}
pub(crate) fn decode_value(bytes: &[u8], limits: &PlanLimits) -> Result<Value, ContractError> {
    limits.validate()?;
    if bytes.len() > limits.max_input_bytes {
        return Err(ContractError::Limit);
    }
    let UniqueValue(value) = serde_json::from_slice(bytes).map_err(|_| ContractError::Encoding)?;
    check_json(&value, limits)?;
    Ok(value)
}

pub(crate) fn validate_plan(p: &PlanSpec, l: &PlanLimits) -> Result<(), ContractError> {
    l.validate()?;
    if p.budget.timeout_ms == 0
        || p.budget.timeout_ms > l.max_timeout_ms
        || p.budget.max_output_bytes == 0
        || p.budget.max_output_bytes > l.max_output_bytes
        || p.budget.max_attempts == 0
        || p.budget.max_attempts > l.max_attempts
        || p.validity.not_before_unix_ms >= p.validity.expires_at_unix_ms
    {
        return Err(ContractError::Budget);
    }
    let platform = p.request.target.platform;
    let run_platform = match &p.run_as {
        RunAs::User { account } => account.platform,
        RunAs::System { platform } => *platform,
    };
    if run_platform != platform
        || matches!(&p.request.target.scope, TargetScope::User { account } if account.platform != platform)
    {
        return Err(ContractError::Context);
    }
    for path in std::iter::once(&p.launch.cwd)
        .chain(&p.constraints.read_paths)
        .chain(&p.constraints.write_paths)
    {
        let absolute = path.starts_with('/')
            || path.starts_with("\\\\")
            || (path.len() >= 3
                && path.as_bytes()[0].is_ascii_alphabetic()
                && path.as_bytes()[1] == b':'
                && b"/\\".contains(&path.as_bytes()[2]));
        if !absolute
            || path.contains('\0')
            || path
                .split(['/', '\\'])
                .any(|part| part == ".." || part == ".")
        {
            return Err(ContractError::Value);
        }
    }
    if p.launch.argv.iter().any(|a| a.contains('\0')) {
        return Err(ContractError::Value);
    }
    for key in p.launch.env.keys() {
        if key.is_empty()
            || key.as_bytes()[0].is_ascii_digit()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(ContractError::Value);
        }
    }
    if let NetworkAccess::Allowlist { destinations } = &p.constraints.network {
        if destinations.is_empty()
            || destinations
                .iter()
                .any(|s| s.is_empty() || s.contains(['\0', '*']))
        {
            return Err(ContractError::Value);
        }
    }
    Ok(())
}

pub(crate) fn unique_json<'de, D: Deserializer<'de>>(d: D) -> Result<Value, D::Error> {
    UniqueValue::deserialize(d).map(|v| v.0)
}
pub(crate) fn unique_map<'de, D, T>(d: D) -> Result<std::collections::BTreeMap<String, T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct MapVisitor<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for MapVisitor<T> {
        type Value = std::collections::BTreeMap<String, T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unique map")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut out = std::collections::BTreeMap::new();
            while let Some(key) = a.next_key::<String>()? {
                if out.contains_key(&key) {
                    return Err(serde::de::Error::custom("duplicate key"));
                }
                out.insert(key, a.next_value()?);
            }
            Ok(out)
        }
    }
    d.deserialize_map(MapVisitor(std::marker::PhantomData))
}

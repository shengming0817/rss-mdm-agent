use crate::{ArgumentRule, CatalogError, Limit};
use serde::{
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Serialize,
};
use serde_json::{Map, Value};
use std::{fmt, io::Write};

/// Host budgets, separate from directory data. All fields must be positive.
#[derive(Debug, Clone, Copy)]
pub struct CatalogLimits {
    /// Maximum input and canonical output bytes.
    pub max_bytes: usize,
    /// Maximum JSON value depth, root is one; at most 64.
    pub max_depth: usize,
    /// Maximum total JSON values, including containers.
    pub max_nodes: usize,
    /// Maximum UTF-8 bytes of any string or object key.
    pub max_string_bytes: usize,
    /// Maximum elements of each JSON object or array.
    pub max_collection_items: usize,
}
impl CatalogLimits {
    pub(crate) fn validate(&self) -> Result<(), CatalogError> {
        for (value, field) in [
            (self.max_bytes, Limit::Bytes),
            (self.max_depth, Limit::Depth),
            (self.max_nodes, Limit::Nodes),
            (self.max_string_bytes, Limit::StringBytes),
            (self.max_collection_items, Limit::CollectionItems),
        ] {
            if value == 0 {
                return Err(CatalogError::InvalidLimits(field));
            }
        }
        if self.max_depth > 64 {
            return Err(CatalogError::InvalidLimits(Limit::Depth));
        }
        Ok(())
    }
}
struct State<'a> {
    limits: &'a CatalogLimits,
    left: usize,
    error: Option<CatalogError>,
}
impl State<'_> {
    fn fail<E: serde::de::Error>(&mut self, e: CatalogError) -> E {
        self.error = Some(e);
        E::custom("catalog input rejected")
    }
    fn string<E: serde::de::Error>(&mut self, s: &str) -> Result<(), E> {
        if s.len() > self.limits.max_string_bytes {
            Err(self.fail(CatalogError::LimitExceeded(Limit::StringBytes)))
        } else {
            Ok(())
        }
    }
}
struct Seed<'a, 'b> {
    state: &'a mut State<'b>,
    depth: usize,
    admitted: bool,
}
impl<'de> DeserializeSeed<'de> for Seed<'_, '_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if !self.admitted {
            return Err(self
                .state
                .fail(CatalogError::LimitExceeded(Limit::CollectionItems)));
        }
        if self.depth > self.state.limits.max_depth {
            return Err(self.state.fail(CatalogError::LimitExceeded(Limit::Depth)));
        }
        if self.state.left == 0 {
            return Err(self.state.fail(CatalogError::LimitExceeded(Limit::Nodes)));
        }
        self.state.left -= 1;
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_, '_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("invalid number"))
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        self.state.string(v)?;
        Ok(v.into())
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> {
        self.state.string(&v)?;
        Ok(v.into())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut result = Vec::new();
        loop {
            let admitted = result.len() < self.state.limits.max_collection_items;
            let Some(value) = a.next_element_seed(Seed {
                state: self.state,
                depth: self.depth + 1,
                admitted,
            })?
            else {
                break;
            };
            result.push(value);
        }
        Ok(Value::Array(result))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut result = Map::new();
        while let Some(key) = a.next_key::<String>()? {
            self.state.string(&key)?;
            if result.contains_key(&key) {
                return Err(self.state.fail(CatalogError::DuplicateKey));
            }
            if result.len() == self.state.limits.max_collection_items {
                return Err(self
                    .state
                    .fail(CatalogError::LimitExceeded(Limit::CollectionItems)));
            }
            let value = a.next_value_seed(Seed {
                state: self.state,
                depth: self.depth + 1,
                admitted: true,
            })?;
            result.insert(key, value);
        }
        Ok(Value::Object(result))
    }
}
pub(crate) fn decode(bytes: &[u8], limits: &CatalogLimits) -> Result<Value, CatalogError> {
    limits.validate()?;
    if bytes.len() > limits.max_bytes {
        return Err(CatalogError::LimitExceeded(Limit::Bytes));
    }
    reject_rounded_integers(bytes)?;
    let mut state = State {
        limits,
        left: limits.max_nodes,
        error: None,
    };
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let result = Seed {
        state: &mut state,
        depth: 1,
        admitted: true,
    }
    .deserialize(&mut decoder);
    let value = result.map_err(|_| state.error.unwrap_or(CatalogError::Malformed))?;
    decoder.end().map_err(|_| CatalogError::Malformed)?;
    Ok(value)
}
struct BoundedWriter {
    bytes: Vec<u8>,
    max: usize,
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.max - self.bytes.len() {
            return Err(std::io::Error::other("catalog byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn encode(value: &impl Serialize, max: usize) -> Result<Vec<u8>, CatalogError> {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        max,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| CatalogError::LimitExceeded(Limit::Bytes))?;
    Ok(writer.bytes)
}

// JSON Schema integer means mathematically integral, not “rounded by f64 to an integer”.
// Check original number tokens before serde loses their decimal precision. String contents
// are skipped, and malformed JSON is still rejected by serde rather than repaired here.
fn reject_rounded_integers(bytes: &[u8]) -> Result<(), CatalogError> {
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
        } else if bytes[i] == b'-' || bytes[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || b".eE+-".contains(&bytes[i])) {
                i += 1;
            }
            let token =
                std::str::from_utf8(&bytes[start..i]).map_err(|_| CatalogError::Malformed)?;
            if let Ok(number) = token.parse::<f64>() {
                if number.is_finite() && number.fract() == 0.0 && !exact_integral(token) {
                    return Err(CatalogError::InvalidArguments(ArgumentRule::RoundedNumber));
                }
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}
fn exact_integral(token: &str) -> bool {
    let token = token.trim_start_matches('-');
    let (mantissa, exponent) = token.split_once(['e', 'E']).unwrap_or((token, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if digits.bytes().all(|c| c == b'0') {
        return true;
    }
    let Ok(exponent) = exponent.parse::<i64>() else {
        return false;
    };
    let fractional = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len()) as i64;
    let trailing = digits.len() - digits.trim_end_matches('0').len();
    exponent
        .saturating_sub(fractional)
        .saturating_add(trailing as i64)
        >= 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn array_limit_rejects_before_deserializing_the_extra_subtree() {
        let limits = CatalogLimits {
            max_bytes: 1024,
            max_depth: 8,
            max_nodes: 20,
            max_string_bytes: 30,
            max_collection_items: 1,
        };
        assert_eq!(
            decode(br#"[1,{"key":1,"key":2}]"#, &limits).unwrap_err(),
            CatalogError::LimitExceeded(Limit::CollectionItems)
        );
        assert_eq!(decode(b"[1]", &limits).unwrap(), serde_json::json!([1]));
    }
}

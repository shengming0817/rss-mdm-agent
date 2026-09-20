use crate::WireRecord;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fmt, sync::LazyLock};

/// Positive host-owned bounds over the whole envelope, including JSON member names.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum raw/encoded UTF-8 envelope bytes.
    pub max_bytes: usize,
    /// Maximum accumulated UTF-8 string/key bytes.
    pub max_text_bytes: usize,
    /// Maximum object/array depth, at most 64.
    pub max_depth: usize,
    /// Maximum JSON value nodes (including containers).
    pub max_nodes: usize,
}
/// Closed diagnostic code shared with TS. No input values are retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostic {
    /// Invalid host limits.
    Configuration,
    /// Invalid strict JSON, UTF-8 or Unicode.
    Encoding,
    /// Duplicate object key, including nested dynamic data.
    DuplicateKey,
    /// Unsupported contract version.
    Version,
    /// Invalid closed wire structure.
    Schema,
    /// Input budget exceeded.
    Limit,
    /// Nonfinite or unsafe JSON integer.
    Number,
    /// Inconsistent related fields.
    Context,
}
/// Value-free error, without a raw serde/provider error chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("AI contract: {code:?}")]
pub struct ContractError {
    /// Stable diagnostic code.
    pub code: Diagnostic,
}
fn error(code: Diagnostic) -> ContractError {
    ContractError { code }
}
fn serde_error<E: serde::de::Error>(code: Diagnostic) -> E {
    E::custom(format!(
        "ai-v4:{}",
        serde_json::to_string(&code).expect("closed diagnostic")
    ))
}
fn from_serde(e: serde_json::Error) -> ContractError {
    let message = e.to_string();
    // serde_json classifies parser-level float overflow as Syntax before visiting.
    // Match only its value-free diagnostic, preserving other syntax errors.
    if e.is_syntax() && message.starts_with("number out of range at line ") {
        return error(Diagnostic::Number);
    }
    let code = message
        .strip_prefix("ai-v4:")
        .and_then(|s| serde_json::from_str(s.split(" at line ").next().unwrap_or(s)).ok())
        .unwrap_or(Diagnostic::Encoding);
    error(code)
}
struct State {
    nodes: usize,
    text: usize,
}
struct Seed<'a> {
    limits: &'a Limits,
    state: &'a mut State,
    depth: usize,
}
impl Seed<'_> {
    fn text<E: serde::de::Error>(&mut self, s: &str) -> Result<(), E> {
        self.state.text = self
            .state
            .text
            .checked_add(s.len())
            .ok_or_else(|| serde_error(Diagnostic::Limit))?;
        if self.state.text > self.limits.max_text_bytes {
            return Err(serde_error(Diagnostic::Limit));
        }
        Ok(())
    }
    fn container<E: serde::de::Error>(&self) -> Result<(), E> {
        if self.depth + 1 > self.limits.max_depth {
            Err(serde_error(Diagnostic::Limit))
        } else {
            Ok(())
        }
    }
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        self.state.nodes += 1;
        if self.state.nodes > self.limits.max_nodes {
            return Err(serde_error(Diagnostic::Limit));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded unique JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        if v.unsigned_abs() > 9_007_199_254_740_991 {
            return Err(serde_error(Diagnostic::Number));
        }
        Ok(v.into())
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        if v > 9_007_199_254_740_991 {
            return Err(serde_error(Diagnostic::Number));
        }
        Ok(v.into())
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        if !v.is_finite() || (v.fract() == 0.0 && v.abs() > 9_007_199_254_740_991.0) {
            return Err(serde_error(Diagnostic::Number));
        }
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| serde_error(Diagnostic::Number))
    }
    fn visit_str<E: serde::de::Error>(mut self, v: &str) -> Result<Value, E> {
        self.text(v)?;
        Ok(v.into())
    }
    fn visit_string<E: serde::de::Error>(mut self, v: String) -> Result<Value, E> {
        self.text(&v)?;
        Ok(v.into())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        self.container()?;
        let mut out = Vec::new();
        while let Some(v) = a.next_element_seed(Seed {
            limits: self.limits,
            state: self.state,
            depth: self.depth + 1,
        })? {
            out.push(v);
        }
        Ok(out.into())
    }
    fn visit_map<A: MapAccess<'de>>(mut self, mut a: A) -> Result<Value, A::Error> {
        self.container()?;
        let mut out = serde_json::Map::new();
        while let Some(key) = a.next_key::<String>()? {
            if out.contains_key(&key) {
                return Err(serde_error(Diagnostic::DuplicateKey));
            }
            self.text(&key)?;
            let v = a.next_value_seed(Seed {
                limits: self.limits,
                state: self.state,
                depth: self.depth + 1,
            })?;
            out.insert(key, v);
        }
        Ok(out.into())
    }
}
static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let schema: Value =
        serde_json::from_str(include_str!("schema.json")).expect("generated schema JSON");
    jsonschema::validator_for(&schema).expect("generated schema validity")
});
fn check_limits(limits: &Limits) -> Result<(), ContractError> {
    if [
        limits.max_bytes,
        limits.max_text_bytes,
        limits.max_depth,
        limits.max_nodes,
    ]
    .contains(&0)
        || limits.max_depth > 64
    {
        return Err(error(Diagnostic::Configuration));
    }
    Ok(())
}
/// Decode strict bounded V3 JSON. Does not authenticate or dispatch anything.
pub fn decode(bytes: &[u8], limits: &Limits) -> Result<WireRecord, ContractError> {
    check_limits(limits)?;
    if bytes.len() > limits.max_bytes {
        return Err(error(Diagnostic::Limit));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let mut state = State { nodes: 0, text: 0 };
    let value = Seed {
        limits,
        state: &mut state,
        depth: 0,
    }
    .deserialize(&mut deserializer)
    .map_err(from_serde)?;
    deserializer.end().map_err(from_serde)?;
    if value.get("schemaVersion").is_some_and(|v| v != 4) {
        return Err(error(Diagnostic::Version));
    }
    if !VALIDATOR.is_valid(&value) {
        return Err(error(Diagnostic::Schema));
    }
    context(&value)?;
    serde_json::from_value(value).map_err(|_| error(Diagnostic::Schema))
}
fn context(v: &Value) -> Result<(), ContractError> {
    let bad = match v["kind"].as_str() {
        Some("event") if v["body"]["type"] == "command_accepted" => {
            context(&v["body"]["command"])?;
            v["commandId"] != v["body"]["command"]["commandId"]
                || v["namespace"]["sessionId"] != v["body"]["command"]["sessionId"]
        }
        Some("snapshotPage") => {
            let mut invalid = v["cursor"] != v["session"]["lastSequence"];
            for field in ["events", "commands", "interactions", "surfaces"] {
                for row in v[field].as_array().into_iter().flatten() {
                    context(row)?;
                    let namespace = if row["kind"] == "commandRecord" {
                        &row["receipt"]["namespace"]
                    } else {
                        &row["namespace"]
                    };
                    invalid |= namespace != &v["session"]["namespace"];
                    invalid |=
                        row["kind"] == "event" && row["sequence"].as_u64() > v["cursor"].as_u64();
                }
            }
            invalid
        }
        Some("event") if v["body"]["type"] == "surface" => {
            v["namespace"] != v["body"]["surface"]["namespace"]
                || (v["body"]["surface"]["status"] != "invalidated"
                    && v["generation"] != v["body"]["surface"]["generation"])
        }
        Some("accessUpdate") if v["update"]["type"] == "event" => {
            context(&v["update"]["event"])?;
            v["sessionId"] != v["update"]["event"]["namespace"]["sessionId"]
        }
        Some("command") => {
            v["input"]["type"] == "prompt"
                && ((v["input"]["policy"] == "steer") != v["input"].get("targetRunId").is_some())
        }
        Some("receipt") => {
            v["acceptedAtMs"].as_u64() > v["retryUntilMs"].as_u64()
                || v["retryUntilMs"].as_u64() > v["receiptUntilMs"].as_u64()
        }
        Some("commandRecord") => {
            context(&v["command"])?;
            context(&v["receipt"])?;
            v["command"]["commandId"] != v["receipt"]["commandId"]
                || v["command"]["sessionId"] != v["receipt"]["namespace"]["sessionId"]
                || hash(&v["command"])? != v["receipt"]["contentHash"]
                || ((v["state"] == "terminal") != v.get("outcome").is_some())
                || (["dispatching", "running", "reconciliation_required"]
                    .contains(&v["state"].as_str().unwrap_or(""))
                    && v.get("dispatch").is_none())
        }
        Some("interaction") => (v["status"] == "answered") != v.get("responseCommandId").is_some(),
        _ => false,
    };
    if bad {
        Err(error(Diagnostic::Context))
    } else {
        Ok(())
    }
}
fn hash(v: &Value) -> Result<String, ContractError> {
    let bytes = serde_json_canonicalizer::to_vec(v).map_err(|_| error(Diagnostic::Encoding))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn bounded_bytes<T: Serialize>(value: &T, limits: &Limits) -> Result<Vec<u8>, ContractError> {
    check_limits(limits)?;
    struct Bounded {
        bytes: Vec<u8>,
        limit: usize,
        exceeded: bool,
    }
    impl std::io::Write for Bounded {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if b.len() > self.limit.saturating_sub(self.bytes.len()) {
                self.exceeded = true;
                return Err(std::io::Error::other("contract byte limit"));
            }
            self.bytes.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Bounded {
        bytes: Vec::new(),
        limit: limits.max_bytes,
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        error(if writer.exceeded {
            Diagnostic::Limit
        } else {
            Diagnostic::Encoding
        })
    })?;
    Ok(writer.bytes)
}
/// Encode and validate constructed data without allocating past the envelope budget.
pub fn encode(record: &WireRecord, limits: &Limits) -> Result<Vec<u8>, ContractError> {
    let bytes = bounded_bytes(record, limits)?;
    decode(&bytes, limits)?;
    Ok(bytes)
}
/// Validate and compute JCS/SHA-256; trusted namespace remains a separate storage key.
pub fn fingerprint(command: &crate::Command, limits: &Limits) -> Result<String, ContractError> {
    let bytes = bounded_bytes(command, limits)?;
    decode(&bytes, limits)?;
    hash(&serde_json::from_slice(&bytes).map_err(|_| error(Diagnostic::Encoding))?)
}

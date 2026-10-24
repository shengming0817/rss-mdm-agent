//! Pure native decoding and query encoding; no provider or asset-field policy.
use rss_mdm_agent_wire::runtime as w;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
pub(super) type Row = BTreeMap<String, Value>;
pub(super) type Status = w::NativeQueryStatus;

#[cfg(any(windows, test))]
pub(super) fn select(definition: &w::NativeQueryDefinition) -> Result<String, Status> {
    definition.validate().map_err(|_| Status::Invalid)?;
    let w::NativeQueryDefinition::WindowsWmi {
        class,
        properties,
        filter,
        ..
    } = definition
    else {
        return Err(Status::Invalid);
    };
    let mut text = format!(
        "SELECT {} FROM {class}",
        properties.keys().cloned().collect::<Vec<_>>().join(",")
    );
    if let Some(f) = filter {
        text.push_str(" WHERE ");
        text.push_str(&predicate(f, properties)?);
    }
    Ok(text)
}
#[cfg(any(windows, test))]
fn predicate(
    f: &w::WmiFilter,
    properties: &BTreeMap<String, w::WmiProperty>,
) -> Result<String, Status> {
    use w::{NativeComparison as C, WmiFilter as F};
    Ok(match f {
        F::Compare {
            property,
            operator,
            value,
        } => {
            let p = properties.get(property).ok_or(Status::Invalid)?;
            let literal = match p.cim_type {
                w::CimType::String | w::CimType::Datetime => format!(
                    "'{}'",
                    value
                        .as_str()
                        .ok_or(Status::Invalid)?
                        .replace('\\', "\\\\")
                        .replace('\'', "\\'")
                ),
                w::CimType::Uint64 => value.as_str().ok_or(Status::Invalid)?.to_owned(),
                w::CimType::Boolean => if value.as_bool().ok_or(Status::Invalid)? {
                    "TRUE"
                } else {
                    "FALSE"
                }
                .into(),
                _ => value.to_string(),
            };
            let op = match operator {
                C::Eq => "=",
                C::Ne => "!=",
                C::Lt => "<",
                C::Le => "<=",
                C::Gt => ">",
                C::Ge => ">=",
            };
            format!("({property} {op} {literal})")
        }
        F::And { terms } | F::Or { terms } => {
            let join = if matches!(f, F::And { .. }) {
                " AND "
            } else {
                " OR "
            };
            format!(
                "({})",
                terms
                    .iter()
                    .map(|t| predicate(t, properties))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(join)
            )
        }
    })
}
#[cfg(any(windows, test))]
pub(super) fn cim_time(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 25 || b[14] != b'.' || !matches!(b[21], b'+' | b'-' | b':') {
        return false;
    }
    if !b[..14]
        .iter()
        .chain(&b[15..21])
        .chain(&b[22..])
        .all(|b| b.is_ascii_digit() || *b == b'*')
    {
        return false;
    }
    if b[21] == b':' {
        return b[22..] == *b"000" && !value.contains('*');
    }
    for (a, z, min, max) in [
        (4, 6, 1, 12),
        (6, 8, 1, 31),
        (8, 10, 0, 23),
        (10, 12, 0, 59),
        (12, 14, 0, 59),
    ] {
        let v = &value[a..z];
        if v.contains('*') {
            if !v.bytes().all(|b| b == b'*') {
                return false;
            }
        } else if !v.parse::<u32>().is_ok_and(|n| (min..=max).contains(&n)) {
            return false;
        }
    }
    true // Preserve CIM text; unspecified offsets never acquire an invented UTC value.
}
#[cfg(any(windows, test))]
pub(super) fn registry(kind: w::RegistryType, actual: u32, bytes: &[u8]) -> Result<Value, Status> {
    use w::RegistryType::*;
    let expected = match kind {
        Sz => 1,
        ExpandSz => 2,
        Dword => 4,
        MultiSz => 7,
        Qword => 11,
    };
    if actual != expected {
        return Err(Status::Invalid);
    }
    let value = match kind {
        Dword => Value::from(u32::from_le_bytes(
            bytes.try_into().map_err(|_| Status::Invalid)?,
        )),
        Qword => Value::String(
            u64::from_le_bytes(bytes.try_into().map_err(|_| Status::Invalid)?).to_string(),
        ),
        Sz | ExpandSz | MultiSz => {
            if !bytes.len().is_multiple_of(2) {
                return Err(Status::Invalid);
            }
            let words: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            if kind == MultiSz {
                if !words.ends_with(&[0, 0]) {
                    return Err(Status::Invalid);
                }
                let text =
                    String::from_utf16(&words[..words.len() - 2]).map_err(|_| Status::Invalid)?;
                if text.is_empty() {
                    Value::Array(vec![])
                } else {
                    if text.split('\0').any(str::is_empty) {
                        return Err(Status::Invalid);
                    }
                    Value::Array(text.split('\0').map(|s| Value::String(s.into())).collect())
                }
            } else {
                let text = String::from_utf16(words.strip_suffix(&[0]).unwrap_or(&words))
                    .map_err(|_| Status::Invalid)?;
                if text.contains('\0') {
                    return Err(Status::Invalid);
                }
                Value::String(text)
            }
        }
    };
    Ok(value)
}
// Software metadata may use either native string type; values stay unexpanded.
#[cfg(any(windows, test))]
pub(super) fn software_string(actual: u32, bytes: &[u8]) -> Result<String, Status> {
    let kind = match actual {
        1 => w::RegistryType::Sz,
        2 => w::RegistryType::ExpandSz,
        _ => return Err(Status::Invalid),
    };
    registry(kind, actual, bytes)?
        .as_str()
        .map(str::to_owned)
        .ok_or(Status::Invalid)
}

#[cfg(any(windows, test))]
pub(super) fn package_installed(state: i32) -> Result<bool, Status> {
    match state {
        2 => Ok(true),
        0 | 1 | 6 => Ok(false),
        _ => Err(Status::Invalid),
    }
}

/// Retains bounded evidence, and never infers natural completion from the row limit.
pub(super) struct Rows<'a> {
    pub query: &'a w::NativeQuery,
    pub result: w::NativeQueryResult,
    keys: BTreeSet<String>,
    bytes: usize,
}
impl<'a> Rows<'a> {
    pub fn new(query: &'a w::NativeQuery, time: i64) -> Self {
        Self {
            query,
            result: super::failure(query, Status::Complete, time),
            keys: Default::default(),
            bytes: 0,
        }
    }
    pub fn push(&mut self, row: Row) -> Result<(), Status> {
        if self.result.rows.len() >= usize::from(self.query.budget.max_rows) {
            return Err(Status::Truncated);
        }
        if self.query.cardinality == w::NativeCardinality::ExactlyOne
            && !self.result.rows.is_empty()
        {
            return Err(Status::Invalid);
        }
        let schema = self.query.definition.properties();
        if !self.query.definition.accepts_row(&row)
            || row
                .iter()
                .any(|(k, v)| schema.get(k).is_none_or(|t| !t.accepts(v)))
        {
            return Err(Status::Invalid);
        }
        let key = self
            .query
            .instance_keys
            .iter()
            .map(|k| row.get(k).filter(|v| v.as_str() != Some("")))
            .collect::<Option<Vec<_>>>()
            .ok_or(Status::Invalid)?;
        if !key.is_empty()
            && !self
                .keys
                .insert(serde_json::to_string(&key).map_err(|_| Status::Invalid)?)
        {
            return Err(Status::Invalid);
        }
        let size = serde_json::to_vec(&row).map_err(|_| Status::Invalid)?.len();
        self.bytes = self.bytes.checked_add(size).ok_or(Status::Truncated)?;
        if self.bytes > self.query.budget.output_bytes as usize {
            return Err(Status::Truncated);
        }
        self.result.rows.push(row);
        self.result.row_count = self.result.rows.len() as u32;
        Ok(())
    }
    pub fn finish(mut self, status: Result<(), Status>) -> w::NativeQueryResult {
        self.result.status = status.err().unwrap_or(Status::Complete);
        self.result.enumeration_complete = self.result.status == Status::Complete;
        if self.result.enumeration_complete
            && self.query.cardinality == w::NativeCardinality::ExactlyOne
            && self.result.rows.is_empty()
        {
            self.result.status = Status::Missing;
        }
        if matches!(
            self.query.definition,
            w::NativeQueryDefinition::System { .. }
        ) && self.result.status == Status::Complete
            && !self
                .result
                .complete_for(self.query, self.result.collected_at)
        {
            self.result.status = Status::Partial;
            self.result.enumeration_complete = false;
        }
        if serde_json::to_vec(&self.result)
            .map_or(true, |b| b.len() > self.query.budget.output_bytes as usize)
        {
            self.result = super::truncated(self.query, &self.result);
        }
        self.result
    }
}

#[cfg(any(windows, test))]
pub(super) enum CimValue {
    Text(String),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(i16),
}
#[cfg(any(windows, test))]
pub(super) fn cim_value(c: w::CimType, raw: CimValue) -> Result<Value, Status> {
    use w::CimType as C;
    use CimValue as V;
    let value = match (c, raw) {
        (C::String | C::Datetime, V::Text(s)) => s.into(),
        (C::Boolean, V::Bool(0)) => false.into(),
        (C::Boolean, V::Bool(-1)) => true.into(),
        (C::Sint8 | C::Sint16, V::I16(v)) => i64::from(v).into(),
        (C::Sint32 | C::Uint16, V::I32(v)) => i64::from(v).into(),
        (C::Uint8, V::U8(v)) => u64::from(v).into(),
        (C::Uint32, V::I32(v)) => u64::from(v as u32).into(),
        (C::Uint32, V::U32(v)) => u64::from(v).into(),
        (C::Sint64, V::Text(s)) => s.parse::<i64>().map_err(|_| Status::Invalid)?.into(),
        (C::Sint64, V::I64(v)) => v.into(),
        (C::Uint64, V::Text(s)) => s
            .parse::<u64>()
            .map_err(|_| Status::Invalid)?
            .to_string()
            .into(),
        (C::Uint64, V::U64(v)) => v.to_string().into(),
        (C::Real32, V::F32(v)) => serde_json::Number::from_f64(f64::from(v))
            .ok_or(Status::Invalid)?
            .into(),
        (C::Real64, V::F64(v)) => serde_json::Number::from_f64(v)
            .ok_or(Status::Invalid)?
            .into(),
        _ => return Err(Status::Invalid),
    };
    if !c.accepts(&value) || c == C::Datetime && !cim_time(value.as_str().ok_or(Status::Invalid)?) {
        return Err(Status::Invalid);
    }
    Ok(value)
}

/// Address enrichment is bounded by the completely enumerated adapter set.
/// IP-only interfaces (notably loopback) are not missing adapter evidence.
#[cfg(any(windows, test))]
pub(super) fn adapter_addresses(
    adapters: &[Row],
    addresses: Vec<Row>,
) -> Result<BTreeMap<u64, BTreeSet<String>>, Status> {
    let index = |r: &Row| {
        r.get("InterfaceIndex")
            .and_then(Value::as_u64)
            .ok_or(Status::Missing)
    };
    let mut result = BTreeMap::new();
    for adapter in adapters {
        if result.insert(index(adapter)?, BTreeSet::new()).is_some() {
            return Err(Status::Invalid);
        }
    }
    for row in addresses {
        let Some(values) = result.get_mut(&index(&row)?) else {
            continue;
        };
        let ip = row
            .get("IPAddress")
            .and_then(Value::as_str)
            .ok_or(Status::Missing)?
            .to_owned();
        // Windows supplies scoped IPv6 strings (for example fe80::1%4). The
        // wire declares strings; IpAddr parsing would reject valid native data.
        values.insert(ip);
        if values.len() > 64 {
            return Err(Status::Truncated);
        }
    }
    Ok(result)
}

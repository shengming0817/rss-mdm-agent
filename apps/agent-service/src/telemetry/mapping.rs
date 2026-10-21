//! Exact frozen-field projections. Producer JSON never becomes an undeclared series.
use agent_client::Error;
use execution_contract::{OutputQuality, ProcessEvidence};
use rss_mdm_agent_wire::runtime as w;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

pub(super) fn readings(
    c: &w::TelemetryConfiguration,
    e: &ProcessEvidence,
    now: i64,
) -> Result<BTreeMap<w::FieldKey, w::TelemetryReading>, Error> {
    if !e.finished || !e.quiescent || e.exit_code != Some(0) || e.quality != OutputQuality::Complete
    {
        return Err(Error::Unavailable);
    }
    if e.total_output_bytes > u64::from(c.output_bytes) {
        return Err(Error::Capacity);
    }
    let output: Json = serde_json::from_slice(&e.stdout).map_err(|_| Error::Protocol)?;
    let mut values = BTreeMap::new();
    match &c.collector {
        w::TelemetryCollector::Script {
            output_schema,
            mappings,
            max_rows,
            ..
        } => {
            if output
                .as_array()
                .is_some_and(|v| v.len() > *max_rows as usize)
            {
                return Err(Error::Capacity);
            }
            admit_schema(output_schema)?;
            if serde_json::to_vec(output_schema)
                .map_err(|_| Error::Protocol)?
                .len()
                .saturating_mul(e.stdout.len())
                > 16 * 1024 * 1024
            {
                return Err(Error::Capacity);
            }
            let validator =
                jsonschema::validator_for(output_schema).map_err(|_| Error::Protocol)?;
            if !validator.is_valid(&output) {
                return Err(Error::Protocol);
            }
            check_keys(c, mappings.keys())?;
            for field in &c.fields {
                let mapping = mappings.get(field.key.as_str()).ok_or(Error::Protocol)?;
                values.insert(
                    field.key,
                    reading(field, project(&output, mapping, 0)?, now)?,
                );
            }
        }
        w::TelemetryCollector::Native { plan, mappings } => {
            let native: w::NativeCollectionResult =
                serde_json::from_value(output).map_err(|_| Error::Protocol)?;
            native.validate_for(plan, now / 1_000_000)?;
            let completion: execution_contract::CollectionCompletion =
                serde_json::from_slice(&e.stderr).map_err(|_| Error::Protocol)?;
            use sha2::{Digest, Sha256};
            if !completion.closed
                || completion.output_sha256 != format!("{:x}", Sha256::digest(&e.stdout))
            {
                return Err(Error::Protocol);
            }
            check_keys(c, mappings.keys())?;
            for field in &c.fields {
                let mapping = mappings.get(field.key.as_str()).ok_or(Error::Protocol)?;
                let query = plan
                    .spec()
                    .queries
                    .get(&mapping.query)
                    .ok_or(Error::Protocol)?;
                mapping.validate_counter_normalization(&query.definition, field)?;
                let result = native.queries.get(&mapping.query).ok_or(Error::Protocol)?;
                let valid = result.complete_for(query, now / 1_000_000);
                let value = if valid {
                    let input = serde_json::to_value(&result.rows).map_err(|_| Error::Protocol)?;
                    let mut projected = project(&input, &mapping.mapping, 0)?;
                    if mapping.counter_normalization.is_some() {
                        // The signed wire contract reserves this property for the Agent.
                        // Spool atomically replaces this candidate with the durable baseline start.
                        let start = field
                            .time_series
                            .as_ref()
                            .and_then(|b| b.interval_start_path.as_ref())
                            .and_then(|p| p.first())
                            .ok_or(Error::Protocol)?;
                        for item in projected.as_array_mut().ok_or(Error::Protocol)? {
                            let fields = item.as_object_mut().ok_or(Error::Protocol)?;
                            if fields.insert(start.clone(), Json::from(now)).is_some() {
                                return Err(Error::Protocol);
                            }
                        }
                    }
                    reading(field, projected, now)?
                } else if result.status == w::NativeQueryStatus::Missing {
                    w::TelemetryReading::Missing {
                        interval_start: None,
                        context: None,
                    }
                } else {
                    w::TelemetryReading::Failed {
                        interval_start: None,
                        context: None,
                    }
                };
                values.insert(field.key, value);
            }
        }
    }
    validate_budget(c, &values, now)?;
    Ok(values)
}
fn check_keys<'a>(
    c: &w::TelemetryConfiguration,
    keys: impl Iterator<Item = &'a String>,
) -> Result<(), Error> {
    let keys = keys.collect::<Vec<_>>();
    if keys.len() != c.fields.len()
        || keys
            .iter()
            .any(|k| !c.fields.iter().any(|f| f.key.as_str() == k.as_str()))
    {
        return Err(Error::Protocol);
    }
    Ok(())
}
fn reading(f: &w::FieldDefinition, value: Json, _now: i64) -> Result<w::TelemetryReading, Error> {
    if value.is_null() {
        return Ok(w::TelemetryReading::Missing {
            interval_start: None,
            context: None,
        });
    }
    let value = scalar(&f.value_type, &value, 0)?;
    f.validate_scalar(&value)?;
    Ok(w::TelemetryReading::Valid {
        value,
        interval_start: None,
    })
}
fn scalar(t: &w::ValueType, v: &Json, depth: usize) -> Result<w::Scalar, Error> {
    if depth > 6 {
        return Err(Error::Capacity);
    }
    use w::ValueType as T;
    Ok(match t {
        T::String{..}|T::Enum{..}=>w::Scalar::String(v.as_str().ok_or(Error::Protocol)?.into()),
        T::Integer=>w::Scalar::Integer(v.as_i64().ok_or(Error::Protocol)?),
        T::Number=>serde_json::from_value(json!({"kind":"number","value":v.as_f64().filter(|v|v.is_finite()).ok_or(Error::Protocol)?})).map_err(|_|Error::Protocol)?,
        T::Time=>w::Scalar::Time(v.as_i64().ok_or(Error::Protocol)?),
        T::Boolean=>w::Scalar::Boolean(v.as_bool().ok_or(Error::Protocol)?),
        T::Array{items,max_items}=>{let a=v.as_array().filter(|a|a.len()<=*max_items as usize).ok_or(Error::Capacity)?;w::Scalar::Array(a.iter().map(|v|scalar(items,v,depth+1)).collect::<Result<_,_>>()?)},
        T::Object{properties}=>{let a=v.as_object().filter(|a|a.len()==properties.len()).ok_or(Error::Protocol)?;w::Scalar::Object(properties.iter().map(|(k,t)|Ok((k.clone(),scalar(t,a.get(k).ok_or(Error::Protocol)?,depth+1)?))).collect::<Result<_,Error>>()?)},
        T::Record{properties}=>{let a=v.as_object().ok_or(Error::Protocol)?;if a.keys().any(|k|!properties.contains_key(k)){return Err(Error::Protocol);}let mut result=BTreeMap::new();for(k,p)in properties{match a.get(k){Some(v)=>{result.insert(k.clone(),scalar(&p.value_type,v,depth+1)?);},None if p.required=>return Err(Error::Protocol),None=>()}}w::Scalar::Object(result)},
    })
}
fn token(v: &Json) -> Result<String, Error> {
    match v {
        Json::String(v) => Ok(v.clone()),
        Json::Number(_) | Json::Bool(_) => Ok(v.to_string()),
        _ => Err(Error::Protocol),
    }
}
fn integer(v: &Json) -> Result<i64, Error> {
    v.as_i64()
        .or_else(|| v.as_str()?.parse().ok())
        .ok_or(Error::Protocol)
}
fn unsigned(v: &Json) -> Result<u64, Error> {
    v.as_u64()
        .or_else(|| v.as_str()?.parse().ok())
        .ok_or(Error::Protocol)
}
fn project(input: &Json, m: &w::TelemetryValueMapping, depth: usize) -> Result<Json, Error> {
    if depth > 6 || m.pointer.len() > 1024 || m.columns.len() > 64 {
        return Err(Error::Capacity);
    }
    let value = input.pointer(&m.pointer).ok_or(Error::Protocol)?;
    if !m.columns.is_empty() {
        if !matches!(m.conversion, w::TelemetryConversion::Identity) {
            return Err(Error::Protocol);
        }
        let rows = value
            .as_array()
            .filter(|v| v.len() <= 4096)
            .ok_or(Error::Capacity)?;
        return Ok(Json::Array(
            rows.iter()
                .map(|row| {
                    m.columns
                        .iter()
                        .map(|(k, v)| Ok((k.clone(), project(row, v, depth + 1)?)))
                        .collect::<Result<serde_json::Map<_, _>, Error>>()
                        .map(Json::Object)
                })
                .collect::<Result<_, _>>()?,
        ));
    }
    use w::TelemetryConversion as C;
    Ok(match &m.conversion {
        C::Identity => value.clone(),
        C::Integer => integer(value)?.into(),
        C::Number => {
            let n = value
                .as_f64()
                .or_else(|| value.as_str()?.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .ok_or(Error::Protocol)?;
            json!(n)
        }
        C::Enum { values } => values.get(&token(value)?).ok_or(Error::Protocol)?.clone(),
        C::ArrayMember {
            member,
            present,
            absent,
        } => {
            let values = value
                .as_array()
                .ok_or(Error::Protocol)?
                .iter()
                .map(integer)
                .collect::<Result<Vec<_>, _>>()?;
            if values.contains(member) {
                present
            } else {
                absent
            }
            .clone()
        }
        C::Scale {
            numerator,
            denominator,
        } => {
            let n = u128::from(unsigned(value)?)
                .checked_mul(u128::from(*numerator))
                .ok_or(Error::Protocol)?;
            if *denominator == 0 || n % u128::from(*denominator) != 0 {
                return Err(Error::Protocol);
            }
            let n = u64::try_from(n / u128::from(*denominator)).map_err(|_| Error::Protocol)?;
            json!(n)
        }
        C::Bitmask { flags } => {
            let mut n = unsigned(value)?;
            let mut result = Vec::new();
            for (k, v) in flags {
                let bit = k.parse::<u64>().map_err(|_| Error::Protocol)?;
                if bit.count_ones() != 1 {
                    return Err(Error::Protocol);
                }
                if n & bit != 0 {
                    result.push(v.clone());
                    n &= !bit;
                }
            }
            if n != 0 {
                return Err(Error::Protocol);
            }
            result.sort();
            json!(result)
        }
        C::CimTime => cim_time(value.as_str().ok_or(Error::Protocol)?)?.into(),
    })
}
fn cim_time(v: &str) -> Result<i64, Error> {
    if v.len() != 25 || !v.is_ascii() || &v[14..15] != "." || !matches!(&v[21..22], "+" | "-") {
        return Err(Error::Protocol);
    }
    let n = |a, b| v[a..b].parse::<i32>().map_err(|_| Error::Protocol);
    let date = time::Date::from_calendar_date(
        n(0, 4)?,
        time::Month::try_from(n(4, 6)? as u8).map_err(|_| Error::Protocol)?,
        n(6, 8)? as u8,
    )
    .map_err(|_| Error::Protocol)?;
    let clock = time::Time::from_hms_micro(
        n(8, 10)? as u8,
        n(10, 12)? as u8,
        n(12, 14)? as u8,
        n(15, 21)? as u32,
    )
    .map_err(|_| Error::Protocol)?;
    let offset = time::UtcOffset::from_whole_seconds(
        n(22, 25)? * 60 * if &v[21..22] == "-" { -1 } else { 1 },
    )
    .map_err(|_| Error::Protocol)?;
    Ok(time::PrimitiveDateTime::new(date, clock)
        .assume_offset(offset)
        .unix_timestamp())
}
fn path<'a>(value: &'a w::Scalar, path: &[String]) -> Option<&'a w::Scalar> {
    let mut v = value;
    for key in path {
        let w::Scalar::Object(fields) = v else {
            return None;
        };
        v = fields.get(key)?;
    }
    Some(v)
}
pub(super) fn validate_budget(
    c: &w::TelemetryConfiguration,
    values: &BTreeMap<w::FieldKey, w::TelemetryReading>,
    now: i64,
) -> Result<(), Error> {
    let mut points = 0usize;
    for f in &c.fields {
        let Some(binding) = &f.time_series else {
            continue;
        };
        let Some(w::TelemetryReading::Valid { value, .. }) = values.get(&f.key) else {
            continue;
        };
        let items = match value {
            w::Scalar::Array(a) => a.iter().collect::<Vec<_>>(),
            v => vec![v],
        };
        if items.len() > binding.max_resources as usize || items.len() > c.max_resources as usize {
            return Err(Error::Capacity);
        }
        points = points.checked_add(items.len()).ok_or(Error::Capacity)?;
        for item in items {
            let v = path(item, &binding.value_path).ok_or(Error::Protocol)?;
            match (binding.kind, v) {
                (w::TelemetryKind::Gauge, w::Scalar::Integer(_) | w::Scalar::Number(_)) => (),
                (
                    w::TelemetryKind::CumulativeCounter | w::TelemetryKind::DeltaCounter,
                    w::Scalar::Integer(n),
                ) if *n >= 0 => (),
                (
                    w::TelemetryKind::CumulativeCounter | w::TelemetryKind::DeltaCounter,
                    w::Scalar::Number(n),
                ) if n.into_inner() >= 0.0 => (),
                (w::TelemetryKind::State, w::Scalar::Boolean(_) | w::Scalar::String(_)) => (),
                _ => return Err(Error::Protocol),
            }
            for dimension in binding.dimensions.values() {
                let v = path(item, &dimension.path).ok_or(Error::Protocol)?;
                let token = match v {
                    w::Scalar::String(s) => s.clone(),
                    w::Scalar::Integer(n) => n.to_string(),
                    _ => return Err(Error::Protocol),
                };
                if !dimension.values.contains(&token) {
                    return Err(Error::Protocol);
                }
            }
            if let Some(start) = &binding.interval_start_path {
                let Some(w::Scalar::Integer(start)) = path(item, start) else {
                    return Err(Error::Protocol);
                };
                if *start < 0 || *start > now {
                    return Err(Error::Protocol);
                }
            }
        }
    }
    if points > c.max_points as usize || points > c.max_resources as usize {
        return Err(Error::Capacity);
    }
    Ok(())
}

/// Restrict remote validation to bounded, non-recursive structural schemas.
pub(super) fn admit_schema(schema: &Json) -> Result<(), Error> {
    fn walk(v: &Json, depth: usize, nodes: &mut usize, literal: bool) -> Result<(), Error> {
        *nodes += 1;
        if depth > 12 || *nodes > 256 {
            return Err(Error::Capacity);
        }
        match v {
            Json::Object(map) => {
                for (key, value) in map {
                    if literal {
                        walk(value, depth + 1, nodes, true)?;
                        continue;
                    }
                    match key.as_str() {
                        "properties" => {
                            let fields = value.as_object().ok_or(Error::Unsupported)?;
                            for schema in fields.values() {
                                walk(schema, depth + 1, nodes, false)?;
                            }
                        }
                        "items" => walk(value, depth + 1, nodes, false)?,
                        "type" | "required" | "enum" | "const" | "minimum" | "maximum"
                        | "exclusiveMinimum" | "exclusiveMaximum" | "minLength" | "maxLength"
                        | "minItems" | "maxItems" | "minProperties" | "maxProperties" | "title"
                        | "description" | "$schema" => walk(value, depth + 1, nodes, true)?,
                        "additionalProperties" if value.is_boolean() => (),
                        _ => return Err(Error::Unsupported),
                    }
                }
            }
            Json::Array(items) => {
                if !literal {
                    return Err(Error::Unsupported);
                }
                for item in items {
                    walk(item, depth + 1, nodes, true)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    walk(schema, 0, &mut 0, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mapping(conversion: w::TelemetryConversion) -> w::TelemetryValueMapping {
        w::TelemetryValueMapping {
            pointer: "/n".into(),
            conversion,
            columns: Default::default(),
        }
    }
    #[test]
    fn projection_rejects_overflow_unknown_bits_and_lossy_scaling() {
        assert!(project(
            &json!({"n":"18446744073709551615"}),
            &mapping(w::TelemetryConversion::Integer),
            0
        )
        .is_err());
        assert!(project(
            &json!({"n":3}),
            &mapping(w::TelemetryConversion::Scale {
                numerator: 1,
                denominator: 2
            }),
            0
        )
        .is_err());
        assert!(project(
            &json!({"n":8}),
            &mapping(w::TelemetryConversion::Bitmask {
                flags: [("1".into(), "one".into())].into()
            }),
            0
        )
        .is_err());
        assert_eq!(
            project(
                &json!({"n":"1048576"}),
                &mapping(w::TelemetryConversion::Scale {
                    numerator: 1,
                    denominator: 1024
                }),
                0
            )
            .unwrap(),
            json!(1024)
        );
        assert!(scalar(&w::ValueType::Integer, &json!(1.5), 0).is_err());
    }
    #[test]
    fn schema_admission_rejects_recursive_and_exponential_work() {
        assert!(admit_schema(&json!({"$ref":"#"})).is_err());
        assert!(admit_schema(&json!({"allOf":[{"type":"integer"},{"type":"integer"}]})).is_err());
        assert!(admit_schema(&json!({"type":"string","pattern":"(a+)+$"})).is_err());
        assert!(admit_schema(&json!({"type":"array","items":{"type":"object","properties":{"load":{"type":"integer"}},"required":["load"],"additionalProperties":false},"maxItems":16})).is_ok());
    }
    #[test]
    fn cim_time_requires_a_complete_calendar_and_timezone() {
        assert_eq!(cim_time("19700101010000.000000+060").unwrap(), 0);
        assert!(cim_time("20260230010000.000000+000").is_err());
        assert!(cim_time("2026**********.******+***").is_err());
    }
}

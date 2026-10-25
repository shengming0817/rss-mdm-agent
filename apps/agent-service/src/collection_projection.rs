//! Shared output adaptation; selection and conversions belong to agent-wire.
//! ref: serde-rs/json src/value/mod.rs@v1.0.145 (RFC 6901 selection).
use agent_client::Error;
use rss_mdm_agent_wire::runtime as w;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

pub(crate) fn scalar(t: &w::ValueType, v: &Json, depth: usize) -> Result<w::Scalar, Error> {
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
/// Convert the validated wire value back to a JSON view, preserving its declared types.
pub(crate) fn json_scalar(value: &w::Scalar) -> Result<Json, Error> {
    Ok(match value {
        w::Scalar::String(v) => json!(v),
        w::Scalar::Integer(v) | w::Scalar::Time(v) => json!(v),
        w::Scalar::Boolean(v) => json!(v),
        w::Scalar::Number(v) => json!(v.into_inner()),
        w::Scalar::Array(v) => Json::Array(v.iter().map(json_scalar).collect::<Result<_, _>>()?),
        w::Scalar::Object(v) => Json::Object(
            v.iter()
                .map(|(k, v)| Ok((k.clone(), json_scalar(v)?)))
                .collect::<Result<_, Error>>()?,
        ),
    })
}
pub(crate) fn project(
    input: &Json,
    mapping: &w::ValueMapping,
    ty: &w::ValueType,
) -> Result<Json, Error> {
    mapping.validate_type(ty)?;
    let value = input.pointer(&mapping.pointer).ok_or(Error::Protocol)?;
    if mapping.columns.is_empty() {
        return mapping.project_selected(value, ty).map_err(Into::into);
    }
    if let w::ValueType::Array { items, max_items } = ty {
        let rows = value
            .as_array()
            .filter(|v| v.len() <= *max_items as usize)
            .ok_or(Error::Capacity)?;
        return rows
            .iter()
            .map(|row| mapping.project_selected(row, items).map_err(Into::into))
            .collect::<Result<Vec<_>, Error>>()
            .map(Json::Array);
    }
    mapping.project_selected(value, ty).map_err(Into::into)
}

/// Validate one converted value without defining another conversion language.
pub(crate) fn field(f: &w::FieldDefinition, value: Json) -> Result<Json, Error> {
    if value.is_null() {
        if !f.nullable {
            return Err(Error::Protocol);
        }
    } else {
        f.validate_scalar(&scalar(&f.value_type, &value, 0)?)?;
    }
    Ok(value)
}

/// Per-field observation; a failed or incomplete query never becomes a deletion.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FieldResult {
    pub value: Option<Json>,
    pub quality: execution_contract::DataQuality,
    pub collected_at: i64,
    pub deleted: bool,
}

pub(crate) fn native(
    fields: &[w::FieldDefinition],
    mappings: &BTreeMap<String, w::AgentCollectionMapping>,
    plan: &w::NativeCollectionPlan,
    result: &w::NativeCollectionResult,
    now: i64,
) -> Result<BTreeMap<String, FieldResult>, Error> {
    use execution_contract::DataQuality as Q;
    let mut values = BTreeMap::new();
    for definition in fields {
        let binding = mappings
            .get(definition.key.as_str())
            .ok_or(Error::Protocol)?;
        binding.validate_field(plan, definition, false)?;
        let query = plan
            .spec()
            .queries
            .get(&binding.query)
            .ok_or(Error::Protocol)?;
        let Some(observation) = result.queries.get(&binding.query) else {
            values.insert(
                definition.key.as_str().to_owned(),
                FieldResult {
                    value: None,
                    quality: Q::Unknown,
                    collected_at: result.collected_at,
                    deleted: false,
                },
            );
            continue;
        };
        let mut value = FieldResult {
            value: None,
            quality: status(observation.status),
            collected_at: observation.collected_at,
            deleted: false,
        };
        let complete = observation.complete_for(query, now);
        if complete
            || (matches!(
                observation.status,
                w::NativeQueryStatus::Partial | w::NativeQueryStatus::Truncated
            ) && !observation.rows.is_empty())
        {
            let input = if query.cardinality == w::NativeCardinality::List {
                serde_json::to_value(&observation.rows)
            } else {
                serde_json::to_value(observation.rows.first().ok_or(Error::Protocol)?)
            }
            .map_err(|_| Error::Protocol)?;
            match project(&input, &binding.mapping, &definition.value_type)
                .and_then(|v| field(definition, v))
            {
                Ok(v) => {
                    value.value = Some(v);
                    value.quality = if complete {
                        Q::Complete
                    } else {
                        status(observation.status)
                    };
                }
                Err(_) => value.quality = Q::Failed,
            }
        } else if observation.status == w::NativeQueryStatus::Missing {
            match binding.missing {
                w::NativeMissing::Missing => value.quality = Q::Missing,
                w::NativeMissing::Null => {
                    value.quality = Q::Complete;
                    value.value = Some(Json::Null);
                }
                w::NativeMissing::Delete => {
                    value.quality = Q::Complete;
                    value.deleted = true;
                }
            }
        } else if observation.status == w::NativeQueryStatus::Complete {
            value.quality = Q::Failed;
        }
        values.insert(definition.key.as_str().to_owned(), value);
    }
    Ok(values)
}

pub(crate) fn status(status: w::NativeQueryStatus) -> execution_contract::DataQuality {
    use execution_contract::DataQuality as Q;
    match status {
        w::NativeQueryStatus::Complete => Q::Complete,
        w::NativeQueryStatus::Missing => Q::Missing,
        w::NativeQueryStatus::Truncated => Q::Truncated,
        w::NativeQueryStatus::Denied => Q::Denied,
        w::NativeQueryStatus::Unsupported => Q::Unsupported,
        w::NativeQueryStatus::Partial | w::NativeQueryStatus::SourceUnavailable => Q::Partial,
        _ => Q::Failed,
    }
}

pub(crate) fn quality(values: &BTreeMap<String, FieldResult>) -> execution_contract::DataQuality {
    use execution_contract::DataQuality as Q;
    if values.is_empty() {
        return Q::NotCollected;
    }
    let first = &values.values().next().expect("nonempty").quality;
    if values.values().all(|v| &v.quality == first) {
        return first.clone();
    }
    Q::Partial
}

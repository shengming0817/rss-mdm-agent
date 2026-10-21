//! Durable native instance baselines; writes share append's sample transaction.
//! ref: rusqlite 0.32.1 src/transaction.rs Transaction::{commit,finish_}, Drop.
use super::*;
#[derive(Serialize, Deserialize)]
struct NativeCounter {
    counter: Counter,
    context: w::Scalar,
}

pub(super) fn install(db: &Connection) -> Result<(), Error> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS native_counters(field TEXT NOT NULL,item TEXT NOT NULL,body BLOB NOT NULL,PRIMARY KEY(field,item));")?;
    Ok(())
}
fn field_key(
    source: &str,
    field: &w::FieldDefinition,
    mapping: &w::TelemetryNativeMapping,
    plan: &w::NativeCollectionPlan,
) -> Result<String, Error> {
    Ok(format!(
        "{source}:{}:{}",
        field.key.as_str(),
        hash(&encode(&(field, mapping, plan))?)
    ))
}
pub(super) fn reclaim(
    db: &Connection,
    configurations: &[w::SignedTelemetryConfiguration],
) -> Result<(), Error> {
    let mut keep = std::collections::BTreeSet::new();
    for signed in configurations {
        let c = &signed.configuration;
        if let w::TelemetryCollector::Native { mappings, plan } = &c.collector {
            for field in &c.fields {
                if let Some(mapping) = mappings
                    .get(field.key.as_str())
                    .filter(|m| m.counter_normalization.is_some())
                {
                    keep.insert(field_key(&source_key(c)?, field, mapping, plan)?);
                }
            }
        }
    }
    let keys = db
        .prepare("SELECT DISTINCT field FROM native_counters")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for key in keys {
        if !keep.contains(&key) {
            db.execute("DELETE FROM native_counters WHERE field=?1", [key])?;
        }
    }
    Ok(())
}
pub(super) fn normalize(
    db: &Connection,
    c: &w::TelemetryConfiguration,
    source: &str,
    boot: &str,
    at: i64,
    values: &mut BTreeMap<w::FieldKey, w::TelemetryReading>,
) -> Result<(), Error> {
    let w::TelemetryCollector::Native { mappings, plan } = &c.collector else {
        return Ok(());
    };
    for field in &c.fields {
        let Some(mapping) = mappings.get(field.key.as_str()).filter(|m| {
            m.counter_normalization == Some(w::NativeCounterNormalization::LocalCumulativeV1)
        }) else {
            continue;
        };
        let key = field_key(source, field, mapping, plan)?;
        if let Some(reading) = values.get_mut(&field.key) {
            normalize_field(db, &key, field, boot, at, reading)?;
        }
    }
    Ok(())
}

fn normalize_field(
    db: &Connection,
    key: &str,
    field: &w::FieldDefinition,
    boot: &str,
    at: i64,
    reading: &mut w::TelemetryReading,
) -> Result<(), Error> {
    let old = db
        .prepare("SELECT item,body FROM native_counters WHERE field=?1")?
        .query_map([&key], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    // Replacing the membership set breaks continuity after missing samples or removal.
    db.execute("DELETE FROM native_counters WHERE field=?1", [&key])?;
    let binding = field.time_series.as_ref().ok_or(Error::Protocol)?;
    let start_key = binding
        .interval_start_path
        .as_ref()
        .and_then(|p| p.first())
        .ok_or(Error::Protocol)?;
    if let w::TelemetryReading::Missing { context, .. }
    | w::TelemetryReading::Failed { context, .. } = reading
    {
        // Carry the last known identities as quality-only context. This commits
        // invalidation with the failed observation, without inventing measurements.
        if context.is_none() {
            let items = old
                .values()
                .map(|bytes| {
                    let mut state: NativeCounter = decode(bytes)?;
                    if let w::Scalar::Object(p) = &mut state.context {
                        p.insert(
                            start_key.clone(),
                            w::Scalar::Integer(state.counter.start.min(at)),
                        );
                    }
                    Ok(state.context)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            *context = Some(w::Scalar::Array(items));
        }
        return Ok(());
    }
    let w::TelemetryReading::Valid {
        value: w::Scalar::Array(items),
        ..
    } = reading
    else {
        return Err(Error::Protocol);
    };
    let value_key = binding.value_path.first().ok_or(Error::Protocol)?;
    let item_key = field.item_key.as_ref().ok_or(Error::Protocol)?;
    let mut warmup = false;
    let mut identities = std::collections::BTreeSet::new();
    for item in items.iter_mut() {
        let w::Scalar::Object(properties) = item else {
            return Err(Error::Protocol);
        };
        let identity = hash(&encode(properties.get(item_key).ok_or(Error::Protocol)?)?);
        if !identities.insert(identity.clone()) {
            return Err(Error::Protocol);
        }
        let Some(w::Scalar::Integer(raw)) = properties.get(value_key) else {
            return Err(Error::Protocol);
        };
        let raw = *raw;
        if raw < 0 {
            return Err(Error::Protocol);
        }
        let previous = old
            .get(&identity)
            .map(|b| decode::<NativeCounter>(b).map(|s| s.counter))
            .transpose()?;
        let (counter, reset) = advance(raw, previous, boot, at)?;
        warmup |= reset;
        let w::Scalar::Integer(first) = &counter.first else {
            return Err(Error::Storage);
        };
        properties.insert(
            value_key.clone(),
            w::Scalar::Integer(raw.checked_sub(*first).ok_or(Error::Protocol)?),
        );
        properties.insert(start_key.clone(), w::Scalar::Integer(counter.start));
        let count: u64 = db.query_row(
            "SELECT (SELECT count(*) FROM counters)+(SELECT count(*) FROM native_counters)",
            [],
            |r| r.get(0),
        )?;
        if count >= 4096 {
            return Err(Error::Capacity);
        }
        let body = encode(&NativeCounter {
            counter,
            context: w::Scalar::Object(properties.clone()),
        })?;
        let bytes: usize = db.query_row(
            "SELECT coalesce(sum(length(body)),0) FROM native_counters",
            [],
            |r| r.get(0),
        )?;
        if bytes.saturating_add(body.len()) > 4 * 1024 * 1024 {
            return Err(Error::Capacity);
        }
        db.execute(
            "INSERT INTO native_counters VALUES(?1,?2,?3)",
            params![key, identity, body],
        )?;
    }
    if warmup {
        *reading = w::TelemetryReading::Missing {
            interval_start: None,
            context: Some(w::Scalar::Array(items.clone())),
        };
    }
    Ok(())
}

fn advance(
    raw: i64,
    previous: Option<Counter>,
    boot: &str,
    at: i64,
) -> Result<(Counter, bool), Error> {
    let previous = previous.filter(|p| {
        p.boot == boot && p.at < at && matches!(p.last, w::Scalar::Integer(last) if last <= raw)
    });
    let reset = previous.is_none();
    let (first, start) = if let Some(previous) = previous {
        let w::Scalar::Integer(first) = previous.first else {
            return Err(Error::Storage);
        };
        (first, previous.start)
    } else {
        (raw, at)
    };
    Ok((
        Counter {
            boot: boot.into(),
            first: w::Scalar::Integer(first),
            last: w::Scalar::Integer(raw),
            start,
            at,
        },
        reset,
    ))
}

#[cfg(test)]
#[path = "native_counter_tests.rs"]
mod tests;

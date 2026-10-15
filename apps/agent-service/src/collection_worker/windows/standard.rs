//! Fixed native aggregations only; asset mappings remain server-owned.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use w::CimType;
pub(super) fn query(
    namespace: &str,
    class: &str,
    properties: &[(&str, CimType)],
    budget: &Budget,
) -> Result<Vec<Row>, Status> {
    let def = w::NativeQueryDefinition::WindowsWmi {
        namespace: namespace.into(),
        class: class.into(),
        properties: properties
            .iter()
            .map(|(k, t)| {
                (
                    k.to_string(),
                    w::WmiProperty {
                        cim_type: *t,
                        array: false,
                    },
                )
            })
            .collect(),
        filter: None,
    };
    let mut rows = Vec::new();
    let mut bytes = 0usize;
    wmi::read(&def, budget, |row| {
        bytes = bytes
            .checked_add(serde_json::to_vec(&row).map_err(|_| Status::Invalid)?.len())
            .ok_or(Status::Truncated)?;
        if rows.len() >= 65535 || bytes > budget.output {
            return Err(Status::Truncated);
        }
        rows.push(row);
        Ok(())
    })?;
    Ok(rows)
}
pub(super) fn text<'a>(r: &'a Row, k: &str) -> Result<&'a str, Status> {
    r.get(k)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(Status::Missing)
}
fn integer(r: &Row, k: &str) -> Result<u64, Status> {
    r.get(k).and_then(|v| v.as_u64()).ok_or(Status::Missing)
}
pub(super) fn item_id(parts: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let text = parts
        .iter()
        .map(|s| format!("{}:{s}", s.len()))
        .collect::<Vec<_>>()
        .join("|");
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
pub(super) fn read(
    def: &w::NativeQueryDefinition,
    budget: &Budget,
    mut emit: impl FnMut(Row) -> Result<(), Status>,
) -> Result<(), Status> {
    use CimType::*;
    match def {
        w::NativeQueryDefinition::WindowsCpu => {
            let rows = query(
                "root\\cimv2",
                "Win32_Processor",
                &[
                    ("Name", String),
                    ("NumberOfCores", Uint32),
                    ("NumberOfLogicalProcessors", Uint32),
                ],
                budget,
            )?;
            if rows.is_empty() {
                return Err(Status::Missing);
            }
            let mut physical = 0u64;
            let mut logical = 0u64;
            let mut names = BTreeSet::new();
            for row in rows {
                let p = integer(&row, "NumberOfCores")?;
                let l = integer(&row, "NumberOfLogicalProcessors")?;
                if p == 0 || l == 0 {
                    return Err(Status::Invalid);
                }
                physical = physical.checked_add(p).ok_or(Status::Invalid)?;
                logical = logical.checked_add(l).ok_or(Status::Invalid)?;
                names.insert(text(&row, "Name")?.to_owned());
            }
            let mut row = Row::from([
                ("physical_cores".into(), physical.into()),
                ("logical_cores".into(), logical.into()),
            ]);
            if names.len() == 1 {
                row.insert(
                    "model".into(),
                    names.into_iter().next().ok_or(Status::Invalid)?.into(),
                );
            }
            emit(row)
        }
        w::NativeQueryDefinition::WindowsNetwork => {
            let adapters = query(
                "root\\StandardCimv2",
                "MSFT_NetAdapter",
                &[
                    ("InterfaceGuid", String),
                    ("Name", String),
                    ("InterfaceIndex", Uint32),
                ],
                budget,
            )?;
            let addresses = query(
                "root\\StandardCimv2",
                "MSFT_NetIPAddress",
                &[("InterfaceIndex", Uint32), ("IPAddress", String)],
                budget,
            )?;
            let mut by_index = super::super::values::adapter_addresses(&adapters, addresses)?;
            let mut seen = BTreeSet::new();
            for row in adapters {
                budget.check()?;
                let guid = uuid::Uuid::parse_str(text(&row, "InterfaceGuid")?)
                    .map_err(|_| Status::Invalid)?;
                if guid.is_nil() || !seen.insert(guid) {
                    return Err(Status::Invalid);
                }
                let addresses = by_index
                    .remove(&integer(&row, "InterfaceIndex")?)
                    .unwrap_or_default();
                emit(Row::from([
                    ("id".into(), item_id(&["network", &guid.to_string()]).into()),
                    ("name".into(), text(&row, "Name")?.into()),
                    (
                        "addresses".into(),
                        serde_json::to_value(addresses).map_err(|_| Status::Invalid)?,
                    ),
                ]))?;
            }
            // The enumerated set is MSFT_NetAdapter. MSFT_NetIPAddress also contains
            // IP-only interfaces (including loopback); those are outside this adapter
            // inventory. Both provider enumerations must still finish successfully.
            // ref: MicrosoftDocs/azure-docs virtual-network/how-to-virtual-machine-mtu.md
            Ok(())
        }
        w::NativeQueryDefinition::WindowsBattery => {
            let status = query(
                "root\\wmi",
                "BatteryStatus",
                &[("InstanceName", String), ("RemainingCapacity", Uint32)],
                budget,
            )?;
            if status.is_empty() {
                return Err(Status::Missing);
            }
            let full = query(
                "root\\wmi",
                "BatteryFullChargedCapacity",
                &[("InstanceName", String), ("FullChargedCapacity", Uint32)],
                budget,
            )?;
            let static_data = query(
                "root\\wmi",
                "BatteryStaticData",
                &[("InstanceName", String), ("Capabilities", Uint32)],
                budget,
            )?;
            let map = |rows: Vec<Row>,
                       field: &str|
             -> Result<BTreeMap<std::string::String, u64>, Status> {
                let mut map = BTreeMap::new();
                for r in rows {
                    if map
                        .insert(text(&r, "InstanceName")?.into(), integer(&r, field)?)
                        .is_some()
                    {
                        return Err(Status::Invalid);
                    }
                }
                Ok(map)
            };
            let remaining = map(status, "RemainingCapacity")?;
            let full = map(full, "FullChargedCapacity")?;
            let capabilities = map(static_data, "Capabilities")?;
            if remaining.keys().ne(full.keys()) || remaining.keys().ne(capabilities.keys()) {
                return Err(Status::Invalid);
            }
            let mut numerator = 0u64;
            let mut denominator = 0u64;
            for (id, value) in remaining {
                let capacity = full[&id];
                // Relative capacities from different packs cannot be energy-weighted.
                if capabilities[&id] & 0x40000000 != 0
                    || capacity == 0
                    || capacity == u32::MAX as u64
                    || value == u32::MAX as u64
                    || value > capacity
                {
                    return Err(Status::Invalid);
                }
                numerator = numerator.checked_add(value).ok_or(Status::Invalid)?;
                denominator = denominator.checked_add(capacity).ok_or(Status::Invalid)?;
            }
            let charge = 100.0 * numerator as f64 / denominator as f64;
            emit(Row::from([(
                "charge_percent".into(),
                serde_json::Value::from(charge),
            )]))
        }
        _ => Err(Status::Unsupported),
    }
}

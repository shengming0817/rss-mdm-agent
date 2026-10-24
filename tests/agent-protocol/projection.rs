//! Minimal explicit output contract for the existing native execution fixtures.
use rss_mdm_agent_wire::runtime::*;
use std::collections::BTreeMap;

pub fn native(
    plan: &NativeCollectionPlan,
) -> (
    Vec<FieldDefinition>,
    BTreeMap<String, AgentCollectionMapping>,
) {
    let (query_name, query) = plan.spec().queries.iter().next().unwrap();
    let (property, ty) = query
        .definition
        .properties()
        .into_iter()
        .find(|(_, t)| !matches!(t, NativeType::Array { .. }))
        .unwrap();
    let (value_type, conversion) = match ty {
        NativeType::String => (
            ValueType::String {
                max_length: 65536,
                allow_empty: true,
            },
            Conversion::Identity,
        ),
        NativeType::CimTime => (ValueType::Time, Conversion::CimTime),
        NativeType::Boolean => (ValueType::Boolean, Conversion::Identity),
        NativeType::Integer => (ValueType::Integer, Conversion::Identity),
        NativeType::UInt64 => (ValueType::Integer, Conversion::Integer),
        NativeType::Number => (ValueType::Number, Conversion::Identity),
        NativeType::Array { .. } => unreachable!(),
    };
    let scalar = ValueMapping {
        pointer: format!("/{property}"),
        conversion,
        columns: BTreeMap::new(),
    };
    let (value_type, mapping) = if query.cardinality == NativeCardinality::List {
        (
            ValueType::Array {
                items: Box::new(ValueType::Object {
                    properties: [("value".into(), value_type)].into(),
                }),
                max_items: u32::from(query.budget.max_rows),
            },
            ValueMapping {
                pointer: String::new(),
                conversion: Conversion::Identity,
                columns: [("value".into(), scalar)].into(),
            },
        )
    } else {
        (value_type, scalar)
    };
    let field = FieldDefinition {
        key: FieldKey::parse("custom.native").unwrap(),
        version: 1,
        value_type,
        nullable: false,
        manual: false,
        sources: [(Source::AgentNative, 0)].into(),
        platforms: [if plan.platform() == TaskPlatform::Windows {
            Platform::Windows
        } else {
            Platform::Macos
        }]
        .into(),
        sensitivity: Sensitivity::Standard,
        unit: None,
        searchable: false,
        item_key: None,
        time_series: None,
    };
    let mappings = [(
        "custom.native".into(),
        AgentCollectionMapping {
            query: query_name.clone(),
            mapping,
            missing: NativeMissing::Missing,
            counter_normalization: None,
        },
    )]
    .into();
    validate_native_projection(std::slice::from_ref(&field), &mappings, plan, false).unwrap();
    (vec![field], mappings)
}

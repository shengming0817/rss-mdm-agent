use super::*;
use serde_json::json;
use values::*;
fn query() -> w::NativeQuery {
    w::NativeQuery {
        version: 1,
        definition: w::NativeQueryDefinition::WindowsWmi {
            namespace: "root\\cimv2".into(),
            class: "Vendor_Inventory".into(),
            properties: [
                (
                    "Id".into(),
                    w::WmiProperty {
                        cim_type: w::CimType::String,
                        array: false,
                    },
                ),
                (
                    "Count".into(),
                    w::WmiProperty {
                        cim_type: w::CimType::Uint64,
                        array: false,
                    },
                ),
            ]
            .into(),
            filter: None,
        },
        cardinality: w::NativeCardinality::List,
        instance_keys: vec!["Id".into()],
        budget: w::NativeBudget {
            timeout_seconds: 10,
            output_bytes: 4096,
            max_rows: 2,
        },
    }
}
fn plan() -> w::NativeCollectionPlan {
    let q = query();
    w::NativeCollectionPlan::new(w::NativePlanSpec {
        version: 1,
        scope: rss_mdm_agent_wire::CollectionScope::Device,
        queries: [("a".into(), q.clone()), ("b".into(), q.clone())].into(),
        budget: w::NativeBudget {
            max_rows: 4,
            ..q.budget
        },
    })
    .unwrap()
}
#[test]
fn conditions_escape_literals_without_turning_them_into_wql() {
    let mut q = query();
    if let w::NativeQueryDefinition::WindowsWmi { filter, .. } = &mut q.definition {
        *filter = Some(w::WmiFilter::Compare {
            property: "Id".into(),
            operator: w::NativeComparison::Eq,
            value: json!("x' OR Id='y\\z"),
        });
    }
    assert_eq!(
        select(&q.definition).unwrap(),
        "SELECT Count,Id FROM Vendor_Inventory WHERE (Id = 'x\\' OR Id=\\'y\\\\z')"
    );
    if let w::NativeQueryDefinition::WindowsWmi { class, .. } = &mut q.definition {
        *class = "Win32_Product".into();
    }
    assert!(select(&q.definition).is_err());
    if let w::NativeQueryDefinition::WindowsWmi { class, .. } = &mut q.definition {
        *class = "X; SELECT *".into();
    }
    assert!(select(&q.definition).is_err());
}
#[test]
fn cim_conversion_preserves_width_carrier_and_unsigned_precision() {
    use w::CimType as C;
    use CimValue as V;
    assert_eq!(
        cim_value(C::Uint64, V::Text(u64::MAX.to_string())).unwrap(),
        json!(u64::MAX.to_string())
    );
    assert_eq!(cim_value(C::Uint32, V::I32(-1)).unwrap(), json!(u32::MAX));
    assert!(cim_value(C::Uint16, V::I32(-1)).is_err());
    assert!(cim_value(C::Sint8, V::I16(128)).is_err());
    assert!(cim_value(C::Uint64, V::Text("18446744073709551616".into())).is_err());
    assert!(cim_value(C::Uint64, V::F64(1.0)).is_err());
    assert!(cim_value(C::Real32, V::F32(f32::INFINITY)).is_err());
    assert!(cim_value(C::Boolean, V::Bool(1)).is_err());
    for (c, v) in [
        (C::Sint64, V::I64(i64::MIN)),
        (C::Uint8, V::U8(255)),
        (C::Uint32, V::U32(12)),
        (C::Uint64, V::U64(u64::MAX)),
        (C::Real64, V::F64(1.5)),
    ] {
        assert!(cim_value(c, v).is_ok());
    }
    let time = "20261015123456.000000+***";
    assert_eq!(
        cim_value(C::Datetime, V::Text(time.into())).unwrap(),
        json!(time)
    );
    assert!(!cim_time("20261315123456.000000+000"));
}
#[test]
fn registry_values_are_typed_exact_and_not_expanded() {
    let utf16 = |s: &str| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        registry(w::RegistryType::ExpandSz, 2, &utf16("%SECRET%\\app\0")).unwrap(),
        json!("%SECRET%\\app")
    );
    assert_eq!(
        registry(w::RegistryType::Qword, 11, &u64::MAX.to_le_bytes()).unwrap(),
        json!(u64::MAX.to_string())
    );
    assert_eq!(
        registry(w::RegistryType::MultiSz, 7, &utf16("one\0two\0\0")).unwrap(),
        json!(["one", "two"])
    );
    assert_eq!(
        registry(w::RegistryType::MultiSz, 7, &utf16("\0\0")).unwrap(),
        json!([])
    );
    assert!(registry(w::RegistryType::Dword, 11, &[0; 8]).is_err());
    assert!(registry(w::RegistryType::Dword, 4, &[0; 3]).is_err());
    assert!(registry(w::RegistryType::Sz, 1, &utf16("unterminated")).is_err());
    assert!(registry(w::RegistryType::Sz, 1, &[0, 0xd8, 0, 0]).is_err());
}
#[test]
fn list_end_keys_and_cardinality_are_independent() {
    let mut q = query();
    let time = now().unwrap();
    assert!(Rows::new(&q, time).finish(Ok(())).complete_for(&q, time));
    let mut rows = Rows::new(&q, time);
    rows.push([("Id".into(), json!("one"))].into()).unwrap();
    assert_eq!(
        rows.push([("Id".into(), json!("one"))].into()),
        Err(Status::Invalid)
    );
    assert!(!rows.finish(Err(Status::Invalid)).complete_for(&q, time));
    let mut rows = Rows::new(&q, time);
    assert_eq!(
        rows.push([("Count".into(), json!("1"))].into()),
        Err(Status::Invalid)
    );
    rows.push([("Id".into(), json!("b"))].into()).unwrap();
    rows.push([("Id".into(), json!("a"))].into()).unwrap();
    assert!(rows.finish(Ok(())).complete_for(&q, time));
    q.cardinality = w::NativeCardinality::ExactlyOne;
    q.instance_keys.clear();
    assert_eq!(Rows::new(&q, time).finish(Ok(())).status, Status::Missing);
    let mut rows = Rows::new(&q, time);
    rows.push(Row::new()).unwrap();
    assert_eq!(rows.push(Row::new()), Err(Status::Invalid));
}
#[test]
fn truncation_does_not_publish_first_n_as_a_complete_snapshot() {
    let mut q = query();
    q.budget.max_rows = 1;
    let time = now().unwrap();
    let mut rows = Rows::new(&q, time);
    rows.push([("Id".into(), json!("a"))].into()).unwrap();
    let end = rows.push([("Id".into(), json!("b"))].into());
    assert_eq!(end, Err(Status::Truncated));
    let result = rows.finish(end);
    assert_eq!(result.rows.len(), 1);
    assert!(!result.complete_for(&q, time));
}
#[test]
fn task_reuses_identical_queries_and_keeps_independent_failures() {
    let p = plan();
    let mut calls = 0;
    let result = collect(&p, now().unwrap() + 30, |_, q, _| {
        calls += 1;
        Ok(Rows::new(q, now().unwrap()).finish(Ok(())))
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result.queries.len(), 2);
    let mut spec = p.spec().clone();
    spec.queries.get_mut("b").unwrap().budget.max_rows = 1;
    let p = w::NativeCollectionPlan::new(spec).unwrap();
    let result = collect(&p, now().unwrap() + 30, |id, q, _| {
        Ok(if id == "a" {
            failure(q, Status::Denied, now().unwrap())
        } else {
            Rows::new(q, now().unwrap()).finish(Ok(()))
        })
    })
    .unwrap();
    assert_eq!(result.queries["a"].status, Status::Denied);
    assert!(result.queries["b"].complete_for(&p.spec().queries["b"], now().unwrap()));
}
#[test]
fn insufficient_envelope_budget_is_rejected_before_execution() {
    let mut spec = plan().spec().clone();
    spec.budget.output_bytes = 1;
    for q in spec.queries.values_mut() {
        q.budget.output_bytes = 1;
    }
    let p = w::NativeCollectionPlan::new(spec).unwrap();
    assert!(preflight(&p).is_err());
}

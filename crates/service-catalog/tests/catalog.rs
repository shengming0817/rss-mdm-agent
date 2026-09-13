use execution_contract::{Id, InputValue};
use serde_json::{json, Value};
use service_catalog::*;

fn limits() -> CatalogLimits {
    CatalogLimits {
        max_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
    }
}
fn parameter_limits() -> ParameterLimits {
    ParameterLimits {
        max_bytes: 4096,
        max_string_bytes: 1024,
        max_parameters: 16,
    }
}
fn catalog() -> FrozenCatalog {
    decode_catalog(include_bytes!("fixtures/catalog.json"), &limits()).unwrap()
}
fn selection(c: &FrozenCatalog, args: Value) -> Value {
    json!({"catalog":c.reference(),"itemId":"diagnostics","variantId":"network-check","arguments":args})
}
fn select(c: &FrozenCatalog, args: Value) -> Result<SelectedOperation, CatalogError> {
    c.select(
        &serde_json::to_vec(&selection(c, args)).unwrap(),
        &limits(),
        &parameter_limits(),
    )
}
#[test]
fn shared_parameters_preserve_the_complete_selection() {
    let c = catalog();
    let selected = select(&c, json!({"host":"example.invalid"})).unwrap();
    assert_eq!(selected.reference().catalog, c.reference());
    assert_eq!(
        selected.operation().resource.version_digest.as_str(),
        "a".repeat(64)
    );
    assert_eq!(
        selected.operation().resource.selector.key.as_str(),
        "powershell"
    );
    assert_eq!(
        selected.parameters()["count"],
        InputValue::Literal { value: json!(3) }
    );
    let projection = c
        .projection(
            &Id::new("diagnostics").unwrap(),
            &Id::new("network-check").unwrap(),
            &parameter_limits(),
        )
        .unwrap();
    assert!(jsonschema::draft202012::is_valid(
        projection.input_schema(),
        &json!({"host":"example.invalid"})
    ));
    assert_eq!(projection.fields().len(), 2);
    assert_eq!(selected.availability(100), CatalogAvailability::Listed);
    assert_eq!(selected.availability(1000), CatalogAvailability::Expired);
}
#[test]
fn unknowns_duplicates_constraints_and_defaults_fail_closed() {
    let c = catalog();
    for args in [
        json!({}),
        json!({"host":7}),
        json!({"host":"a"}),
        json!({"host":"x.example","count":10}),
        json!({"host":"x.example","actor":"admin"}),
    ] {
        assert!(select(&c, args).is_err());
    }
    let mut v: Value = serde_json::from_slice(include_bytes!("fixtures/catalog.json")).unwrap();
    for (pointer, value) in [
        ("/schemaVersion", json!(2)),
        (
            "/items/0/operations/0/parameters/count/rule/default",
            json!(50),
        ),
        (
            "/items/0/operations/0/parameters/count/rule/type",
            json!("float"),
        ),
    ] {
        let old = v.pointer(pointer).unwrap().clone();
        *v.pointer_mut(pointer).unwrap() = value;
        assert!(decode_catalog(&serde_json::to_vec(&v).unwrap(), &limits()).is_err());
        *v.pointer_mut(pointer).unwrap() = old;
    }
    let bytes = serde_json::to_string(&selection(&c, json!({"host":"example.invalid"})))
        .unwrap()
        .replace(
            "\"host\":\"example.invalid\"",
            "\"host\":\"example.invalid\",\"host\":\"other.invalid\"",
        );
    assert!(c
        .select(bytes.as_bytes(), &limits(), &parameter_limits())
        .is_err());
}
#[test]
fn exact_snapshot_and_withdrawal_cannot_be_substituted() {
    let c = catalog();
    let mut v: Value = serde_json::from_slice(include_bytes!("fixtures/catalog.json")).unwrap();
    v["items"][0]["state"] = json!("withdrawn");
    let next = decode_catalog(&serde_json::to_vec(&v).unwrap(), &limits()).unwrap();
    assert_ne!(c.reference(), next.reference());
    let old = serde_json::to_vec(&selection(&c, json!({"host":"example.invalid"}))).unwrap();
    assert!(next.select(&old, &limits(), &parameter_limits()).is_err());
    assert_eq!(
        select(&next, json!({"host":"example.invalid"}))
            .unwrap()
            .availability(100),
        CatalogAvailability::Withdrawn
    );
}

#[test]
fn every_parameter_type_uses_the_same_projection_and_runtime() {
    let mut v: Value = serde_json::from_slice(include_bytes!("fixtures/catalog.json")).unwrap();
    let fields = &mut v["items"][0]["operations"][0]["parameters"];
    fields["secret"] = json!({"title":"Credential","description":"Reference only","required":false,"rule":{"type":"secretReference"}});
    fields["enabled"] = json!({"title":"Enabled","description":"Option","required":false,"rule":{"type":"boolean","default":false}});
    fields["host"]["rule"]["choices"] = json!(["example.invalid", "other.invalid"]);
    let c = decode_catalog(&serde_json::to_vec(&v).unwrap(), &limits()).unwrap();
    let projection = c
        .projection(
            &Id::new("diagnostics").unwrap(),
            &Id::new("network-check").unwrap(),
            &parameter_limits(),
        )
        .unwrap();
    let validator = jsonschema::draft202012::new(projection.input_schema()).unwrap();
    for args in [
        json!({"host":"example.invalid"}),
        json!({"host":"other.invalid","count":3.0,"secret":{"id":"credential","revision":"r7"}}),
        json!({"host":"bad.invalid"}),
        json!({"host":"example.invalid","count":"3"}),
        json!({"host":"example.invalid","secret":"secret-material"}),
        json!({"host":"example.invalid","secret":{"id":"credential","revision":"r7","token":"secret-material"}}),
        json!({"host":"example.invalid","enabled":1}),
    ] {
        let bytes = serde_json::to_vec(&args).unwrap();
        let human = projection.validate(&bytes);
        let ai = select(&c, args.clone());
        assert_eq!(human.is_ok(), validator.is_valid(&args), "{args}");
        match (human, ai) {
            (Ok(h), Ok(a)) => assert_eq!(&h, a.parameters()),
            (Err(h), Err(a)) => assert_eq!(h, a),
            _ => panic!("entrypoint disagreement"),
        }
    }
    let result = select(
        &c,
        json!({"host":"example.invalid","secret":{"id":"credential","revision":"r7"}}),
    )
    .unwrap();
    assert!(matches!(
        result.parameters()["secret"],
        InputValue::Secret { .. }
    ));
    assert!(!format!("{result:?}").contains("credential"));
    for rule in [
        json!({"type":"secretReference","default":"secret-material"}),
        json!({"type":"secretReference","choices":[]}),
    ] {
        v["items"][0]["operations"][0]["parameters"]["secret"]["rule"] = rule;
        let err = decode_catalog(&serde_json::to_vec(&v).unwrap(), &limits()).unwrap_err();
        assert!(!format!("{err:?} {err}").contains("secret-material"));
    }
}
#[test]
fn precision_limits_defaults_and_unicode_are_bounded() {
    let c = catalog();
    let p = c
        .projection(
            &Id::new("diagnostics").unwrap(),
            &Id::new("network-check").unwrap(),
            &parameter_limits(),
        )
        .unwrap();
    for number in [
        "9007199254740992",
        "3.00000000000000001",
        "1e-999",
        "9007199254740990.9",
    ] {
        let input = format!("{{\"host\":\"example.invalid\",\"count\":{number}}}");
        assert!(p.validate(input.as_bytes()).is_err(), "{number}");
    }
    for number in ["3.0", "30e-1", "0.03e2", "3"] {
        let input = format!("{{\"host\":\"example.invalid\",\"count\":{number}}}");
        assert_eq!(
            p.validate(input.as_bytes()).unwrap()["count"],
            InputValue::Literal { value: json!(3) }
        );
    }
    let raw = br#"{"host":"example.invalid"}"#;
    let mut pl = parameter_limits();
    pl.max_bytes = raw.len();
    let p = c
        .projection(
            &Id::new("diagnostics").unwrap(),
            &Id::new("network-check").unwrap(),
            &pl,
        )
        .unwrap();
    assert_eq!(p.validate(raw).unwrap_err(), CatalogError::LimitExceeded); // expanded count default
    let mut budget = limits();
    budget.max_bytes = include_bytes!("fixtures/catalog.json").len() - 1;
    assert_eq!(
        decode_catalog(include_bytes!("fixtures/catalog.json"), &budget).unwrap_err(),
        CatalogError::LimitExceeded
    );
    for modify in [
        |l: &mut CatalogLimits| l.max_depth = 2,
        |l: &mut CatalogLimits| l.max_nodes = 2,
        |l: &mut CatalogLimits| l.max_string_bytes = 2,
        |l: &mut CatalogLimits| l.max_collection_items = 2,
    ] {
        let mut l = limits();
        modify(&mut l);
        assert_eq!(
            decode_catalog(include_bytes!("fixtures/catalog.json"), &l).unwrap_err(),
            CatalogError::LimitExceeded
        );
    }
    budget.max_depth = 65;
    assert_eq!(
        decode_catalog(b"{}", &budget).unwrap_err(),
        CatalogError::InvalidLimits
    );
}
#[test]
fn data_extensions_and_every_resource_pin_change_identity() {
    let base: Value = serde_json::from_slice(include_bytes!("fixtures/catalog.json")).unwrap();
    let c = catalog();
    for (path, val) in [
        (
            "/items/0/operations/0/resource/reference/revision",
            json!("r2"),
        ),
        (
            "/items/0/operations/0/resource/versionDigest",
            json!("b".repeat(64)),
        ),
        (
            "/items/0/operations/0/resource/selector/key",
            json!("other"),
        ),
        (
            "/items/0/operations/0/resource/selector/architecture",
            json!("aarch64"),
        ),
        (
            "/items/0/operations/0/resource/selector/platform",
            json!("macos"),
        ),
        ("/expiresAtUnixMs", json!(2000)),
    ] {
        let mut value = base.clone();
        *value.pointer_mut(path).unwrap() = val;
        let changed = decode_catalog(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap();
        assert_ne!(c.reference(), changed.reference(), "{path}");
    }
    let mut value = base.clone();
    let mut software = base["items"][0].clone();
    software["id"] = json!("software");
    software["kind"] = json!("software");
    software["operations"][0]["action"] = json!("install");
    value["items"].as_array_mut().unwrap().push(software);
    let extended = decode_catalog(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap();
    assert_eq!(extended.snapshot().items.len(), 2);
    value["items"].as_array_mut().unwrap().reverse();
    assert_eq!(
        extended.reference(),
        decode_catalog(&serde_json::to_vec(&value).unwrap(), &limits())
            .unwrap()
            .reference()
    );
    value["items"][1]["id"] = json!("software");
    assert!(decode_catalog(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}
#[test]
fn external_explanations_require_exact_selection_target_and_freshness() {
    use execution_contract::{DeviceId, Platform, Target, TargetScope};
    let selected = select(&catalog(), json!({"host":"example.invalid"})).unwrap();
    let target = Target {
        device: DeviceId::new("device-1").unwrap(),
        platform: Platform::Windows,
        scope: TargetScope::Device {},
    };
    let mut explanation = ExternalAssessment {
        selection: selected.reference().clone(),
        target: target.clone(),
        checked_at_unix_ms: 100,
        expires_at_unix_ms: 200,
        status: ExternalStatus::MissingCapability,
    };
    assert_eq!(
        selected.external_status(&target, 100, None).unwrap(),
        ExternalStatus::Unknown
    );
    assert_eq!(
        selected
            .external_status(&target, 100, Some(&explanation))
            .unwrap(),
        ExternalStatus::MissingCapability
    );
    for now in [99, 200] {
        assert_eq!(
            selected
                .external_status(&target, now, Some(&explanation))
                .unwrap(),
            ExternalStatus::Unknown
        );
    }
    explanation.target.device = DeviceId::new("device-2").unwrap();
    assert_eq!(
        selected
            .external_status(&target, 100, Some(&explanation))
            .unwrap_err(),
        CatalogError::ReferenceMismatch
    );
    explanation.target = target.clone();
    explanation.selection.catalog.authority = execution_contract::Authority::Enterprise {
        id: Id::new("issuer").unwrap(),
        tenant: Id::new("tenant").unwrap(),
    };
    assert_eq!(
        selected
            .external_status(&target, 100, Some(&explanation))
            .unwrap_err(),
        CatalogError::ReferenceMismatch
    );
}

#[test]
fn frozen_schema_and_catalog_golden() {
    use sha2::{Digest as _, Sha256};
    let c = catalog();
    assert_eq!(
        c.reference().digest.as_str(),
        include_str!("fixtures/catalog.sha256").trim()
    );
    let schema = serde_json::to_value(catalog_schema()).unwrap();
    assert!(jsonschema::draft202012::meta::is_valid(&schema));
    assert!(jsonschema::draft202012::is_valid(
        &schema,
        &serde_json::from_slice::<Value>(include_bytes!("fixtures/catalog.json")).unwrap()
    ));
    let hash = Sha256::digest(serde_json_canonicalizer::to_vec(&schema).unwrap());
    assert_eq!(
        format!("{hash:x}"),
        include_str!("fixtures/schema.sha256").trim()
    );
    assert_eq!(
        FrozenCatalog::freeze(c.snapshot().clone(), &limits())
            .unwrap()
            .reference(),
        c.reference()
    );
}

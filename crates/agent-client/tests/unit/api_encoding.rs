//! Fixed task vectors; Enrollment now includes administrator/invocation authorization in its signature.
//! These fixtures are independent of regenerated Rust serialization and signers.

use rss_mdm_agent_wire::runtime::TaskPayload;
use sha2::{Digest, Sha256};
#[test]
fn preserves_task_signatures_and_step_digest() {
    for (body, expected) in [
        (
            include_bytes!("../fixtures/script-task-v1.json").as_slice(),
            "6080741b91b8c100415abd7828401dae5e984cef7f1025bd4f89509fd5bbe7db",
        ),
        (
            include_bytes!("../fixtures/software-task-v1.json").as_slice(),
            "b4f5cb76c97d00154ca902da454558c5a921b153b616e0b3968a191d08af42d6",
        ),
        (
            include_bytes!("../fixtures/enrollment-task-v1.json").as_slice(),
            "339725f5ba4c5f37e2f00279595e7b7b3c9622c4266598d86dce799f3787d849",
        ),
    ] {
        let task: TaskPayload = serde_json::from_slice(body).unwrap();
        assert_eq!(serde_json::to_vec(&task).unwrap(), body);
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(task.signing_bytes("fixture-key").unwrap())
            ),
            expected
        );
        if let TaskPayload::Software(spec) = task {
            assert_eq!(
                spec.steps[0].digest().unwrap(),
                [
                    0x78, 0xa2, 0x7e, 0x01, 0x20, 0x43, 0x10, 0xa6, 0xe6, 0x16, 0x92, 0x10, 0xfb,
                    0x87, 0x16, 0x6e, 0xd6, 0xcb, 0x14, 0x7f, 0x54, 0xb5, 0xfe, 0x50, 0xe8, 0x4b,
                    0xc8, 0x9e, 0x2f, 0xa1, 0x4a, 0x4e
                ]
            );
        }
    }
}
#[test]
fn validates_raw_json_before_normalization() {
    let value: serde_json::Value =
        serde_json::from_slice(include_bytes!("../fixtures/script-task-v1.json")).unwrap();
    let mut unknown = value.clone();
    unknown["futureField"] = true.into();
    let mut nil = value.clone();
    nil["task"]["tenantId"] = "00000000-0000-0000-0000-000000000000".into();
    let mut version = value.clone();
    version["task"]["wireVersion"] = 2.into();
    let mut omitted = value;
    omitted["task"].as_object_mut().unwrap().remove("content");
    for invalid in [unknown, nil, version, omitted] {
        assert!(
            serde_json::from_slice::<TaskPayload>(&serde_json::to_vec(&invalid).unwrap()).is_err()
        );
    }
}

fn report_value(
    shape: rss_mdm_agent_wire::runtime::ValueType,
    value: rss_mdm_agent_wire::runtime::Scalar,
) -> Result<rss_mdm_agent_wire::runtime::ReportRequest, crate::Error> {
    use rss_mdm_agent_wire::runtime::{CollectedValue, FieldValue, ReportBody, ReportRequest};
    let original = crate::test_support::collections().remove(0);
    let mut fields = original.fields().to_vec();
    fields[0].value_type = shape;
    let field = fields[0].key;
    let collection = rss_mdm_agent_wire::runtime::CollectionDefinition::new(
        original.dataset(),
        original.version(),
        original.source(),
        fields,
    )?;
    Ok(ReportRequest::new(
        uuid::Uuid::from_u128(1),
        1,
        1,
        collection,
        ReportBody::Snapshot(vec![FieldValue {
            field,
            value: CollectedValue::Value(value),
        }]),
    )?)
}

#[test]
fn report_strings_preserve_character_budget_and_allowed_text() {
    use rss_mdm_agent_wire::runtime::{Scalar, ValueType};
    let shape = ValueType::String {
        max_length: 4,
        allow_empty: false,
    };
    for text in ["设备一", "设备一号", "a\n\tb"] {
        assert!(report_value(shape.clone(), Scalar::String(text.into())).is_ok());
    }
    for text in ["设备一号机", "", "a\0b"] {
        assert!(matches!(
            report_value(shape.clone(), Scalar::String(text.into())),
            Err(crate::Error::Protocol)
        ));
    }
    assert!(report_value(
        ValueType::String {
            max_length: 4,
            allow_empty: true,
        },
        Scalar::String(String::new()),
    )
    .is_ok());
}

#[test]
fn report_nested_strings_use_the_same_character_contract() {
    use rss_mdm_agent_wire::runtime::{Scalar, ValueType};
    let shape = ValueType::Array {
        max_items: 1,
        items: Box::new(ValueType::Object {
            properties: [(
                "name".into(),
                ValueType::String {
                    max_length: 4,
                    allow_empty: false,
                },
            )]
            .into(),
        }),
    };
    for (text, accepted) in [
        ("设备一号", true),
        ("a\n\tb", true),
        ("设备一号机", false),
        ("a\0b", false),
    ] {
        let value = Scalar::Array(vec![Scalar::Object(
            [("name".into(), Scalar::String(text.into()))].into(),
        )]);
        assert_eq!(report_value(shape.clone(), value).is_ok(), accepted);
    }
}

#[test]
fn reports_sort_fields_and_preserve_explicit_nullable_metadata() {
    use rss_mdm_agent_wire::runtime::{
        CollectedValue, FieldValue, ReportBody, ReportRequest, Scalar,
    };
    let collection = crate::test_support::collections().remove(0);
    let values = collection
        .fields()
        .iter()
        .rev()
        .map(|field| FieldValue {
            field: field.key,
            value: CollectedValue::Value(Scalar::String("fixture".into())),
        })
        .collect();
    let report = ReportRequest::new(
        uuid::Uuid::from_u128(1),
        1,
        1,
        collection,
        ReportBody::Snapshot(values),
    )
    .unwrap();
    assert_eq!(serde_json::to_vec(report.body()).unwrap(), br#"{"kind":"snapshot","values":[{"field":"device.model","value":{"kind":"value","value":{"kind":"string","value":"fixture"}}},{"field":"device.os.version","value":{"kind":"value","value":{"kind":"string","value":"fixture"}}}]}"#);
    let bytes = report.canonical().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for field in value["collection"]["fields"].as_array().unwrap() {
        assert_eq!(field.get("unit"), Some(&serde_json::Value::Null));
        assert_eq!(field.get("itemKey"), Some(&serde_json::Value::Null));
    }
    let decoded: ReportRequest = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded.canonical().unwrap(), bytes);
}

#[test]
fn native_vectors_bind_independent_signing_domains_and_result_identity() {
    use base64::Engine;
    use rss_mdm_agent_wire::runtime::*;
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/native-v3-vectors.json")).unwrap();
    for vectors in vectors.as_array().unwrap() {
        let plan: NativeCollectionPlan = serde_json::from_value(vectors["plan"].clone()).unwrap();
        let result: NativeCollectionResult =
            serde_json::from_value(vectors["result"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(plan.digest()).unwrap(),
            vectors["planDigest"]
        );
        assert_eq!(
            serde_json::to_value(result.digest()).unwrap(),
            vectors["resultDigest"]
        );
        let pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[42; 32]).unwrap();
        use ring::signature::KeyPair;
        for task in vectors["tasks"].as_array().unwrap() {
            let payload: TaskPayload =
                serde_json::from_str(task["payloadJson"].as_str().unwrap()).unwrap();
            assert_eq!(
                serde_json::to_string(&payload).unwrap(),
                task["payloadJson"]
            );
            let bytes = payload.signing_bytes("vector-key").unwrap();
            let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(task["signature"].as_str().unwrap())
                .unwrap();
            ring::signature::UnparsedPublicKey::new(
                &ring::signature::ED25519,
                pair.public_key().as_ref(),
            )
            .verify(&bytes, &signature)
            .unwrap();
            let raw: serde_json::Value =
                serde_json::from_str(task["payloadJson"].as_str().unwrap()).unwrap();
            assert!(serde_json::from_value::<TaskPayload>(raw["task"].clone()).is_err());
        }
    }
}

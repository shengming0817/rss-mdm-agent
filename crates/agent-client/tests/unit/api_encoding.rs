//! Fixed byte vectors from the consumed Agent V1 encoding at c0581d5c.
//! These fixtures are independent of regenerated Rust serialization and signers.
use crate::protocol::{SoftwareTaskStepExt, TaskPayloadExt};
use agent_api::runtime::TaskPayload;
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
            "44df350a55b6aecb293a326c1086687aa1102acf73ebdde86aec53f4c77f6f8d",
        ),
    ] {
        let task: TaskPayload = agent_api::decode(body).unwrap();
        assert_eq!(serde_json::to_vec(&task).unwrap(), body);
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(task.signing_bytes("fixture-key").unwrap())
            ),
            expected
        );
        if let TaskPayload::SoftwareTaskSpec(spec) = task {
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
    nil["tenantId"] = "00000000-0000-0000-0000-000000000000".into();
    let mut version = value.clone();
    version["wireVersion"] = 2.into();
    let mut omitted = value;
    omitted.as_object_mut().unwrap().remove("content");
    for invalid in [unknown, nil, version, omitted] {
        assert!(agent_api::decode::<TaskPayload>(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
}

fn report_value(
    shape: agent_api::runtime::ValueType,
    value: agent_api::runtime::Scalar,
) -> Result<agent_api::runtime::ReportRequest, crate::Error> {
    use crate::protocol::ReportRequestExt;
    use agent_api::runtime::{CollectedValue, FieldValue, ReportBody, ReportRequest};
    let mut collection = crate::test_support::collections().remove(0);
    collection.fields[0].value_type = shape;
    let field = collection.fields[0].key.clone();
    ReportRequest::new(
        uuid::Uuid::from_u128(1),
        1,
        1,
        collection,
        ReportBody::Snapshot {
            values: vec![FieldValue {
                field,
                value: CollectedValue::Value(value),
            }],
        },
    )
}

#[test]
fn report_strings_preserve_character_budget_and_allowed_text() {
    use agent_api::runtime::{Scalar, ValueType};
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
    use agent_api::runtime::{Scalar, ValueType};
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

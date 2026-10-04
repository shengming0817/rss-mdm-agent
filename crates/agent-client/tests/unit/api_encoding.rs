//! Fixed byte vectors from the consumed Agent V1 encoding at 149c34d5.
//! These fixtures are independent of regenerated Rust serialization and signers.
use crate::protocol::{SoftwareTaskStepExt, TaskPayloadExt};
use agent_api::runtime::TaskPayload;
use sha2::{Digest, Sha256};
#[test]
fn preserves_task_signatures_and_step_digest() {
    for (body, expected) in [
        (
            include_bytes!("../fixtures/script-task-v1.json").as_slice(),
            "9e1d0a9a1f59f8588b5aa18f9999bfb0ba94f4e1455d84efd39c90afa0faa12a",
        ),
        (
            include_bytes!("../fixtures/software-task-v1.json").as_slice(),
            "7872d388c2a2aef2de0f9a91c4087664d07989af27d6d2a31d48f072540d1b17",
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

use super::*;
#[test]
fn candidate_requires_current_format_and_valid_identity() {
    let root = crate::protocol_test_support::Root::new();
    let image = Artifact {
        path: root.path.join("service"),
        sha256: "a".repeat(64),
        cdhash: None,
    };
    let candidate = Candidate {
        version: 1,
        format: rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
        product_version: "test".into(),
        target: Target::current().unwrap(),
        root: root.path.clone(),
        service: image,
        desktop: Artifact {
            path: root.path.join("desktop"),
            sha256: "b".repeat(64),
            cdhash: None,
        },
        runtime_root: root.path.join("runtime"),
        runtime_sha256: "c".repeat(64),
        origin: "https://agent.example.org".into(),
        tenant: uuid::Uuid::new_v4(),
        signing_keys: BTreeMap::from([(
            "key".into(),
            "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE".into(),
        )]),
        ca_pem: Some(String::from_utf8(crate::protocol_test_support::pki::ca()).unwrap()),
        signing_mode: SigningMode::Development,
    };
    let current = serde_json::to_value(&candidate).unwrap();
    let decoded: Candidate = serde_json::from_value(current.clone()).unwrap();
    // Current metadata reaches native artifact checks; this pure fixture has no installed images.
    assert_eq!(decoded.validate(), Err(Error::Identity));
    let mut old = current.clone();
    old["version"] = 99.into();
    let decoded: Candidate = serde_json::from_value(old).unwrap();
    assert_eq!(decoded.validate(), Err(Error::Configuration));
    let mut old_field = current;
    old_field["unexpected"] = true.into();
    assert!(serde_json::from_value::<Candidate>(old_field).is_err());
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
}

#[test]
fn installation_image_reader_is_not_the_small_configuration_reader() {
    let root = crate::protocol_test_support::Root::new();
    let path = root.path.join("worker-image");
    let bytes = vec![42; 128 * 1024];
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read_image(&path, &hash(&bytes)).unwrap(), bytes);
    assert_eq!(read_image(&path, &"0".repeat(64)), Err(Error::Identity));
    assert!(read_image(&root.path, &hash(&bytes)).is_err());
    // Native administrator path protection remains a separate mandatory outer check.
    assert!(Artifact {
        path: path.clone(),
        sha256: hash(&bytes),
        cdhash: None
    }
    .verify(&path)
    .is_err());
}

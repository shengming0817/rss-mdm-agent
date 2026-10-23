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
        service_id: crate::protocol_test_support::SERVICE_ID,
        connections: crate::protocol_test_support::signed_connections(
            "https://agent.example.org".into(),
            uuid::Uuid::from_u128(2),
            1,
        ),
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
        tenant: uuid::Uuid::from_u128(2),
        signing_keys: BTreeMap::from([("test".into(), {
            use base64::Engine;
            use ring::signature::KeyPair;
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
                ring::signature::Ed25519KeyPair::from_seed_unchecked(&[7; 32])
                    .unwrap()
                    .public_key()
                    .as_ref(),
            )
        })]),
        ca_pem: Some(String::from_utf8(crate::protocol_test_support::pki::ca()).unwrap()),
        signing_mode: SigningMode::Development,
    };
    // Exercise the actual public installer CLI through its bounded external reader.
    // Native image checks deliberately remain in the privileged installation path.
    let public = root.path.join("candidate.json");
    std::fs::write(&public, serde_json::to_vec(&candidate).unwrap()).unwrap();
    let args = [
        "validate-trust".into(),
        "--candidate".into(),
        public.clone().into_os_string(),
    ];
    assert_eq!(lifecycle::run(&args), Ok(()));
    std::fs::write(
        &public,
        vec![b' '; rss_mdm_agent_wire::connections::MAX_FILE_BYTES + 1],
    )
    .unwrap();
    assert_eq!(lifecycle::run(&args), Err(Error::Configuration));
    std::fs::remove_file(public).unwrap();
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

#[test]
fn program_refresh_keeps_stable_namespace_and_source_grants_across_signed_seed_changes() {
    let mut old = crate::deployment::tests::deployment();
    old.software_read_grants
        .push(agent_client::SoftwareReadGrant {
            credential: rss_mdm_agent_wire::runtime::SoftwareReadCredential {
                source: "private-source".into(),
                reference: "read-only-token".into(),
            },
            origin: "https://software.example.org".into(),
        });
    let mut next = old.clone();
    next.connections.configuration.revision += 1;
    next.connections.configuration.origins = vec!["https://new-agent.example.org:9445".into()];
    next.connections.configuration.preferred_origin =
        next.connections.configuration.origins[0].clone();
    next.connections =
        crate::protocol_test_support::sign_connections(next.connections.configuration);
    assert_eq!(old.namespace().unwrap(), next.namespace().unwrap());
    lifecycle::preserved(&old, &next).unwrap();
    next.service_id = uuid::Uuid::new_v4();
    assert_eq!(lifecycle::preserved(&old, &next), Err(Error::Identity));
    assert_ne!(old.namespace().unwrap(), next.namespace().unwrap());
    next = old.clone();
    next.software_read_grants.clear();
    assert_eq!(lifecycle::preserved(&old, &next), Err(Error::Identity));
    next = old.clone();
    next.signing_keys
        .insert("new-root".into(), "unapproved".into());
    assert_eq!(lifecycle::preserved(&old, &next), Err(Error::Identity));
}

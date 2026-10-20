use agent_client::{native_source, Error, OpenMode};
use rss_mdm_agent_wire::runtime::*;
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use support::{
    software_native::{pack, TOKEN},
    *,
};

#[tokio::test]
async fn complete_native_binding_and_local_identity_remain_inside_signature_admission() {
    use agent_client::protocol::{verify_task, TaskVerification};
    use sha2::{Digest as _, Sha256};
    for brew in [false, true] {
        let server = Server::tls().await;
        server
            .data
            .lock()
            .unwrap()
            .software_native(&server.url, brew);
        let signed = server.data.lock().unwrap().offer.clone().unwrap();
        let TaskPayload::Software(spec) = &signed.payload else {
            unreachable!()
        };
        let keys = server.config().registration_keys;
        let context = TaskVerification {
            key_id: "test",
            public_key: &keys["test"],
            tenant_id: spec.tenant_id,
            device_id: &spec.device_id,
            platform: spec.platform,
            architecture: spec.architecture,
            registration_id: spec.registration_id,
            generation: spec.generation,
            task_id: spec.task_id,
            attempt_id: spec.attempt_id,
            permit: TaskPermit::Offer,
            now: 1,
        };
        verify_task(&signed, &context).unwrap();
        for field in [
            "target",
            "publication",
            "target_digest",
            "document",
            "credential",
            "generation",
            "device",
            "tenant",
            "registration",
        ] {
            let mut changed = signed.clone();
            let TaskPayload::Software(spec) = &mut changed.payload else {
                unreachable!()
            };
            let binding = match &mut spec.steps[0].export {
                SoftwareTaskExport::Brew { binding, .. }
                | SoftwareTaskExport::Winget { binding, .. } => binding,
                _ => unreachable!(),
            };
            match field {
                "target" => binding.target.push_str("-other"),
                "publication" => binding.publication[0] ^= 1,
                "target_digest" => binding.target_digest[0] ^= 1,
                "document" => binding.document_sha256[0] ^= 1,
                "credential" => match &mut spec.steps[0].export {
                    SoftwareTaskExport::Brew {
                        credential_reference,
                        ..
                    } => credential_reference.push_str("-other"),
                    SoftwareTaskExport::Winget { identifier, .. } => identifier.push_str("-other"),
                    _ => unreachable!(),
                },
                "generation" => spec.generation += 1,
                "device" => spec.device_id.push_str("-other"),
                "tenant" => spec.tenant_id = uuid::Uuid::new_v4(),
                "registration" => spec.registration_id = uuid::Uuid::new_v4(),
                _ => unreachable!(),
            }
            spec.definition_digest =
                Sha256::digest(serde_json::to_vec(&spec.steps).unwrap()).into();
            assert!(verify_task(&changed, &context).is_err(), "{field}");
        }
    }
}

fn provision(server: &Server) {
    let credential = server
        .data
        .lock()
        .unwrap()
        .software_source
        .as_ref()
        .unwrap()
        .credential
        .clone();
    if let Some(credential) = credential {
        server.secrets.0.lock().unwrap().insert(
            native_source::credential_key(&credential).unwrap(),
            TOKEN.into(),
        );
    }
}

#[tokio::test]
async fn native_manifest_keeps_frozen_installers_and_never_promotes_locale_extensions() {
    let server = Server::tls().await;
    server
        .data
        .lock()
        .unwrap()
        .software_native(&server.url, false);
    let data = server.data.lock().unwrap();
    let TaskPayload::Software(spec) = &data.offer.as_ref().unwrap().payload else {
        unreachable!()
    };
    let SoftwareTaskExport::Winget { binding, .. } = &spec.steps[0].export else {
        unreachable!()
    };
    let original = data.software_source.as_ref().unwrap().document.clone();
    let manifest = native_source::winget_manifest(
        &serde_json::to_vec(&original).unwrap(),
        binding,
        &spec.steps[0].action,
    )
    .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(
        manifest["Installers"],
        original["Versions"][0]["Installers"]
    );
    assert_eq!(manifest["PackageIdentifier"], original["PackageIdentifier"]);
    assert_eq!(manifest["PackageVersion"], "1.0");
    let mut changed = original.clone();
    changed["Versions"][0]["Installers"][0]["InstallerUrl"] =
        "https://other.example.test/evil.msi".into();
    assert!(native_source::winget_manifest(
        &serde_json::to_vec(&changed).unwrap(),
        binding,
        &spec.steps[0].action
    )
    .is_err());
    let mut extended = original;
    extended["Versions"][0]["DefaultLocale"]["InstallerSwitches"] =
        serde_json::json!({"Silent":"unapproved"});
    let mut binding = binding.clone();
    binding.document_sha256 = winget_document_digest(&extended).unwrap();
    let output: serde_json::Value = serde_json::from_slice(
        &native_source::winget_manifest(
            &serde_json::to_vec(&extended).unwrap(),
            &binding,
            &spec.steps[0].action,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(output.get("InstallerSwitches").is_none());
}

#[tokio::test]
async fn frozen_sources_prepare_before_start_and_recover_without_source_io() {
    for brew in [false, true] {
        let server = Server::tls().await;
        server
            .data
            .lock()
            .unwrap()
            .software_native(&server.url, brew);
        provision(&server);
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        server.register(&mut client).await;
        let offer = client.claim().await.unwrap().offer.unwrap();
        let materials = client.prepare(&offer).await.unwrap();
        assert_eq!(materials.files().len(), 2);
        materials.native_source(0).unwrap().reader().unwrap();
        assert!(!server.data.lock().unwrap().started);
        client.received(&offer).await.unwrap();
        let start = client.request_start(&offer, &materials).await.unwrap();
        assert!(start.matches_payload(offer.payload()));
        let task = offer.task_id();
        let requests = server
            .data
            .lock()
            .unwrap()
            .software_source
            .as_ref()
            .unwrap()
            .requests
            .len();
        server
            .data
            .lock()
            .unwrap()
            .software_source
            .as_mut()
            .unwrap()
            .denied = true;
        drop((start, materials, offer, client));
        let mut client = server.client(&root, OpenMode::Existing);
        let resumed = client.recover_start(task).await.unwrap();
        resumed
            .materials
            .native_source(0)
            .unwrap()
            .reader()
            .unwrap();
        assert!(resumed.start.matches_payload(resumed.offer.payload()));
        assert_eq!(
            server
                .data
                .lock()
                .unwrap()
                .software_source
                .as_ref()
                .unwrap()
                .requests
                .len(),
            requests
        );
    }
}

#[tokio::test]
async fn rejected_or_changed_source_never_starts_or_revokes_device_identity() {
    for brew in [false, true] {
        let server = Server::tls().await;
        server
            .data
            .lock()
            .unwrap()
            .software_native(&server.url, brew);
        provision(&server);
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        server.register(&mut client).await;
        let registration = client.registration().unwrap();
        let offer = client.claim().await.unwrap().offer.unwrap();
        server
            .data
            .lock()
            .unwrap()
            .software_source
            .as_mut()
            .unwrap()
            .denied = true;
        assert!(matches!(client.prepare(&offer).await, Err(Error::Denied)));
        assert_eq!(
            client.registration().unwrap().registration_id,
            registration.registration_id
        );
        server
            .data
            .lock()
            .unwrap()
            .software_source
            .as_mut()
            .unwrap()
            .denied = false;
        if brew {
            server
                .data
                .lock()
                .unwrap()
                .software_source
                .as_mut()
                .unwrap()
                .pack[12] ^= 1;
        } else {
            server
                .data
                .lock()
                .unwrap()
                .software_source
                .as_mut()
                .unwrap()
                .document["Versions"][0]["PackageVersion"] = "2.0".into();
        }
        assert!(client.prepare(&offer).await.is_err());
        assert!(!server.data.lock().unwrap().started);
    }
}

#[tokio::test]
async fn read_token_only_leaves_for_approved_origin_and_is_redacted() {
    let server = Server::tls().await;
    server
        .data
        .lock()
        .unwrap()
        .software_native(&server.url, true);
    provision(&server);
    let mut cfg = server.config();
    cfg.software_read_grants[0].origin = "https://other.example.test".into();
    let root = Root::new();
    let mut client = agent_client::Client::open(
        &root.path,
        cfg,
        OpenMode::Create,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    client.set_profiles(vec![ExecutorProfile::PosixSh]).unwrap();
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    assert!(matches!(client.prepare(&offer).await, Err(Error::Denied)));
    assert!(server
        .data
        .lock()
        .unwrap()
        .software_source
        .as_ref()
        .unwrap()
        .requests
        .is_empty());
    assert_eq!(
        client
            .output_policy()
            .unwrap()
            .redact(&format!("output {TOKEN}"))
            .unwrap(),
        "output [redacted]"
    );
}

#[tokio::test]
async fn git_pack_checks_commit_inventory_and_document_bytes_before_checkout() {
    let bytes = b"class Fixture < Formula\nend\n";
    let digest = brew_documents_digest([("Formula/fixture.rb", bytes.as_slice())]).unwrap();
    let (commit, valid) = pack(&[("Formula/fixture.rb", bytes, "100644")], false);
    let root = Root::new();
    assert_eq!(
        native_source::import_brew_pack(&root.path, &valid, &commit, &digest, None)
            .await
            .unwrap(),
        vec![("Formula/fixture.rb".into(), bytes.to_vec())]
    );
    for documents in [
        vec![("Formula/fixture.rb", bytes.as_slice(), "100755")],
        vec![("Hooks/fixture.rb", bytes.as_slice(), "100644")],
        vec![("Formula/fixture.rb", b"/tmp/foreign".as_slice(), "120000")],
    ] {
        let (commit, pack) = pack(&documents, false);
        let root = Root::new();
        assert!(
            native_source::import_brew_pack(&root.path, &pack, &commit, &digest, None)
                .await
                .is_err()
        );
    }
    let (child, pack) = pack(&[("Formula/fixture.rb", bytes, "100644")], true);
    let root = Root::new();
    assert!(
        native_source::import_brew_pack(&root.path, &pack, &child, &digest, None)
            .await
            .is_err()
    );
    let root = Root::new();
    assert!(
        native_source::import_brew_pack(&root.path, &valid, &commit, &[9; 32], None)
            .await
            .is_err()
    );
    let root = Root::new();
    let counter = std::sync::atomic::AtomicU64::new(0);
    assert!(matches!(
        native_source::import_brew_pack(&root.path, &valid, &commit, &digest, Some((&counter, 1)))
            .await,
        Err(Error::Capacity)
    ));
}

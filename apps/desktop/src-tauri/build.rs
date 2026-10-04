// ref: Tauri crates/tauri-build/src/acl.rs@tauri-build-v2.6.2
#[path = "src/organization_config.rs"]
mod organization_config;

// ref: Tauri crates/tauri-utils/src/acl/build.rs@tauri-utils-v2.9.3
fn write_if_changed(path: &std::path::Path, bytes: &[u8]) {
    match std::fs::read(path) {
        Ok(current) if current == bytes => return,
        Ok(_) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => panic!("read generated input {}: {error}", path.display()),
    }
    std::fs::write(path, bytes).expect("write generated build input");
}

fn main() {
    println!("cargo:rerun-if-env-changed=RSS_BUILD_MDM_ORGANIZATION");
    println!("cargo:rerun-if-changed=src/organization_config.rs");
    let organization =
        std::env::var("RSS_BUILD_MDM_ORGANIZATION")
            .ok()
            .map(|input| {
                serde_json::from_str::<organization_config::OrganizationConfiguration>(&input)
            .expect("RSS_BUILD_MDM_ORGANIZATION must contain the allowed connection fields and optional CA certificate")
            .normalize()
            .unwrap_or_else(|error| panic!("{error}"))
            });
    write_if_changed(
        &std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR"))
            .join("default_organization.json"),
        &serde_json::to_vec(&organization).expect("default organization JSON"),
    );
    let private_link = "../../../crates/native-process/private-link-v1.json";
    println!("cargo:rerun-if-changed={private_link}");
    let contract: serde_json::Value =
        serde_json::from_slice(&std::fs::read(private_link).expect("private-link contract"))
            .expect("private-link contract JSON");
    let magic = contract["magic"].as_array().expect("private-link magic");
    assert_eq!(magic.len(), 3);
    let version = contract["version"].as_u64().expect("private-link version");
    let max = contract["maxFrameBytes"]
        .as_u64()
        .expect("private-link max frame");
    let queued_bytes = contract["maxQueuedBytes"]
        .as_u64()
        .expect("private-link queue bytes");
    let queued_frames = contract["maxQueuedFrames"]
        .as_u64()
        .expect("private-link queue frames");
    let generated = format!(
        "const MAGIC: [u8; 4] = [{}, {}, {}, {version}];\nconst MAX: usize = {max};\nconst QUEUED_BYTES: usize = {queued_bytes};\nconst QUEUED_FRAMES: usize = {queued_frames};\n",
        magic[0].as_u64().expect("magic byte"),
        magic[1].as_u64().expect("magic byte"),
        magic[2].as_u64().expect("magic byte"),
    );
    write_if_changed(
        &std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR"))
            .join("private_link_contract.rs"),
        generated.as_bytes(),
    );
    let manifest = "resources/ai-host-runtime/manifest.json";
    // ref: Cargo src/cargo/core/compiler/fingerprint/mod.rs@0.97.0
    // Subscribe to a real directory while optional assembly is absent. Once staged,
    // the manifest itself detects modification and deletion without scanning Node modules.
    let bytes = match std::fs::read(manifest) {
        Ok(bytes) => {
            println!("cargo:rerun-if-changed={manifest}");
            Some(bytes)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("cargo:rerun-if-changed=resources");
            None
        }
        Err(error) => panic!("read staged runtime manifest: {error}"),
    };
    if let Some(bytes) = bytes {
        let candidate: serde_json::Value =
            serde_json::from_slice(&bytes).expect("staged runtime manifest");
        let digest = candidate["runtimeTreeSha256"]
            .as_str()
            .expect("staged runtime digest");
        assert!(
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        println!("cargo:rustc-env=RSS_BUNDLED_RUNTIME_SHA256={digest}");
    }
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "self_service_policy_directory",
            "self_service_policy_invoke",
            "self_service_policy_confirm",
            "self_service_policy_status",
            "self_service_observe_software",
            "self_service_snapshot",
            "self_service_execute",
            "self_service_confirm",
            "self_service_cancel",
            "execution_task_details",
            "save_connection",
            "test_users",
            "select_guest",
            "account_logout",
            "account_status",
            "account_organizations",
            "account_save_organization",
            "account_login",
            "select_test_user",
            "execution_service_status",
            "agent_registration_status",
            "agent_request_registration",
            "appearance_snapshot",
            "development_assembly",
            "ai_host_status",
            "ai_restart_host",
            "ai_export_diagnostics",
            "ai_connect",
            "ai_receive",
            "ai_send",
            "ai_disconnect",
        ]),
    ))
    .expect("desktop capability manifest");
}

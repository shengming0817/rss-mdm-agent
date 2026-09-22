// ref: Tauri crates/tauri-build/src/acl.rs@tauri-build-v2.6.2
fn main() {
    let manifest = "resources/ai-host-runtime/manifest.json";
    println!("cargo:rerun-if-changed={manifest}");
    if let Ok(bytes) = std::fs::read(manifest) {
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
            "self_service_snapshot",
            "self_service_preview",
            "self_service_submit",
            "self_service_respond",
            "self_service_approve",
            "self_service_cancel",
            "execution_task_details",
            "save_connection",
            "test_users",
            "select_test_user",
            "local_service_status",
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

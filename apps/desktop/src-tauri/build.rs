// ref: Tauri crates/tauri-build/src/acl.rs@tauri-build-v2.6.2
fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "self_service_snapshot",
            "self_service_preview",
            "self_service_submit",
            "self_service_respond",
            "self_service_approve",
            "self_service_cancel",
            "execution_task_details",
            "enter_connection_credential",
            "test_users",
            "select_test_user",
            "ai_connect",
            "ai_receive",
            "ai_send",
            "ai_disconnect",
        ]),
    ))
    .expect("desktop capability manifest");
}

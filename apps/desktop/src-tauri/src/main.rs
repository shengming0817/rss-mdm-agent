// ref: prmonitor src-tauri/src/main.rs@4dcc87264ad740da6559824e0a8b04a1c2914d4b
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run RSS MDM Agent desktop");
}

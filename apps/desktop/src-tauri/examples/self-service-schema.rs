// ref: schemars src/generate.rs@v1.2.2 (explicit serialization contract)
fn main() {
    println!(
        "{}",
        serde_json::to_string(&rss_mdm_desktop::self_service::ipc::wire_schema()).unwrap()
    );
}

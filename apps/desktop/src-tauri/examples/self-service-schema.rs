// ref: schemars src/generate.rs@v1.2.2 (explicit serialization contract)
fn main() {
    println!(
        "{}",
        serde_json::to_string(&rss_mdm_desktop::composition::ipc::wire_schema()).unwrap()
    );
}

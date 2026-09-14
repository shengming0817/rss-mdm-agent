fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&service_catalog::catalog_schema()).unwrap()
    );
}

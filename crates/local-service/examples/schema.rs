fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&schemars::schema_for!(local_service::ServiceView)).unwrap()
    );
}

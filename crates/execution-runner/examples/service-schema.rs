fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&schemars::schema_for!(execution_runner::host::ServiceView))
            .unwrap()
    );
}

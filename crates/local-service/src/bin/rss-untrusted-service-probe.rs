//! Negative platform fixture: never add this executable to the installed policy.
fn main() {
    // Observation only. The platform driver brackets this with trusted queries.
    println!(
        "{}",
        serde_json::json!({"admitted": local_service::query().is_ok()})
    );
}

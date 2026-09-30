//! Empty browser projection: no fixture tasks or execution owner are installed in production.
fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&rss_mdm_desktop::self_service::Snapshot {
            available: vec![],
            requests: vec![],
            next: None,
        })
        .unwrap()
    );
}

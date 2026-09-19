//! Generate read-only browser data by exercising the actual Rust fixture owner.
use execution_contract::RequestId;
use rss_mdm_desktop::self_service::{Draft, FieldInput, FixtureService, Submission};
fn main() {
    let mut service = FixtureService::new("browser-preview".into(), 1000).expect("fixture");
    let catalog = service.snapshot(1000).expect("snapshot").catalog;
    for item in catalog.iter().filter(|item| {
        matches!(
            item.item_id.as_str(),
            "office" | "diagnostics" | "restart" | "unknown"
        )
    }) {
        let mut fields = std::collections::BTreeMap::new();
        if item.item_id.as_str() == "diagnostics" {
            fields.insert(
                "host".into(),
                FieldInput::Text {
                    value: "example.invalid".into(),
                },
            );
        }
        let plan = service
            .preview(
                Draft {
                    instance_id: "browser-preview".into(),
                    request_id: RequestId::new(format!("preview-{}", item.item_id.as_str()))
                        .expect("id"),
                    revision: 1,
                    catalog: item.catalog.clone(),
                    item_id: item.item_id.clone(),
                    variant_id: item.variant_id.clone(),
                    fields,
                },
                1000,
            )
            .expect("preview");
        service
            .submit(
                Submission {
                    instance_id: "browser-preview".into(),
                    request_id: plan.request_id,
                    plan_id: plan.plan_id,
                    digest: plan.digest,
                },
                1000,
            )
            .expect("submit");
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&service.snapshot(1000).expect("snapshot")).expect("JSON")
    );
}

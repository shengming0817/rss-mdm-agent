use local_service::{Capability, ServiceView, Status};
fn main() {
    let status = Status {
        version: 1,
        installation: "consumer".into(),
        platform: "fixture".into(),
        build: "fixture".into(),
        capability: Capability::StatusOnly,
    };
    let bytes = serde_json::to_vec(&status).unwrap();
    assert_eq!(serde_json::from_slice::<Status>(&bytes).unwrap(), status);
    assert!(serde_json::from_str::<Capability>(r#""execute""#).is_err());
    let mut edited = serde_json::to_value(&status).unwrap();
    edited["credential"] = serde_json::json!("untrusted");
    assert!(serde_json::from_value::<Status>(edited).is_err());
    assert_eq!(
        serde_json::to_value(ServiceView::Rejected).unwrap()["phase"],
        "rejected"
    );
    // This isolated consumer deliberately never contacts a live user's service.
}

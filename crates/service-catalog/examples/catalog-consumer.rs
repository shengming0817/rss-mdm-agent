use service_catalog::{decode_catalog, CatalogAvailability, CatalogLimits, ParameterLimits};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = std::fs::read(args.next().ok_or("catalog fixture path required")?)?;
    let expected = std::fs::read_to_string(args.next().ok_or("expected catalog digest required")?)?;
    let limits = CatalogLimits {
        max_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
    };
    let catalog = decode_catalog(&input, &limits)?;
    assert_eq!(catalog.reference().digest.as_str(), expected.trim());
    let item = &catalog.snapshot().items[0];
    let op = &item.operations[0];
    let parameters = ParameterLimits {
        max_bytes: 4096,
        max_string_bytes: 1024,
        max_parameters: 16,
    };
    let projection = catalog.projection(&item.id, &op.id, &parameters)?;
    let normalized = projection.validate(br#"{"host":"example.invalid"}"#)?;
    let selected=catalog.select(&serde_json::to_vec(&serde_json::json!({"catalog":catalog.reference(),"itemId":item.id,"variantId":op.id,"arguments":{"host":"example.invalid"}}))?,&limits,&parameters)?;
    assert_eq!(&normalized, selected.parameters());
    assert_eq!(selected.operation().resource, op.resource);
    assert_eq!(selected.operation().requirements, op.requirements);
    assert_eq!(selected.availability(1000), CatalogAvailability::Expired);
    assert!(projection
        .validate(br#"{"host":"example.invalid","approved":true}"#)
        .is_err());
    println!("catalog consumer: exact pin, shared defaults, constraints, expiry and unknown-input rejection passed (data only)");
    Ok(())
}

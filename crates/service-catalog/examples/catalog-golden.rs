use sha2::{Digest as _, Sha256};
fn main() {
    let l = service_catalog::CatalogLimits {
        max_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
    };
    let c = service_catalog::decode_catalog(include_bytes!("../tests/fixtures/catalog.json"), &l)
        .unwrap();
    println!("{}", c.reference().digest.as_str());
    println!(
        "{:x}",
        Sha256::digest(
            serde_json_canonicalizer::to_vec(&service_catalog::catalog_schema()).unwrap()
        )
    );
}

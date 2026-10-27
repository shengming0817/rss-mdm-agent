use execution_contract::*;

#[test]
fn local_page_links_round_trip_without_fabricated_enterprise_identity() {
    for node in PAGE_NODES
        .iter()
        .filter(|n| !n.ends_with(".enterprise") && **n != "tasks.management")
    {
        let uri = format!("rssmdm://view/v1/node/{node}");
        let intent = EntryIntent::parse(&uri).unwrap();
        assert!(intent.namespace.is_none());
        assert_eq!(intent.uri().unwrap(), uri);
    }
}
#[test]
fn external_addresses_reject_ambiguous_and_noncanonical_authority() {
    for uri in [
        "RSSMDM://view/v1/node/home",
        "rssmdm://view:80/v1/node/home",
        "rssmdm://user@view/v1/node/home",
        "rssmdm://view/v2/node/home",
        "rssmdm://view/v1/node/home#fragment",
        "rssmdm://view/v1/node/home/",
        "rssmdm://view/v1/node/unknown",
        "rssmdm://view/v1/node/%FF",
        "rssmdm://view/v1/node/%",
        "rssmdm://view/v1/node/home?confirm=true",
        "rssmdm://view/v1/node/home?step=a&step=b",
        "rssmdm://view/v1/node/home?server=https://evil.test",
        "rssmdm://view/v1/node/home?parameters=%7B%22secret%22%3A%22password%22%7D",
        "rssmdm://invoke/v1/node/home",
        "rssmdm://view/v1/alias/repair",
        "rssmdm://view/v1/node/home?instance=only-one-coordinate",
        "rssmdm://view/v1/node/home?attempt=unexpected",
        "rssmdm://view/v1/node/home?revision=1",
        "rssmdm://view/v1/node/%00home",
    ] {
        assert!(EntryIntent::parse(uri).is_err(), "{uri}");
    }
    assert!(
        EntryIntent::parse(&format!("rssmdm://view/v1/node/home?{}", "x".repeat(4096))).is_err()
    );
}
#[test]
fn enterprise_link_is_bound_and_only_carries_non_authoritative_suggestions() {
    let uri = "rssmdm://invoke/v1/alias/Repair?instance=17c72b26-774a-492c-9bf4-5e1c47c8501e&tenant=534610c5-1c82-42b1-bc9d-4b834c973cb3&device=device-1&registration=360a2191-9d5f-4fea-9f5c-6ecb02394aad&generation=3&version=v1&revision=4&parameters=%7B%22label%22%3A%22A%20%2B%20B%22%7D";
    let intent = EntryIntent::parse(uri).unwrap();
    assert_eq!(intent.action, EntryAction::Invoke);
    assert_eq!(intent.parameters["label"], "A + B");
    let canonical = intent.uri().unwrap();
    assert!(canonical.contains("/alias/repair?"));
    assert_eq!(
        EntryIntent::parse(&canonical).unwrap().uri().unwrap(),
        canonical
    );
    for changed in [
        uri.replace("generation=3", "generation=0"),
        uri.replace(
            "instance=17c72b26-774a-492c-9bf4-5e1c47c8501e",
            "instance=https%3A%2F%2Fevil.test",
        ),
    ] {
        assert!(EntryIntent::parse(&changed).is_err());
    }
}
#[test]
fn display_names_are_not_exact_tool_selectors() {
    assert!(EntrySelector::parse("tool", "Repair laptop").is_err());
    assert!(EntrySelector::parse("displayName", "Repair laptop").is_err());
    assert!(EntrySelector::parse("tool", "policy_534610c51c8242b1bc9d4b834c973cb3").is_ok());
    assert!(EntrySelector::parse("alias", "Repair").is_ok());
    assert!(EntrySelector::parse("event", "Repair").is_ok());
}

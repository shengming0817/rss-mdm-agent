use super::*;
#[path = "../../../tests/agent-protocol/contract.rs"]
#[allow(dead_code)]
mod contract;

#[test]
fn production_bindings_match_the_exact_wire_commit_http_surface() {
    let contract = contract::fixed();
    let mut seen = std::collections::BTreeSet::new();
    for spec in specs() {
        assert!(
            seen.insert((spec.method.to_string(), spec.path)),
            "duplicate consumer operation"
        );
        let endpoint = contract
            .endpoint(spec.method.as_str(), spec.path)
            .expect(spec.path);
        assert_eq!(endpoint["path"], spec.path);
        let successes: Vec<_> = endpoint["responses"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["status"].as_u64().unwrap() < 300)
            .collect();
        let statuses: Vec<_> = successes
            .iter()
            .map(|r| r["status"].as_u64().unwrap() as u16)
            .collect();
        assert_eq!(statuses, spec.statuses, "{} {}", spec.method, spec.path);
        for response in successes {
            assert_eq!(
                response["body"]
                    .get("content_type")
                    .and_then(serde_json::Value::as_str),
                spec.media,
                "{}",
                spec.path
            );
        }
    }
    assert!(!seen.is_empty());
}

#[test]
fn exact_success_status_and_media_do_not_accept_other_2xx() {
    assert!(Call::<Report>::check_status(StatusCode::ACCEPTED).is_ok());
    assert!(Call::<Report>::check_status(StatusCode::OK).is_err());
    assert!(Call::<TrustAck>::check_status(StatusCode::OK).is_err());
    assert!(Call::<FilevaultReceipt>::check_status(StatusCode::NO_CONTENT).is_err());
    assert!(Call::<TaskContent>::check_status(StatusCode::PARTIAL_CONTENT).is_ok());
    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        "application/json; charset=utf-8".parse().unwrap(),
    );
    assert!(Call::<Report>::check_media(&headers).is_ok());
    assert!(Call::<TaskContent>::check_media(&headers).is_err());
}

#[test]
fn path_parameters_are_encoded_once_without_changing_the_fixed_route() {
    let origin = url::Url::parse("https://agent.example.test/").unwrap();
    let resolved = url::<Activity>(&origin, &["batch:a/b?c#d%20"], Some("view=summary")).unwrap();
    assert_eq!(
        resolved.path(),
        "/api/v1/agent/runtime/management-activities/batch:a%2Fb%3Fc%23d%2520"
    );
    assert_eq!(resolved.query(), Some("view=summary"));
    assert!(url::<Activity>(&origin, &[], None).is_err());
    assert!(url::<Activity>(&origin, &[".."], None).is_err());
    assert!(url::<CurrentUser>(&origin, &["extra"], None).is_err());
}

#[test]
fn frozen_input_is_checked_for_the_endpoint_and_never_reencoded() {
    let http = reqwest::Client::new();
    let origin = url::Url::parse("https://agent.example.test/").unwrap();
    let ack = task_trust::Acknowledgement {
        operation: uuid::Uuid::new_v4(),
        version: 1,
        registration: uuid::Uuid::new_v4(),
        generation: 1,
        revision: 1,
        digest: "0".repeat(64),
    };
    let bytes = serde_json::to_vec_pretty(&ack).unwrap();
    let call = Call::<TaskTrustAck>::frozen(&http, &origin, &[], &bytes).unwrap();
    assert_eq!(
        call.builder
            .build()
            .unwrap()
            .body()
            .unwrap()
            .as_bytes()
            .unwrap(),
        bytes
    );
    assert!(Call::<Report>::frozen(&http, &origin, &[], &bytes).is_err());
}

#[test]
fn nullable_receipt_and_local_ack_are_checked_against_the_real_producer_schema() {
    let contract = contract::fixed();
    let receipt = contract
        .endpoint("GET", FilevaultReceipt::SPEC.path)
        .unwrap();
    let response = receipt["responses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == 200)
        .unwrap();
    assert!(contract
        .validate(
            &response["body"]["schema"],
            &serde_json::Value::Null,
            "response"
        )
        .is_ok());
    assert!(contract
        .validate(
            &response["body"]["schema"],
            &serde_json::json!({}),
            "response"
        )
        .is_err());
    let ack = contract.endpoint("POST", AccessUse::SPEC.path).unwrap();
    let response = ack["responses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == 200)
        .unwrap();
    let valid = serde_json::json!({"accepted":true});
    assert!(contract
        .validate(&response["body"]["schema"], &valid, "response")
        .is_ok());
    assert!(
        serde_json::from_value::<RecoveryUseAck>(valid)
            .unwrap()
            .accepted
    );
    assert!(contract
        .validate(
            &response["body"]["schema"],
            &serde_json::json!({"accepted":"true"}),
            "response"
        )
        .is_err());
}

#[test]
fn fixture_requests_and_responses_are_not_guessed_from_another_success_shape() {
    let contract = contract::fixed();
    let uri = "/api/v1/agent/runtime/filevault/operations/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
        .parse()
        .unwrap();
    let headers = HeaderMap::new();
    assert!(contract.request("GET", &uri, &headers, b"").is_ok());
    assert!(contract.request("POST", &uri, &headers, b"").is_err());
    assert!(contract.request("GET", &uri, &headers, b"null").is_err());
    let extra = format!("{uri}?unbound=1").parse().unwrap();
    assert!(contract.request("GET", &extra, &headers, b"").is_err());
    let endpoint = contract
        .endpoint("GET", FilevaultReceipt::SPEC.path)
        .unwrap();
    let mut headers = headers;
    headers.insert("content-type", "application/json".parse().unwrap());
    assert!(contract.response(endpoint, 200, &headers, b"null").is_ok());
    assert!(contract.response(endpoint, 204, &headers, b"").is_err());
    assert!(contract.response(endpoint, 200, &headers, b"{}").is_err());
}

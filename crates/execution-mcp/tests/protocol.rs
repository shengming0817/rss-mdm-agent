#[allow(dead_code)]
#[path = "support/mod.rs"]
mod fixture;

use execution_mcp::*;
use fixture::{limits, TestService};
use serde_json::{json, Value};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf},
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

const CATALOG: &[u8] = include_bytes!("../../service-catalog/tests/fixtures/catalog.json");
const PLAN: &[u8] = include_bytes!("../../execution-contract/tests/fixtures/plan.json");
fn service() -> Arc<TestService> {
    Arc::new(TestService::new(CATALOG, PLAN))
}
struct Wire {
    input: WriteHalf<DuplexStream>,
    output: BufReader<ReadHalf<DuplexStream>>,
    task: JoinHandle<Result<(), ServiceError>>,
    stop: CancellationToken,
}
impl Wire {
    async fn open(service: Arc<TestService>, limits: McpLimits) -> Self {
        let (client, server) = tokio::io::duplex(1024);
        let (r, w) = tokio::io::split(server);
        let stop = CancellationToken::new();
        let run_stop = stop.clone();
        let adapter = ExecutionMcp::new(service, limits).unwrap();
        let task = tokio::spawn(async move { adapter.serve(r, w, run_stop).await });
        let (output, input) = tokio::io::split(client);
        let mut wire = Self {
            input,
            output: BufReader::new(output),
            task,
            stop,
        };
        wire.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).await;
        assert_eq!(wire.recv().await["result"]["protocolVersion"], "2025-11-25");
        wire.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        wire
    }
    async fn send(&mut self, value: Value) {
        self.raw(&value.to_string()).await;
    }
    async fn raw(&mut self, raw: &str) {
        timeout(Duration::from_secs(3), async {
            self.input.write_all(raw.as_bytes()).await.unwrap();
            self.input.write_all(b"\n").await.unwrap();
            self.input.flush().await.unwrap();
        })
        .await
        .unwrap();
    }
    async fn recv(&mut self) -> Value {
        let mut line = String::new();
        let count = timeout(Duration::from_secs(3), self.output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        assert!(count > 0, "unexpected EOF");
        serde_json::from_str(&line).unwrap()
    }
    async fn call(&mut self, id: u64, name: &str, args: Value) -> Value {
        self.send(call(id, name, args)).await;
        let result = self.recv().await;
        assert_eq!(result["id"], id);
        result
    }
    async fn close(self) {
        self.finish().await.unwrap();
    }
    async fn finish(self) -> Result<(), ServiceError> {
        self.stop.cancel();
        timeout(Duration::from_secs(3), self.task)
            .await
            .unwrap()
            .unwrap()
    }
}
fn call(id: u64, name: &str, args: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})
}
fn selection(s: &TestService, id: &str, args: Value) -> Value {
    json!({"operationRequestId":id,"catalog":s.catalog.reference(),"itemId":"diagnostics","variantId":"network-check","arguments":args})
}
fn value(reply: &Value) -> &Value {
    &reply["result"]["structuredContent"]["result"]
}
fn error(reply: &Value) -> &Value {
    &reply["result"]["structuredContent"]["error"]
}
async fn plan(w: &mut Wire, s: &TestService, id: &str) -> Value {
    let result = w
        .call(
            10,
            "execution_preview",
            json!({"catalog":{"selection":selection(s,id,json!({"host":"example.invalid"}))}}),
        )
        .await;
    assert_eq!(result["result"]["isError"], false, "{result}");
    value(&result)["plan"].clone()
}

#[tokio::test]
async fn discovery_schemas_and_shared_parameter_projection() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    w.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let list = w.recv().await;
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7);
    assert!(tools
        .iter()
        .all(|t| !t["name"].as_str().unwrap().contains("approve")));
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
        assert_eq!(tool["outputSchema"]["type"], "object", "{}", tool["name"]);
        jsonschema::draft202012::new(&tool["inputSchema"]).unwrap();
        jsonschema::draft202012::new(&tool["outputSchema"]).unwrap();
    }
    let catalog = w.call(3, "execution_catalog", json!({})).await;
    let expected = s
        .catalog
        .projection(
            &execution_contract::Id::new("diagnostics").unwrap(),
            &execution_contract::Id::new("network-check").unwrap(),
            &limits().parameters,
        )
        .unwrap();
    assert_eq!(
        &value(&catalog)["items"][0]["parameters"][0]["inputSchema"],
        expected.input_schema()
    );
    assert!(jsonschema::draft202012::is_valid(
        &tools[0]["outputSchema"],
        &catalog["result"]["structuredContent"]
    ));
    let schema = &tools
        .iter()
        .find(|t| t["name"] == "execution_submit")
        .unwrap()["inputSchema"];
    assert!(!jsonschema::draft202012::is_valid(
        schema,
        &json!({"plan":{}})
    ));
    let p = plan(&mut w, &s, "same-rules").await;
    let mut human_wire = selection(&s, "same-rules", json!({"host":"example.invalid"}));
    human_wire
        .as_object_mut()
        .unwrap()
        .remove("operationRequestId");
    let selected = s
        .catalog
        .select(
            &serde_json::to_vec(&human_wire).unwrap(),
            &limits().catalog,
            &limits().parameters,
        )
        .unwrap();
    let human = s
        .preview(
            PreviewRequest::Catalog(Box::new(CatalogCandidate {
                operation_request_id: execution_contract::RequestId::new("same-rules").unwrap(),
                selection: selected,
            })),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(p, serde_json::to_value(human.plan).unwrap());
    w.close().await;
}

#[tokio::test]
async fn raw_numbers_are_not_rounded_before_the_catalog_validates_them() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    for (i, numeric) in ["1.0000000000000001", "9007199254740990.9", "1e-999"]
        .iter()
        .enumerate()
    {
        let input = call(
            i as u64 + 20,
            "execution_preview",
            json!({"catalog":{"selection":selection(&s,"bad-number",json!({"host":"example.invalid","count":"NUMBER"}))}}),
        );
        w.raw(&input.to_string().replace("\"NUMBER\"", numeric))
            .await;
        let reply = w.recv().await;
        assert_eq!(error(&reply)["code"], "invalidInput", "{reply}");
        assert_eq!(
            error(&reply)["catalogReason"],
            json!({"kind":"invalidArguments","rule":"roundedNumber"})
        );
        assert!(!reply.to_string().contains(numeric));
    }
    for (i, args) in [
        json!({"host":7}),
        json!({"host":"example.invalid","count":99}),
        json!({"host":"example.invalid","actor":"admin"}),
    ]
    .into_iter()
    .enumerate()
    {
        let r = w
            .call(
                30 + i as u64,
                "execution_preview",
                json!({"catalog":{"selection":selection(&s,"bad",args)}}),
            )
            .await;
        assert_eq!(error(&r)["code"], "invalidInput");
    }
    w.close().await;
}

#[tokio::test]
async fn catalog_diagnostics_are_structured_closed_and_match_output_schemas() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    w.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let list = w.recv().await;
    let tools = list["result"]["tools"].as_array().unwrap();
    for (args, rule) in [
        (json!({}), "required"),
        (json!({"host":7}), "type"),
        (json!({"host":"example.invalid","count":99}), "range"),
        (
            json!({"host":"example.invalid","private":"diagnostic-canary"}),
            "unknownParameter",
        ),
    ] {
        let reply = w
            .call(
                3,
                "execution_preview",
                json!({"catalog":{"selection":selection(&s,"invalid",args)}}),
            )
            .await;
        assert_eq!(
            error(&reply)["catalogReason"],
            json!({"kind":"invalidArguments","rule":rule})
        );
        assert!(!reply.to_string().contains("diagnostic-canary"));
        for tool in tools {
            assert!(jsonschema::draft202012::is_valid(
                &tool["outputSchema"],
                &reply["result"]["structuredContent"]
            ));
        }
    }
    let mut missing = selection(&s, "missing", json!({"host":"example.invalid"}));
    missing["itemId"] = json!("unknown-item-canary");
    let reply = w
        .call(
            4,
            "execution_preview",
            json!({"catalog":{"selection":missing}}),
        )
        .await;
    assert_eq!(error(&reply)["catalogReason"], json!({"kind":"notFound"}));
    assert!(!reply.to_string().contains("unknown-item-canary"));
    for tool in tools {
        let schema = jsonschema::draft202012::new(&tool["outputSchema"]).unwrap();
        assert!(schema.is_valid(&reply["result"]["structuredContent"]));
        for invalid in [
            json!("InvalidArguments(RoundedNumber)"),
            json!({"kind":"unrecognized"}),
            json!({"kind":"invalidArguments"}),
            json!({"kind":"invalidArguments","rule":"parameterTitle"}),
            json!({"kind":"invalidArguments","rule":"required","raw":"diagnostic-canary"}),
            json!({"kind":"limitExceeded","coordinate":"raw-field-name"}),
            json!({"kind":"notFound","rule":"required"}),
        ] {
            let output =
                json!({"status":"error","error":{"code":"invalidInput","catalogReason":invalid}});
            assert!(!schema.is_valid(&output), "schema accepted {output}");
        }
    }
    w.close().await;

    let mut l = limits();
    l.parameters.max_parameters = 1;
    let mut w = Wire::open(s, l).await;
    let reply = w.call(2, "execution_catalog", json!({})).await;
    assert_eq!(
        error(&reply)["catalogReason"],
        json!({"kind":"limitExceeded","coordinate":"parameters"})
    );
    assert!(jsonschema::draft202012::is_valid(
        &tools[0]["outputSchema"],
        &reply["result"]["structuredContent"]
    ));
    w.close().await;
}

#[tokio::test]
async fn trusted_context_cannot_be_forged_and_query_authorization_is_rechecked() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let p = plan(&mut w, &s, "owned").await;
    let args = json!({"operationRequestId":"owned","plan":p});
    let accepted = w.call(20, "execution_submit", args.clone()).await;
    assert_eq!(value(&accepted)["phase"], "accepted");
    for key in [
        "actor",
        "tenant",
        "device",
        "delegation",
        "approved",
        "readOnlyHint",
    ] {
        let mut forged = args.clone();
        forged[key] = json!("injected-sensitive-value");
        let r = w.call(21, "execution_submit", forged).await;
        assert_eq!(error(&r)["code"], "invalidInput");
        assert!(!r.to_string().contains("injected-sensitive-value"));
    }
    // Protocol metadata never becomes trusted context.
    let mut msg = call(22, "execution_submit", args);
    msg["params"]["_meta"] = json!({"actor":"admin","approved":true});
    w.send(msg).await;
    assert_eq!(value(&w.recv().await)["phase"], "accepted");
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
    s.denied.store(true, Ordering::SeqCst);
    for name in [
        "execution_catalog",
        "execution_capabilities",
        "execution_status",
        "execution_cancel",
    ] {
        let args = if name.ends_with("status") || name.ends_with("cancel") {
            json!({"operationRequestId":"owned"})
        } else {
            json!({})
        };
        let r = w.call(23, name, args).await;
        assert_eq!(error(&r)["code"], "denied");
    }
    w.close().await;
}

#[tokio::test]
async fn accepted_response_loss_reconnect_and_concurrent_retry_preserve_one_attempt() {
    let s = service();
    let mut l = limits();
    l.request_timeout = Duration::from_millis(60);
    let mut w = Wire::open(s.clone(), l.clone()).await;
    let p = plan(&mut w, &s, "retry").await;
    let args = json!({"operationRequestId":"retry","plan":p});
    s.submit_delay_ms.store(500, Ordering::SeqCst);
    let unknown = w.call(20, "execution_submit", args.clone()).await;
    assert_eq!(error(&unknown)["code"], "outcomeUnknown");
    assert_eq!(s.active_waits.load(Ordering::SeqCst), 0);
    w.close().await;
    s.submit_delay_ms.store(0, Ordering::SeqCst);
    let mut w = Wire::open(s.clone(), l).await;
    let status = w
        .call(2, "execution_status", json!({"operationRequestId":"retry"}))
        .await;
    assert_eq!(value(&status)["phase"], "accepted");
    for id in 30..34 {
        w.send(call(id, "execution_submit", args.clone())).await;
    }
    for _ in 30..34 {
        let r = w.recv().await;
        assert_eq!(value(&r)["attemptId"], "test-attempt");
        assert_eq!(value(&r)["phase"], "accepted");
    }
    let mut conflict = args.clone();
    conflict["plan"]["digest"] = json!("a".repeat(64));
    assert_eq!(
        error(&w.call(40, "execution_submit", conflict).await)["code"],
        "conflict"
    );
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
    w.close().await;
}

#[tokio::test]
async fn bound_namespace_prevents_cross_actor_tenant_device_and_delegation_replay() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let p = plan(&mut w, &s, "same-id").await;
    w.call(
        20,
        "execution_submit",
        json!({"operationRequestId":"same-id","plan":p}),
    )
    .await;
    for dimension in ["authority", "actor", "tenant", "device", "delegation"] {
        let mut other = TestService::new(CATALOG, PLAN);
        let field = match dimension {
            "authority" => &mut other.namespace.authority,
            "actor" => &mut other.namespace.actor,
            "tenant" => &mut other.namespace.tenant,
            "device" => &mut other.namespace.device,
            "delegation" => &mut other.namespace.delegation,
            _ => unreachable!(),
        };
        *field = format!("other-{dimension}");
        other.store = s.store.clone();
        let mut alien = Wire::open(Arc::new(other), limits()).await;
        assert_eq!(
            error(
                &alien
                    .call(
                        2,
                        "execution_status",
                        json!({"operationRequestId":"same-id"})
                    )
                    .await
            )["code"],
            "notFound"
        );
        assert_eq!(
            error(
                &alien
                    .call(
                        3,
                        "execution_submit",
                        json!({"operationRequestId":"same-id","plan":p})
                    )
                    .await
            )["code"],
            "notFound"
        );
        alien.close().await;
    }
    w.close().await;
}

#[tokio::test]
async fn protocol_cancellation_releases_wait_and_does_not_cancel_business_operation() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let p = plan(&mut w, &s, "cancel-wait").await;
    s.submit_delay_ms.store(1000, Ordering::SeqCst);
    w.send(call(
        20,
        "execution_submit",
        json!({"operationRequestId":"cancel-wait","plan":p}),
    ))
    .await;
    timeout(Duration::from_secs(1), async {
        while s.attempts.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    w.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":20,"reason":"untrusted sensitive reason"}})).await;
    timeout(Duration::from_secs(1), async {
        while s.active_waits.load(Ordering::SeqCst) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let status = w
        .call(
            21,
            "execution_status",
            json!({"operationRequestId":"cancel-wait"}),
        )
        .await;
    assert_eq!(value(&status)["phase"], "accepted");
    let cancelled = w
        .call(
            22,
            "execution_cancel",
            json!({"operationRequestId":"cancel-wait"}),
        )
        .await;
    assert_eq!(value(&cancelled)["disposition"], "requested");
    assert_eq!(value(&cancelled)["operation"]["phase"], "accepted");
    w.close().await;
}

#[tokio::test]
async fn script_candidates_are_immutable_bounded_and_never_echoed() {
    let s = service();
    let mut w = Wire::open(s, limits()).await;
    let interpreter =
        json!({"resource":{"id":"test-interpreter","revision":"1"},"sha256":"b".repeat(64)});
    let args = json!({"script":{"operationRequestId":"script","sourceUtf8":"sensitive-script-source","interpreter":interpreter}});
    let candidate = w.call(20, "execution_propose", args.clone()).await;
    assert_eq!(candidate["result"]["isError"], false);
    assert!(!candidate.to_string().contains("sensitive-script-source"));
    let duplicate = w.call(21, "execution_propose", args.clone()).await;
    assert_eq!(value(&candidate), value(&duplicate));
    let mut changed = args.clone();
    changed["script"]["sourceUtf8"] = json!("changed");
    assert_eq!(
        error(&w.call(22, "execution_propose", changed).await)["code"],
        "conflict"
    );
    let preview = w.call(23, "execution_preview", json!({"candidate":{"operationRequestId":"script","candidate":value(&candidate)["candidate"]}})).await;
    assert_eq!(preview["result"]["isError"], false);
    assert!(!preview.to_string().contains("sensitive-script-source"));
    let mut large = args;
    large["script"]["sourceUtf8"] = json!("x".repeat(limits().parameters.max_bytes + 1));
    assert_eq!(
        error(&w.call(24, "execution_propose", large).await)["code"],
        "invalidInput"
    );
    w.close().await;
}

#[tokio::test]
async fn input_failures_close_without_dispatch_or_raw_error_echo() {
    for bad in [
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"execution_capabilities","arguments":{"x":1,"x":2}}}"#.to_owned(),
        format!(r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"execution_capabilities","arguments":{{"x":{}0{}}}}}}}"#, "[".repeat(40), "]".repeat(40)),
        r#"{"jsonrpc":"2.0","method":"notifications/roots/list_changed"}"#.to_owned(),
    ] {
        let s = service();
        let mut w = Wire::open(s.clone(), limits()).await;
        w.raw(&bad).await;
        let mut response = String::new();
        assert_eq!(timeout(Duration::from_secs(2), w.output.read_line(&mut response)).await.unwrap().unwrap(), 0);
        assert_eq!(s.calls.load(Ordering::SeqCst), 0);
        assert_eq!(w.finish().await, Err(ServiceError::InvalidInput));
    }
}

#[tokio::test]
async fn concurrency_is_admitted_before_sdk_dispatch_and_busy_is_bounded() {
    let s = service();
    s.capability_delay_ms.store(150, Ordering::SeqCst);
    let mut l = limits();
    l.in_flight = 1;
    let mut w = Wire::open(s.clone(), l).await;
    w.send(call(20, "execution_capabilities", json!({}))).await;
    timeout(Duration::from_secs(1), async {
        while s.active_waits.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    for id in 21..25 {
        w.send(call(id, "execution_capabilities", json!({}))).await;
    }
    for _ in 21..25 {
        let busy = w.recv().await;
        assert!(busy["error"].is_object(), "{busy}");
    }
    assert_eq!(w.recv().await["id"], 20);
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    w.close().await;
}

#[tokio::test]
async fn response_budget_write_timeout_and_unterminated_input_are_bounded() {
    let s = service();
    let mut l = limits();
    l.response_bytes = 1024;
    let mut w = Wire::open(s, l).await;
    w.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let r = w.recv().await;
    assert!(r["error"].is_object());
    assert!(r.to_string().len() < 1024);
    w.close().await;
    let s = service();
    let mut l = limits();
    l.frame_bytes = 256;
    l.io_timeout = Duration::from_millis(40);
    let mut w = Wire::open(s.clone(), l).await;
    w.input.write_all(&vec![b' '; 300]).await.unwrap();
    let mut line = String::new();
    assert_eq!(
        timeout(Duration::from_secs(1), w.output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert_eq!(w.finish().await, Err(ServiceError::Limit));
    let mut l = limits();
    l.io_timeout = Duration::from_millis(40);
    let mut w = Wire::open(s.clone(), l).await;
    w.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    // Do not read the large response: 1024-byte duplex cannot hold it.
    assert_eq!(
        timeout(Duration::from_secs(1), &mut w.task)
            .await
            .unwrap()
            .unwrap(),
        Err(ServiceError::Unavailable)
    );
    assert_eq!(s.active_waits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn official_rmcp_client_can_consume_the_bounded_service() {
    use rmcp::ServiceExt;
    let (client, server) = tokio::io::duplex(1024);
    let (r, w) = tokio::io::split(server);
    let task = tokio::spawn(ExecutionMcp::new(service(), limits()).unwrap().serve(
        r,
        w,
        CancellationToken::new(),
    ));
    let client = ().serve(client).await.unwrap();
    assert_eq!(
        client
            .peer()
            .list_tools(Default::default())
            .await
            .unwrap()
            .tools
            .len(),
        7
    );
    client.cancel().await.unwrap();
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[test]
fn unbound_identity_and_invalid_host_limits_reject_startup() {
    let s = service();
    s.bound.store(false, Ordering::SeqCst);
    assert!(matches!(
        ExecutionMcp::new(s, limits()),
        Err(ServiceError::Unbound)
    ));
    let mut l = limits();
    l.in_flight = 0;
    assert!(matches!(
        ExecutionMcp::new(service(), l),
        Err(ServiceError::InvalidLimits)
    ));
}

#[tokio::test]
async fn stale_catalog_and_terminal_test_evidence_remain_distinct() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let mut stale = selection(&s, "stale", json!({"host":"example.invalid"}));
    stale["catalog"]["digest"] = json!("b".repeat(64));
    assert_eq!(
        error(
            &w.call(
                2,
                "execution_preview",
                json!({"catalog":{"selection":stale}})
            )
            .await
        )["code"],
        "expired"
    );
    let p = plan(&mut w, &s, "terminal").await;
    w.call(
        20,
        "execution_submit",
        json!({"operationRequestId":"terminal","plan":p}),
    )
    .await;
    s.complete_test_result("terminal");
    let result = w
        .call(
            21,
            "execution_cancel",
            json!({"operationRequestId":"terminal"}),
        )
        .await;
    assert_eq!(value(&result)["disposition"], "alreadyTerminal");
    assert_eq!(value(&result)["operation"]["phase"], "testCompleted");
    assert_eq!(value(&result)["operation"]["evidence"][0], "test-result");
    w.close().await;
}

#[tokio::test]
async fn session_stop_drops_pending_service_waits_and_frame_budget_is_enforced() {
    let s = service();
    s.capability_delay_ms.store(1000, Ordering::SeqCst);
    let mut w = Wire::open(s.clone(), limits()).await;
    w.send(call(2, "execution_capabilities", json!({}))).await;
    timeout(Duration::from_secs(1), async {
        while s.active_waits.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    w.close().await;
    assert_eq!(s.active_waits.load(Ordering::SeqCst), 0);

    let mut l = limits();
    l.session_frames = 2; // initialize request and initialized notification
    let mut w = Wire::open(service(), l).await;
    w.send(call(2, "execution_capabilities", json!({}))).await;
    let mut response = String::new();
    assert_eq!(
        timeout(Duration::from_secs(1), w.output.read_line(&mut response))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert_eq!(w.finish().await, Err(ServiceError::Limit));
}

#[tokio::test]
async fn clean_eof_and_read_timeout_have_distinct_host_results() {
    let mut w = Wire::open(service(), limits()).await;
    w.input.shutdown().await.unwrap();
    assert_eq!(
        timeout(Duration::from_secs(1), w.task)
            .await
            .unwrap()
            .unwrap(),
        Ok(())
    );

    let mut l = limits();
    l.io_timeout = Duration::from_millis(30);
    let mut w = Wire::open(service(), l).await;
    w.input.write_all(b"{\"partial\"").await.unwrap();
    assert_eq!(
        timeout(Duration::from_secs(1), w.task)
            .await
            .unwrap()
            .unwrap(),
        Err(ServiceError::Unavailable)
    );
}

#[tokio::test]
async fn failed_output_writer_returns_a_static_host_error() {
    struct BrokenWriter;
    impl tokio::io::AsyncWrite for BrokenWriter {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            std::task::Poll::Ready(Err(std::io::Error::other("private writer diagnostic")))
        }
        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }
    let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n";
    let result = timeout(
        Duration::from_secs(1),
        ExecutionMcp::new(service(), limits()).unwrap().serve(
            &input[..],
            BrokenWriter,
            CancellationToken::new(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(result, Err(ServiceError::Unavailable));
}

#[tokio::test]
async fn every_tool_advertises_and_times_out_according_to_its_effects() {
    let s = service();
    let mut l = limits();
    l.request_timeout = Duration::from_millis(100);
    let mut w = Wire::open(s.clone(), l).await;
    let p = plan(&mut w, &s, "original-id").await;
    let submit = json!({"operationRequestId":"original-id","plan":p});
    assert_eq!(
        value(&w.call(11, "execution_submit", submit.clone()).await)["phase"],
        "accepted"
    );
    w.send(json!({"jsonrpc":"2.0","id":12,"method":"tools/list","params":{}}))
        .await;
    let list = w.recv().await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let selected =
        json!({"catalog":{"selection":selection(&s,"delayed",json!({"host":"example.invalid"}))}});
    let operation = json!({"operationRequestId":"original-id"});
    let cases = [
        ("execution_catalog", json!({}), true, "unavailable"),
        ("execution_capabilities", json!({}), true, "unavailable"),
        ("execution_status", operation.clone(), true, "unavailable"),
        (
            "execution_propose",
            selected.clone(),
            false,
            "outcomeUnknown",
        ),
        ("execution_submit", submit, false, "outcomeUnknown"),
        ("execution_cancel", operation, false, "outcomeUnknown"),
        ("execution_preview", selected, false, "outcomeUnknown"),
    ];
    assert_eq!(tools.len(), cases.len());
    s.catalog_delay_ms.store(1000, Ordering::SeqCst);
    s.capability_delay_ms.store(1000, Ordering::SeqCst);
    s.status_delay_ms.store(1000, Ordering::SeqCst);
    s.submit_delay_ms.store(1000, Ordering::SeqCst);
    for (name, args, read_only, code) in cases {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        let reply = w.call(20, name, args).await;
        assert_eq!(error(&reply)["code"], code, "{name}");
        assert!(jsonschema::draft202012::is_valid(
            &tool["outputSchema"],
            &reply["result"]["structuredContent"]
        ));
        assert_eq!(tool["annotations"]["readOnlyHint"], read_only, "{name}");
    }
    w.close().await;
}

#[tokio::test]
async fn interrupted_frame_reads_resume_without_losing_bytes() {
    let s = service();
    s.capability_delay_ms.store(20, Ordering::SeqCst);
    let mut w = Wire::open(s, limits()).await;
    w.send(call(2, "execution_capabilities", json!({}))).await;
    let next = call(3, "execution_capabilities", json!({})).to_string();
    let middle = next.len() / 2;
    w.input.write_all(&next.as_bytes()[..middle]).await.unwrap();
    // Outgoing response interrupts rmcp's receive future with half a frame buffered.
    assert_eq!(w.recv().await["id"], 2);
    w.raw(&next[middle..]).await;
    assert_eq!(w.recv().await["id"], 3);
    w.close().await;
}

#[tokio::test]
async fn sdk_tracing_never_receives_raw_tool_arguments_metadata_or_cancel_reason() {
    use std::sync::Mutex;
    use tracing::{field::Visit, span, Event, Metadata, Subscriber};
    #[derive(Clone)]
    struct Capture(Arc<Mutex<String>>);
    struct Fields<'a>(&'a mut String);
    impl Visit for Fields<'_> {
        fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            use std::fmt::Write;
            writeln!(self.0, "{value:?}").unwrap();
        }
    }
    impl Subscriber for Capture {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, attrs: &span::Attributes<'_>) -> span::Id {
            attrs.record(&mut Fields(&mut self.0.lock().unwrap()));
            span::Id::from_u64(1)
        }
        fn record(&self, _: &span::Id, values: &span::Record<'_>) {
            values.record(&mut Fields(&mut self.0.lock().unwrap()));
        }
        fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
        fn event(&self, event: &Event<'_>) {
            event.record(&mut Fields(&mut self.0.lock().unwrap()));
        }
        fn enter(&self, _: &span::Id) {}
        fn exit(&self, _: &span::Id) {}
    }
    let logs = Arc::new(Mutex::new(String::new()));
    // This integration-test binary installs its only subscriber once. Capture TRACE,
    // including SDK tasks, instead of trusting an unused RUST_LOG environment value.
    tracing::subscriber::set_global_default(Capture(logs.clone())).unwrap();
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let mut request = call(
        2,
        "execution_propose",
        json!({"script":{
            "operationRequestId":"log-test", "sourceUtf8":"script-log-canary",
            "interpreter":{"resource":{"id":"test-interpreter","revision":"1"},"sha256":"b".repeat(64)}
        }}),
    );
    request["params"]["_meta"] = json!({"private":"metadata-log-canary"});
    w.send(request).await;
    assert_eq!(w.recv().await["result"]["isError"], false);
    let reply = w.call(3, "execution_preview", json!({"catalog":{"selection":selection(
        &s, "log-secret", json!({"host":"example.invalid","credential":{"kind":"secret","reference":{"id":"secret-log-canary","revision":"1"}}})
    )}})).await;
    assert!(reply["result"].is_object());
    s.capability_delay_ms.store(1000, Ordering::SeqCst);
    w.send(call(4, "execution_capabilities", json!({}))).await;
    timeout(Duration::from_secs(1), async {
        while s.active_waits.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    w.send(
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{
            "requestId":4,"reason":"cancel-log-canary","_meta":{"private":"notification-log-canary"}
        }}),
    )
    .await;
    timeout(Duration::from_secs(1), async {
        while s.active_waits.load(Ordering::SeqCst) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    w.close().await;
    let captured = logs.lock().unwrap();
    assert!(
        captured.contains("received request"),
        "SDK capture must be active"
    );
    for marker in [
        "script-log-canary",
        "secret-log-canary",
        "metadata-log-canary",
        "cancel-log-canary",
        "notification-log-canary",
    ] {
        assert!(!captured.contains(marker), "SDK leaked {marker}");
    }
}

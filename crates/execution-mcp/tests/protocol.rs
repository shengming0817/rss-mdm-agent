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

fn service() -> Arc<TestService> {
    Arc::new(TestService::new())
}
struct Wire {
    input: WriteHalf<DuplexStream>,
    output: BufReader<ReadHalf<DuplexStream>>,
    task: JoinHandle<Result<(), ServiceError>>,
    stop: CancellationToken,
}
impl Wire {
    async fn tools(&mut self, id: u64) -> Vec<Value> {
        let mut found = Vec::new();
        let mut cursor = Value::Null;
        for _ in 0..32 {
            self.send(
                json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{"cursor":cursor}}),
            )
            .await;
            let response = self.recv().await;
            found.extend(
                response["result"]["tools"]
                    .as_array()
                    .expect("bounded tool page")
                    .iter()
                    .cloned(),
            );
            cursor = response["result"]["nextCursor"].clone();
            if cursor.is_null() {
                return found;
            }
        }
        panic!("tool metadata did not advance")
    }
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
fn selection(_: &TestService, id: &str, _: Value) -> Value {
    json!({"task":"test-task","attempt":"test-attempt","revision":"a".repeat(64),"request":id})
}
fn control_args(request: &str) -> Value {
    json!({"reference":{"kind":"request","request":request},"id":"original-control","scope":"test-scope","expectedRevision":{"owner":0,"transfers":[]},"action":"cancel"})
}
fn value(reply: &Value) -> &Value {
    let result = &reply["result"]["structuredContent"]["result"];
    if result.get("facts").is_some() {
        return &result["facts"]["value"]["status"];
    }
    if result["kind"] == "control" {
        return &result["value"];
    }
    result
}
fn error(reply: &Value) -> &Value {
    &reply["result"]["structuredContent"]["error"]
}
async fn action_input(_: &mut Wire, s: &TestService, id: &str) -> Value {
    selection(s, id, json!({}))
}

#[tokio::test]
async fn discovery_exposes_backend_references_and_rejects_old_script_and_catalog_inputs() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let tools = w.tools(2).await;
    assert_eq!(tools.len(), 19);
    for tool in &tools {
        jsonschema::draft202012::new(&tool["inputSchema"]).unwrap();
        jsonschema::draft202012::new(&tool["outputSchema"]).unwrap();
    }
    let tasks = w.call(3, "execution_offers", json!({})).await;
    assert_eq!(value(&tasks)[0]["task"], "test-task");
    for old in [
        json!({"script":{"sourceUtf8":"private-script","operationRequestId":"new","interpreter":{}}}),
        json!({"catalog":{"selection":{}}}),
        json!({"task":"test-task","attempt":"test-attempt","revision":"a".repeat(64),"request":"r","approved":true}),
    ] {
        let reply = w.call(4, "execution_execute", old).await;
        assert_eq!(error(&reply)["code"], "invalidInput");
        assert!(!reply.to_string().contains("private-script"));
    }
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
    w.close().await;
}
#[tokio::test]
async fn trusted_context_cannot_be_forged_and_query_authorization_is_rechecked() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let p = action_input(&mut w, &s, "owned").await;
    let args = p.clone();
    let accepted = w.call(20, "execution_execute", args.clone()).await;
    assert_eq!(value(&accepted)["confirmationRequired"], true);
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
        let r = w.call(21, "execution_execute", forged).await;
        assert_eq!(error(&r)["code"], "invalidInput");
        assert!(!r.to_string().contains("injected-sensitive-value"));
    }
    // Protocol metadata never becomes trusted context.
    let mut msg = call(22, "execution_execute", args);
    msg["params"]["_meta"] = json!({"actor":"admin","approved":true});
    w.send(msg).await;
    assert_eq!(value(&w.recv().await)["confirmationRequired"], true);
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
    s.denied.store(true, Ordering::SeqCst);
    for name in [
        "execution_offers",
        "execution_capabilities",
        "execution_task_detail",
        "execution_task_control",
    ] {
        let args = if name == "execution_task_control" {
            control_args("owned")
        } else if name == "execution_task_detail" {
            json!({"kind":"request","request":"owned"})
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
    let p = action_input(&mut w, &s, "retry").await;
    let args = p.clone();
    s.submit_delay_ms.store(500, Ordering::SeqCst);
    let unknown = w.call(20, "execution_execute", args.clone()).await;
    assert_eq!(error(&unknown)["code"], "outcomeUnknown");
    assert_eq!(s.active_waits.load(Ordering::SeqCst), 0);
    w.close().await;
    s.submit_delay_ms.store(0, Ordering::SeqCst);
    let mut w = Wire::open(s.clone(), l).await;
    let status = w
        .call(
            2,
            "execution_task_detail",
            json!({"kind":"request","request":"retry"}),
        )
        .await;
    assert_eq!(value(&status)["phase"], "accepted");
    for id in 30..34 {
        w.send(call(id, "execution_execute", args.clone())).await;
    }
    for _ in 30..34 {
        let r = w.recv().await;
        assert_eq!(value(&r)["request"], "retry");
        assert_eq!(value(&r)["confirmationRequired"], true);
    }
    let mut conflict = args.clone();
    conflict["attempt"] = json!("other-attempt");
    assert_eq!(
        error(&w.call(40, "execution_execute", conflict).await)["code"],
        "conflict"
    );
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
    w.close().await;
}

#[tokio::test]
async fn bound_namespace_prevents_cross_actor_tenant_device_and_delegation_replay() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let p = action_input(&mut w, &s, "same-id").await;
    w.call(20, "execution_execute", p.clone()).await;
    for dimension in ["authority", "actor", "tenant", "device", "delegation"] {
        let mut other = TestService::new();
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
                        "execution_task_detail",
                        json!({"kind":"request","request":"same-id"})
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
    let p = action_input(&mut w, &s, "cancel-wait").await;
    s.submit_delay_ms.store(1000, Ordering::SeqCst);
    w.send(call(20, "execution_execute", p.clone())).await;
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
            "execution_task_detail",
            json!({"kind":"request","request":"cancel-wait"}),
        )
        .await;
    assert_eq!(value(&status)["phase"], "accepted");
    let cancelled = w
        .call(22, "execution_task_control", control_args("cancel-wait"))
        .await;
    assert_eq!(value(&cancelled)["disposition"], "confirmed");
    assert_eq!(value(&cancelled)["id"], "original-control");
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
    let mut found = Vec::new();
    let mut cursor = None;
    loop {
        let page = client
            .peer()
            .list_tools(cursor.clone().map(|cursor| {
                let mut p = rmcp::model::PaginatedRequestParams::default();
                p.cursor = Some(cursor);
                p
            }))
            .await
            .unwrap();
        found.extend(page.tools);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(found.len(), 19);
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
    stale["revision"] = json!("b".repeat(64));
    assert_eq!(
        error(&w.call(2, "execution_execute", stale).await)["code"],
        "expired"
    );
    let p = action_input(&mut w, &s, "terminal").await;
    w.call(20, "execution_execute", p.clone()).await;
    s.complete_test_result("terminal");
    let result = w
        .call(21, "execution_task_control", control_args("terminal"))
        .await;
    assert_eq!(value(&result)["disposition"], "confirmed");
    let original = w
        .call(
            22,
            "execution_task_detail",
            json!({"kind":"request","request":"terminal"}),
        )
        .await;
    assert_eq!(value(&original)["phase"], "verified");
    assert_eq!(value(&original)["mode"], "test");
    assert_eq!(
        value(&original)["evidence"][0]["reference"]["id"],
        "test-result"
    );
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
    let p = action_input(&mut w, &s, "original-id").await;
    let submit = p.clone();
    assert_eq!(
        value(&w.call(11, "execution_execute", submit.clone()).await)["confirmationRequired"],
        true
    );
    let tools = w.tools(12).await;
    let operation = json!({"operationRequestId":"original-id"});
    let cases = [
        (
            "execution_policy_invoke",
            json!({"selector":{"kind":"alias","alias":"repair.app"},"parameters":{}}),
            false,
            "outcomeUnknown",
        ),
        (
            "execution_software_read",
            json!({"read":{"selection":{"kind":"query","query":{"id":"native:test","revision":"a".repeat(64)}},"after":null,"limit":16,"maxAgeSeconds":null}}),
            true,
            "unavailable",
        ),
        (
            "execution_software_prepare",
            json!({"software":{"query":{"id":"native:test","revision":"a".repeat(64)},"installation":"test-app"},"action":"uninstall"}),
            false,
            "outcomeUnknown",
        ),
        (
            "execution_software_status",
            json!({"request":"original-id"}),
            true,
            "unavailable",
        ),
        (
            "execution_task_list",
            json!({"view":"history","source":null,"taskKind":null,"status":null,"from":null,"until":null,"subject":null,"policy":null,"cursor":null,"limit":16}),
            true,
            "unavailable",
        ),
        (
            "execution_query_directory",
            json!({"after":null,"limit":16,"refreshPolicies":false,"policyCursor":null}),
            true,
            "unavailable",
        ),
        (
            "execution_data_read",
            json!({"selection":{"kind":"request","request":"original-id"},"after":null,"limit":16,"maxAgeSeconds":null}),
            true,
            "unavailable",
        ),
        (
            "execution_query_submit",
            json!({"request":"original-id","query":{"id":"native:test","revision":"a".repeat(64)},"parameters":{}}),
            false,
            "outcomeUnknown",
        ),
        (
            "execution_query_status",
            json!({"request":"original-id"}),
            true,
            "unavailable",
        ),
        (
            "execution_task_control_status",
            json!({"reference":{"kind":"request","request":"original-id"},"id":"original-control","scope":"test-scope"}),
            true,
            "unavailable",
        ),
        (
            "execution_operation_prepare",
            json!({"scene":"policy.parameters.v1", "selector":{"kind":"alias","alias":"repair.app"},  "parameters":{}}),
            false,
            "outcomeUnknown",
        ),
        (
            "execution_policy_tools",
            json!({"after":null}),
            true,
            "unavailable",
        ),
        (
            "execution_policy_status",
            operation.clone(),
            true,
            "unavailable",
        ),
        (
            "execution_software_observe",
            json!({"selection": p, "step": 0}),
            true,
            "unavailable",
        ),
        ("execution_offers", json!({}), true, "unavailable"),
        ("execution_capabilities", json!({}), true, "unavailable"),
        (
            "execution_task_detail",
            json!({"kind":"request","request":"original-id"}),
            true,
            "unavailable",
        ),
        ("execution_execute", submit, false, "outcomeUnknown"),
        (
            "execution_task_control",
            control_args("original-id"),
            false,
            "outcomeUnknown",
        ),
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
        "execution_execute",
        json!({"script":{
            "operationRequestId":"log-test", "sourceUtf8":"script-log-canary",
            "interpreter":{"resource":{"id":"test-interpreter","revision":"1"},"sha256":"b".repeat(64)}
        }}),
    );
    request["params"]["_meta"] = json!({"private":"metadata-log-canary"});
    w.send(request).await;
    assert_eq!(w.recv().await["result"]["isError"], true);
    let reply = w.call(3, "execution_execute", json!({"task":"test-task","attempt":"test-attempt","revision":"a".repeat(64),"request":"log-secret","credential":"secret-log-canary"})).await;
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

#[test]
fn policy_mcp_descriptor_preserves_resource_input_constraints_and_local_references() {
    use rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem;
    let mut item: SelfServiceDirectoryItem = serde_json::from_value(json!({
        "capability":{"id":"00000000-0000-4000-a000-000000000001","objectType":"policy","toolName":"policy_0000000000004000a000000000000001","alias":null,"scope":"00000000-0000-4000-a000-000000000003","instance":"fixture","tenant":"00000000-0000-4000-a000-000000000004","revision":3,"version":"00000000-0000-4000-a000-000000000002","operations":["invoke"],"inputSchema":{"type":"object","$defs":{"name":{"type":"string","maxLength":32}},"properties":{"name":{"$ref":"#/$defs/name"}},"required":["name"],"additionalProperties":false},"outputSchema":{}},
        "policyId":"00000000-0000-4000-a000-000000000001","policyRevision":3,"policyVersion":"00000000-0000-4000-a000-000000000002",
            "subject":{"kind":"device"},
        "selfService":{"published": true, "allowAi": true, "riskLevel": 2, "presentation": {"displayName": "Repair", "description": "Repair app", "prerequisites": "Employee", "sideEffects": "Restart", "category": "Support", "keywords": []}},
        "task":{"kind":"script","runAs":"system","resource":{"id":"r","version":"v1","variant":"default"},
        "parameterSources":{"name":{"kind":"input"},"path":{"kind":"fixed","value":"/Applications"}},
        "parameterSchema":{"type":"object","properties":{"name":{"type":"string","maxLength":32},"path":{"type":"string"}},"required":["name","path"],"additionalProperties":false}}
    })).unwrap();
    let descriptor = policy_tool(&item).unwrap();
    let validator = jsonschema::validator_for(&descriptor["inputSchema"]).unwrap();
    assert!(validator.is_valid(&json!({"parameters":{"name":"Acme"}})));
    assert!(!validator.is_valid(&json!({"parameters":{"name":"Acme","path":"/Library"}})));
    assert!(!validator.is_valid(&json!({"parameters":{"name":"x".repeat(33)}})));
    assert!(!validator.is_valid(&json!({})));
    assert_eq!(descriptor["annotations"]["readOnlyHint"], false);
    fn parameter_schema(item: &mut SelfServiceDirectoryItem) -> &mut serde_json::Value {
        &mut item.capability.input_schema
    }
    let saved = parameter_schema(&mut item).clone();
    parameter_schema(&mut item)["properties"]["name"] =
        json!({"type":"array","items":{"type":"integer","minimum":i64::MIN,"maximum":i64::MAX}});
    let descriptor = policy_tool(&item).unwrap();
    assert_eq!(
        descriptor["inputSchema"]["properties"]["parameters"]["properties"]["name"]["items"]
            ["maximum"],
        9_007_199_254_740_991_i64
    );
    assert_eq!(
        descriptor["inputSchema"]["properties"]["parameters"]["properties"]["name"]["items"]
            ["minimum"],
        -9_007_199_254_740_991_i64
    );
    assert_eq!(
        parameter_schema(&mut item)["properties"]["name"]["items"]["maximum"],
        i64::MAX
    );
    for unsupported in [
        json!({"type":"integer","minimum":i64::MAX}),
        json!({"type":"integer","const":i64::MAX}),
        json!({"type":"integer","enum":[1,i64::MAX]}),
    ] {
        parameter_schema(&mut item)["properties"]["name"] = unsupported;
        assert!(matches!(
            policy_tool(&item),
            Err(ServiceError::InvalidInput)
        ));
    }
    *parameter_schema(&mut item) = saved;
    let original = policy_tool_name(&item);
    item.policy_revision += 1;
    assert_eq!(policy_tool_name(&item), original);
    item.self_service.allow_ai = false;
    assert!(matches!(policy_tool(&item), Err(ServiceError::Denied)));
    item.self_service.allow_ai = true;
    item.self_service.risk_level = 0;
    assert!(matches!(policy_tool(&item), Err(ServiceError::Denied)));
    item.self_service.risk_level = 3;
    assert!(matches!(policy_tool(&item), Err(ServiceError::Denied)));
}

#[tokio::test]
async fn data_tools_preserve_original_request_and_typed_results_through_real_mcp() {
    let s = service();
    let mut w = Wire::open(s.clone(), limits()).await;
    let directory = w
        .call(
            2,
            "execution_query_directory",
            json!({"after":null,"limit":16,"refreshPolicies":false,"policyCursor":null}),
        )
        .await;
    assert_eq!(value(&directory)["kind"], "directory");
    let query = value(&directory)["value"]["items"][0]["reference"].clone();
    let request = "11111111-1111-4111-a111-111111111111";
    let submit = json!({"request":request,"query":query,"parameters":{}});
    for n in 3..5 {
        let result = w.call(n, "execution_query_submit", submit.clone()).await;
        assert_eq!(value(&result)["kind"], "status");
        assert_eq!(value(&result)["value"]["request"], request);
    }
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
    let status = w
        .call(5, "execution_query_status", json!({"request":request}))
        .await;
    assert_eq!(value(&status)["value"]["phase"], "completed");
    let read = w.call(6, "execution_data_read", json!({"selection":{"kind":"request","request":request},"after":null,"limit":16,"maxAgeSeconds":null})).await;
    assert_eq!(value(&read)["kind"], "read");
    assert_eq!(value(&read)["value"]["rows"][0]["fixture"], true);
    let cancelled = w
        .call(7, "execution_task_control", control_args(request))
        .await;
    assert_eq!(value(&cancelled)["disposition"], "confirmed");
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
    w.close().await;
}

#[tokio::test]
async fn local_software_status_preserves_verified_state_in_the_real_mcp_response() {
    use execution_contract::*;
    let s = service();
    let request = RequestId::new("local-detection").unwrap();
    let prepared: LocalSoftwarePrepared = serde_json::from_value(json!({
        "request":request,"digest":"a".repeat(64),
        "choice":{"request":request,"software":{"query":{"id":"native:apps","revision":"b".repeat(64)},"installation":"app"},"action":"detect"},
        "name":"App","initiator":{"kind":"human","osSession":{"device":"device","account":{"platform":"macos","subject":"uid:501"},"session":"login"}},
        "identity":"system","targetVersion":null,"startBeforeUnixMs":1000
    })).unwrap();
    let mut w = Wire::open(s.clone(), limits()).await;
    w.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let list = w.recv().await;
    let output = &list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "execution_software_status")
        .unwrap()["outputSchema"];
    let validator = jsonschema::validator_for(output).unwrap();
    for detected in [
        Some(SoftwareState::Present {
            version: PackageValue::new("1.2.3").unwrap(),
        }),
        Some(SoftwareState::Absent {}),
        Some(SoftwareState::Unknown {
            reason: SoftwareDetectionFailure::Unavailable,
        }),
        None,
    ] {
        s.software_result(LocalSoftwareResult {
            detected: detected.clone(),
            prepared: prepared.clone(),
            operation: OperationStatus {
                mode: execution_lifecycle::ExecutionMode::Test,
                process: None,
                assessment: None,
                cancel_requested: false,
                operation_request_id: request.clone(),
                content_digest: prepared.digest.clone(),
                phase: OperationPhase::ExecutionEnded,
                attempt_id: None,
                evidence: vec![],
            },
        });
        let reply = w
            .call(3, "execution_software_status", json!({"request":request}))
            .await;
        assert_eq!(
            value(&reply)["detected"],
            serde_json::to_value(detected).unwrap()
        );
        assert_eq!(value(&reply)["operation"]["phase"], "executionEnded");
        assert!(validator.is_valid(&reply["result"]["structuredContent"]));
    }
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
    w.close().await;
}

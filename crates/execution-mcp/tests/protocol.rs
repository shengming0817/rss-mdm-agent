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
fn value(reply: &Value) -> &Value {
    &reply["result"]["structuredContent"]["result"]
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
    w.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let list = w.recv().await;
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 5);
    for tool in tools {
        jsonschema::draft202012::new(&tool["inputSchema"]).unwrap();
        jsonschema::draft202012::new(&tool["outputSchema"]).unwrap();
    }
    let tasks = w.call(3, "execution_tasks", json!({})).await;
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
        "execution_tasks",
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
        .call(2, "execution_status", json!({"operationRequestId":"retry"}))
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
                        "execution_status",
                        json!({"operationRequestId":"same-id"})
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
        5
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
    stale["revision"] = json!("b".repeat(64));
    assert_eq!(
        error(&w.call(2, "execution_execute", stale).await)["code"],
        "expired"
    );
    let p = action_input(&mut w, &s, "terminal").await;
    w.call(20, "execution_execute", p.clone()).await;
    s.complete_test_result("terminal");
    let result = w
        .call(
            21,
            "execution_cancel",
            json!({"operationRequestId":"terminal"}),
        )
        .await;
    assert_eq!(value(&result)["disposition"], "alreadyTerminal");
    assert_eq!(value(&result)["operation"]["phase"], "verified");
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
    let p = action_input(&mut w, &s, "original-id").await;
    let submit = p.clone();
    assert_eq!(
        value(&w.call(11, "execution_execute", submit.clone()).await)["confirmationRequired"],
        true
    );
    w.send(json!({"jsonrpc":"2.0","id":12,"method":"tools/list","params":{}}))
        .await;
    let list = w.recv().await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let operation = json!({"operationRequestId":"original-id"});
    let cases = [
        ("execution_tasks", json!({}), true, "unavailable"),
        ("execution_capabilities", json!({}), true, "unavailable"),
        ("execution_status", operation.clone(), true, "unavailable"),
        ("execution_execute", submit, false, "outcomeUnknown"),
        ("execution_cancel", operation, false, "outcomeUnknown"),
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

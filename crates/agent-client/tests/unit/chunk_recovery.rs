//! Private queue seam: real HTTP and SQLite, no bypass exposed in the production API.
use crate::{test_support::*, wire::*, *};
#[tokio::test]
async fn lost_chunk_ack_replays_identical_bytes_after_restart_and_requires_every_chunk() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let task = offer.task_id();
    let attempt = offer.payload().attempt_id();
    let items: Vec<_> = (0..1500)
        .map(|index| {
            serde_json::json!({
                "id":format!("installation-{index}"), "native_id":format!("product-{index}"),
                "name":"Application", "version":"2.1", "publisher":"Publisher",
                "scope":"user", "owner":format!("S-1-5-21-{}", index % 3),
                "origin":"win32.uninstall.64", "location":"C:\\Program Files\\Application"
            })
        })
        .collect();
    let value = serde_json::json!({"format":1,"status":"complete",
        "fields":{"device.software.installations":items}});
    assert!(serde_json::to_vec(&value).unwrap().len() > OUTPUT_CHUNK_BYTES);
    let result = TaskResult::new(
        Some(0),
        OutputQuality::Complete,
        value.clone(),
        TaskDiagnostics::new("".into(), "".into(), 1, 1, None).unwrap(),
        1000,
    )
    .unwrap();
    let request = client
        .delivery_request("result", task, attempt, TaskEvent::Result(result), "[]")
        .unwrap();
    server.data.lock().unwrap().chunk_failure = true;
    assert_eq!(
        client.send_output_chunks(task, &request).await,
        Err(Error::Unavailable)
    );
    assert!(server.data.lock().unwrap().results.is_empty());
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    client.send_output_chunks(task, &request).await.unwrap();
    {
        let data = server.data.lock().unwrap();
        assert_eq!(data.chunk_calls.len(), 3);
        assert_eq!(data.chunk_calls[0], data.chunk_calls[1]);
        let TaskEvent::ChunkedResult(reference) = request.event() else {
            panic!("large output was not chunked")
        };
        assert_eq!(
            reference
                .assemble(data.chunks.values().cloned().collect())
                .unwrap()
                .output(),
            &value
        );
    }
    client
        .store
        .conn
        .execute("DELETE FROM requests WHERE key LIKE 'chunk/%/01'", [])
        .unwrap();
    assert_eq!(
        client.send_output_chunks(task, &request).await,
        Err(Error::Storage)
    );
}

#[tokio::test]
async fn native_result_replays_original_chunks_and_rejects_missing_or_conflicting_bytes() {
    let server = Server::new().await;
    server.data.lock().unwrap().native_collection();
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    assert!(materials.files().is_empty());
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let TaskPayload::Collection(spec) = start.payload() else {
        panic!("native task required")
    };
    let q = &spec.plan.spec().queries["items"];
    let rows = (0..1000)
        .map(|i| {
            [
                ("Id".into(), serde_json::json!(i.to_string())),
                ("Name".into(), serde_json::json!("x".repeat(512))),
            ]
            .into()
        })
        .collect::<Vec<_>>();
    let result = NativeCollectionResult {
        plan_digest: spec.plan_digest,
        collected_at: 1000,
        queries: [(
            "items".into(),
            NativeQueryResult {
                query_digest: q.digest(),
                collected_at: 1000,
                status: NativeQueryStatus::Complete,
                enumeration_complete: true,
                row_count: rows.len() as u32,
                rows,
                users: None,
                scan: None,
            },
        )]
        .into(),
    };
    result.validate_for(&spec.plan, 1000).unwrap();
    let bytes = serde_json::to_vec(&result).unwrap();
    let request = client
        .delivery_request(
            "native-result",
            offer.task_id(),
            offer.attempt_id(),
            TaskEvent::CollectionResult(result),
            "[]",
        )
        .unwrap();
    server.data.lock().unwrap().chunk_failure = true;
    assert_eq!(
        client.send_output_chunks(offer.task_id(), &request).await,
        Err(Error::Unavailable)
    );
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    client
        .send_output_chunks(offer.task_id(), &request)
        .await
        .unwrap();
    let TaskEvent::ChunkedCollectionResult { manifest } = request.event() else {
        panic!("native chunks required")
    };
    {
        let d = server.data.lock().unwrap();
        assert_eq!(d.chunk_calls[0], d.chunk_calls[1]);
        let mut chunks = d.chunks.values().cloned().collect::<Vec<_>>();
        chunks.reverse();
        assert_eq!(manifest.assemble(chunks.clone()).unwrap(), bytes);
        chunks.push(chunks[0].clone());
        assert!(manifest.assemble(chunks).is_err());
    }
    client
        .store
        .conn
        .execute("DELETE FROM requests WHERE key LIKE 'chunk/%/01'", [])
        .unwrap();
    assert_eq!(
        client.send_output_chunks(offer.task_id(), &request).await,
        Err(Error::Storage)
    );
}

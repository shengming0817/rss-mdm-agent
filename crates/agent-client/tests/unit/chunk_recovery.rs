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

//! Anonymous native↔Host control transport. No local listener or discoverable socket path.
use super::credentials::MasterKey;
use crate::self_service::{error, Result};
use ai_session_contract::{
    Counter, NativeAttachData, NativeCall, NativeCallData, NativeCallKind, NativeCallSchemaVersion,
    NativeControlFrame, NativeDetachData, NativeEvent, NativeEventKind, NativeEventSchemaVersion,
    NativeReply, NativeReplyFailureKind, NativeReplyFailureSchemaVersion, NativeReplySuccessKind,
    NativeReplySuccessSchemaVersion, NativeSaveConnectionData, NativeSuspendData,
};
use futures_util::{SinkExt, StreamExt};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{DuplexStream, WriteHalf},
    sync::{mpsc, oneshot},
};
use tokio_util::{
    codec::{FramedRead, FramedWrite, LinesCodec},
    sync::CancellationToken,
};
fn unavailable() -> crate::self_service::ServiceError {
    error("ai_unavailable", "AI 服务不可用")
}
type Pending = BTreeMap<u64, oneshot::Sender<Result<Value>>>;
pub struct Control {
    writer: tokio::sync::Mutex<FramedWrite<WriteHalf<DuplexStream>, LinesCodec>>,
    pending: Mutex<Pending>,
    views: Mutex<BTreeMap<String, mpsc::Sender<Value>>>,
    sequence: AtomicU64,
    stop: CancellationToken,
    reader_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}
impl Control {
    pub fn start(stream: DuplexStream, master: Arc<MasterKey>) -> Arc<Self> {
        let (reader, writer) = tokio::io::split(stream);
        let control = Arc::new(Self {
            writer: tokio::sync::Mutex::new(FramedWrite::new(
                writer,
                LinesCodec::new_with_max_length(524288),
            )),
            pending: Mutex::new(BTreeMap::new()),
            views: Mutex::new(BTreeMap::new()),
            sequence: AtomicU64::new(1),
            stop: CancellationToken::new(),
            reader_task: Mutex::new(None),
        });
        let task = control.clone();
        let credential = master.permit.clone();
        let reader_task = tokio::spawn(async move {
            let mut reader = FramedRead::new(reader, LinesCodec::new_with_max_length(524288));
            loop {
                let line = tokio::select! { _ = task.stop.cancelled() => break, line = reader.next() => match line { Some(Ok(line)) => line, _ => break } };
                let Ok(ai_session_contract::WireRecord::NativeControlFrame(frame)) =
                    ai_session_contract::decode(
                        line.as_bytes(),
                        &ai_session_contract::Limits {
                            max_bytes: 524288,
                            max_text_bytes: 262144,
                            max_depth: 32,
                            max_nodes: 16384,
                        },
                    )
                else {
                    break;
                };
                match frame {
                    ai_session_contract::NativeControlFrame::Reply(
                        ai_session_contract::NativeReply::Success { id, value, .. },
                    ) => {
                        let Ok(id) = u64::try_from(id.0) else { break };
                        if let Some(pending) = task.pending.lock().unwrap().remove(&id) {
                            let _ = pending.send(Ok(value));
                        }
                    }
                    ai_session_contract::NativeControlFrame::Reply(
                        ai_session_contract::NativeReply::Failure { id, .. },
                    ) => {
                        let Ok(id) = u64::try_from(id.0) else { break };
                        if let Some(pending) = task.pending.lock().unwrap().remove(&id) {
                            let _ = pending.send(Err(unavailable()));
                        }
                    }
                    ai_session_contract::NativeControlFrame::Event(event) => {
                        let id = String::from(event.channel);
                        let mut views = task.views.lock().unwrap();
                        if let Some(view) = views.get(id.as_str()) {
                            if view.try_send(event.message).is_err() {
                                views.remove(id.as_str());
                            }
                        }
                    }
                    ai_session_contract::NativeControlFrame::Call(
                        ai_session_contract::NativeCall::MasterKey { id, data, .. },
                    ) => {
                        let Ok(id) = u64::try_from(id.0) else { break };
                        let create = data.create;
                        let response = task.clone();
                        let master = master.clone();
                        let credential = credential.clone();
                        tokio::spawn(async move {
                            let permit = tokio::select! {
                                _ = response.stop.cancelled() => return,
                                permit = tokio::time::timeout(Duration::from_secs(5), credential.acquire_owned()) => match permit {
                                    Ok(Ok(permit)) => permit,
                                    _ => {
                                        let _ = response.write(NativeControlFrame::Reply(NativeReply::Failure { id: Counter(id as i64), kind: NativeReplyFailureKind::NativeReply, schema_version: NativeReplyFailureSchemaVersion::VALUE, ok: false })).await;
                                        return;
                                    }
                                }
                            };
                            let (sender, receiver) = oneshot::channel();
                            // Keychain APIs are synchronous and not cancellable. One detached OS thread
                            // owns the sole permit until it really returns; the control reader stays live.
                            std::thread::spawn(move || {
                                let _permit = permit;
                                let _ = sender.send(master.get(create));
                            });
                            let value = tokio::select! {
                                _ = response.stop.cancelled() => return,
                                value = tokio::time::timeout(Duration::from_secs(10), receiver) => value,
                            };
                            let reply = match value {
                                Ok(Ok(Ok(value))) => NativeReply::Success {
                                    id: Counter(id as i64),
                                    kind: NativeReplySuccessKind::NativeReply,
                                    schema_version: NativeReplySuccessSchemaVersion::VALUE,
                                    ok: true,
                                    value: Value::Array(
                                        value.into_iter().map(Value::from).collect(),
                                    ),
                                },
                                _ => NativeReply::Failure {
                                    id: Counter(id as i64),
                                    kind: NativeReplyFailureKind::NativeReply,
                                    schema_version: NativeReplyFailureSchemaVersion::VALUE,
                                    ok: false,
                                },
                            };
                            if response
                                .write(NativeControlFrame::Reply(reply))
                                .await
                                .is_err()
                            {
                                response.close();
                            }
                        });
                    }
                    _ => break,
                }
            }
            task.close();
        });
        *control.reader_task.lock().unwrap() = Some(reader_task);
        control
    }
    async fn write(&self, frame: NativeControlFrame) -> Result<()> {
        let record = ai_session_contract::WireRecord::NativeControlFrame(frame);
        let bytes = String::from_utf8(
            ai_session_contract::encode(
                &record,
                &ai_session_contract::Limits {
                    max_bytes: 524288,
                    max_text_bytes: 262144,
                    max_depth: 32,
                    max_nodes: 16384,
                },
            )
            .map_err(|_| unavailable())?,
        )
        .map_err(|_| unavailable())?;
        if bytes.len() > 524288 || self.stop.is_cancelled() {
            return Err(unavailable());
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            self.writer
                .lock()
                .await
                .send(bytes)
                .await
                .map_err(|_| unavailable())
        })
        .await
        .map_err(|_| unavailable())?
    }
    async fn call(
        &self,
        make: impl FnOnce(Counter) -> NativeCall,
        timeout: Duration,
    ) -> Result<Value> {
        let id = self.sequence.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending.lock().map_err(|_| unavailable())?;
            if pending.len() >= 16 || self.stop.is_cancelled() {
                return Err(unavailable());
            }
            pending.insert(id, sender);
        }
        let result = async {
            self.write(NativeControlFrame::Call(make(Counter(
                i64::try_from(id).map_err(|_| unavailable())?,
            ))))
            .await?;
            tokio::time::timeout(timeout, receiver)
                .await
                .map_err(|_| unavailable())?
                .map_err(|_| unavailable())?
        }
        .await;
        self.pending.lock().unwrap().remove(&id);
        result
    }
    pub async fn health(&self) -> Result<()> {
        let reply = self
            .call(
                |id| NativeCall::Health {
                    id,
                    kind: NativeCallKind::NativeCall,
                    schema_version: NativeCallSchemaVersion::VALUE,
                    data: NativeCallData {},
                },
                Duration::from_secs(15),
            )
            .await?;
        match ai_session_contract::decode(
            &serde_json::to_vec(&reply).map_err(|_| unavailable())?,
            &ai_session_contract::Limits {
                max_bytes: 4096,
                max_text_bytes: 2048,
                max_depth: 8,
                max_nodes: 64,
            },
        ) {
            Ok(ai_session_contract::WireRecord::HostHealth(_)) => Ok(()),
            _ => Err(crate::self_service::error(
                "unsupported_version",
                "AI Host 协议版本不兼容",
            )),
        }
    }
    pub async fn attach(
        &self,
        channel: &str,
        context: &ai_session_contract::UserContext,
        timeout: Duration,
    ) -> Result<Value> {
        let data = NativeAttachData {
            channel: channel.try_into().map_err(|_| unavailable())?,
            context: context.clone(),
        };
        self.call(
            |id| NativeCall::Attach {
                id,
                data,
                kind: NativeCallKind::NativeCall,
                schema_version: NativeCallSchemaVersion::VALUE,
            },
            timeout,
        )
        .await
    }
    pub async fn suspend(
        &self,
        context: &ai_session_contract::UserContext,
        timeout: Duration,
    ) -> Result<Value> {
        self.call(
            |id| NativeCall::Suspend {
                id,
                data: NativeSuspendData {
                    context: context.clone(),
                },
                kind: NativeCallKind::NativeCall,
                schema_version: NativeCallSchemaVersion::VALUE,
            },
            timeout,
        )
        .await
    }
    pub async fn detach_remote(&self, channel: &str, timeout: Duration) -> Result<Value> {
        let data = NativeDetachData {
            channel: channel.try_into().map_err(|_| unavailable())?,
        };
        self.call(
            |id| NativeCall::Detach {
                id,
                data,
                kind: NativeCallKind::NativeCall,
                schema_version: NativeCallSchemaVersion::VALUE,
            },
            timeout,
        )
        .await
    }
    pub async fn save_connection(
        &self,
        generation: &str,
        connection: ai_session_contract::Connection,
        expected: Option<u64>,
        secret: Option<String>,
        timeout: Duration,
    ) -> Result<Value> {
        let data = NativeSaveConnectionData {
            generation: generation.try_into().map_err(|_| unavailable())?,
            connection,
            expected: expected
                .map(|value| i64::try_from(value).map(Counter))
                .transpose()
                .map_err(|_| unavailable())?,
            secret: secret
                .map(TryInto::try_into)
                .transpose()
                .map_err(|_| unavailable())?,
        };
        self.call(
            |id| NativeCall::SaveConnection {
                id,
                data,
                kind: NativeCallKind::NativeCall,
                schema_version: NativeCallSchemaVersion::VALUE,
            },
            timeout,
        )
        .await
    }
    pub fn view(&self, id: String) -> Result<mpsc::Receiver<Value>> {
        let (sender, receiver) = mpsc::channel(64);
        let mut views = self.views.lock().map_err(|_| unavailable())?;
        if views.len() >= 4 || views.contains_key(&id) || self.stop.is_cancelled() {
            return Err(unavailable());
        }
        views.insert(id, sender);
        Ok(receiver)
    }
    pub async fn send(&self, id: &str, message: Value) -> Result<()> {
        self.write(NativeControlFrame::Event(NativeEvent {
            schema_version: NativeEventSchemaVersion::VALUE,
            kind: NativeEventKind::NativeEvent,
            channel: id.try_into().map_err(|_| unavailable())?,
            message,
        }))
        .await
    }
    pub fn detach(&self, id: &str) {
        self.views.lock().unwrap().remove(id);
    }
    pub fn closed(&self) -> bool {
        self.stop.is_cancelled()
    }
    pub async fn shutdown(&self) {
        self.close();
        let task = self.reader_task.lock().unwrap().take();
        if let Some(mut task) = task {
            if tokio::time::timeout(Duration::from_secs(5), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
    }
    pub fn close(&self) {
        self.stop.cancel();
        self.views.lock().unwrap().clear();
        for (_, pending) in std::mem::take(&mut *self.pending.lock().unwrap()) {
            let _ = pending.send(Err(unavailable()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::credentials::KeyBackend;
    use super::*;
    use futures_util::StreamExt;
    struct SlowKey;
    impl KeyBackend for SlowKey {
        fn read(
            &self,
        ) -> std::result::Result<Option<Vec<u8>>, super::super::credentials::KeyUnavailable>
        {
            std::thread::sleep(Duration::from_millis(300));
            Ok(None)
        }
        fn create(
            &self,
            _key: &[u8],
        ) -> std::result::Result<(), super::super::credentials::KeyUnavailable> {
            Ok(())
        }
    }
    #[tokio::test]
    async fn blocking_keychain_does_not_block_events_or_control_shutdown() {
        let (native, peer) = tokio::io::duplex(1048576);
        let control = Control::start(native, Arc::new(MasterKey::new(SlowKey)));
        let mut view = control.view("view".into()).unwrap();
        let mut writer = FramedWrite::new(peer, LinesCodec::new_with_max_length(524288));
        writer
            .send(json!({"schemaVersion":5,"kind":"nativeCall","id":1,"method":"masterKey","data":{"create":false}}).to_string())
            .await
            .unwrap();
        writer
            .send(json!({"schemaVersion":5,"kind":"nativeEvent","channel":"view","message":{"ready":true}}).to_string())
            .await
            .unwrap();
        let event = tokio::time::timeout(Duration::from_millis(100), view.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event["ready"], true);
        control.close();
    }

    struct FailedKey;
    impl KeyBackend for FailedKey {
        fn read(
            &self,
        ) -> std::result::Result<Option<Vec<u8>>, super::super::credentials::KeyUnavailable>
        {
            Err(super::super::credentials::KeyUnavailable)
        }
        fn create(
            &self,
            _key: &[u8],
        ) -> std::result::Result<(), super::super::credentials::KeyUnavailable> {
            Err(super::super::credentials::KeyUnavailable)
        }
    }

    #[tokio::test]
    async fn master_key_failure_replies_without_leaking_and_keeps_control_live() {
        let (native, peer) = tokio::io::duplex(1048576);
        let control = Control::start(native, Arc::new(MasterKey::new(FailedKey)));
        let mut view = control.view("view".into()).unwrap();
        let (reader, writer) = tokio::io::split(peer);
        let mut reader = FramedRead::new(reader, LinesCodec::new_with_max_length(524288));
        let mut writer = FramedWrite::new(writer, LinesCodec::new_with_max_length(524288));
        writer
            .send(json!({"schemaVersion":5,"kind":"nativeCall","id":1,"method":"masterKey","data":{"create":true}}).to_string())
            .await
            .unwrap();
        let reply = tokio::time::timeout(Duration::from_secs(2), reader.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&reply).unwrap(),
            json!({"schemaVersion":5,"kind":"nativeReply","id":1,"ok":false})
        );
        assert!(!reply.contains("key"));
        writer
            .send(json!({"schemaVersion":5,"kind":"nativeEvent","channel":"view","message":{"ready":true}}).to_string())
            .await
            .unwrap();
        assert_eq!(view.recv().await.unwrap()["ready"], true);
        control.close();
    }

    #[tokio::test]
    async fn duplicate_native_fields_close_the_strict_control_ingress() {
        let (native, peer) = tokio::io::duplex(1048576);
        let control = Control::start(native, Arc::new(MasterKey::new(FailedKey)));
        let mut writer = FramedWrite::new(peer, LinesCodec::new_with_max_length(524288));
        writer
            .send(String::from(
                r#"{"schemaVersion":5,"schemaVersion":5,"kind":"nativeEvent","channel":"view","message":{}}"#,
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), control.stop.cancelled())
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod incarnation_tests {
    use super::super::credentials::{KeyBackend, KeyUnavailable};
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Slow {
        calls: Arc<AtomicUsize>,
    }
    impl KeyBackend for Slow {
        fn read(&self) -> std::result::Result<Option<Vec<u8>>, KeyUnavailable> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(300));
            Ok(None)
        }
        fn create(&self, _: &[u8]) -> std::result::Result<(), KeyUnavailable> {
            Err(KeyUnavailable)
        }
    }
    #[tokio::test]
    async fn a_cancelled_incarnation_retains_the_shared_key_permit_until_the_native_call_returns() {
        let calls = Arc::new(AtomicUsize::new(0));
        let master = Arc::new(MasterKey::new(Slow {
            calls: calls.clone(),
        }));
        let (a, pa) = tokio::io::duplex(1048576);
        let first = Control::start(a, master.clone());
        let mut wa = FramedWrite::new(pa, LinesCodec::new());
        wa.send(json!({"schemaVersion":5,"kind":"nativeCall","id":1,"method":"masterKey","data":{"create":false}}).to_string()).await.unwrap();
        for _ in 0..100 {
            if calls.load(Ordering::SeqCst) == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        first.close();
        let (b, pb) = tokio::io::duplex(1048576);
        let second = Control::start(b, master.clone());
        let mut wb = FramedWrite::new(pb, LinesCodec::new());
        wb.send(json!({"schemaVersion":5,"kind":"nativeCall","id":2,"method":"masterKey","data":{"create":false}}).to_string()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(master.permit.available_permits(), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        first.shutdown().await;
        second.shutdown().await;
    }
}

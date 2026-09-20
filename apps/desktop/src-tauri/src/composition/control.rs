//! Anonymous native↔Host control transport. No local listener or discoverable socket path.
use super::credentials::{KeyBackend, MasterKey};
use crate::self_service::{error, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    net::{unix::OwnedWriteHalf, UnixStream},
    sync::{mpsc, oneshot, Semaphore},
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
    writer: tokio::sync::Mutex<FramedWrite<OwnedWriteHalf, LinesCodec>>,
    pending: Mutex<Pending>,
    views: Mutex<BTreeMap<String, mpsc::Sender<Value>>>,
    sequence: AtomicU64,
    stop: CancellationToken,
}
impl Control {
    pub fn start<B: KeyBackend + 'static>(stream: UnixStream, backend: B) -> Arc<Self> {
        let (reader, writer) = stream.into_split();
        let control = Arc::new(Self {
            writer: tokio::sync::Mutex::new(FramedWrite::new(
                writer,
                LinesCodec::new_with_max_length(524288),
            )),
            pending: Mutex::new(BTreeMap::new()),
            views: Mutex::new(BTreeMap::new()),
            sequence: AtomicU64::new(1),
            stop: CancellationToken::new(),
        });
        let task = control.clone();
        let master = Arc::new(MasterKey::new(backend));
        let credential = Arc::new(Semaphore::new(1));
        tokio::spawn(async move {
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
                                        let _ = response.write(json!({"schemaVersion":5,"kind":"nativeReply","id":id,"ok":false})).await;
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
                                Ok(Ok(Ok(value))) => {
                                    json!({"schemaVersion":5,"kind":"nativeReply","id":id,"ok":true,"value":value})
                                }
                                _ => {
                                    json!({"schemaVersion":5,"kind":"nativeReply","id":id,"ok":false})
                                }
                            };
                            if response.write(reply).await.is_err() {
                                response.close();
                            }
                        });
                    }
                    _ => break,
                }
            }
            task.close();
        });
        control
    }
    async fn write(&self, frame: Value) -> Result<()> {
        let record = ai_session_contract::decode(
            &serde_json::to_vec(&frame).map_err(|_| unavailable())?,
            &ai_session_contract::Limits {
                max_bytes: 524288,
                max_text_bytes: 262144,
                max_depth: 32,
                max_nodes: 16384,
            },
        )
        .map_err(|_| unavailable())?;
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
    async fn call(&self, method: &str, data: Value, timeout: Duration) -> Result<Value> {
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
            self.write(
                json!({"schemaVersion":5,"kind":"nativeCall","id":id,"method":method,"data":data}),
            )
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
    pub async fn attach(
        &self,
        channel: &str,
        context: &ai_session_contract::UserContext,
        timeout: Duration,
    ) -> Result<Value> {
        self.call(
            "attach",
            json!({"channel":channel,"context":context}),
            timeout,
        )
        .await
    }
    pub async fn suspend(
        &self,
        context: &ai_session_contract::UserContext,
        timeout: Duration,
    ) -> Result<Value> {
        self.call("suspend", json!({"context":context}), timeout)
            .await
    }
    pub async fn detach_remote(&self, channel: &str, timeout: Duration) -> Result<Value> {
        self.call("detach", json!({"channel":channel}), timeout)
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
        self.call(
            "saveConnection",
            json!({"generation":generation,"connection":connection,"expected":expected,"secret":secret}),
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
        self.write(json!({"schemaVersion":5,"kind":"nativeEvent","channel":id,"message":message}))
            .await
    }
    pub fn detach(&self, id: &str) {
        self.views.lock().unwrap().remove(id);
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
        let (native, peer) = UnixStream::pair().unwrap();
        let control = Control::start(native, SlowKey);
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
        let (native, peer) = UnixStream::pair().unwrap();
        let control = Control::start(native, FailedKey);
        let mut view = control.view("view".into()).unwrap();
        let (reader, writer) = peer.into_split();
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
        let (native, peer) = UnixStream::pair().unwrap();
        let control = Control::start(native, FailedKey);
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

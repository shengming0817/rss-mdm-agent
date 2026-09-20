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
        tokio::spawn(async move {
            let mut reader = FramedRead::new(reader, LinesCodec::new_with_max_length(524288));
            loop {
                let line = tokio::select! { _ = task.stop.cancelled() => break, line = reader.next() => match line { Some(Ok(line)) => line, _ => break } };
                let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                    break;
                };
                match frame["type"].as_str() {
                    Some("reply") => {
                        let Some(id) = frame["id"].as_u64() else {
                            break;
                        };
                        if let Some(pending) = task.pending.lock().unwrap().remove(&id) {
                            let _ = pending.send(if frame["ok"] == true {
                                Ok(frame["value"].clone())
                            } else {
                                Err(unavailable())
                            });
                        }
                    }
                    Some("event") => {
                        let Some(id) = frame["channel"].as_str() else {
                            break;
                        };
                        let mut views = task.views.lock().unwrap();
                        if let Some(view) = views.get(id) {
                            if view.try_send(frame["message"].clone()).is_err() {
                                views.remove(id);
                            }
                        }
                    }
                    Some("call") if frame["method"] == "master_key" => {
                        let (Some(id), Some(create)) =
                            (frame["id"].as_u64(), frame["data"]["create"].as_bool())
                        else {
                            break;
                        };
                        let master = master.clone();
                        let value = tokio::task::spawn_blocking(move || master.get(create)).await;
                        let reply = match value {
                            Ok(Ok(value)) => {
                                json!({"type":"reply","id":id,"ok":true,"value":value})
                            }
                            _ => json!({"type":"reply","id":id,"ok":false}),
                        };
                        if task.write(reply).await.is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
            task.close();
        });
        control
    }
    async fn write(&self, frame: Value) -> Result<()> {
        let bytes = serde_json::to_string(&frame).map_err(|_| unavailable())?;
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
    pub async fn call(&self, method: &str, data: Value, timeout: Duration) -> Result<Value> {
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
            self.write(json!({"type":"call","id":id,"method":method,"data":data}))
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
        self.write(json!({"type":"event","channel":id,"message":message}))
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

//! Owns bounded renderer channels and their cancellation/attachment lifecycle.
use super::{control::Control, host::Process};
use crate::self_service as ui;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
fn unavailable() -> ui::ServiceError {
    ui::error("ai_unavailable", "AI 服务不可用；已登记任务仍可查询")
}
pub(super) struct ViewChannel {
    pub(super) generation: String,
    reader: Mutex<mpsc::Receiver<Value>>,
    stop: CancellationToken,
    control: Arc<Control>,
}
pub(super) struct ViewChannels {
    connections: Mutex<BTreeMap<String, Arc<ViewChannel>>>,
    next: AtomicU64,
}
impl ViewChannels {
    #[cfg(test)]
    pub(super) async fn is_empty(&self) -> bool {
        self.connections.lock().await.is_empty()
    }
    pub(super) fn new() -> Self {
        Self {
            connections: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        }
    }
    pub(super) async fn connect(
        &self,
        process: Arc<Process>,
        context: &ai_session_contract::UserContext,
    ) -> ui::Result<String> {
        let generation = context.generation.as_str();
        let mut connections = self.connections.lock().await;
        if connections.len() >= 4 {
            return Err(unavailable());
        }
        let id = format!("view-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let reader = process.control.view(id.clone())?;
        if let Err(error) = process
            .control
            .attach(&id, &context, Duration::from_secs(15))
            .await
        {
            process.control.detach(&id);
            return Err(error);
        }
        connections.insert(
            id.clone(),
            Arc::new(ViewChannel {
                generation: generation.into(),
                reader: Mutex::new(reader),
                stop: process.stop.child_token(),
                control: process.control.clone(),
            }),
        );
        Ok(id)
    }
    pub(super) async fn connection(&self, id: &str) -> ui::Result<Arc<ViewChannel>> {
        self.connections
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(unavailable)
    }
    pub(super) async fn receive(&self, c: &ViewChannel) -> ui::Result<Option<Value>> {
        let mut reader = c.reader.try_lock().map_err(|_| unavailable())?;
        let value = tokio::select! {
            _ = c.stop.cancelled() => Err(unavailable()),
            _ = tokio::time::sleep(Duration::from_secs(20)) => Ok(None),
            message = reader.recv() => message.map(Some).ok_or_else(unavailable),
        };
        value
    }
    pub(super) async fn send(
        &self,
        id: &str,
        connection: &ViewChannel,
        message: Value,
    ) -> ui::Result<()> {
        if serde_json::to_vec(&message)
            .map_err(|_| unavailable())?
            .len()
            > 262144
        {
            return Err(unavailable());
        }
        if connection.stop.is_cancelled() {
            return Err(unavailable());
        }
        connection.control.send(id, message).await
    }
    pub async fn disconnect(&self, id: &str) {
        let connection = self.connections.lock().await.remove(id);
        if let Some(connection) = connection {
            connection.stop.cancel();
            connection.control.detach(id);
            let _ = connection
                .control
                .detach_remote(id, Duration::from_secs(2))
                .await;
        }
    }
    pub async fn detach_all(&self) {
        let connections = std::mem::take(&mut *self.connections.lock().await);
        for (id, connection) in connections {
            connection.stop.cancel();
            connection.control.detach(&id);
            let _ = connection
                .control
                .detach_remote(&id, Duration::from_secs(2))
                .await;
        }
    }
}

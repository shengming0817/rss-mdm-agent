use crate::{server::Handler, ExecutionServicePort, McpLimits, ServiceError};
use futures_util::StreamExt;
use rmcp::{
    model::*,
    service::{NotificationContext, RequestContext, RxJsonRpcMessage, Service, TxJsonRpcMessage},
    transport::Transport,
    ErrorData, RoleServer,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    future::Future,
    io,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    sync::{OwnedSemaphorePermit, Semaphore},
    time::{timeout_at, Instant},
};
use tokio_util::{
    codec::{FramedRead, LinesCodec, LinesCodecError},
    sync::CancellationToken,
};

#[derive(Clone)]
pub(crate) struct OriginalArguments(pub Arc<str>);
struct Lease {
    _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
struct RequestLease(Arc<Lease>);
pub(crate) struct Session {
    failure: Mutex<Option<ServiceError>>,
    active: Mutex<HashMap<RequestId, Arc<Lease>>>,
    // SDK drops cancelled responses outside Transport::send. Do not allow their ID
    // to be reused by a new request while a late cancelled response is still queued.
    cancelled: Mutex<HashSet<RequestId>>,
    pub slots: Arc<Semaphore>,
    pub stop: CancellationToken,
}
impl Session {
    pub fn new(limits: &McpLimits, stop: CancellationToken) -> Arc<Self> {
        Arc::new(Self {
            failure: Mutex::new(None),
            active: Mutex::new(HashMap::new()),
            cancelled: Mutex::new(HashSet::new()),
            slots: Arc::new(Semaphore::new(limits.in_flight)),
            stop,
        })
    }
    fn finish(&self, id: &RequestId) {
        self.active.lock().expect("session lock").remove(id);
    }
    fn cancel(&self, id: &RequestId) {
        self.cancelled
            .lock()
            .expect("session lock")
            .insert(id.clone());
        self.finish(id);
    }
    pub fn close(&self) {
        self.stop.cancel();
        self.active.lock().expect("session lock").clear();
    }
    fn fail(&self, error: ServiceError) {
        self.failure
            .lock()
            .expect("session lock")
            .get_or_insert(error);
        self.close();
    }
    pub fn failure(&self) -> Option<ServiceError> {
        *self.failure.lock().expect("session lock")
    }
}

type PendingWrite = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;
pub(crate) struct BoundedTransport<R, W> {
    reader: FramedRead<R, LinesCodec>,
    writer: Arc<tokio::sync::Mutex<W>>,
    session: Arc<Session>,
    limits: McpLimits,
    frames: usize,
    deadline: Option<Instant>,
    pending: Option<PendingWrite>,
}
impl<R: AsyncRead, W: AsyncWrite> BoundedTransport<R, W> {
    pub fn new(reader: R, writer: W, session: Arc<Session>, limits: McpLimits) -> Self {
        Self {
            reader: FramedRead::new(reader, LinesCodec::new_with_max_length(limits.frame_bytes)),
            writer: Arc::new(tokio::sync::Mutex::new(writer)),
            session,
            limits,
            frames: 0,
            deadline: None,
            pending: None,
        }
    }
}
fn io_error() -> io::Error {
    io::Error::other("controlled MCP transport closed")
}
fn response_id(message: &TxJsonRpcMessage<RoleServer>) -> Option<RequestId> {
    match message {
        JsonRpcMessage::Response(r) => Some(r.id.clone()),
        JsonRpcMessage::Error(r) => r.id.clone(),
        _ => None,
    }
}
fn write<W: AsyncWrite + Unpin + Send + 'static>(
    writer: Arc<tokio::sync::Mutex<W>>,
    session: Arc<Session>,
    limits: McpLimits,
    mut message: TxJsonRpcMessage<RoleServer>,
    release: bool,
) -> PendingWrite {
    // No upstream parser/handler diagnostic may expose supplied values.
    if let JsonRpcMessage::Error(ref mut error) = message {
        error.error.message = "MCP request rejected".into();
        error.error.data = None;
    }
    let id = response_id(&message);
    let bytes =
        crate::output::encode(&message, limits.response_bytes.saturating_sub(1)).or_else(|_| {
            crate::output::encode(
                &JsonRpcMessage::<ServerRequest, ServerResult, ServerNotification>::error(
                    ErrorData::internal_error("output budget exceeded", None),
                    id.clone(),
                ),
                limits.response_bytes.saturating_sub(1),
            )
        });
    let deadline = Instant::now() + limits.io_timeout;
    Box::pin(async move {
        let mut bytes = match bytes {
            Ok(bytes) => bytes,
            Err(_) => {
                session.fail(ServiceError::Limit);
                return Err(io_error());
            }
        };
        bytes.push(b'\n');
        let sent = timeout_at(deadline, async {
            let mut w = writer.lock().await;
            w.write_all(&bytes).await?;
            w.flush().await
        })
        .await;
        if release {
            if let Some(id) = id {
                session.finish(&id);
            }
        }
        match sent {
            Ok(Ok(())) => Ok(()),
            _ => {
                session.fail(ServiceError::Unavailable);
                Err(io_error())
            }
        }
    })
}
impl<R, W> Transport<RoleServer> for BoundedTransport<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    type Error = io::Error;
    fn send(
        &mut self,
        message: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = io::Result<()>> + Send + 'static {
        write(
            self.writer.clone(),
            self.session.clone(),
            self.limits.clone(),
            message,
            true,
        )
    }
    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleServer>> {
        loop {
            if self.session.stop.is_cancelled() {
                return None;
            }
            if let Some(pending) = self.pending.as_mut() {
                let result = pending.await;
                self.pending = None;
                if result.is_err() {
                    return None;
                }
            }
            let deadline = *self
                .deadline
                .get_or_insert_with(|| Instant::now() + self.limits.io_timeout);
            let line = match timeout_at(deadline, self.reader.next()).await {
                Ok(Some(Ok(line))) => line,
                Ok(None) => {
                    self.session.close();
                    return None;
                }
                Ok(Some(Err(LinesCodecError::MaxLineLengthExceeded))) => {
                    self.session.fail(ServiceError::Limit);
                    return None;
                }
                _ => {
                    self.session.fail(ServiceError::Unavailable);
                    return None;
                }
            };
            self.deadline = None;
            self.frames += 1;
            if self.frames > self.limits.session_frames {
                self.session.fail(ServiceError::Limit);
                return None;
            }
            let raw = match crate::ingress::inspect(
                line.as_bytes(),
                self.limits.json_depth,
                self.limits.json_nodes,
            ) {
                Ok(raw) => raw,
                Err(_) => {
                    self.session.fail(ServiceError::InvalidInput);
                    return None;
                }
            };
            let mut message: RxJsonRpcMessage<RoleServer> = match serde_json::from_str(&line) {
                Ok(m) => m,
                Err(_) => {
                    self.session.fail(ServiceError::InvalidInput);
                    return None;
                }
            };
            match &mut message {
                JsonRpcMessage::Request(r) => {
                    if matches!(&r.id, NumberOrString::String(s) if s.len() > 128)
                        || self
                            .session
                            .active
                            .lock()
                            .expect("session lock")
                            .contains_key(&r.id)
                        || self
                            .session
                            .cancelled
                            .lock()
                            .expect("session lock")
                            .contains(&r.id)
                    {
                        self.session.fail(ServiceError::InvalidInput);
                        return None;
                    }
                    let permit = match self.session.slots.clone().try_acquire_owned() {
                        Ok(p) => p,
                        Err(_) => {
                            self.pending = Some(write(
                                self.writer.clone(),
                                self.session.clone(),
                                self.limits.clone(),
                                JsonRpcMessage::error(
                                    ErrorData::invalid_request("request capacity exceeded", None),
                                    Some(r.id.clone()),
                                ),
                                false,
                            ));
                            continue;
                        }
                    };
                    let lease = Arc::new(Lease { _permit: permit });
                    self.session
                        .active
                        .lock()
                        .expect("session lock")
                        .insert(r.id.clone(), lease.clone());
                    r.request.extensions_mut().insert(RequestLease(lease));
                    if let Some(raw) = raw {
                        r.request
                            .extensions_mut()
                            .insert(OriginalArguments(Arc::from(raw)));
                    }
                    // rmcp traces public request DTOs before dispatch. Business input
                    // travels only through opaque, non-Debug Extensions above.
                    *r.request.get_meta_mut() = RequestMetaObject::new();
                    if let ClientRequest::CallToolRequest(call) = &mut r.request {
                        call.params.arguments = None;
                        call.params.input_responses = None;
                        call.params.request_state = None;
                    }
                }
                JsonRpcMessage::Notification(n) => {
                    // No resources, roots, logging, progress or arbitrary custom notification tasks.
                    *n.notification.get_meta_mut() = NotificationMetaObject::new();
                    match &mut n.notification {
                        ClientNotification::InitializedNotification(_) => {}
                        ClientNotification::CancelledNotification(cancelled) => {
                            cancelled.params.reason = None;
                        }
                        _ => {
                            self.session.fail(ServiceError::InvalidInput);
                            return None;
                        }
                    }
                }
                // This server does not initiate client requests.
                _ => {
                    self.session.fail(ServiceError::InvalidInput);
                    return None;
                }
            }
            return Some(message);
        }
    }
    async fn close(&mut self) -> io::Result<()> {
        self.session.close();
        let result = timeout_at(Instant::now() + self.limits.io_timeout, async {
            self.writer.lock().await.shutdown().await
        })
        .await;
        match result {
            Ok(Ok(())) => Ok(()),
            _ => {
                self.session.fail(ServiceError::Unavailable);
                Err(io_error())
            }
        }
    }
}
pub(crate) struct BoundedService<S> {
    pub handler: Handler<S>,
    pub session: Arc<Session>,
}
impl<S: ExecutionServicePort> Service<RoleServer> for BoundedService<S> {
    async fn handle_request(
        &self,
        request: ClientRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<ServerResult, ErrorData> {
        let lease = context
            .extensions
            .get::<RequestLease>()
            .cloned()
            .ok_or_else(|| ErrorData::invalid_request("bounded ingress required", None))?;
        let _lease = lease.0;
        let id = context.id.clone();
        let cancelled = context.ct.clone();
        let tool = matches!(&request, ClientRequest::CallToolRequest(_));
        let timeout_error = match &request {
            ClientRequest::CallToolRequest(call)
                if matches!(
                    call.params.name.as_ref(),
                    "execution_propose"
                        | "execution_preview"
                        | "execution_submit"
                        | "execution_cancel"
                ) =>
            {
                ServiceError::OutcomeUnknown
            }
            _ => ServiceError::Unavailable,
        };
        let outcome = tokio::select! {
            biased;
            _ = self.session.stop.cancelled() => {
                self.session.cancel(&id);
                Err(ErrorData::internal_error("session closed", None))
            }
            _ = cancelled.cancelled() => {
                self.session.cancel(&id);
                Err(ErrorData::internal_error("wait cancelled", None))
            }
            r = tokio::time::timeout(self.handler.limits.request_timeout, Service::handle_request(&self.handler, request, context)) => {
                match r {
                    Ok(r) => r,
                    Err(_) if tool => Ok(ServerResult::CallToolResult(crate::server::failure(timeout_error))),
                    Err(_) => Err(ErrorData::internal_error("request timeout", None)),
                }
            }
        };
        // On timeout the future was dropped; signal any service wait it handed off.
        // Never translate this token into cancellation of an accepted business operation.
        cancelled.cancel();
        outcome
    }
    async fn handle_notification(
        &self,
        n: ClientNotification,
        c: NotificationContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        Service::handle_notification(&self.handler, n, c).await
    }
    fn get_info(&self) -> ServerConfig {
        Service::get_info(&self.handler)
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Service::supported_protocol_versions(&self.handler)
    }
}

//! Bounded controlled-execution MCP adapter. Execution authority belongs to the service.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod config;
mod ingress;
mod model;
mod output;
mod port;
mod server;
mod transport;
pub use config::{McpLimits, StdioServiceConfig};
pub use model::*;
pub use port::ExecutionServicePort;

use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::sync::CancellationToken;

/// A single host-bound controlled tools service; construct a new instance per connection.
/// The host owns authentication and must exclude raw rmcp tracing from its log subscriber.
pub struct ExecutionMcp<S> {
    service: Arc<S>,
    limits: McpLimits,
}
struct SessionGuard(Arc<transport::Session>);
impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.close();
    }
}
impl<S: ExecutionServicePort> ExecutionMcp<S> {
    /// Check host limits and the service's out-of-band identity binding before accepting input.
    pub fn new(service: Arc<S>, limits: McpLimits) -> Result<Self, ServiceError> {
        limits.validate()?;
        service.check_binding()?;
        Ok(Self { service, limits })
    }
    /// Serve a host-owned stdio/byte-stream connection until EOF, stop or protocol failure.
    ///
    /// Input errors fail closed. All stream writes and shutdown waits are bounded. Dropping
    /// or stopping this session never cancels an accepted business operation. The service
    /// must retain durable requests independently of this future/process.
    pub async fn serve<R, W>(
        self,
        reader: R,
        writer: W,
        stop: CancellationToken,
    ) -> Result<(), ServiceError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let session = transport::Session::new(&self.limits, stop.child_token());
        let _guard = SessionGuard(session.clone());
        let transport =
            transport::BoundedTransport::new(reader, writer, session.clone(), self.limits.clone());
        let handler = transport::BoundedService {
            handler: server::Handler {
                service: self.service,
                limits: self.limits.clone(),
            },
            session: session.clone(),
        };
        let initialized = tokio::time::timeout(
            self.limits.request_timeout,
            handler.serve_with_ct(transport, session.stop.clone()),
        )
        .await;
        let result = match initialized {
            Ok(Ok(running)) => running
                .waiting()
                .await
                .map(|_| ())
                .map_err(|_| ServiceError::Unavailable),
            _ => Err(ServiceError::Unavailable),
        };
        session.close();
        // Quiesce service futures independently of rmcp's transport-loop completion.
        let _drained = tokio::time::timeout(
            self.limits.request_timeout,
            session
                .slots
                .clone()
                .acquire_many_owned(self.limits.in_flight as u32),
        )
        .await
        .map_err(|_| ServiceError::Unavailable)?
        .map_err(|_| ServiceError::Unavailable)?;
        result
    }
}

//! One bounded IPC owner driving the existing application. Peer identity is never a wire field.
use execution_app::{AppHost, ExecutionApp, RequestContext, RunnerPort};
use execution_contract::{PlanLimits, RequestId};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::sync::atomic::AtomicBool;

/// Native connection facts, borrowed while the native transport retains the connection.
/// Fields are private and no deserializer exists; production verification belongs to #2564.
pub struct Peer {
    pub(crate) pid: u32,
    pub(crate) uid: Option<u32>,
    pub(crate) session: u32,
    pub(crate) native: usize,
}
impl Peer {
    /// Borrowed native connection identity; valid only during this handler call.
    pub fn native_handle(&self) -> usize {
        self.native
    }
    /// Kernel peer process identity, never accepted from request bytes.
    pub fn pid(&self) -> u32 {
        self.pid
    }
    /// Unix peer UID; absent on Windows, where native token verification is required.
    pub fn uid(&self) -> Option<u32> {
        self.uid
    }
    /// Native peer session identifier, not product authentication.
    pub fn session(&self) -> u32 {
        self.session
    }
}
/// Local IPC V3 request. Idempotency and authorization still belong to ExecutionApp.
#[derive(Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Request {
    /// Submit an exact V3 frozen plan through the application admission funnel.
    Submit {
        /// Raw bounded JSON retained for duplicate-key and canonical contract checks.
        plan: Box<serde_json::value::RawValue>,
    },
    /// Read an authorized existing request without execution authority.
    Status {
        /// Original durable business request identity.
        request: RequestId,
    },
    /// Record cancellation against the original request; not a termination proof.
    Cancel {
        /// Original durable business request identity.
        request: RequestId,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Envelope {
    version: u8,
    request: RawRequest,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum Method {
    Submit,
    Status,
    Cancel,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    method: Method,
    plan: Option<Box<serde_json::value::RawValue>>,
    request: Option<RequestId>,
}
impl RawRequest {
    fn into_request(self) -> Option<Request> {
        match (self.method, self.plan, self.request) {
            (Method::Submit, Some(plan), None) => Some(Request::Submit { plan }),
            (Method::Status, None, Some(request)) => Some(Request::Status { request }),
            (Method::Cancel, None, Some(request)) => Some(Request::Cancel { request }),
            _ => None,
        }
    }
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
/// Closed transport reply; no backend details or raw capture are exposed.
pub enum Reply {
    /// Read an authorized existing request without execution authority.
    Status {
        /// Permission-filtered application projection.
        value: execution_app::ExecutionStatus,
    },
    /// Invalid or unauthorized request.
    Rejected,
    /// Processing could not be confirmed; recover by original request identity.
    Unavailable,
}
/// #2564 verifies current caller/registration/policy. The transport cannot create RequestContext.
pub trait Ingress: Send {
    /// Verify current native peer trust; transport facts alone cannot authorize execution.
    fn authenticate(&self, peer: &Peer) -> Result<RequestContext, execution_app::Error>;
}
/// The native OS service owns one handler; ticks and calls are serialized by the owner loop.
pub trait Handler: Send {
    /// Process one bounded request serially; a lost reply does not prove no mutation.
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply;
    /// Reconcile existing attempts without obtaining replacement dispatch authority.
    fn tick(&mut self) -> Result<(), execution_app::Error>;
    /// Stop owned work and persist available facts before returning.
    fn stop(&mut self) -> Result<(), execution_app::Error>;
}
/// Executable's default assembly: there is deliberately no environment/test authority fallback.
pub struct Unbound;
impl Handler for Unbound {
    /// Process one bounded request serially; a lost reply does not prove no mutation.
    fn handle(&mut self, _: &Peer, _: Request) -> Reply {
        Reply::Rejected
    }
    /// Reconcile existing attempts without obtaining replacement dispatch authority.
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        Ok(())
    }
    /// Stop owned work and persist available facts before returning.
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        Ok(())
    }
}
/// Adapter around the sole execution application; no duplicate admission or execution state.
pub struct Endpoint<H, R, I> {
    app: ExecutionApp<H, R>,
    ingress: I,
    limits: PlanLimits,
    cursor: Option<RequestId>,
}
impl<H: AppHost, R: RunnerPort, I: Ingress> Endpoint<H, R, I> {
    /// Bind the existing application and trusted ingress; creates no credentials or authority.
    pub fn new(app: ExecutionApp<H, R>, ingress: I, limits: PlanLimits) -> Self {
        Self {
            app,
            ingress,
            limits,
            cursor: None,
        }
    }
}
impl<H: AppHost + Send, R: RunnerPort + Send, I: Ingress> Handler for Endpoint<H, R, I> {
    /// Process one bounded request serially; a lost reply does not prove no mutation.
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply {
        let result = (|| {
            let caller = self.ingress.authenticate(peer)?;
            match request {
                Request::Submit { plan } => {
                    let spec = execution_contract::decode_plan(plan.get().as_bytes(), &self.limits)
                        .map_err(|_| execution_app::Error::InvalidInput)?;
                    let plan = execution_contract::FrozenPlan::freeze(spec, &self.limits)
                        .map_err(|_| execution_app::Error::InvalidInput)?;
                    let request = &plan.spec().request.request_id;
                    self.app.submit(&caller, request, &plan)
                }
                Request::Status { request } => self.app.status(&caller, &request),
                Request::Cancel { request } => self.app.cancel(&caller, &request),
            }
        })();
        match result {
            Ok(value) => Reply::Status { value },
            Err(
                execution_app::Error::Denied
                | execution_app::Error::Unbound
                | execution_app::Error::NotFound
                | execution_app::Error::InvalidInput
                | execution_app::Error::Conflict
                | execution_app::Error::Capability
                | execution_app::Error::Unsupported,
            ) => Reply::Rejected,
            Err(_) => Reply::Unavailable,
        }
    }
    /// Reconcile existing attempts without obtaining replacement dispatch authority.
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        self.cursor = self.app.reconcile_page(self.cursor.as_ref(), 32)?;
        Ok(())
    }
    /// Stop owned work and persist available facts before returning.
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        self.app.stop_active(128)
    }
}
/// Decode only bounded, current envelopes; malformed input never invokes a handler.
pub fn dispatch(handler: &mut dyn Handler, peer: &Peer, bytes: &[u8]) -> Vec<u8> {
    let reply = if bytes.len() > 65536 {
        Reply::Rejected
    } else {
        match serde_json::from_slice::<Envelope>(bytes) {
            Ok(e) if e.version == 2 => match e.request.into_request() {
                Some(request) => handler.handle(peer, request),
                None => Reply::Rejected,
            },
            _ => Reply::Rejected,
        }
    };
    serde_json::to_vec(&reply).unwrap_or_else(|_| b"{\"kind\":\"unavailable\"}".to_vec())
}
/// Native service lifecycle; the binary always passes Unbound until production trust is supplied.
#[cfg(target_os = "macos")]
pub fn run(
    handler: Box<dyn Handler>,
    stop: &'static AtomicBool,
) -> Result<(), execution_app::Error> {
    crate::macos_service::run(handler, stop)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_versions_and_unbound_calls_never_execute() {
        let peer = Peer {
            pid: 1,
            uid: Some(0),
            session: 1,
            native: 0,
        };
        let mut handler = Unbound;
        for bytes in [
            br#"{"version":1,"request":{"method":"status","request":"r"}}"#.as_slice(),
            br#"{"version":2,"request":{"method":"status","request":"r"}}"#,
            br#"{"version":2,"actor":"root","request":{"method":"cancel","request":"r"}}"#,
        ] {
            assert_eq!(
                dispatch(&mut handler, &peer, bytes),
                br#"{"kind":"rejected"}"#
            )
        }
    }
}

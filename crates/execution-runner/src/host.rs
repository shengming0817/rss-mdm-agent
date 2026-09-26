//! One bounded IPC owner driving the existing application. Peer identity is never a wire field.
use execution_app::{AppHost, ExecutionApp, RequestContext, RunnerPort};
use execution_contract::{PlanLimits, RequestId};
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

/// Native connection facts, borrowed while the native transport retains the connection.
/// Fields are private and no deserializer exists; production verification belongs to #2564.
pub struct Peer {
    pub(crate) pid: u32,
    pub(crate) uid: u32,
    pub(crate) session: u32,
    pub(crate) native: usize,
}
impl Peer {
    /// Borrowed native connection identity; valid only during this handler call.
    pub fn native_handle(&self) -> usize {
        self.native
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn uid(&self) -> u32 {
        self.uid
    }
    pub fn session(&self) -> u32 {
        self.session
    }
}
/// Local IPC V2 request. Idempotency and authorization still belong to ExecutionApp.
#[derive(Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Request {
    Submit {
        plan: Box<serde_json::value::RawValue>,
    },
    Status {
        request: RequestId,
    },
    Cancel {
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
pub enum Reply {
    Status {
        value: execution_app::ExecutionStatus,
    },
    Rejected,
    Unavailable,
}
/// #2564 verifies current caller/registration/policy. The transport cannot create RequestContext.
pub trait Ingress: Send {
    fn authenticate(&self, peer: &Peer) -> Result<RequestContext, execution_app::Error>;
}
/// The native OS service owns one handler; ticks and calls are serialized by the owner loop.
pub trait Handler: Send {
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply;
    fn tick(&mut self) -> Result<(), execution_app::Error>;
    fn stop(&mut self) -> Result<(), execution_app::Error>;
}
/// Executable's default assembly: there is deliberately no environment/test authority fallback.
pub struct Unbound;
impl Handler for Unbound {
    fn handle(&mut self, _: &Peer, _: Request) -> Reply {
        Reply::Rejected
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        Ok(())
    }
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
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        self.cursor = self.app.reconcile_page(self.cursor.as_ref(), 32)?;
        Ok(())
    }
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
            uid: 0,
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

//! Backend-to-local execution adapter; neither transport nor journal authority.
use agent_client::{Error, Offer};
use rss_mdm_agent_wire::runtime as wire;
use sha2::Digest as _;
pub(crate) mod bridge;
pub(crate) mod collection;
pub(crate) mod commands;
mod credential;
mod dispatch;
mod enrollment;
pub(crate) mod gate;
pub(crate) mod host;
mod offer;
pub(crate) mod plan;
pub(crate) mod software;
#[cfg(test)]
pub(crate) use bridge::settled;
pub(crate) use bridge::ExecutionBridge;
#[cfg(test)]
pub(crate) use dispatch::dispatch_started;
pub(crate) use dispatch::submit;
pub(crate) use offer::offered;

pub(crate) fn request_id(offer: &Offer) -> Result<execution_contract::RequestId, Error> {
    fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, Error> {
        serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Protocol)
    }
    let authorization = match offer.payload() {
        wire::TaskPayload::DeviceAccess(_) => return Err(Error::Unsupported),
        wire::TaskPayload::Unmanage(_) => return Err(Error::Unsupported),
        wire::TaskPayload::Credential(v) => Some(&v.authorization),
        wire::TaskPayload::Collection(v) => Some(&v.authorization),
        wire::TaskPayload::Script(v) => Some(&v.authorization),
        wire::TaskPayload::Software(v) => Some(&v.authorization),
        wire::TaskPayload::Enrollment(v) => Some(&v.authorization),
    };
    if let Some(wire::TaskAuthorization::SelfService { request_id, .. }) = authorization {
        return execution_contract::RequestId::new(request_id.to_string())
            .map_err(|_| Error::Protocol);
    }
    let bytes = match offer.payload() {
        wire::TaskPayload::DeviceAccess(_) => return Err(Error::Unsupported),
        wire::TaskPayload::Unmanage(_) => return Err(Error::Unsupported),
        wire::TaskPayload::Credential(v) => encode(&(
            v.tenant_id,
            &v.device_id,
            v.registration_id,
            v.generation,
            v.task_id,
        ))?,
        wire::TaskPayload::Collection(v) => encode(&(
            v.tenant_id,
            &v.device_id,
            v.registration_id,
            v.generation,
            v.task_id,
        ))?,
        wire::TaskPayload::Script(v) => encode(&(
            v.tenant_id,
            &v.device_id,
            v.registration_id,
            v.generation,
            v.task_id,
        ))?,
        wire::TaskPayload::Software(v) => encode(&(
            v.tenant_id,
            &v.device_id,
            v.registration_id,
            v.generation,
            v.task_id,
        ))?,
        wire::TaskPayload::Enrollment(v) => encode(&(
            v.tenant_id,
            &v.device_id,
            v.registration_id,
            v.generation,
            v.task_id,
        ))?,
    };
    execution_contract::RequestId::new(format!("agent-v1-{:x}", sha2::Sha256::digest(&bytes)))
        .map_err(|_| Error::Protocol)
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod collection_tests;

#[cfg(test)]
mod credential_tests;

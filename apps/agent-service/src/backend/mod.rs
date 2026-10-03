//! Backend-to-local execution adapter; neither transport nor journal authority.
use agent_client::{wire, Error, Offer};
use sha2::Digest as _;
mod bridge;
mod commands;
mod dispatch;
pub(crate) mod host;
mod offer;
pub(crate) mod plan;
pub(crate) mod software;
pub(crate) use bridge::ExecutionBridge;
pub(crate) use dispatch::submit;
pub(crate) use offer::offered;

pub(crate) fn request_id(offer: &Offer) -> Result<execution_contract::RequestId, Error> {
    fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, Error> {
        serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Protocol)
    }
    let bytes = match offer.payload() {
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
        _ => return Err(Error::Unsupported),
    };
    execution_contract::RequestId::new(format!("agent-v5-{:x}", sha2::Sha256::digest(&bytes)))
        .map_err(|_| Error::Protocol)
}

#[cfg(test)]
mod live_mdm;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

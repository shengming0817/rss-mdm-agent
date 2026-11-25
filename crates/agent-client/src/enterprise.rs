//! Recover an already attached challenge from its authenticated browser transaction.
//! ref: rss-mdm@df060faa crates/device-access-http/src/enterprise_broker.rs
use crate::{client::bounded, tls, Error};
use rss_mdm_agent_wire::{enterprise as wire, runtime::Secret};
use std::time::Duration;
use uuid::Uuid;

/// Read the existing operation before recovering its exact original challenge.
pub async fn challenge(
    origin: &str,
    ca: Option<&str>,
    tenant: Uuid,
    cookie: &Secret,
    begin: &wire::Begin,
) -> Result<wire::Challenge, Error> {
    let url = url::Url::parse(origin).map_err(|_| Error::Configuration)?;
    if url.scheme() != "https"
        || url.origin().ascii_serialization() != origin
        || tenant.is_nil()
        || begin.operation_id.is_nil()
        || begin.binding_id.is_nil()
    {
        return Err(Error::Configuration);
    }
    let budget = Duration::from_secs(2);
    let http = if let Some(pem) = ca {
        tls::transport(Some(pem.as_bytes()), true, budget, budget, None)?
    } else {
        reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(budget)
            .build()
            .map_err(|_| Error::Network)?
    };
    let base = format!("{origin}/api/v1/tenants/{tenant}/device-access/broker");
    let authenticated = |request: reqwest::RequestBuilder| {
        request
            .header("origin", origin)
            .header("cookie", format!("__Host-rss-broker={}", cookie.expose()))
            .timeout(budget)
    };
    async fn read<T: serde::de::DeserializeOwned>(
        request: reqwest::RequestBuilder,
    ) -> Result<T, Error> {
        let response = request.send().await.map_err(|_| Error::Network)?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(Error::Denied);
        }
        serde_json::from_slice(&bounded(response, 65536).await?).map_err(|_| Error::Protocol)
    }
    let status: wire::Status = read(authenticated(
        http.get(format!("{base}/{}", begin.operation_id)),
    ))
    .await?;
    if status.operation_id != begin.operation_id || status.phase != wire::Phase::AwaitingProof {
        return Err(Error::Denied);
    }
    let c: wire::Challenge = read(authenticated(http.post(base).json(begin))).await?;
    if c.operation_id != begin.operation_id
        || c.challenge_id.is_nil()
        || c.signing_payload.is_empty()
        || c.signing_payload.len() > 16384
        || c.local_session.validate().is_err()
    {
        return Err(Error::Untrusted);
    }
    Ok(c)
}

//! Device-authenticated employee HTTP; no caller can supply a different endpoint or credential origin.
use crate::{
    store::{decode, encode},
    wire, Client, Clock, Error, IdentityProvider,
};
use reqwest::Method;
use serde::de::DeserializeOwned;
use uuid::Uuid;
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Submit the original journal-frozen explicit event. Producer idempotency binds
    /// operation_id to exact content and returns the original batch after response loss.
    pub async fn invoke_policy_event(
        &self,
        request: &wire::PolicyEventRequest,
    ) -> Result<wire::PolicyEventAccepted, Error> {
        let value: wire::PolicyEventAccepted = self
            .self_service_json(None, Method::POST, "policy-events", Some(encode(request)?))
            .await?;
        if value.operation_id != request.operation_id || value.batch_id.is_nil() {
            return Err(Error::Untrusted);
        }
        Ok(value)
    }
    pub(crate) async fn self_service_json<T: DeserializeOwned>(
        &self,
        secret: Option<&str>,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<T, Error> {
        self.active()?;
        self.now()?;
        if secret.is_some_and(|secret| {
            secret.len() != 64
                || !secret
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err(Error::Identity);
        }
        let mut request = self.http.request(method, self.url(path)?).timeout(
            self.store
                .cfg
                .limits
                .request_timeout
                .min(std::time::Duration::from_secs(2)),
        );
        if let Some(secret) = secret {
            request = request.header("x-rss-user-session", secret);
        }
        if let Some(body) = body {
            if body.len() > 131072 {
                return Err(Error::Capacity);
            }
            request = request
                .header("content-type", "application/json")
                .body(body);
        }
        let (status, bytes) = self
            .agent_json_transport(request, self.store.cfg.limits.response_bytes)
            .await?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::NotFound);
        }
        if !status.is_success() {
            self.runtime_error(&bytes)?;
        }
        decode(&bytes)
    }
    /// Current authorized directory for an origin attested by the system service.
    pub async fn self_service_directory(
        &self,
        secret: Option<&str>,
        query: &wire::SelfServiceDirectory,
    ) -> Result<wire::SelfServiceDirectoryResponse, Error> {
        let source = match query.source {
            wire::SelfServiceCaller::Human => "human",
            wire::SelfServiceCaller::Ai => "ai",
        };
        let mut url = url::Url::parse("https://query.invalid/self-service")
            .map_err(|_| Error::Configuration)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("source", source);
            for (key, value) in [
                ("filter", &query.filter),
                ("sort", &query.sort),
                ("fields", &query.fields),
                ("cursor", &query.cursor),
            ] {
                if let Some(value) = value {
                    pairs.append_pair(key, value);
                }
            }
            if let Some(limit) = query.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
        }
        let path = format!("self-service?{}", url.query().ok_or(Error::Protocol)?);
        let response: wire::SelfServiceDirectoryResponse = self
            .self_service_json(secret, Method::GET, &path, None)
            .await?;
        if response.items.len() > usize::from(query.limit.unwrap_or(64))
            || response.as_of < 0
            || response
                .next_cursor
                .as_ref()
                .is_some_and(|next| Some(next) == query.cursor.as_ref() || next.is_empty())
        {
            return Err(Error::Protocol);
        }
        Ok(response)
    }
    /// Resolve one exact capability; discovery pages are never lookup authority.
    pub async fn resolve_self_service(
        &self,
        secret: Option<&str>,
        request: &wire::SelfServiceResolve,
    ) -> Result<wire::SelfServiceDirectoryItem, Error> {
        let item: wire::SelfServiceDirectoryItem = self
            .self_service_json(
                secret,
                Method::POST,
                "self-service/resolve",
                Some(encode(request)?),
            )
            .await?;
        if item.capability.id != item.policy_id
            || request
                .selector
                .policy_id()?
                .is_some_and(|id| id != item.policy_id)
            || matches!(&request.selector, wire::CapabilitySelector::Alias { alias } if item.capability.alias.as_ref() != Some(alias))
        {
            return Err(Error::Untrusted);
        }
        Ok(item)
    }
    /// Send the original journal-frozen invocation. A lost response is recovered by request ID.
    pub async fn invoke_self_service(
        &self,
        secret: Option<&str>,
        request: &wire::SelfServiceInvoke,
    ) -> Result<wire::SelfServiceAccepted, Error> {
        let response: wire::SelfServiceAccepted = self
            .self_service_json(
                secret,
                Method::POST,
                "self-service/invocations",
                Some(encode(request)?),
            )
            .await?;
        if response.request_id != request.operation_id
            || response.policy_id != request.policy_id
            || response.run_id.is_nil()
        {
            return Err(Error::Untrusted);
        }
        Ok(response)
    }
    /// Employee-authorized status and structured result of the same original invocation.
    pub async fn self_service_status(
        &self,
        secret: Option<&str>,
        request: Uuid,
    ) -> Result<wire::SelfServiceStatus, Error> {
        let response: wire::SelfServiceStatus = self
            .self_service_json(
                secret,
                Method::GET,
                &format!("self-service/invocations/{request}"),
                None,
            )
            .await?;
        if response.request_id != request || response.run_id.is_nil() {
            return Err(Error::Untrusted);
        }
        Ok(response)
    }
}

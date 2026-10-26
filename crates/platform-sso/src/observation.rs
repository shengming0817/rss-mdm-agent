//! Actual LoginManager readback, separate from the bootstrap-derived expected configuration.
//! ref: Apple AuthenticationServices/ASAuthorizationProviderExtensionLoginManager.h (SDK 26.4).
use crate::{Context, Error};
use rss_mdm_agent_wire::platform_sso as w;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeConfiguration {
    pub authentication: u32,
    pub client_id: String,
    pub issuer: String,
    pub audience: String,
    pub token_endpoint: String,
    pub jwks_endpoint: String,
    pub nonce_endpoint: String,
    pub nonce_response_keypath: String,
    pub server_nonce_claim: String,
    pub additional_scopes: String,
    pub login_request_values: Vec<(String, String)>,
    pub login_user_name: Option<String>,
}
impl NativeConfiguration {
    pub fn matches(
        &self,
        context: &Context,
        client: Uuid,
        authentication: w::AuthenticationMode,
        source: Uuid,
    ) -> Result<bool, Error> {
        context.validate()?;
        let endpoints =
            w::Endpoints::new(&context.origin, context.tenant).map_err(|_| Error::Identity)?;
        Ok(self.authentication
            == match authentication {
                w::AuthenticationMode::Password => 1,
                w::AuthenticationMode::SecureEnclave => 2,
            }
            && Uuid::parse_str(&self.client_id).ok() == Some(client)
            && self.issuer == endpoints.issuer
            && self.audience == endpoints.issuer
            && self.token_endpoint == endpoints.token
            && self.jwks_endpoint == endpoints.jwks
            && self.nonce_endpoint == endpoints.nonce
            && self.nonce_response_keypath == "nonce"
            && self.server_nonce_claim == "request_nonce"
            && self.additional_scopes == w::SCOPE
            && self.login_request_values == [("platform_sso_version".into(), "1.0".into())]
            && (context.kind == w::Kind::Device
                || self.login_user_name.as_deref() == Some(source.to_string().as_str())))
    }
}

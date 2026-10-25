// ref: Apple AuthenticationServices/ASPasskeyCredentialRequest{,Parameters}.h (SDK 26.4).
use passkey_types::{
    ctap2::{get_assertion as get, make_credential as make},
    webauthn as w,
};
use passkey_vault::{ceremony::Authorizer, Entry, Error, Result, Vault};
use serde::Deserialize;
#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Register {
        rp: String,
        hash: Vec<u8>,
        user_handle: Vec<u8>,
        username: String,
        algorithms: Vec<i32>,
        exclude: Vec<Vec<u8>>,
    },
    Assert {
        rp: String,
        hash: Vec<u8>,
        allowed: Vec<Vec<u8>>,
        identity: Option<Identity>,
    },
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub credential_id: Vec<u8>,
    pub user_handle: Vec<u8>,
}
impl Request {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Capacity);
        }
        let request: Self = serde_json::from_slice(bytes).map_err(|_| Error::Input)?;
        request.validate()?;
        Ok(request)
    }
    pub fn rp(&self) -> &str {
        match self {
            Self::Register { rp, .. } | Self::Assert { rp, .. } => rp,
        }
    }
    pub fn hash(&self) -> &[u8] {
        match self {
            Self::Register { hash, .. } | Self::Assert { hash, .. } => hash,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.hash().len() != 32 || self.rp().is_empty() || self.rp().len() > 253 {
            return Err(Error::Input);
        }
        let ids = match self {
            Self::Register {
                user_handle,
                username,
                algorithms,
                exclude,
                ..
            } => {
                if user_handle.is_empty()
                    || user_handle.len() > 64
                    || username.len() > 1024
                    || algorithms.len() > 128
                    || !algorithms.contains(&-7)
                {
                    return Err(Error::Input);
                }
                exclude
            }
            Self::Assert {
                allowed, identity, ..
            } => {
                if let Some(identity) = identity {
                    if identity.credential_id.is_empty()
                        || identity.credential_id.len() > 1023
                        || identity.user_handle.is_empty()
                        || identity.user_handle.len() > 64
                    {
                        return Err(Error::Input);
                    }
                }
                allowed
            }
        };
        if ids.len() > 128 || ids.iter().any(|id| id.is_empty() || id.len() > 1023) {
            return Err(Error::Input);
        }
        Ok(())
    }
    pub fn matches(&self, entry: &Entry) -> bool {
        if entry.rp_id != self.rp() {
            return false;
        }
        match self {
            Self::Register { .. } => false,
            Self::Assert {
                allowed, identity, ..
            } => {
                (allowed.is_empty() || allowed.contains(&entry.credential_id))
                    && identity.as_ref().is_none_or(|i| {
                        i.credential_id == entry.credential_id && i.user_handle == entry.user_handle
                    })
            }
        }
    }
    pub(crate) async fn execute(
        &self,
        vault: &Vault,
        approval: &dyn Authorizer,
    ) -> Result<serde_json::Value> {
        let hash = self.hash().to_vec().into();
        match self {
            Self::Register {
                rp,
                user_handle,
                username,
                exclude,
                ..
            } => {
                let response = vault
                    .register(
                        make::Request {
                            client_data_hash: hash,
                            rp: make::PublicKeyCredentialRpEntity {
                                id: rp.clone(),
                                name: None,
                            },
                            user: w::PublicKeyCredentialUserEntity {
                                id: user_handle.clone().into(),
                                name: username.clone(),
                                display_name: username.clone(),
                            },
                            pub_key_cred_params: vec![w::PublicKeyCredentialParameters {
                                ty: w::PublicKeyCredentialType::PublicKey,
                                alg: passkey_crypto::iana::Algorithm::ES256,
                            }],
                            exclude_list: Some(descriptors(exclude)),
                            extensions: None,
                            options: make::Options {
                                rk: true,
                                up: true,
                                uv: true,
                            },
                            pin_auth: None,
                            pin_protocol: None,
                        },
                        approval,
                    )
                    .await?;
                let id = response
                    .auth_data
                    .attested_credential_data
                    .as_ref()
                    .ok_or(Error::Protocol)?
                    .credential_id();
                // WebAuthn attestation uses text keys, unlike the CTAP response's integer keys.
                use passkey_crypto::coset::cbor::{ser, value::Value};
                let object = Value::Map(vec![
                    (Value::Text("fmt".into()), Value::Text(response.fmt.clone())),
                    (
                        Value::Text("authData".into()),
                        Value::Bytes(response.auth_data.to_vec()),
                    ),
                    (Value::Text("attStmt".into()), response.att_stmt.clone()),
                ]);
                let mut bytes = Vec::new();
                ser::into_writer(&object, &mut bytes).map_err(|_| Error::Protocol)?;
                Ok(serde_json::json!({"credentialId":id,"attestationObject":bytes}))
            }
            Self::Assert { rp, allowed, .. } => {
                let response = vault
                    .assert(
                        get::Request {
                            rp_id: rp.clone(),
                            client_data_hash: hash,
                            allow_list: Some(descriptors(allowed)),
                            extensions: None,
                            options: get::Options { up: true, uv: true },
                            pin_auth: None,
                            pin_protocol: None,
                        },
                        approval,
                    )
                    .await?;
                Ok(
                    serde_json::json!({"credentialId":response.credential.ok_or(Error::Protocol)?.id.as_slice(),
                    "authenticatorData":response.auth_data.to_vec(),"signature":response.signature.as_slice(),
                    "userHandle":response.user.ok_or(Error::Protocol)?.id.as_slice()}),
                )
            }
        }
    }
}
fn descriptors(ids: &[Vec<u8>]) -> Vec<w::PublicKeyCredentialDescriptor> {
    ids.iter()
        .map(|id| w::PublicKeyCredentialDescriptor {
            ty: w::PublicKeyCredentialType::PublicKey,
            id: id.clone().into(),
            transports: None,
        })
        .collect()
}

//! Native Provider seam, not a browser/JSON signing endpoint. The calling adapter
//! must establish the actual origin from its OS/browser request before calling here.
use crate::{
    model::{Content, Record},
    Entry, Error, Result, Vault,
};
use passkey_authenticator::{
    Authenticator, CredentialStore, DiscoverabilitySupport, StoreInfo, UiHint, UserCheck,
    UserValidationMethod,
};
use passkey_crypto::{
    coset::{AsCborValue, CborSerializable},
    iana,
    rust_crypto::RustCryptoBackend,
};
use passkey_types::{
    ctap2::{get_assertion as get, make_credential as make, Ctap2Error, StatusCode},
    webauthn::PublicKeyCredentialDescriptor,
    Passkey,
};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use zeroize::Zeroize;

pub enum Kind {
    Register,
    Assert,
}
pub struct Context<'a> {
    pub kind: Kind,
    pub origin: &'a str,
    pub rp_id: &'a str,
    pub client_data_hash: &'a [u8],
    pub accounts: &'a [Entry],
    pub verification_required: bool,
}
/// A result from the trusted native UI, never deserialize this from ordinary IPC.
pub struct Decision {
    pub selected: Option<Vec<u8>>,
    pub verified: bool,
}
pub trait Authorizer {
    /// Obtain explicit consent and, if required, verification for this exact request.
    /// Cancellation returns an error; implementations must bound interactive waits.
    fn authorize(&self, request: &Context<'_>) -> Result<Decision>;
}
fn origin(rp: &str, source: &str) -> Result<()> {
    let url = url::Url::parse(source).map_err(|_| Error::Input)?;
    let host = url.host_str().ok_or(Error::Input)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || rp.len() > 253
        || rp != rp.to_ascii_lowercase()
        || rp.ends_with('.')
        || psl::domain(rp.as_bytes()).is_none()
        || !(host == rp || host.strip_suffix(rp).is_some_and(|p| p.ends_with('.')))
    {
        return Err(Error::Input);
    }
    Ok(())
}
fn bounded(ids: Option<&[PublicKeyCredentialDescriptor]>) -> Result<()> {
    if ids.is_some_and(|list| {
        list.len() > 128 || list.iter().any(|v| v.id.is_empty() || v.id.len() > 1023)
    }) {
        return Err(Error::Input);
    }
    Ok(())
}
fn protocol(error: StatusCode) -> Error {
    if error == Ctap2Error::CredentialExcluded.into() {
        Error::Excluded
    } else {
        Error::Protocol
    }
}
// Upstream COSE values do not implement Zeroize; retain a wiping accessor locally.
struct Sensitive(Passkey);
fn wipe_key(key: &mut passkey_crypto::CoseKey) {
    for (_, value) in &mut key.params {
        if let passkey_crypto::coset::cbor::value::Value::Bytes(bytes) = value {
            bytes.zeroize();
        }
    }
}
impl Drop for Sensitive {
    fn drop(&mut self) {
        wipe_key(&mut self.0.key);
    }
}
impl passkey_authenticator::PasskeyAccessor for Sensitive {
    fn key(&self) -> std::borrow::Cow<'_, passkey_crypto::CoseKey> {
        std::borrow::Cow::Borrowed(&self.0.key)
    }
    fn credential_id(&self) -> &[u8] {
        &self.0.credential_id
    }
    fn rp_id(&self) -> &str {
        &self.0.rp_id
    }
    fn user_handle(&self) -> Option<&[u8]> {
        self.0.user_handle.as_ref().map(|h| h.as_slice())
    }
    fn username(&self) -> Option<&str> {
        self.0.username.as_deref()
    }
    fn user_display_name(&self) -> Option<&str> {
        self.0.user_display_name.as_deref()
    }
    fn counter(&self) -> Option<u32> {
        None
    }
    fn set_counter(&mut self, _: u32) {}
    fn extensions(&self) -> std::borrow::Cow<'_, passkey_types::CredentialExtensions> {
        std::borrow::Cow::Borrowed(&self.0.extensions)
    }
}
// Serialize borrowed COSE parameters: no intermediate Value owns a copied private scalar.
struct BorrowedCose<'a>(&'a passkey_crypto::CoseKey);
impl serde::Serialize for BorrowedCose<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::{Error as _, SerializeMap};
        let key = self.0;
        if !key.key_id.is_empty() || !key.key_ops.is_empty() || !key.base_iv.is_empty() {
            return Err(S::Error::custom("unsupported key metadata"));
        }
        let mut map = serializer
            .serialize_map(Some(1 + usize::from(key.alg.is_some()) + key.params.len()))?;
        map.serialize_entry(
            &1,
            &key.kty.clone().to_cbor_value().map_err(S::Error::custom)?,
        )?;
        if let Some(alg) = &key.alg {
            map.serialize_entry(&3, &alg.clone().to_cbor_value().map_err(S::Error::custom)?)?;
        }
        for (label, value) in &key.params {
            map.serialize_entry(
                &label.clone().to_cbor_value().map_err(S::Error::custom)?,
                value,
            )?;
        }
        map.end()
    }
}
struct Approved {
    verified: bool,
}
#[async_trait::async_trait]
impl UserValidationMethod for Approved {
    type PasskeyItem = Sensitive;
    async fn check_user<'a>(
        &self,
        hint: UiHint<'a, Sensitive>,
        _: bool,
        required: bool,
    ) -> std::result::Result<UserCheck, Ctap2Error> {
        if matches!(hint, UiHint::InformExcludedCredentialFound(_)) {
            return Err(Ctap2Error::CredentialExcluded);
        }
        if required && !self.verified {
            return Err(Ctap2Error::OperationDenied);
        }
        Ok(UserCheck {
            presence: true,
            verification: self.verified,
        })
    }
    fn is_presence_enabled(&self) -> bool {
        true
    }
    fn is_verification_enabled(&self) -> Option<bool> {
        Some(true)
    }
}
struct Memory {
    content: Mutex<Content>,
    selected: Option<Vec<u8>>,
}
fn passkey(r: &Record) -> std::result::Result<Sensitive, StatusCode> {
    let key =
        passkey_crypto::CoseKey::from_slice(&r.cose).map_err(|_| Ctap2Error::InvalidCredential)?;
    Ok(Sensitive(Passkey {
        key,
        credential_id: r.entry.credential_id.clone().into(),
        rp_id: r.entry.rp_id.clone(),
        user_handle: Some(r.entry.user_handle.clone().into()),
        username: Some(r.entry.username.clone()),
        user_display_name: Some(r.entry.display_name.clone()),
        counter: None,
        extensions: Default::default(),
    }))
}
#[async_trait::async_trait]
impl CredentialStore for &Memory {
    type PasskeyItem = Sensitive;
    async fn find_credentials(
        &self,
        ids: Option<&[PublicKeyCredentialDescriptor]>,
        rp: &str,
        _: Option<&[u8]>,
    ) -> std::result::Result<Vec<Sensitive>, StatusCode> {
        // Exclusion must match any account under the RP, not just the registering user handle.
        self.content
            .lock()
            .map_err(|_| Ctap2Error::Other)?
            .records
            .iter()
            .filter(|r| r.entry.rp_id == rp)
            .filter(|r| {
                self.selected
                    .as_ref()
                    .is_none_or(|s| s == &r.entry.credential_id)
            })
            .filter(|r| {
                ids.is_none_or(|ids| {
                    ids.iter()
                        .any(|i| i.id.as_slice() == r.entry.credential_id.as_slice())
                })
            })
            .map(passkey)
            .collect()
    }
    async fn save_credential(
        &mut self,
        cred: Passkey,
        _: make::PublicKeyCredentialUserEntity,
        _: make::PublicKeyCredentialRpEntity,
        _: make::Options,
    ) -> std::result::Result<(), StatusCode> {
        let cred = Sensitive(cred);
        let mut cose = zeroize::Zeroizing::new(Vec::new());
        passkey_crypto::coset::cbor::ser::into_writer(&BorrowedCose(&cred.0.key), &mut *cose)
            .map_err(|_| Ctap2Error::Other)?;
        let entry = Entry {
            credential_id: cred.0.credential_id.to_vec(),
            rp_id: cred.0.rp_id.clone(),
            user_handle: cred
                .0
                .user_handle
                .as_ref()
                .ok_or(Ctap2Error::InvalidCredential)?
                .to_vec(),
            username: cred.0.username.clone().unwrap_or_default(),
            display_name: cred.0.user_display_name.clone().unwrap_or_default(),
            backup_eligible: true,
            backed_up: false,
        };
        let record = Record {
            cose: std::mem::take(&mut *cose),
            entry,
        };
        self.content
            .lock()
            .map_err(|_| Ctap2Error::Other)?
            .records
            .push(record);
        Ok(())
    }
    async fn update_credential(&mut self, _: &Sensitive) -> std::result::Result<(), StatusCode> {
        Err(Ctap2Error::UnsupportedOption.into())
    }
    async fn get_info(&self) -> StoreInfo {
        StoreInfo {
            discoverability: DiscoverabilitySupport::ForcedDiscoverable,
        }
    }
}

impl Vault {
    pub async fn register(
        &self,
        source: &str,
        request: make::Request,
        authorizer: &dyn Authorizer,
    ) -> Result<make::Response> {
        origin(&request.rp.id, source)?;
        bounded(request.exclude_list.as_deref())?;
        if request.client_data_hash.len() != 32
            || request.user.id.is_empty()
            || request.user.id.len() > 64
            || request.user.name.len() > 1024
            || request.user.display_name.len() > 1024
            || !request.options.up
            || request.extensions.is_some()
            || request.pin_auth.is_some()
            || request.pin_protocol.is_some()
        {
            return Err(Error::Input);
        }
        let epoch = self.generation();
        let started = Instant::now();
        let decision = authorizer.authorize(&Context {
            kind: Kind::Register,
            origin: source,
            rp_id: &request.rp.id,
            client_data_hash: &request.client_data_hash,
            accounts: &[],
            verification_required: request.options.uv,
        })?;
        if started.elapsed() > Duration::from_secs(120) {
            return Err(Error::Cancelled);
        }
        self.current(epoch)?;
        let tx = self.store.transaction()?;
        let mut value = tx.load(&self.store.owner)?.ok_or(Error::NotFound)?;
        let key = self.session_key(&value)?;
        let memory = Memory {
            content: Mutex::new(Self::content(&value, &key)?),
            selected: None,
        };
        let mut authenticator = Authenticator::new(
            [0; 16].into(),
            &memory,
            Approved {
                verified: decision.verified,
            },
            RustCryptoBackend,
        );
        authenticator
            .set_algorithms(&[iana::Algorithm::ES256])
            .map_err(|_| Error::Protocol)?;
        let response = authenticator
            .make_credential(request)
            .await
            .map_err(protocol)?;
        Self::encrypt_content(
            &mut value,
            &key,
            &*memory.content.lock().map_err(|_| Error::Busy)?,
        )?;
        self.current(epoch)?;
        tx.save(&value)?;
        self.current(epoch)?;
        Ok(response)
    }
    pub async fn assert(
        &self,
        source: &str,
        request: get::Request,
        authorizer: &dyn Authorizer,
    ) -> Result<get::Response> {
        origin(&request.rp_id, source)?;
        bounded(request.allow_list.as_deref())?;
        if request.client_data_hash.len() != 32
            || !request.options.up
            || request.extensions.is_some()
            || request.pin_auth.is_some()
            || request.pin_protocol.is_some()
        {
            return Err(Error::Input);
        }
        let epoch = self.generation();
        let started = Instant::now();
        let initial = self.status()?;
        if !initial.unlocked {
            return Err(Error::Locked);
        }
        let accounts: Vec<_> = initial
            .entries
            .into_iter()
            .filter(|e| e.rp_id == request.rp_id)
            .filter(|e| {
                request
                    .allow_list
                    .as_ref()
                    .filter(|v| !v.is_empty())
                    .is_none_or(|ids| {
                        ids.iter()
                            .any(|i| i.id.as_slice() == e.credential_id.as_slice())
                    })
            })
            .collect();
        let decision = authorizer.authorize(&Context {
            kind: Kind::Assert,
            origin: source,
            rp_id: &request.rp_id,
            client_data_hash: &request.client_data_hash,
            accounts: &accounts,
            verification_required: request.options.uv,
        })?;
        if started.elapsed() > Duration::from_secs(120) {
            return Err(Error::Cancelled);
        }
        let selected = decision.selected.ok_or(Error::NotFound)?;
        if !accounts.iter().any(|e| e.credential_id == selected) {
            return Err(Error::NotFound);
        }
        self.current(epoch)?;
        let tx = self.store.transaction()?;
        let value = tx.load(&self.store.owner)?.ok_or(Error::NotFound)?;
        if value.revision != initial.revision {
            return Err(Error::Conflict);
        }
        let key = self.session_key(&value)?;
        let memory = Memory {
            content: Mutex::new(Self::content(&value, &key)?),
            selected: Some(selected),
        };
        let mut authenticator = Authenticator::new(
            [0; 16].into(),
            &memory,
            Approved {
                verified: decision.verified,
            },
            RustCryptoBackend,
        );
        authenticator
            .set_algorithms(&[iana::Algorithm::ES256])
            .map_err(|_| Error::Protocol)?;
        let response = authenticator
            .get_assertion(request)
            .await
            .map_err(protocol)?;
        self.current(epoch)?;
        Ok(response)
    }
}

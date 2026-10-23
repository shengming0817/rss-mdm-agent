//! The communication owner persists signed locations before selecting them or reporting installation.
//! ref: ring src/ec/curve25519/ed25519/verification.rs; rusqlite src/transaction.rs.
use crate::{Client, Clock, Config, Error, IdentityProvider, Transport};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rss_mdm_agent_wire::connections::{
    Acknowledgement, FailureReason, FailureReport, SignedConfiguration, MAX_FILE_BYTES,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Verify the wire-owned signature encoding using only independently protected installation keys.
pub fn verify_connections(
    document: &SignedConfiguration,
    service: Uuid,
    tenant: Uuid,
    keys: &BTreeMap<String, Vec<u8>>,
) -> Result<(), Error> {
    document.validate()?;
    let c = &document.configuration;
    if c.service_id != service || c.tenant_id != tenant {
        return Err(Error::Identity);
    }
    let key = keys.get(&c.key_id).ok_or(Error::Untrusted)?;
    let signature = URL_SAFE_NO_PAD
        .decode(&document.signature)
        .map_err(|_| Error::Untrusted)?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key)
        .verify(&c.signing_bytes()?, &signature)
        .map_err(|_| Error::Untrusted)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    document: SignedConfiguration,
    origin: String,
    ack: Acknowledgement,
    confirmed: bool,
    registration: Option<(Uuid, u64)>,
    failure: Option<FailureReport>,
    refresh_at: i64,
    next_drive: i64,
    attempts: u8,
}
impl State {
    fn new(document: SignedConfiguration, origin: String) -> Result<Self, Error> {
        let c = &document.configuration;
        let ack = Acknowledgement {
            operation_id: Uuid::new_v4(),
            revision: c.revision,
            digest: c.digest()?,
            origin: origin.clone(),
        };
        ack.validate_for(c)?;
        Ok(Self {
            document,
            origin,
            ack,
            confirmed: false,
            registration: None,
            failure: None,
            refresh_at: 0,
            next_drive: 0,
            attempts: 0,
        })
    }
    fn validate(&self, cfg: &Config) -> Result<(), Error> {
        verify_connections(
            &self.document,
            cfg.service_id,
            cfg.tenant,
            &cfg.registration_keys,
        )?;
        self.ack.validate_for(&self.document.configuration)?;
        if self.next_drive < 0
            || self.refresh_at < 0
            || self.attempts > 6
            || self.ack.origin != self.origin
            || (self.confirmed && self.registration.is_none())
            || self
                .registration
                .is_some_and(|(id, generation)| id.is_nil() || generation == 0)
        {
            return Err(Error::Storage);
        }
        Ok(())
    }
}

pub(crate) fn initial_state(cfg: &Config) -> Result<Vec<u8>, Error> {
    let state = State::new(
        cfg.connections.clone(),
        cfg.connections.configuration.preferred_origin.clone(),
    )?;
    state.validate(cfg)?;
    crate::store::encode(&state)
}
fn read_state(conn: &rusqlite::Connection, cfg: &Config) -> Result<State, Error> {
    use rusqlite::OptionalExtension;
    let body: Option<Vec<u8>> = conn.query_row("SELECT CASE WHEN typeof(body)='blob' AND length(body)<=?1 THEN body END FROM state WHERE key='connections'", [MAX_FILE_BYTES * 2], |row| row.get(0)).optional()?.flatten();
    let state: State = crate::store::decode(&body.ok_or(Error::Storage)?)?;
    state.validate(cfg)?;
    Ok(state)
}
pub(crate) fn validate_stored(conn: &rusqlite::Connection, cfg: &Config) -> Result<(), Error> {
    read_state(conn, cfg).map(|_| ())
}

/// Installed facts from protected local storage, independent of the backend's desired revision.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionStatus {
    /// Immutable backend installation identity.
    pub service_id: Uuid,
    /// Actual installed revision, including offline imports.
    pub revision: u64,
    /// Selected authorized address.
    pub origin: String,
    /// Whether the backend confirmed this exact revision and address operation.
    pub acknowledged: bool,
    /// Last retained closed failure classification.
    pub failure: Option<FailureReason>,
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    fn connection_state(&self) -> Result<State, Error> {
        read_state(&self.store.conn, &self.store.cfg)
    }
    pub(crate) fn initialize_connections(&self) -> Result<(), Error> {
        // A program update's older package seed never replaces a newer installed document.
        self.connection_state().map(|_| ())
    }
    fn persist_connections(&self, state: &State) -> Result<(), Error> {
        state.validate(&self.store.cfg)?;
        let body = crate::store::encode(state)?;
        if body.len() > MAX_FILE_BYTES * 2 {
            return Err(Error::Capacity);
        }
        let tx = self.store.conn.unchecked_transaction()?;
        tx.execute("INSERT INTO state VALUES('connections',?1) ON CONFLICT(key) DO UPDATE SET body=excluded.body", [&body])?;
        tx.commit()?;
        let readback = read_state(&self.store.conn, &self.store.cfg)?;
        if crate::store::encode(&readback)? != body {
            return Err(Error::Storage);
        }
        Ok(())
    }
    /// Read installed configuration facts without any network effects.
    pub fn connection_status(&self) -> Result<ConnectionStatus, Error> {
        let state = self.connection_state()?;
        Ok(ConnectionStatus {
            service_id: self.store.cfg.service_id,
            revision: state.document.configuration.revision,
            origin: state.origin,
            acknowledged: state.confirmed,
            failure: state.failure.map(|f| f.reason),
        })
    }
    pub(crate) fn connection_origin(&self) -> Result<url::Url, Error> {
        if self.store.cfg.transport == Transport::TestLoopback {
            return Ok(self.store.cfg.origin.clone());
        }
        url::Url::parse(&self.connection_state()?.origin).map_err(|_| Error::Storage)
    }
    /// Verify and atomically install a bounded signed file. It carries no new trust keys or credentials.
    /// Callers must use the installation owner and its exclusive communication lease for offline use.
    pub fn import_connections(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(Error::Capacity);
        }
        self.install_connections(crate::store::decode(bytes)?)
    }
    pub(crate) fn install_connections(&self, document: SignedConfiguration) -> Result<(), Error> {
        verify_connections(
            &document,
            self.store.cfg.service_id,
            self.store.cfg.tenant,
            &self.store.cfg.registration_keys,
        )?;
        let old = self.connection_state()?;
        document
            .configuration
            .validate_successor(&old.document.configuration)
            .map_err(|_| Error::Conflict)?;
        if document == old.document {
            return Ok(());
        }
        let origin = document.configuration.preferred_origin.clone();
        let mut state = State::new(document, origin)?;
        state.next_drive = old.next_drive;
        state.attempts = old.attempts;
        state.failure = old.failure;
        self.persist_connections(&state)
    }
    pub(crate) fn network_failed(&self) -> Result<(), Error> {
        let old = self.connection_state()?;
        let origins = &old.document.configuration.origins;
        let index = origins
            .iter()
            .position(|o| o == &old.origin)
            .ok_or(Error::Storage)?;
        let next = origins[(index + 1) % origins.len()].clone();
        let failure = old.failure.clone();
        let schedule = (old.next_drive, old.attempts);
        let mut state = if next != old.origin {
            State::new(old.document, next)?
        } else {
            old
        };
        state.failure = failure;
        (state.next_drive, state.attempts) = schedule;
        state.refresh_at = self.now()?.saturating_add(30);
        if state.failure.is_none() {
            state.failure = Some(FailureReport {
                operation_id: Uuid::new_v4(),
                revision: state.document.configuration.revision,
                reason: FailureReason::NetworkUnavailable,
            });
        }
        self.persist_connections(&state)
    }
    pub(crate) fn handle_transport_error(&self, error: Error) -> Error {
        if error == Error::Network {
            self.network_failed().err().unwrap_or(Error::Unavailable)
        } else {
            error
        }
    }
    // Never replay a write here. Its existing business owner retains the exact operation and
    // performs its own status lookup/retry on a later drive through the newly selected address.
    pub(crate) async fn agent_json_transport(
        &self,
        request: reqwest::RequestBuilder,
        max: usize,
    ) -> Result<(reqwest::StatusCode, Vec<u8>), Error> {
        match crate::client::json_transport(request, max).await {
            Err(Error::Network) => {
                self.network_failed()?;
                Err(Error::Unavailable)
            }
            result => result,
        }
    }
    pub(crate) async fn agent_send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, Error> {
        match request.send().await.map_err(crate::client::transport_error) {
            Err(Error::Network) => {
                self.network_failed()?;
                Err(Error::Unavailable)
            }
            result => result,
        }
    }
    /// Fetch, install and acknowledge from the existing service drive. Failures preserve identity.
    pub async fn drive_connections(&mut self) -> Result<(), Error> {
        self.active()?;
        let now = self.now()?;
        let mut state = self.connection_state()?;
        if now < state.next_drive {
            return Ok(());
        }
        state.attempts = state.attempts.saturating_add(1).min(6);
        state.next_drive = now.saturating_add((1_i64 << state.attempts).min(60));
        self.persist_connections(&state)?;
        let result = self.drive_connections_inner().await;
        if result.is_ok() {
            let mut state = self.connection_state()?;
            state.attempts = 0;
            state.next_drive = now.saturating_add(2);
            self.persist_connections(&state)?;
        }
        result
    }
    async fn drive_connections_inner(&mut self) -> Result<(), Error> {
        let mut state = self.connection_state()?;
        let current = self.registration()?;
        let binding = (current.registration_id, current.generation);
        if state.registration != Some(binding) {
            state.registration = Some(binding);
            state.confirmed = false;
            state.ack.operation_id = Uuid::new_v4();
            self.persist_connections(&state)?;
        }
        if self.now()? >= state.refresh_at {
            let url = self
                .connection_origin()?
                .join("api/v1/agent/enroll/connections")
                .map_err(|_| Error::Configuration)?;
            let (status, bytes) = self
                .agent_json_transport(
                    self.http
                        .get(url)
                        .timeout(self.store.cfg.limits.request_timeout),
                    MAX_FILE_BYTES,
                )
                .await?;
            if !status.is_success() {
                return Err(Error::Protocol);
            }
            let document: SignedConfiguration = crate::store::decode(&bytes)?;
            if let Err(error) = self.install_connections(document.clone()) {
                let reason = match error {
                    Error::Identity => FailureReason::IdentityMismatch,
                    Error::Conflict => FailureReason::RevisionRejected,
                    Error::Storage => FailureReason::PersistenceFailed,
                    _ => FailureReason::SignatureRejected,
                };
                state = self.connection_state()?;
                let revision = document.configuration.revision;
                if (1..=i64::MAX as u64).contains(&revision) && state.failure.is_none() {
                    state.failure = Some(FailureReport {
                        operation_id: Uuid::new_v4(),
                        revision,
                        reason,
                    });
                    self.persist_connections(&state)?;
                }
                let _ = self.flush_connection_failure().await;
                return Err(error);
            }
            state = self.connection_state()?;
            state.refresh_at = self.now()?.saturating_add(30);
            self.persist_connections(&state)?;
        }
        let _ = self.flush_connection_failure().await;
        // Failure reporting may select a different route. Read the immutable ACK again.
        state = self.connection_state()?;
        if !state.confirmed {
            if state.registration != Some(binding) {
                state.registration = Some(binding);
                self.persist_connections(&state)?;
            }
            let url = self
                .connection_origin()?
                .join("api/v1/agent/enroll/connections/acknowledgements")
                .map_err(|_| Error::Configuration)?;
            let (status, bytes) = self
                .agent_json_transport(
                    self.http
                        .post(url)
                        .header("content-type", "application/json")
                        .body(crate::store::encode(&state.ack)?)
                        .timeout(self.store.cfg.limits.request_timeout),
                    MAX_FILE_BYTES,
                )
                .await?;
            if status != reqwest::StatusCode::NO_CONTENT || !bytes.is_empty() {
                return Err(Error::Protocol);
            }
            state.confirmed = true;
            self.persist_connections(&state)?;
        }
        Ok(())
    }
    async fn flush_connection_failure(&self) -> Result<(), Error> {
        let mut state = self.connection_state()?;
        let Some(failure) = &state.failure else {
            return Ok(());
        };
        let url = self
            .connection_origin()?
            .join("api/v1/agent/enroll/connections/failures")
            .map_err(|_| Error::Configuration)?;
        let (status, bytes) = self
            .agent_json_transport(
                self.http
                    .post(url)
                    .header("content-type", "application/json")
                    .body(crate::store::encode(failure)?)
                    .timeout(self.store.cfg.limits.request_timeout),
                MAX_FILE_BYTES,
            )
            .await?;
        if status != reqwest::StatusCode::NO_CONTENT || !bytes.is_empty() {
            return Err(Error::Protocol);
        }
        state.failure = None;
        self.persist_connections(&state)
    }
}

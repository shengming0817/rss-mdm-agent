//! Separate installation of signing trust, local source qualification, and report settlement.
use super::*;
use crate::http as h;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rss_mdm_agent_wire::{runtime::AccessConfiguration, task_trust};
use std::collections::{BTreeMap, BTreeSet};
use wire::{ApplicationState, ConfigurationField, DeliveryPermission, SourceState};

/// A complete machine-owned source observation. A caller cannot qualify network content as managed.
pub struct ManagedSnapshot {
    /// Completeness and validity of the machine read.
    pub state: SourceState,
    /// All machine-delivered candidates in this observation.
    pub sources: Vec<ManagedSource>,
}
/// Envelope discovered through the machine management channel.
pub struct ManagedSource {
    /// Original signed bytes; provenance alone never grants signing authority.
    pub envelope: wire::SignedConfiguration,
}
/// Machine policy publication and readback supplied by the installed module.
pub trait PolicyConsumer {
    /// Read the complete protected machine source before evaluating a backend candidate.
    fn managed_snapshot(&self) -> ManagedSnapshot;
    /// Publish or withdraw effective client settings and independently read the active selection.
    fn apply(
        &mut self,
        settings: Option<&AccessConfiguration>,
    ) -> Result<Option<AccessConfiguration>, Error>;
    /// Independent observations of binding and factor readiness.
    fn health(&self) -> (wire::Health, wire::Health);
}

fn verify(
    envelope: &wire::SignedConfiguration,
    trust: &wire::ConfigurationTrust,
    id: &wire::ReportIdentity,
    now: i64,
    delivery: DeliveryPermission,
) -> Result<wire::Configuration, Error> {
    let key = trust
        .keys
        .iter()
        .find(|k| {
            k.id == envelope.key_id
                && matches!(
                    k.state,
                    task_trust::KeyState::Active | task_trust::KeyState::VerifyOnly
                )
        })
        .ok_or(Error::Untrusted)?;
    let public = URL_SAFE_NO_PAD
        .decode(&key.public_key)
        .map_err(|_| Error::Untrusted)?;
    let sig = URL_SAFE_NO_PAD
        .decode(&envelope.signature)
        .map_err(|_| Error::Untrusted)?;
    let bytes = wire::SignedConfiguration::signing_bytes(&envelope.payload, &envelope.key_id)?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public)
        .verify(&bytes, &sig)
        .map_err(|_| Error::Untrusted)?;
    let config = envelope.decode()?;
    if !config.permits(id, &id.device_id)
        || config.trust_revision > trust.revision
        || config.issued_at > now
        || config.expires_at <= now
        || config.delivery != delivery
    {
        return Err(Error::Untrusted);
    }
    Ok(config)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Current {
    state: SourceState,
    publication: Option<serde_json::Value>,
    artifact: Option<wire::SignedConfiguration>,
}
type Locks = BTreeMap<Uuid, BTreeSet<ConfigurationField>>;

impl<S: IdentityProvider, C: Clock> Client<S, C> {
    async fn install_trust(
        &self,
        http: &reqwest::Client,
        id: &wire::ReportIdentity,
    ) -> Result<wire::ConfigurationTrust, Error> {
        let trust = self.call::<h::DaTrust>(http, None, None).await?;
        let digest = trust.installation_digest()?;
        if trust.service_id != id.service_id || trust.tenant != id.tenant {
            return Err(Error::Identity);
        }
        let old: Option<wire::ConfigurationTrust> = read(&self.journal.db, "trust")?;
        let mut known: BTreeMap<String, String> =
            read(&self.journal.db, "known-keys")?.unwrap_or_default();
        if let Some(old) = old {
            if old.revision > trust.revision
                || old.revision == trust.revision && old.installation_digest()? != digest
            {
                return Err(Error::Conflict);
            }
        }
        for k in &trust.keys {
            if known
                .insert(k.id.clone(), k.public_key.clone())
                .is_some_and(|v| v != k.public_key)
            {
                return Err(Error::Untrusted);
            }
        }
        let tx = self.journal.db.unchecked_transaction()?;
        write(&tx, "trust", &trust)?;
        write(&tx, "known-keys", &known)?;
        tx.commit()?;
        let installed: wire::ConfigurationTrust =
            read(&self.journal.db, "trust")?.ok_or(Error::Storage)?;
        if installed.installation_digest()? != digest {
            return Err(Error::Storage);
        }
        let mut ack = read::<task_trust::Acknowledgement>(&self.journal.db, "trust-ack")?
            .filter(|v| v.digest == digest && v.device_id == id.device_id);
        if ack.is_none() {
            ack = Some(task_trust::Acknowledgement {
                operation: Uuid::new_v4(),
                version: 1,
                device_id: id.device_id,
                revision: installed.revision,
                digest: digest.clone(),
            });
            write(&self.journal.db, "trust-ack", &ack)?;
        }
        let ack = ack.ok_or(Error::Storage)?;
        if read::<Uuid>(&self.journal.db, "trust-confirmed")? != Some(ack.operation) {
            self.call::<h::DaTrustAck>(http, None, Some(&ack)).await?;
            write(&self.journal.db, "trust-confirmed", &ack.operation)?;
        }
        Ok(installed)
    }
    pub(super) async fn trust_and_reports(
        &self,
        http: &reqwest::Client,
        registration: &wire::Registration,
        pki: &pki::Trust,
        consumer: &mut impl PolicyConsumer,
        snapshot: ManagedSnapshot,
    ) -> Result<(), Error> {
        let id = wire::ReportIdentity {
            service_id: registration.service_id,
            tenant: registration.tenant,
            device_id: registration.device,
        };
        let digest = pki.installation_digest();
        let ack = match read::<pki::Acknowledgement>(&self.journal.db, "pki-ack")? {
            Some(old) if old.digest == digest => old,
            _ => {
                let a = pki::Acknowledgement {
                    operation: Uuid::new_v4(),
                    version: 1,
                    revision: pki.revision,
                    digest,
                };
                write(&self.journal.db, "pki-ack", &a)?;
                a
            }
        };
        if read::<Uuid>(&self.journal.db, "pki-confirmed")? != Some(ack.operation) {
            self.call::<h::DaPkiAck>(http, None, Some(&ack)).await?;
            write(&self.journal.db, "pki-confirmed", &ack.operation)?;
        }
        self.flush(http).await?;
        let trust = self.install_trust(http, &id).await?;
        let value = self.call::<h::DaConfiguration>(http, None, None).await?;
        let current: Current = serde_json::from_value(value).map_err(|_| Error::Protocol)?;
        let result = self
            .configure(http, &id, &trust, current, snapshot, consumer)
            .await;
        let now = self.now()?;
        let last = read::<i64>(&self.journal.db, "status-at")?.unwrap_or(0);
        let (binding, factor) = consumer.health();
        if now - last >= 300
            || result.is_err() && read::<bool>(&self.journal.db, "status-failed")? != Some(true)
        {
            let sequence = self.next("status")?;
            let event = wire::StatusReport {
                identity: id,
                sequence,
                observed_at: Some(now),
                client_version: env!("CARGO_PKG_VERSION").into(),
                contract_version: 1,
                binding,
                factor,
                health: if result.is_err() {
                    wire::Health::Blocked
                } else if binding == wire::Health::Ready && factor == wire::Health::Ready {
                    wire::Health::Ready
                } else {
                    wire::Health::Unknown
                },
                configuration_digest: read(&self.journal.db, "effective-digest")?.flatten(),
                errors: result.as_ref().err().map(diagnostic).into_iter().collect(),
            };
            event.validate()?;
            self.queue("status", sequence, &event)?;
            self.flush(http).await?;
            write(&self.journal.db, "status-at", &now)?;
            write(&self.journal.db, "status-failed", &result.is_err())?;
        }
        result
    }
    fn next(&self, kind: &str) -> Result<i64, Error> {
        read::<i64>(&self.journal.db, &format!("{kind}-sequence"))?
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(Error::Capacity)
    }
    fn queue(&self, kind: &str, sequence: i64, report: &impl Serialize) -> Result<(), Error> {
        let t = self.journal.db.unchecked_transaction()?;
        let pending = format!("{kind}-pending");
        if read::<serde_json::Value>(&t, &pending)?.is_some() {
            return Err(Error::Conflict);
        }
        write(&t, &pending, report)?;
        write(&t, &format!("{kind}-sequence"), &sequence)?;
        t.commit()?;
        Ok(())
    }
    async fn flush(&self, http: &reqwest::Client) -> Result<(), Error> {
        for kind in ["application", "status"] {
            let key = format!("{kind}-pending");
            let Some(report) = read::<serde_json::Value>(&self.journal.db, &key)? else {
                continue;
            };
            let receipt = if kind == "application" {
                self.call::<h::DaApplication>(
                    http,
                    None,
                    Some(&serde_json::from_value(report.clone()).map_err(|_| Error::Storage)?),
                )
                .await?
            } else {
                self.call::<h::DaStatus>(
                    http,
                    None,
                    Some(&serde_json::from_value(report.clone()).map_err(|_| Error::Storage)?),
                )
                .await?
            };
            let sequence = report["sequence"].as_i64().ok_or(Error::Storage)?;
            if receipt["sequence"].as_i64() != Some(sequence)
                || receipt["evidence"] != "reported"
                || receipt["receivedAt"].as_i64().is_none_or(|v| v <= 0)
            {
                return Err(Error::Protocol);
            }
            let tx = self.journal.db.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO receipts VALUES(?1,?2,?3,?4)",
                rusqlite::params![
                    kind,
                    sequence,
                    crate::store::encode(&report)?,
                    crate::store::encode(&receipt)?
                ],
            )?;
            tx.execute("DELETE FROM state WHERE name=?1", [&key])?;
            tx.commit()?;
        }
        Ok(())
    }
    async fn application(
        &self,
        http: &reqwest::Client,
        id: &wire::ReportIdentity,
        doc: &wire::Configuration,
        envelope: &wire::SignedConfiguration,
        state: ApplicationState,
        sources: &[wire::ReportedSource],
        error: Option<wire::Diagnostic>,
    ) -> Result<(), Error> {
        let discriminator = crate::store::hash(&crate::store::encode(&(
            doc.operation_id,
            &envelope.digest,
            state,
            sources,
            error,
        ))?);
        let lastkey = match state {
            ApplicationState::Received => "app-stage/received",
            ApplicationState::Validated => "app-stage/validated",
            ApplicationState::Applied | ApplicationState::Unknown | ApplicationState::Rejected => {
                "app-stage/outcome"
            }
        };
        if read::<String>(&self.journal.db, lastkey)?.as_ref() != Some(&discriminator) {
            let sequence = self.next("application")?;
            let event = wire::ApplicationReport {
                identity: id.clone(),
                operation_id: Uuid::new_v4(),
                sequence,
                artifact: doc.operation_id,
                digest: envelope.digest.clone(),
                sources: sources.to_vec(),
                state,
                error,
            };
            event.validate()?;
            let tx = self.journal.db.unchecked_transaction()?;
            if read::<serde_json::Value>(&tx, "application-pending")?.is_some() {
                return Err(Error::Conflict);
            }
            write(&tx, "application-pending", &event)?;
            write(&tx, "application-sequence", &sequence)?;
            write(&tx, lastkey, &discriminator)?;
            tx.commit()?;
        }
        self.flush(http).await
    }
    async fn configure(
        &self,
        http: &reqwest::Client,
        id: &wire::ReportIdentity,
        trust: &wire::ConfigurationTrust,
        current: Current,
        snapshot: ManagedSnapshot,
        consumer: &mut impl PolicyConsumer,
    ) -> Result<(), Error> {
        let Some(envelope) = current.artifact else {
            self.withdraw(consumer)?;
            let locks: Locks = read(&self.journal.db, "locks")?.unwrap_or_default();
            return if locks.is_empty()
                && snapshot.sources.is_empty()
                && snapshot.state == SourceState::Missing
                && matches!(current.state, SourceState::Missing | SourceState::Withdrawn)
            {
                Ok(())
            } else {
                Err(Error::Unavailable)
            };
        };
        let doc = verify(&envelope, trust, id, self.now()?, DeliveryPermission::Da)?;
        let publication = current.publication.ok_or(Error::Protocol)?;
        if current.state != SourceState::Present
            || publication["id"] != doc.id.to_string()
            || publication["version"] != doc.version.to_string()
            || publication["revision"].as_u64() != Some(doc.revision)
            || publication["enabled"] != true
        {
            return Err(Error::Conflict);
        }
        let backend_source = wire::ReportedSource {
            artifact: doc.operation_id,
            digest: envelope.digest.clone(),
            delivery: DeliveryPermission::Da,
            state: SourceState::Present,
        };
        let mut sources = vec![backend_source];
        self.application(
            http,
            id,
            &doc,
            &envelope,
            ApplicationState::Received,
            &sources,
            None,
        )
        .await?;
        let result = self.resolve(id, trust, &doc, &publication, snapshot, &mut sources);
        let effective = match result {
            Ok(value) => value,
            Err(e) => {
                consumer.apply(None)?;
                write(&self.journal.db, "effective-digest", &None::<String>)?;
                self.application(
                    http,
                    id,
                    &doc,
                    &envelope,
                    ApplicationState::Rejected,
                    &sources,
                    Some(diagnostic(&e)),
                )
                .await?;
                return Err(e);
            }
        };
        self.application(
            http,
            id,
            &doc,
            &envelope,
            ApplicationState::Validated,
            &sources,
            None,
        )
        .await?;
        let confirmed = consumer
            .apply(Some(&effective))
            .is_ok_and(|actual| actual.as_ref() == Some(&effective));
        write(
            &self.journal.db,
            "effective-digest",
            &confirmed.then_some(&envelope.digest),
        )?;
        let state = if confirmed {
            ApplicationState::Applied
        } else {
            ApplicationState::Unknown
        };
        self.application(http, id, &doc, &envelope, state, &sources, None)
            .await?;
        if confirmed {
            Ok(())
        } else {
            Err(Error::Unavailable)
        }
    }

    pub(super) fn withdraw(&self, consumer: &mut impl PolicyConsumer) -> Result<(), Error> {
        consumer.apply(None)?;
        let tx = self.journal.db.unchecked_transaction()?;
        write(&tx, "effective-digest", &None::<String>)?;
        tx.execute("DELETE FROM state WHERE name IN ('app-stage/received','app-stage/validated','app-stage/outcome')",[])?;
        tx.commit()?;
        Ok(())
    }
    pub(super) fn resolve(
        &self,
        id: &wire::ReportIdentity,
        trust: &wire::ConfigurationTrust,
        backend: &wire::Configuration,
        publication: &serde_json::Value,
        snapshot: ManagedSnapshot,
        sources: &mut Vec<wire::ReportedSource>,
    ) -> Result<AccessConfiguration, Error> {
        let key = format!("publication/{}", backend.id);
        if let Some((revision, version)) = read::<(u64, Uuid)>(&self.journal.db, &key)? {
            if revision > backend.revision
                || revision == backend.revision && version != backend.version
            {
                return Err(Error::Conflict);
            }
        }
        write(&self.journal.db, &key, &(backend.revision, backend.version))?;
        let authorized: BTreeSet<ConfigurationField> =
            serde_json::from_value(publication["managedFields"].clone())
                .map_err(|_| Error::Protocol)?;
        let mut locks: Locks = read(&self.journal.db, "locks")?.unwrap_or_default();
        if let Some(held) = locks.get_mut(&backend.id) {
            held.retain(|field| authorized.contains(field));
        }
        locks.retain(|_, v| !v.is_empty());
        write(&self.journal.db, "locks", &locks)?;
        if !matches!(snapshot.state, SourceState::Missing | SourceState::Present)
            || snapshot.sources.len() > 15
        {
            return Err(Error::Unavailable);
        }
        let mut owners = BTreeMap::new();
        let mut found = Locks::new();
        let mut effective = backend.content.clone();
        for source in snapshot.sources {
            let doc = verify(
                &source.envelope,
                trust,
                id,
                self.now()?,
                DeliveryPermission::ManagedMandatory,
            )?;
            sources.push(wire::ReportedSource {
                artifact: doc.operation_id,
                digest: source.envelope.digest,
                delivery: DeliveryPermission::ManagedMandatory,
                state: SourceState::Present,
            });
            for field in &doc.managed_fields {
                if doc.id == backend.id && !authorized.contains(field) {
                    continue;
                }
                let value = match field {
                    ConfigurationField::Mac => crate::store::encode(&(
                        &doc.content.mac,
                        &doc.native.mac,
                        &doc.native.bindings,
                    ))?,
                    ConfigurationField::Windows => crate::store::encode(&doc.content.windows)?,
                };
                found.entry(doc.id).or_default().insert(*field);
                locks.entry(doc.id).or_default().insert(*field);
                write(&self.journal.db, "locks", &locks)?;
                if owners
                    .insert(*field, value.clone())
                    .is_some_and(|other| other != value)
                {
                    return Err(Error::Conflict);
                }
                match field {
                    ConfigurationField::Mac => effective.mac = doc.content.mac.clone(),
                    ConfigurationField::Windows => effective.windows = doc.content.windows.clone(),
                }
            }
        }
        if locks.iter().any(|(policy, required)| {
            found
                .get(policy)
                .is_none_or(|seen| !required.is_subset(seen))
        }) {
            return Err(Error::Unavailable);
        }
        effective.validate()?;
        Ok(effective)
    }
}
fn diagnostic(error: &Error) -> wire::Diagnostic {
    match error {
        Error::Conflict => wire::Diagnostic::SourceConflict,
        Error::Unavailable => wire::Diagnostic::SourceUnavailable,
        Error::Denied => wire::Diagnostic::PermissionDenied,
        _ => wire::Diagnostic::InvalidConfiguration,
    }
}

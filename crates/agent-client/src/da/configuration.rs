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
/// Selected DA settings and the exact native identities whose readback can confirm them.
#[derive(Clone, Serialize, Deserialize)]
pub struct Selection {
    /// Device identity authenticated on the DA listener.
    pub identity: wire::ReportIdentity,
    /// Resolved client settings; backend authorization fields retain their original values.
    pub settings: AccessConfiguration,
    /// Platform expectations from the source that supplied the selected Mac settings.
    pub native: wire::NativeExpectation,
}
/// Machine policy publication and readback supplied by the installed module.
pub trait PolicyConsumer {
    /// Read the complete protected machine source before evaluating a backend candidate.
    fn managed_snapshot(&self) -> ManagedSnapshot;
    /// Publish or withdraw effective client settings and independently read the active selection.
    fn apply(&mut self, settings: Option<&Selection>)
        -> Result<Option<AccessConfiguration>, Error>;
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
#[derive(Serialize, Deserialize)]
pub(super) struct AppliedProof {
    pub(super) until: i64,
    sources: BTreeSet<String>,
    effective: Selection,
}

fn fingerprints(snapshot: &ManagedSnapshot) -> Result<BTreeSet<String>, Error> {
    snapshot
        .sources
        .iter()
        .map(|source| Ok(crate::store::hash(&crate::store::encode(&source.envelope)?)))
        .collect()
}
type Locks = BTreeMap<Uuid, BTreeSet<ConfigurationField>>;

impl<S: Credentials, C: Clock> Client<S, C> {
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
        let trust = self.install_trust(http, &id).await?;
        let value = self.call::<h::DaConfiguration>(http, None, None).await?;
        let current: Current = serde_json::from_value(value).map_err(|_| Error::Protocol)?;
        let result = self.configure(&id, &trust, current, snapshot, consumer);
        if matches!(
            result,
            Err(Error::Untrusted | Error::Conflict | Error::Protocol | Error::Identity)
        ) {
            self.withdraw(consumer)?;
        }
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
        let tx = self.journal.db.unchecked_transaction()?;
        Self::enqueue(&tx, kind, sequence, report)?;
        tx.commit()?;
        Ok(())
    }
    fn enqueue(
        tx: &rusqlite::Connection,
        kind: &str,
        sequence: i64,
        report: &impl Serialize,
    ) -> Result<(), Error> {
        let pending: i64 = tx.query_row(
            "SELECT count(*) FROM reports WHERE body IS NOT NULL AND error IS NULL",
            [],
            |r| r.get(0),
        )?;
        if pending >= 256 {
            return Err(Error::Capacity);
        }
        let body = crate::store::encode(report)?;
        let value: serde_json::Value = crate::store::decode(&body)?;
        tx.execute(
            "INSERT INTO reports(kind,sequence,body,operation) VALUES(?1,?2,?3,?4)",
            rusqlite::params![kind, sequence, body, value["operationId"].as_str()],
        )?;
        write(tx, &format!("{kind}-sequence"), &sequence)?;
        Ok(())
    }
    pub(super) async fn flush(&self, http: &reqwest::Client) -> Result<(), Error> {
        use rusqlite::OptionalExtension;
        for kind in ["application", "status"] {
            for _ in 0..16 {
                let pending:Option<(i64,Vec<u8>)>=self.journal.db.query_row("SELECT sequence,body FROM reports WHERE kind=?1 AND body IS NOT NULL AND error IS NULL ORDER BY sequence LIMIT 1",[kind],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
                let Some((sequence, body)) = pending else {
                    break;
                };
                let result = if kind == "application" {
                    self.call::<h::DaApplication>(http, None, Some(&crate::store::decode(&body)?))
                        .await
                } else {
                    self.call::<h::DaStatus>(http, None, Some(&crate::store::decode(&body)?))
                        .await
                };
                let receipt = match result {
                    Ok(v) => v,
                    Err(Error::Conflict) => {
                        self.journal.db.execute(
                            "UPDATE reports SET error='conflict' WHERE kind=?1 AND sequence=?2",
                            rusqlite::params![kind, sequence],
                        )?;
                        continue;
                    }
                    Err(e) => return Err(e),
                };
                if receipt.as_object().is_none_or(|object| object.len() != 3)
                    || receipt["sequence"].as_i64() != Some(sequence)
                    || receipt["evidence"] != "reported"
                    || receipt["receivedAt"].as_i64().is_none_or(|v| v <= 0)
                {
                    return Err(Error::Protocol);
                }
                // Preserve the exact receipt but release the full payload after acceptance.
                self.journal.db.execute(
                    "UPDATE reports SET body=NULL,receipt=?1 WHERE kind=?2 AND sequence=?3",
                    rusqlite::params![crate::store::encode(&receipt)?, kind, sequence],
                )?;
            }
        }
        Ok(())
    }
    fn application(
        &self,
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
            Self::enqueue(&tx, "application", sequence, &event)?;
            write(&tx, lastkey, &discriminator)?;
            tx.commit()?;
        }
        Ok(())
    }
    pub(super) fn check_local(
        &self,
        snapshot: &ManagedSnapshot,
        consumer: &mut impl PolicyConsumer,
    ) -> Result<(), Error> {
        let Some(proof) =
            read::<Option<AppliedProof>>(&self.journal.db, "applied-proof")?.flatten()
        else {
            return Ok(());
        };
        if !matches!(snapshot.state, SourceState::Missing | SourceState::Present)
            || fingerprints(snapshot)? != proof.sources
            || self.now()? >= proof.until
            || !consumer
                .apply(Some(&proof.effective))
                .is_ok_and(|v| v.as_ref() == Some(&proof.effective.settings))
        {
            self.withdraw(consumer)?;
        }
        Ok(())
    }
    fn configure(
        &self,
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
            id,
            &doc,
            &envelope,
            ApplicationState::Received,
            &sources,
            None,
        )?;
        let source_ids = fingerprints(&snapshot)?;
        let mut native = doc.native.clone();
        let authorized: BTreeSet<ConfigurationField> =
            serde_json::from_value(publication["managedFields"].clone())
                .map_err(|_| Error::Protocol)?;
        for source in &snapshot.sources {
            let managed = source.envelope.decode()?;
            if managed.managed_fields.contains(&ConfigurationField::Mac)
                && (managed.id != doc.id || authorized.contains(&ConfigurationField::Mac))
            {
                native = managed.native;
            }
        }
        let mut until = doc.expires_at;
        for source in &snapshot.sources {
            until = until.min(source.envelope.decode()?.expires_at);
        }
        let document: pki::Trust = read(&self.journal.db, "pki")?.ok_or(Error::Identity)?;
        let pki = VerifiedTrust::device(
            document,
            self.backend.instance,
            self.backend.tenant,
            pki::Purpose::DaDevice,
            self.now()?,
        )?;
        let active = self.active()?.ok_or(Error::Identity)?;
        let (_, cert_until, _) = crate::tls::device_validity(
            &active.chain,
            &active.key,
            &self.secrets,
            self.now()?,
            &pki,
        )?;
        until = until.min(pki.expires).min(cert_until);
        let result = self.resolve(id, trust, &doc, &publication, snapshot, &mut sources);
        let effective = match result {
            Ok(value) => value,
            Err(e) => {
                consumer.apply(None)?;
                write(&self.journal.db, "effective-digest", &None::<String>)?;
                self.application(
                    id,
                    &doc,
                    &envelope,
                    ApplicationState::Rejected,
                    &sources,
                    Some(diagnostic(&e)),
                )?;
                return Err(e);
            }
        };
        self.application(
            id,
            &doc,
            &envelope,
            ApplicationState::Validated,
            &sources,
            None,
        )?;
        let selected = Selection {
            identity: id.clone(),
            settings: effective.clone(),
            native,
        };
        // Publish the bounds before the native owner can expose the selected settings.
        write(
            &self.journal.db,
            "applied-proof",
            &Some(AppliedProof {
                until,
                sources: source_ids,
                effective: selected.clone(),
            }),
        )?;
        let confirmed = consumer
            .apply(Some(&selected))
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
        self.application(id, &doc, &envelope, state, &sources, None)?;
        if confirmed {
            Ok(())
        } else {
            Err(Error::Unavailable)
        }
    }

    pub(super) fn withdraw(&self, consumer: &mut impl PolicyConsumer) -> Result<(), Error> {
        let tx = self.journal.db.unchecked_transaction()?;
        write(&tx, "applied-proof", &None::<AppliedProof>)?;
        write(&tx, "effective-digest", &None::<String>)?;
        tx.execute("DELETE FROM state WHERE name IN ('app-stage/received','app-stage/validated','app-stage/outcome')",[])?;
        tx.commit()?;
        consumer.apply(None)?;
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
                let watermark = format!("managed/{}/{field:?}", doc.id);
                let digest = crate::store::hash(&value);
                if let Some((revision, version, previous_digest)) =
                    read::<(u64, Uuid, String)>(&self.journal.db, &watermark)?
                {
                    if revision > doc.revision
                        || revision == doc.revision
                            && (version != doc.version || previous_digest != digest)
                    {
                        return Err(Error::Conflict);
                    }
                }
                write(
                    &self.journal.db,
                    &watermark,
                    &(doc.revision, doc.version, digest),
                )?;
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

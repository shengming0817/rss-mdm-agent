//! Offline Windows RSS factor owner. No network, IPC, or Windows password storage.
//! ref: RFC 6238 §4–5; SQLite lang_transaction.html (BEGIN IMMEDIATE).
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{digest, hmac};
use rss_mdm_agent_wire::runtime::{
    AccessAccount, AccessRecoveryUse, AccessRssFacts, CredentialSubject,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zeroize::Zeroizing;

pub mod hello;
mod registration;
mod storage;
pub use registration::Registration;

/// Canonical Entra object coordinate encoded in a Windows cloud SID; not proof of its issuer.
pub fn entra_object_id(sid: &str) -> Option<Uuid> {
    let mut words = sid.strip_prefix("S-1-12-1-")?.split('-');
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_exact_mut(4) {
        let text = words.next()?;
        let value: u32 = text.parse().ok()?;
        if value.to_string() != text {
            return None;
        }
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    let id = Uuid::from_bytes_le(bytes);
    (words.next().is_none() && !id.is_nil()).then_some(id)
}
#[cfg(test)]
mod tests;
#[cfg(windows)]
pub mod windows;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("offline storage unavailable")]
    Storage,
    #[error("invalid factor state")]
    Invalid,
    #[error("factor authority changed")]
    Changed,
}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}

/// Implementations must bind ciphertext to the SYSTEM identity and this installation.
pub trait Protector {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, Error>;
    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error>;
}
#[cfg(windows)]
pub struct SystemProtector;
#[cfg(windows)]
impl Protector for SystemProtector {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
        windows::require_system()?;
        Ok(platform_credentials::protect_key(plaintext)?)
    }
    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        windows::require_system()?;
        Ok(Zeroizing::new(platform_credentials::unprotect_key(
            ciphertext,
        )?))
    }
}

/// UTC seconds, monotonic seconds, and OS boot identity are sampled together by native ingress.
#[derive(Clone, Copy)]
pub struct Clock {
    pub utc: i64,
    pub monotonic: u64,
    pub boot: Uuid,
}
impl Clock {
    fn validate(self) -> Result<(), Error> {
        if self.utc <= 0 || self.monotonic > i64::MAX as u64 || self.boot.is_nil() {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    pub instance: Uuid,
    pub principal: Uuid,
    pub source: rss_mdm_agent_wire::runtime::AccessWindowsAuthority,
    pub login: String,
    pub join: String,
    pub tenant: Uuid,
    pub device: String,
    pub registration: Uuid,
    pub registration_generation: u64,
    pub binding: Uuid,
    pub binding_generation: i64,
    pub factor_generation: i64,
    pub sid: String,
}
impl Authority {
    fn validate(&self) -> Result<(), Error> {
        AccessAccount::Windows {
            sid: self.sid.clone(),
            authority: self.source.clone(),
        }
        .validate()
        .map_err(|_| Error::Invalid)?;
        if self.instance.is_nil()
            || self.principal.is_nil()
            || self.join.is_empty()
            || self.join.len() > 512
            || self.join.chars().any(char::is_control)
            || self.login.is_empty()
            || self.login.len() > 512
            || self.login.chars().any(char::is_control)
            || self.tenant.is_nil()
            || self.registration.is_nil()
            || self.binding.is_nil()
            || self.device.is_empty()
            || self.device.len() > 256
            || self.device.chars().any(char::is_control)
            || self.registration_generation == 0
            || self.binding_generation < 1
            || self.factor_generation < 1
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    fn accepts(&self, facts: &AccessRssFacts) -> bool {
        facts.factor_generation == self.factor_generation
            && facts.recovery.validate().is_ok()
            && facts.recovery.subject
                == CredentialSubject::RssLogin {
                    binding: self.binding,
                    binding_generation: self.binding_generation,
                    sid: self.sid.clone(),
                }
    }
}

/// Never Debug: plaintext exists only during setup and the existing escrow encryption call.
pub struct Candidate {
    secret: Zeroizing<Vec<u8>>,
    recovery: Zeroizing<String>,
    confirmed_step: Option<i64>,
}
impl Candidate {
    pub fn generate() -> Result<Self, Error> {
        let secret = Zeroizing::new(platform_credentials::random_bytes(20)?);
        let bytes = Zeroizing::new(platform_credentials::random_bytes(32)?);
        Ok(Self {
            secret,
            recovery: Zeroizing::new(URL_SAFE_NO_PAD.encode(&*bytes)),
            confirmed_step: None,
        })
    }
    pub fn totp_secret(&self) -> &[u8] {
        &self.secret
    }
    pub fn recovery_secret(&self) -> &str {
        &self.recovery
    }
    pub fn confirm(&mut self, code: &str, clock: Clock) -> Result<bool, Error> {
        clock.validate()?;
        self.confirmed_step = matching_step(&self.secret, code, clock.utc, -1);
        Ok(self.confirmed_step.is_some())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SealedCandidate {
    secret: Vec<u8>,
    recovery: String,
    confirmed_step: i64,
}
impl Drop for SealedCandidate {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.secret.zeroize();
        self.recovery.zeroize();
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    authority: Authority,
    revision: i64,
    enabled: bool,
    revoked: bool,
    secret: Vec<u8>, // DPAPI ciphertext, never a clear TOTP key.
    recovery_hash: Vec<u8>,
    recovery_secret: Vec<u8>,
    facts: AccessRssFacts,
    consumed_step: i64,
    failures: u32,
    lock_seconds: u64,
    boot: Uuid,
    monotonic: u64,
    utc: i64,
    lock_until: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Accepted,
    Denied,
    Locked,
    ClockRollback,
    Disabled,
}
pub enum Factor<'a> {
    Totp(&'a str),
    Recovery(&'a str),
}

/// Each call holds an immediate transaction through durable factor consumption.
/// A caller may serialize the Windows password only after Accepted is returned.
pub struct Store {
    conn: Connection,
    path: PathBuf,
    _directory: platform_private_storage::PrivateDirectory,
}
impl Store {
    pub fn create(root: &Path, installation: Uuid) -> Result<Self, Error> {
        storage::open(root, installation, true)
    }
    /// LogonUI uses Existing exclusively; a lost/corrupt database never resets counters.
    pub fn open(root: &Path, installation: Uuid) -> Result<Self, Error> {
        storage::open(root, installation, false)
    }
    pub fn stage(
        &mut self,
        operation: Uuid,
        candidate: &Candidate,
        protector: &impl Protector,
    ) -> Result<(), Error> {
        if operation.is_nil() {
            return Err(Error::Invalid);
        }
        let confirmed_step = candidate.confirmed_step.ok_or(Error::Invalid)?;
        let plain = SealedCandidate {
            secret: candidate.secret.to_vec(),
            recovery: candidate.recovery.to_string(),
            confirmed_step,
        };
        let bytes = Zeroizing::new(serde_json::to_vec(&plain).map_err(|_| Error::Invalid)?);
        let sealed = protector.seal(&bytes)?;
        self.check_files()?;
        // Immutable operation identity: a retry must load its original candidate.
        self.conn.execute(
            "INSERT INTO candidates VALUES(?1,?2)",
            params![operation.to_string(), sealed],
        )?;
        Ok(())
    }
    pub fn reconfirm(
        &mut self,
        operation: Uuid,
        code: &str,
        clock: Clock,
        protector: &impl Protector,
    ) -> Result<bool, Error> {
        clock.validate()?;
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sealed: Vec<u8> = tx.query_row(
            "SELECT body FROM candidates WHERE id=?1 AND length(body)<=16384",
            [operation.to_string()],
            |r| r.get(0),
        )?;
        let bytes = protector.open(&sealed)?;
        let mut plain: SealedCandidate =
            serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
        if plain.secret.len() != 20 || !valid_recovery(&plain.recovery) {
            return Err(Error::Invalid);
        }
        let Some(step) = matching_step(&plain.secret, code, clock.utc, -1) else {
            return Ok(false);
        };
        plain.confirmed_step = plain.confirmed_step.max(step);
        let bytes = Zeroizing::new(encode(&plain)?);
        tx.execute(
            "UPDATE candidates SET body=?2 WHERE id=?1",
            params![operation.to_string(), protector.seal(&bytes)?],
        )?;
        tx.commit()?;
        Ok(true)
    }
    pub fn retire_candidate(&mut self, operation: Uuid) -> Result<(), Error> {
        self.check_files()?;
        self.conn.execute(
            "DELETE FROM candidates WHERE id=?1",
            [operation.to_string()],
        )?;
        Ok(())
    }
    /// Replace only recovery material under an exact started credential task. TOTP state survives.
    pub fn rotate_recovery(
        &mut self,
        task: &rss_mdm_agent_wire::runtime::CredentialTaskSpec,
        subject: &CredentialSubject,
        recovery: &str,
        receipt: rss_mdm_agent_wire::runtime::CredentialReceipt,
        protector: &impl Protector,
    ) -> Result<(), Error> {
        task.validate().map_err(|_| Error::Invalid)?;
        receipt.validate().map_err(|_| Error::Invalid)?;
        if task.permit != rss_mdm_agent_wire::runtime::TaskPermit::Start
            || receipt.subject != *subject
            || !valid_recovery(recovery)
        {
            return Err(Error::Invalid);
        }
        let CredentialSubject::RssLogin {
            binding,
            binding_generation,
            sid,
        } = subject
        else {
            return Err(Error::Invalid);
        };
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut record = load(&tx, sid)?.ok_or(Error::Changed)?;
        registration::admit(&tx, &record.authority)?;
        hello::require_rss(&tx, &record.authority.sid)?;
        if record.revoked {
            return Err(Error::Changed);
        }
        let authority = &record.authority;
        if authority.binding != *binding
            || authority.binding_generation != *binding_generation
            || authority.tenant != task.tenant_id
            || authority.device != task.device_id
            || authority.registration != task.registration_id
            || authority.registration_generation != task.generation
        {
            return Err(Error::Changed);
        }
        use rss_mdm_agent_wire::runtime::CredentialOperation as Op;
        if !matches!(&task.operation, Op::RotateRssLogin { binding: b, binding_generation: g, sid: s } if b == binding && g == binding_generation && s == sid)
        {
            return Err(Error::Invalid);
        }
        let hash = recovery_hash(authority, recovery);
        if record.facts.recovery == receipt {
            return if constant_equal(&record.recovery_hash, &hash) {
                Ok(())
            } else {
                Err(Error::Changed)
            };
        }
        if receipt.material != record.facts.recovery.material
            || receipt.version <= record.facts.recovery.version
        {
            return Err(Error::Changed);
        }
        record.recovery_hash = hash;
        record.recovery_secret = protector.seal(recovery.as_bytes())?;
        record.facts.recovery = receipt;
        save(&tx, &record)?;
        tx.commit()?;
        Ok(())
    }
    pub fn recovery_material(
        &self,
        task: &rss_mdm_agent_wire::runtime::CredentialTaskSpec,
        subject: &CredentialSubject,
        protector: &impl Protector,
    ) -> Result<
        (
            Zeroizing<String>,
            rss_mdm_agent_wire::runtime::CredentialReceipt,
        ),
        Error,
    > {
        task.validate().map_err(|_| Error::Invalid)?;
        use rss_mdm_agent_wire::runtime::{CredentialOperation as Op, TaskPermit};
        if task.permit != TaskPermit::Start
            || !matches!((&task.operation, subject),
            (Op::RotateRssLogin { binding: b, binding_generation: g, sid: s } | Op::VerifyRssLogin { binding: b, binding_generation: g, sid: s },
             CredentialSubject::RssLogin { binding, binding_generation, sid }) if b == binding && g == binding_generation && s == sid)
        {
            return Err(Error::Invalid);
        }
        self.check_files()?;
        let CredentialSubject::RssLogin {
            binding,
            binding_generation,
            sid,
        } = subject
        else {
            return Err(Error::Invalid);
        };
        let record = load(&self.conn, sid)?.ok_or(Error::Changed)?;
        registration::admit(&self.conn, &record.authority)?;
        if record.revoked {
            return Err(Error::Changed);
        }
        let a = &record.authority;
        if a.tenant != task.tenant_id
            || a.device != task.device_id
            || a.registration != task.registration_id
            || a.registration_generation != task.generation
            || a.binding != *binding
            || a.binding_generation != *binding_generation
        {
            return Err(Error::Changed);
        }
        let bytes = protector.open(&record.recovery_secret)?;
        let secret = Zeroizing::new(
            std::str::from_utf8(&bytes)
                .map_err(|_| Error::Invalid)?
                .to_owned(),
        );
        if !valid_recovery(&secret)
            || !constant_equal(&record.recovery_hash, &recovery_hash(a, &secret))
        {
            return Err(Error::Invalid);
        }
        Ok((secret, record.facts.recovery))
    }
    pub fn verify_recovery_material(
        &self,
        task: &rss_mdm_agent_wire::runtime::CredentialTaskSpec,
        subject: &CredentialSubject,
        recovery: &str,
        receipt: &rss_mdm_agent_wire::runtime::CredentialReceipt,
    ) -> Result<bool, Error> {
        task.validate().map_err(|_| Error::Invalid)?;
        use rss_mdm_agent_wire::runtime::{CredentialOperation as Op, TaskPermit};
        if task.permit != TaskPermit::Start
            || !matches!((&task.operation, subject),
            (Op::RotateRssLogin { binding: b, binding_generation: g, sid: s } | Op::VerifyRssLogin { binding: b, binding_generation: g, sid: s },
             CredentialSubject::RssLogin { binding, binding_generation, sid }) if b == binding && g == binding_generation && s == sid)
        {
            return Err(Error::Invalid);
        }
        self.check_files()?;
        let CredentialSubject::RssLogin {
            binding,
            binding_generation,
            sid,
        } = subject
        else {
            return Err(Error::Invalid);
        };
        let record = load(&self.conn, sid)?.ok_or(Error::Changed)?;
        registration::admit(&self.conn, &record.authority)?;
        if record.revoked {
            return Err(Error::Changed);
        }
        let a = &record.authority;
        Ok(a.tenant == task.tenant_id
            && a.device == task.device_id
            && a.registration == task.registration_id
            && a.registration_generation == task.generation
            && a.binding == *binding
            && a.binding_generation == *binding_generation
            && &record.facts.recovery == receipt
            && constant_equal(&record.recovery_hash, &recovery_hash(a, recovery)))
    }
    pub fn has_candidate(&self, operation: Uuid) -> Result<bool, Error> {
        self.check_files()?;
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM candidates WHERE id=?1)",
            [operation.to_string()],
            |r| r.get(0),
        )?)
    }
    pub fn candidate(
        &self,
        operation: Uuid,
        protector: &impl Protector,
    ) -> Result<Candidate, Error> {
        self.check_files()?;
        let sealed: Vec<u8> = self.conn.query_row(
            "SELECT body FROM candidates WHERE id=?1 AND length(body)<=16384",
            [operation.to_string()],
            |r| r.get(0),
        )?;
        let bytes = protector.open(&sealed)?;
        let plain: SealedCandidate = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
        if plain.secret.len() != 20 || !valid_recovery(&plain.recovery) || plain.confirmed_step < 0
        {
            return Err(Error::Invalid);
        }
        Ok(Candidate {
            secret: Zeroizing::new(plain.secret.clone()),
            recovery: Zeroizing::new(plain.recovery.clone()),
            confirmed_step: Some(plain.confirmed_step),
        })
    }
    /// Only the worker with an authenticated escrow ACK may call this method.
    // Keep the distinct authority, operation, receipt and sampled clock explicit at ingress.
    #[allow(clippy::too_many_arguments)]
    pub fn activate(
        &mut self,
        authority: Authority,
        revision: i64,
        enabled: bool,
        operation: Uuid,
        facts: AccessRssFacts,
        clock: Clock,
        protector: &impl Protector,
    ) -> Result<(), Error> {
        authority.validate()?;
        clock.validate()?;
        if revision < 1 || !authority.accepts(&facts) {
            return Err(Error::Invalid);
        }
        let candidate = self.candidate(operation, protector)?;
        let secret = protector.seal(&candidate.secret)?;
        let mut record = Record {
            recovery_hash: recovery_hash(&authority, &candidate.recovery),
            recovery_secret: protector.seal(candidate.recovery.as_bytes())?,
            authority,
            revision,
            enabled,
            revoked: false,
            secret,
            facts,
            consumed_step: candidate.confirmed_step.ok_or(Error::Invalid)?,
            failures: 0,
            lock_seconds: 0,
            boot: clock.boot,
            monotonic: clock.monotonic,
            utc: clock.utc,
            lock_until: 0,
        };
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        registration::admit(&tx, &record.authority)?;
        if let Some(old) = load(&tx, &record.authority.sid)? {
            if old.authority == record.authority
                && old.facts == record.facts
                && old.revision == revision
                && old.enabled == enabled
            {
                // Do not reset consumed steps or failure budgets on delivery replay.
                return Ok(());
            }
            if !old.revoked
                && (old.authority.tenant != record.authority.tenant
                    || old.authority.instance != record.authority.instance
                    || old.authority.principal != record.authority.principal
                    || old.authority.source != record.authority.source
                    || old.authority.join != record.authority.join
                    || old.authority.registration != record.authority.registration
                    || old.authority.registration_generation
                        != record.authority.registration_generation
                    || old.authority.binding != record.authority.binding
                    || old.authority.binding_generation > record.authority.binding_generation
                    || old.authority.factor_generation >= record.authority.factor_generation
                    || old.revision >= revision)
            {
                return Err(Error::Changed);
            }
            if old.authority.binding == record.authority.binding
                && old.authority.factor_generation >= record.authority.factor_generation
            {
                return Err(Error::Changed);
            }
            // A new factor cannot erase the native account's durable failure budget or clock floor.
            record.failures = old.failures;
            record.lock_seconds = old.lock_seconds;
            record.lock_until = if old.boot == clock.boot {
                old.lock_until
            } else {
                clock.monotonic.saturating_add(old.lock_seconds)
            };
            record.utc = old.utc.max(clock.utc);
            if old.boot == clock.boot && clock.monotonic < old.monotonic {
                return Err(Error::Changed);
            }
        }
        save(&tx, &record)?;
        tx.commit()?;
        // Readback is required before reporting Applied.
        let actual = self.facts(&record.authority)?;
        if actual != record.facts {
            return Err(Error::Storage);
        }
        Ok(())
    }
    pub fn configure(
        &mut self,
        authority: &Authority,
        revision: i64,
        enabled: bool,
        facts: &AccessRssFacts,
    ) -> Result<(), Error> {
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut record = load(&tx, &authority.sid)?.ok_or(Error::Changed)?;
        registration::admit(&tx, authority)?;
        if record.revoked
            || record.authority != *authority
            || record.facts != *facts
            || revision < record.revision
        {
            return Err(Error::Changed);
        }
        record.revision = revision;
        record.enabled = enabled;
        save(&tx, &record)?;
        tx.commit()?;
        Ok(())
    }
    pub fn revoke(&mut self, authority: &Authority, revision: i64) -> Result<(), Error> {
        authority.validate()?;
        if revision < 1 {
            return Err(Error::Invalid);
        }
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        registration::require(&tx, authority)?;
        if let Some(mut record) = load(&tx, &authority.sid)? {
            if record.authority != *authority
                || record.revision > revision
                || (record.revision == revision && !record.revoked)
            {
                return Err(Error::Changed);
            }
            record.revision = revision;
            record.enabled = false;
            record.revoked = true;
            save(&tx, &record)?;
        }
        registration::record_revocation(&tx, authority, revision)?;
        tx.commit()?;
        Ok(())
    }
    pub fn enabled(&self, sid: &str) -> Result<bool, Error> {
        Ok(self.authority(sid)?.is_some())
    }
    pub fn facts(&self, authority: &Authority) -> Result<AccessRssFacts, Error> {
        self.check_files()?;
        let record = load(&self.conn, &authority.sid)?.ok_or(Error::Changed)?;
        if record.revoked || record.authority != *authority {
            return Err(Error::Changed);
        }
        Ok(record.facts)
    }
    pub fn verify(
        &mut self,
        expected: &Authority,
        factor: Factor<'_>,
        clock: Clock,
        protector: &impl Protector,
    ) -> Result<Decision, Error> {
        expected.validate()?;
        let sid = &expected.sid;
        clock.validate()?;
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(mut record) = load(&tx, sid)? else {
            return Ok(Decision::Disabled);
        };
        registration::require(&tx, &record.authority)?;
        if record.revoked || !record.enabled {
            return Ok(Decision::Disabled);
        }
        if &record.authority != expected {
            return Err(Error::Changed);
        }
        let same_boot = record.boot == clock.boot;
        let monotonic_ok = !same_boot || clock.monotonic >= record.monotonic;
        // Reboot cannot erase a pending delay. Restart its bounded duration on the new boot.
        if !same_boot {
            record.lock_until = clock.monotonic.saturating_add(record.lock_seconds);
        }
        let rollback = clock.utc < record.utc || !monotonic_ok;
        let decision = if !monotonic_ok || clock.monotonic < record.lock_until {
            Decision::Locked
        } else {
            record.lock_seconds = 0;
            let accepted = match factor {
                Factor::Totp(code) if !rollback => {
                    let secret = protector.open(&record.secret)?;
                    if secret.len() != 20 {
                        return Err(Error::Invalid);
                    }
                    match matching_step(&secret, code, clock.utc, record.consumed_step) {
                        Some(step) => {
                            record.consumed_step = step;
                            true
                        }
                        None => false,
                    }
                }
                Factor::Totp(_) => false,
                Factor::Recovery(code) => {
                    valid_recovery(code)
                        && constant_equal(
                            &record.recovery_hash,
                            &recovery_hash(&record.authority, code),
                        )
                }
            };
            if accepted {
                record.failures = 0;
                record.lock_until = 0;
                if matches!(factor, Factor::Recovery(_)) {
                    let event = AccessRecoveryUse {
                        operation_id: Uuid::new_v4(),
                        material: record.facts.recovery.material,
                        version: record.facts.recovery.version,
                        used_at: clock.utc,
                    };
                    tx.execute(
                        "INSERT INTO recovery_uses VALUES(?1,?2)",
                        params![event.operation_id.to_string(), encode(&event)?],
                    )?;
                }
                Decision::Accepted
            } else {
                record.failures = record.failures.saturating_add(1);
                if record.failures >= 5 {
                    record.lock_seconds = (30_u64 << (record.failures - 5).min(4)).min(300);
                    record.lock_until = clock.monotonic.saturating_add(record.lock_seconds);
                }
                if rollback {
                    Decision::ClockRollback
                } else {
                    Decision::Denied
                }
            }
        };
        record.utc = record.utc.max(clock.utc);
        if monotonic_ok {
            record.monotonic = clock.monotonic;
        }
        record.boot = clock.boot;
        save(&tx, &record)?;
        tx.commit()?;
        Ok(decision)
    }
    pub fn recovery_uses(&self) -> Result<Vec<AccessRecoveryUse>, Error> {
        self.check_files()?;
        let mut query = self
            .conn
            .prepare("SELECT body FROM recovery_uses ORDER BY rowid LIMIT 128")?;
        let rows = query.query_map([], |r| r.get::<_, Vec<u8>>(0))?;
        rows.map(|r| serde_json::from_slice(&r?).map_err(|_| Error::Invalid))
            .collect()
    }
    pub fn acknowledge_recovery_use(&mut self, operation: Uuid) -> Result<(), Error> {
        self.check_files()?;
        self.conn.execute(
            "DELETE FROM recovery_uses WHERE id=?1",
            [operation.to_string()],
        )?;
        Ok(())
    }
}
fn load(conn: &Connection, sid: &str) -> Result<Option<Record>, Error> {
    let body: Option<Vec<u8>> = conn
        .query_row(
            "SELECT body FROM bindings WHERE sid=?1 AND length(body)<=32768",
            [sid],
            |r| r.get(0),
        )
        .optional()?;
    body.map(|b| {
        let record: Record = serde_json::from_slice(&b).map_err(|_| Error::Invalid)?;
        record.authority.validate()?;
        if record.authority.sid != sid
            || !record.authority.accepts(&record.facts)
            || record.revision < 1
            || record.secret.is_empty()
            || record.secret.len() > 16384
            || record.recovery_hash.len() != 32
            || record.recovery_secret.is_empty()
            || record.recovery_secret.len() > 16384
            || record.consumed_step < 0
            || record.lock_seconds > 300
            || record.boot.is_nil()
            || record.utc < 1
        {
            return Err(Error::Invalid);
        }
        Ok(record)
    })
    .transpose()
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(|_| Error::Invalid)
}
fn save(conn: &Connection, record: &Record) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO bindings VALUES(?1,?2) ON CONFLICT(sid) DO UPDATE SET body=excluded.body",
        params![record.authority.sid, encode(record)?],
    )?;
    Ok(())
}
fn valid_recovery(value: &str) -> bool {
    value.len() == 43
        && URL_SAFE_NO_PAD
            .decode(value)
            .is_ok_and(|bytes| bytes.len() == 32 && URL_SAFE_NO_PAD.encode(bytes) == value)
}
fn recovery_hash(authority: &Authority, value: &str) -> Vec<u8> {
    let mut context = digest::Context::new(&digest::SHA256);
    context.update(b"rss-mdm/offline-recovery/v2\0");
    context.update(&serde_json::to_vec(authority).expect("authority serialization"));
    context.update(value.as_bytes());
    context.finish().as_ref().to_vec()
}
fn constant_equal(a: &[u8], b: &[u8]) -> bool {
    // HMAC verification provides a maintained constant-time comparison primitive.
    let key = hmac::Key::new(hmac::HMAC_SHA256, a);
    let tag = hmac::sign(&key, b"rss-mdm/factor-compare/v1");
    hmac::verify(
        &hmac::Key::new(hmac::HMAC_SHA256, b),
        b"rss-mdm/factor-compare/v1",
        tag.as_ref(),
    )
    .is_ok()
}
fn otp(secret: &[u8], step: i64) -> String {
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret),
        &(step as u64).to_be_bytes(),
    );
    let bytes = tag.as_ref();
    let offset = (bytes[19] & 15) as usize;
    let binary = u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("HMAC offset"))
        & 0x7fff_ffff;
    format!("{:06}", binary % 1_000_000)
}
fn matching_step(secret: &[u8], code: &str, utc: i64, consumed: i64) -> Option<i64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let step = utc / 30;
    let mut matched = None;
    for candidate in [step - 1, step, step + 1] {
        if candidate >= 0
            && candidate > consumed
            && constant_equal(otp(secret, candidate).as_bytes(), code.as_bytes())
        {
            matched = Some(candidate);
        }
    }
    matched
}

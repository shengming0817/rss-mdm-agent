//! Fixed sampling identities and atomic batch membership, outside the command journal.
use super::{w, Configuration, Error};
use crate::store::{decode, encode, hash};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// Explicit product limits; the spool never consumes the command journal's quota.
#[derive(Clone, Copy)]
pub struct Limits {
    /// Maximum retained sample and upload body bytes.
    pub bytes: usize,
    /// Maximum retained observations, including frozen batch members.
    pub observations: usize,
    /// Maximum SQLite pages, including reserved configuration/diagnostic space.
    pub pages: u32,
}
impl Limits {
    /// Production storage budget; test callers may choose smaller bounds.
    pub fn production() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            observations: 4096,
            pages: 18432,
        }
    }
}
/// Loss vocabulary belongs to the shared protocol owner.
pub use w::TelemetryGapReason as GapReason;
#[path = "gap_spool.rs"]
mod gap_spool;
pub use gap_spool::PendingGap;
/// Aggregate loss evidence. Bounds describe the span, not continuous coverage.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Gap {
    /// Number of dropped or missed sampling instants.
    pub count: u64,
    /// Earliest known UTC microsecond.
    pub first: i64,
    /// Latest known UTC microsecond.
    pub last: i64,
}
/// Bounded local diagnostic projection; never contains sample values or credentials.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Summary {
    /// Unconfirmed sampling instants.
    pub pending: u64,
    /// Bytes retained in sample and upload bodies.
    pub bytes: u64,
    /// Local gaps grouped by a closed reason.
    pub gaps: BTreeMap<String, Gap>,
    /// Last durable receiver delivery observation, independent of query availability.
    pub delivery: Option<String>,
}
/// Immutable batch read from protected storage, not constructible by callers.
pub struct Pending {
    id: Uuid,
    registration: Uuid,
    generation: u64,
    body: Vec<u8>,
    attempts: u32,
    count: u64,
    first: i64,
    last: i64,
}
impl Pending {
    /// Stable logical upload identity.
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub(crate) fn body(&self) -> &[u8] {
        &self.body
    }
    /// Complete immutable request size for bandwidth accounting.
    pub fn bytes(&self) -> usize {
        self.body.len()
    }
    /// Whether a submission could already have reached the server.
    pub fn attempted(&self) -> bool {
        self.attempts != 0
    }
    pub(crate) fn belongs_to(&self, r: &w::BindingProfile) -> bool {
        self.registration == r.registration_id && self.generation == r.generation
    }
}
/// The communication owner holds this separate connection under its existing namespace lease.
pub struct Spool {
    conn: Connection,
    limits: Limits,
    root: PathBuf,
    _lease: std::fs::File,
}
impl Spool {
    pub(super) fn open(root: &Path, cfg: &crate::Config, limits: Limits) -> Result<Self, Error> {
        if limits.bytes == 0
            || limits.observations == 0
            || limits.observations > 4096
            || limits.pages == 0
        {
            return Err(Error::Configuration);
        }
        platform_private_storage::validate(root)?;
        let lock_path = root.join("telemetry.lock");
        let lease = match platform_private_storage::create_new(&lock_path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                platform_private_storage::open_existing(&lock_path)?
            }
            Err(_) => return Err(Error::Storage),
        };
        fs2::FileExt::try_lock_exclusive(&lease).map_err(|_| Error::Storage)?;
        let path = root.join("telemetry.sqlite");
        match platform_private_storage::create_new(&path) {
            Ok(file) => {
                drop(file);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                drop(platform_private_storage::open_existing(&path)?);
            }
            Err(_) => return Err(Error::Storage),
        };
        // SQLite-created journal files inherit private-directory protection. Reject preexisting links.
        for suffix in ["-journal", "-wal", "-shm"] {
            let side = root.join(format!("telemetry.sqlite{suffix}"));
            if side.symlink_metadata().is_ok() {
                drop(platform_private_storage::open_existing(&side)?);
            }
        }
        let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.busy_timeout(std::time::Duration::from_millis(50))?;
        conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE;")?;
        let binding = encode(&(
            cfg.origin.as_str(),
            cfg.tenant,
            cfg.platform,
            cfg.architecture,
        ))?;
        // A crash before the schema transaction commits leaves a protected blank database.
        // ref: rusqlite src/transaction.rs (schema and identity commit atomically).
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let application: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let objects: i64 =
            conn.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
        if version == 0 && application == 0 && objects == 0 {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch("CREATE TABLE identity(binding BLOB NOT NULL);
                CREATE TABLE configurations(policy TEXT PRIMARY KEY,body BLOB NOT NULL);
                CREATE TABLE sequences(source TEXT PRIMARY KEY,next INTEGER NOT NULL);
                CREATE TABLE counters(key TEXT PRIMARY KEY,body BLOB NOT NULL);
                CREATE TABLE samples(id TEXT PRIMARY KEY,policy TEXT NOT NULL,registration TEXT NOT NULL,generation INTEGER NOT NULL,binding TEXT NOT NULL,body BLOB NOT NULL,observed INTEGER NOT NULL,expires INTEGER NOT NULL);
                CREATE TABLE batches(id TEXT PRIMARY KEY,registration TEXT NOT NULL,generation INTEGER NOT NULL,body BLOB NOT NULL,digest TEXT NOT NULL,count INTEGER NOT NULL,first INTEGER NOT NULL,last INTEGER NOT NULL,expires INTEGER NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,due INTEGER NOT NULL DEFAULT 0);
                CREATE TABLE gaps(reason TEXT PRIMARY KEY,count INTEGER NOT NULL,first INTEGER NOT NULL,last INTEGER NOT NULL);
                CREATE TABLE receipt(singleton INTEGER PRIMARY KEY CHECK(singleton=1),body BLOB NOT NULL);
                PRAGMA application_id=1381192781; PRAGMA user_version=1;")?;
            tx.execute("INSERT INTO identity VALUES(?1)", [&binding])?;
            tx.commit()?;
        }
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let application: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
        if version != 1 || application != 1381192781 {
            return Err(Error::Schema);
        }
        let original: Vec<u8> = conn.query_row("SELECT binding FROM identity", [], |r| r.get(0))?;
        if original != binding {
            return Err(Error::Untrusted);
        }
        // Add diagnostic tables without altering existing samples, batches or unowned legacy gaps.
        gap_spool::install(&conn)?;
        conn.pragma_update(None, "max_page_count", limits.pages)?;
        let mut spool = Self {
            conn,
            limits,
            root: root.to_owned(),
            _lease: lease,
        };
        // Protected persisted configuration freezes old observations; it does not authorize execution.
        for signed in spool.configurations()? {
            spool.freeze_all(&Configuration(signed))?;
        }
        Ok(spool)
    }
    /// Private directory used for disk-reserve checks; not a caller-selected path.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Persist a verified lease. Refreshing cannot rewrite samples from an earlier version.
    pub fn install(&mut self, configuration: &Configuration) -> Result<(), Error> {
        let p = configuration.payload();
        let previous: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT body FROM configurations WHERE policy=?1",
                [p.policy.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(bytes) = previous {
            let old: w::SignedTelemetryConfiguration = decode(&bytes)?;
            if old.configuration.issued_at > p.issued_at {
                return Err(Error::Untrusted);
            }
            if old.configuration.binding != p.binding {
                self.freeze_all(&Configuration(old))?;
            }
        }
        let others: u64 = self.conn.query_row(
            "SELECT count(*) FROM configurations WHERE policy<>?1",
            [p.policy.to_string()],
            |r| r.get(0),
        )?;
        if others >= 256 {
            return Err(Error::Capacity);
        }
        let bytes = encode(configuration.signed())?;
        let total: usize = self.conn.query_row(
            "SELECT coalesce(sum(length(body)),0) FROM configurations WHERE policy<>?1",
            [p.policy.to_string()],
            |r| r.get(0),
        )?;
        if total.saturating_add(bytes.len()) > 4 * 1024 * 1024 {
            return Err(Error::Capacity);
        }
        self.conn.execute("INSERT INTO configurations VALUES(?1,?2) ON CONFLICT(policy) DO UPDATE SET body=excluded.body",params![p.policy.to_string(),bytes])?;
        self.reclaim_counters()?;
        Ok(())
    }
    fn reclaim_counters(&self) -> Result<(), Error> {
        let mut keep = std::collections::BTreeSet::new();
        for signed in self.configurations()? {
            let c = signed.configuration;
            let source = source_key(&c)?;
            for field in &c.fields {
                keep.insert(format!(
                    "{source}:{}:{}",
                    field.key.as_str(),
                    hash(&encode(field)?)
                ));
            }
        }
        let mut q = self.conn.prepare("SELECT key FROM counters")?;
        let keys = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for key in keys {
            if !keep.contains(&key) {
                self.conn
                    .execute("DELETE FROM counters WHERE key=?1", [key])?;
            }
        }
        Ok(())
    }
    /// Retained signed leases still require current signature/identity/time verification.
    pub fn configurations(&self) -> Result<Vec<w::SignedTelemetryConfiguration>, Error> {
        let mut q = self
            .conn
            .prepare("SELECT body FROM configurations ORDER BY policy LIMIT 257")?;
        let rows = q
            .query_map([], |r| r.get::<_, Vec<u8>>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > 256 {
            return Err(Error::Capacity);
        }
        rows.iter().map(|b| decode(b)).collect()
    }
    /// Remove only authoritatively withdrawn configurations, leaving frozen uploads unchanged.
    pub fn retain_configurations(
        &mut self,
        directory: &w::TelemetryDirectory,
    ) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        let keys = {
            let mut q = tx.prepare("SELECT policy FROM configurations")?;
            let rows = q
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for key in keys {
            if !directory
                .assignments
                .iter()
                .any(|a| a.supported && a.policy.to_string() == key)
            {
                tx.execute("DELETE FROM configurations WHERE policy=?1", [key])?;
            }
        }
        tx.commit()?;
        self.reclaim_counters()?;
        Ok(())
    }
    /// Atomically allocate a source sequence and persist one immutable sample.
    pub fn append(
        &mut self,
        configuration: &Configuration,
        at: i64,
        mut values: BTreeMap<w::FieldKey, w::TelemetryReading>,
        boot: &str,
    ) -> Result<Uuid, Error> {
        let c = configuration.payload();
        if at < 0 || at / 1_000_000 >= c.expires_at || at / 1_000_000 < c.issued_at {
            return Err(Error::Expired);
        }
        self.prune(at / 1_000_000, c.registration_id, c.generation)?;
        let source = source_key(c)?;
        let tx = self.conn.transaction()?;
        let sequence: u64 = tx
            .query_row(
                "SELECT next FROM sequences WHERE source=?1",
                ["observations"],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        normalize_counters(&tx, c, &source, boot, at, &mut values)?;
        let observation = w::TelemetryObservation {
            id: Uuid::new_v4(),
            sequence,
            observed_at: at,
            values,
        };
        let upload = w::TelemetryUpload {
            batch_id: Uuid::new_v4(),
            policy: c.policy,
            policy_version: c.policy_version,
            binding: c.binding.clone(),
            observations: vec![observation.clone()],
        };
        upload.validate()?;
        if !within_budget(c, &upload)? || encode(&upload)?.len() > c.max_bytes as usize {
            return Err(Error::Capacity);
        }
        let body = encode(&observation)?;
        room(&tx, self.limits, body.len(), 1, at)?;
        let next = sequence
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(Error::Capacity)?;
        tx.execute("INSERT INTO sequences VALUES(?1,?2) ON CONFLICT(source) DO UPDATE SET next=excluded.next",params!["observations",next])?;
        tx.execute(
            "INSERT INTO samples VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                observation.id.to_string(),
                c.policy.to_string(),
                c.registration_id.to_string(),
                c.generation,
                c.binding,
                body,
                at,
                at / 1_000_000 + i64::from(c.backfill_seconds)
            ],
        )?;
        tx.commit()?;
        Ok(observation.id)
    }
    /// Drain all queued samples under their original frozen configuration.
    pub fn freeze_all(&mut self, configuration: &Configuration) -> Result<(), Error> {
        loop {
            let count: u64 = self.conn.query_row(
                "SELECT count(*) FROM samples WHERE binding=?1",
                [&configuration.payload().binding],
                |r| r.get(0),
            )?;
            if count == 0 {
                return Ok(());
            }
            self.freeze(configuration)?;
            let remaining: u64 = self.conn.query_row(
                "SELECT count(*) FROM samples WHERE binding=?1",
                [&configuration.payload().binding],
                |r| r.get(0),
            )?;
            if remaining >= count {
                return Err(Error::Capacity);
            }
        }
    }
    /// Freeze up to the admitted batch budget. Samples are never reassigned once sent.
    pub fn freeze(&mut self, configuration: &Configuration) -> Result<(), Error> {
        let c = configuration.payload();
        let tx = self.conn.transaction()?;
        let rows = {
            let mut q = tx.prepare(
                "SELECT body,expires FROM samples WHERE binding=?1 ORDER BY observed,id LIMIT ?2",
            )?;
            let rows = q
                .query_map(params![c.binding, c.max_points.min(4096)], |r| {
                    Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let mut batch = w::TelemetryUpload {
            batch_id: Uuid::new_v4(),
            policy: c.policy,
            policy_version: c.policy_version,
            binding: c.binding.clone(),
            observations: vec![],
        };
        let mut expires = i64::MAX;
        for (bytes, expiry) in rows {
            let observation: w::TelemetryObservation = decode(&bytes)?;
            batch.observations.push(observation);
            if !within_budget(c, &batch)? || encode(&batch)?.len() > c.max_bytes as usize {
                batch.observations.pop();
                break;
            }
            expires = expires.min(expiry);
        }
        if batch.observations.is_empty() {
            return Ok(());
        }
        batch.validate()?;
        let body = encode(&batch)?;
        let first = batch
            .observations
            .first()
            .ok_or(Error::Storage)?
            .observed_at;
        let last = batch.observations.last().ok_or(Error::Storage)?.observed_at;
        for observation in &batch.observations {
            tx.execute(
                "DELETE FROM samples WHERE id=?1",
                [observation.id.to_string()],
            )?;
        }
        // Account for the envelope overhead before committing the new immutable batch.
        room(&tx, self.limits, body.len(), batch.observations.len(), last)?;
        tx.execute("INSERT INTO batches(id,registration,generation,body,digest,count,first,last,expires) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![batch.batch_id.to_string(),c.registration_id.to_string(),c.generation,body,hash(&body),batch.observations.len(),first,last,expires])?;
        tx.commit()?;
        Ok(())
    }
    /// Fair caller-selected oldest/newest batch; only due work is returned.
    pub fn pending(&self, now: i64, newest: bool) -> Result<Option<Pending>, Error> {
        let sql = if newest {
            "SELECT id,registration,generation,body,digest,attempts,count,first,last FROM batches WHERE due<=?1 ORDER BY first DESC,id DESC LIMIT 1"
        } else {
            "SELECT id,registration,generation,body,digest,attempts,count,first,last FROM batches WHERE due<=?1 ORDER BY first,id LIMIT 1"
        };
        let row = self
            .conn
            .query_row(sql, [now], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u64>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, u32>(5)?,
                    r.get::<_, u64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                ))
            })
            .optional()?;
        row.map(
            |(id, registration, generation, body, digest, attempts, count, first, last)| {
                if body.len() > self.limits.bytes || hash(&body) != digest {
                    return Err(Error::Storage);
                }
                Ok(Pending {
                    id: Uuid::parse_str(&id).map_err(|_| Error::Storage)?,
                    registration: Uuid::parse_str(&registration).map_err(|_| Error::Storage)?,
                    generation,
                    body,
                    attempts,
                    count,
                    first,
                    last,
                })
            },
        )
        .transpose()
    }
    /// Persist the uncertain state before I/O; crashes recover through a status lookup.
    pub fn attempting(&mut self, p: &Pending, now: i64) -> Result<(), Error> {
        self.conn.execute(
            "UPDATE batches SET attempts=min(attempts+1,31),due=?2 WHERE id=?1",
            params![p.id.to_string(), now.saturating_add(2)],
        )?;
        Ok(())
    }
    /// Persist bounded exponential backoff with stable batch-specific jitter.
    pub fn retry(&mut self, p: &Pending, now: i64, delay: Option<u64>) -> Result<(), Error> {
        let backoff = (1u64 << p.attempts.min(10)).min(900) + u64::from(p.id.as_bytes()[0]) % 7;
        let seconds = delay.unwrap_or(0).max(backoff).min(86400);
        self.conn.execute(
            "UPDATE batches SET due=?2 WHERE id=?1",
            params![p.id.to_string(), now.saturating_add(seconds as i64)],
        )?;
        Ok(())
    }
    /// Matching durable receiver evidence releases the body, not an assertion of query visibility.
    pub fn acknowledge(&mut self, p: &Pending, r: &w::TelemetryReceipt) -> Result<(), Error> {
        if r.batch_id != p.id || r.received_at < 0 {
            return Err(Error::Protocol);
        }
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO receipt VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET body=excluded.body",[encode(r)?])?;
        tx.execute("DELETE FROM batches WHERE id=?1", [p.id.to_string()])?;
        tx.commit()?;
        Ok(())
    }
    /// A permanent failure leaves bounded loss evidence rather than blocking the queue.
    pub fn reject(&mut self, p: &Pending, reason: GapReason) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        gap_spool::record(
            &tx,
            p.registration,
            p.generation,
            reason,
            p.count,
            p.first,
            p.last,
        )?;
        tx.execute("DELETE FROM batches WHERE id=?1", [p.id.to_string()])?;
        tx.commit()?;
        Ok(())
    }
    /// Record loss/pause evidence in the reserved metadata budget.
    pub fn gap(
        &mut self,
        reason: GapReason,
        count: u64,
        first: i64,
        last: i64,
    ) -> Result<(), Error> {
        gap(&self.conn, reason, count, first, last)
    }
    /// Expiry and registration fencing never rewrite old samples into a new source.
    pub fn prune(&mut self, now: i64, registration: Uuid, generation: u64) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        for table in ["samples", "batches"] {
            let (count, first, last) = if table == "samples" {
                ("count(*)", "min(observed)", "max(observed)")
            } else {
                ("sum(count)", "min(first)", "max(last)")
            };
            for (predicate, reason) in [
                (
                    "registration<>?1 OR generation<>?2",
                    GapReason::Registration,
                ),
                ("expires<=?3", GapReason::Expired),
            ] {
                let select = format!(
                    "SELECT registration,generation,coalesce({count},0),coalesce({first},0),coalesce({last},0) FROM {table} WHERE {predicate} AND ?1 IS NOT NULL AND ?2 IS NOT NULL AND ?3 IS NOT NULL GROUP BY registration,generation"
                );
                let rows = tx
                    .prepare(&select)?
                    .query_map(params![registration.to_string(), generation, now], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, u64>(1)?,
                            r.get::<_, u64>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                for (owner, owner_generation, n, a, b) in rows {
                    gap_spool::record(
                        &tx,
                        Uuid::parse_str(&owner).map_err(|_| Error::Storage)?,
                        owner_generation,
                        reason,
                        n,
                        a,
                        b,
                    )?;
                }
                tx.execute(&format!("DELETE FROM {table} WHERE {predicate} AND ?1 IS NOT NULL AND ?2 IS NOT NULL AND ?3 IS NOT NULL"),params![registration.to_string(),generation,now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    /// Inspect local health without exposing configuration, identity or sampled values.
    pub fn summary(&self) -> Result<Summary, Error> {
        let (pending, bytes) = usage(&self.conn)?;
        let mut q = self
            .conn
            .prepare("SELECT reason,count,first,last FROM gaps")?;
        let gaps = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    Gap {
                        count: r.get(1)?,
                        first: r.get(2)?,
                        last: r.get(3)?,
                    },
                ))
            })?
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let receipt: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT body FROM receipt WHERE singleton=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        let delivery = receipt
            .map(|b| {
                decode::<w::TelemetryReceipt>(&b)
                    .and_then(|r| serde_json::to_string(&r.delivery).map_err(|_| Error::Protocol))
            })
            .transpose()?;
        Ok(Summary {
            pending,
            bytes,
            gaps,
            delivery,
        })
    }
}
fn usage(c: &Connection) -> Result<(u64, u64), Error> {
    Ok(c.query_row("SELECT (SELECT count(*) FROM samples)+(SELECT coalesce(sum(count),0) FROM batches),(SELECT coalesce(sum(length(body)),0) FROM samples)+(SELECT coalesce(sum(length(body)),0) FROM batches)",[],|r|Ok((r.get(0)?,r.get(1)?)))?)
}
fn gap(c: &Connection, reason: GapReason, count: u64, first: i64, last: i64) -> Result<(), Error> {
    if count == 0 {
        return Ok(());
    }
    if first < 0 || last < first || count > i64::MAX as u64 {
        return Err(Error::Clock);
    }
    c.execute("INSERT INTO gaps VALUES(?1,?2,?3,?4) ON CONFLICT(reason) DO UPDATE SET count=min(9223372036854775807-gaps.count,excluded.count)+gaps.count,first=min(gaps.first,excluded.first),last=max(gaps.last,excluded.last)",params![gap_spool::reason_key(reason)?,count,first,last])?;
    Ok(())
}
fn room(
    c: &Connection,
    limits: Limits,
    extra: usize,
    observations: usize,
    _at: i64,
) -> Result<(), Error> {
    if extra > limits.bytes || observations > limits.observations {
        return Err(Error::Capacity);
    }
    loop {
        let (count, bytes) = usage(c)?;
        if count.saturating_add(observations as u64) <= limits.observations as u64
            && bytes.saturating_add(extra as u64) <= limits.bytes as u64
        {
            return Ok(());
        }
        // Oldest unconfirmed data is evicted atomically with its loss evidence.
        let sample: Option<(String, i64)> = c
            .query_row(
                "SELECT id,observed FROM samples ORDER BY observed,id LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let batch: Option<(String, u64, i64, i64)> = c
            .query_row(
                "SELECT id,count,first,last FROM batches ORDER BY first,id LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        match (sample, batch) {
            (Some((id, time)), b) if b.as_ref().is_none_or(|v| time <= v.2) => {
                gap_spool::record_object(c, "samples", &id, GapReason::Capacity, 1, time, time)?;
                c.execute("DELETE FROM samples WHERE id=?1", [id])?;
            }
            (_, Some((id, n, first, last))) => {
                gap_spool::record_object(c, "batches", &id, GapReason::Capacity, n, first, last)?;
                c.execute("DELETE FROM batches WHERE id=?1", [id])?;
            }
            _ => {
                return Err(Error::Capacity);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Counter {
    boot: String,
    first: w::Scalar,
    last: w::Scalar,
    start: i64,
    at: i64,
}
fn numeric(v: &w::Scalar) -> Option<f64> {
    match v {
        w::Scalar::Integer(n) => Some(*n as f64),
        w::Scalar::Number(n) => Some(n.into_inner()),
        _ => None,
    }
}
fn normalize_counters(
    db: &Connection,
    c: &w::TelemetryConfiguration,
    source: &str,
    boot: &str,
    at: i64,
    values: &mut BTreeMap<w::FieldKey, w::TelemetryReading>,
) -> Result<(), Error> {
    if boot.is_empty() || boot.len() > 128 {
        return Err(Error::Untrusted);
    }
    for field in &c.fields {
        let Some(binding) = &field.time_series else {
            continue;
        };
        if binding.interval_start_path.is_some()
            || !binding.value_path.is_empty()
            || !matches!(
                binding.kind,
                w::TelemetryKind::CumulativeCounter | w::TelemetryKind::DeltaCounter
            )
        {
            continue;
        }
        let key = format!("{source}:{}:{}", field.key.as_str(), hash(&encode(field)?));
        let Some(reading) = values.get_mut(&field.key) else {
            continue;
        };
        let w::TelemetryReading::Valid { value, .. } = reading else {
            continue;
        };
        let value = value.clone();
        let number = numeric(&value)
            .filter(|n| n.is_finite() && *n >= 0.0)
            .ok_or(Error::Protocol)?;
        let old: Option<Vec<u8>> = db
            .query_row("SELECT body FROM counters WHERE key=?1", [&key], |r| {
                r.get(0)
            })
            .optional()?;
        let old = old.map(|b| decode::<Counter>(&b)).transpose()?;
        let existing = old.is_some();
        let compatible = old.filter(|v| {
            v.boot == boot
                && v.at < at
                && (binding.kind == w::TelemetryKind::DeltaCounter
                    || match (&value, &v.last) {
                        (w::Scalar::Integer(n), w::Scalar::Integer(old)) => n >= old,
                        (w::Scalar::Number(n), w::Scalar::Number(old)) => n >= old,
                        _ => false,
                    })
        });
        let state = if let Some(old) = compatible {
            let start = if binding.kind == w::TelemetryKind::DeltaCounter {
                old.at
            } else {
                old.start
            };
            let adjusted = if binding.kind == w::TelemetryKind::CumulativeCounter {
                match (&value,&old.first) {
                    (w::Scalar::Integer(n),w::Scalar::Integer(base))=>w::Scalar::Integer(n.checked_sub(*base).ok_or(Error::Protocol)?),
                    _=>serde_json::from_value(serde_json::json!({"kind":"number","value":number-numeric(&old.first).ok_or(Error::Protocol)?})).map_err(|_|Error::Protocol)?,
                }
            } else {
                value.clone()
            };
            *reading = w::TelemetryReading::Valid {
                value: adjusted,
                interval_start: Some(start),
            };
            Counter {
                boot: boot.into(),
                first: old.first,
                last: value,
                start: old.start,
                at,
            }
        } else {
            let count: u64 = db.query_row("SELECT count(*) FROM counters", [], |r| r.get(0))?;
            if !existing && count >= 4096 {
                return Err(Error::Capacity);
            }
            *reading = w::TelemetryReading::Missing {
                interval_start: Some(at),
                context: None,
            };
            Counter {
                boot: boot.into(),
                first: value.clone(),
                last: value,
                start: at,
                at,
            }
        };
        db.execute(
            "INSERT INTO counters VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
            params![key, encode(&state)?],
        )?;
    }
    Ok(())
}

fn within_budget(c: &w::TelemetryConfiguration, batch: &w::TelemetryUpload) -> Result<bool, Error> {
    let mut resources = std::collections::BTreeSet::new();
    let mut points = 0usize;
    for o in &batch.observations {
        for f in &c.fields {
            let Some(binding) = &f.time_series else {
                continue;
            };
            let value = match o.values.get(&f.key) {
                Some(w::TelemetryReading::Valid { value, .. }) => Some(value),
                Some(
                    w::TelemetryReading::Missing { context, .. }
                    | w::TelemetryReading::Failed { context, .. },
                ) => context.as_ref(),
                None => return Err(Error::Protocol),
            };
            let items = match value {
                Some(w::Scalar::Array(a)) => a.iter().map(Some).collect::<Vec<_>>(),
                v => vec![v],
            };
            if items.len() > binding.max_resources as usize {
                return Ok(false);
            }
            let mut local = std::collections::BTreeSet::new();
            for item in items {
                let identity = if let Some(key) = &f.item_key {
                    let Some(w::Scalar::Object(values)) = item else {
                        return Err(Error::Protocol);
                    };
                    encode(values.get(key).ok_or(Error::Protocol)?)?
                } else {
                    b"device".to_vec()
                };
                if !local.insert(identity.clone()) {
                    return Err(Error::Protocol);
                }
                resources.insert((f.key, identity));
                points += 1;
                if resources.len() > c.max_resources as usize || points > c.max_points as usize {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn source_key(c: &w::TelemetryConfiguration) -> Result<String, Error> {
    Ok(hash(&encode(&(
        c.registration_id,
        c.generation,
        c.source,
        c.source_epoch,
    ))?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn full_counter_metadata_allows_existing_reset_and_reclaims_retired_fields() {
        use crate::{
            test_support::{telemetry as fixture, Root, Server},
            Clock, OpenMode,
        };
        let server = Server::new().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let r = server.register(&mut client).await;
        let mut signed = fixture::configuration(&server, &r);
        let binding = signed.configuration.fields[0].time_series.as_mut().unwrap();
        binding.kind = w::TelemetryKind::CumulativeCounter;
        binding.aggregations = [w::TelemetryAggregation::Increase].into();
        let c = client
            .verify_telemetry(fixture::sign(
                &server.data.lock().unwrap(),
                signed.configuration,
            ))
            .unwrap();
        let mut spool = client.telemetry_spool(Limits::production()).unwrap();
        spool.install(&c).unwrap();
        let tx = spool.conn.transaction().unwrap();
        for n in 0..4095 {
            tx.execute(
                "INSERT INTO counters VALUES(?1,?2)",
                params![format!("retired-{n}"), b"{}".as_slice()],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let values = || {
            [(
                w::FieldKey::parse("custom.load").unwrap(),
                w::TelemetryReading::Valid {
                    value: w::Scalar::Integer(42),
                    interval_start: None,
                },
            )]
            .into()
        };
        let at = server.time.now().unwrap() * 1_000_000;
        spool.append(&c, at, values(), "boot-a").unwrap();
        spool.append(&c, at + 1, values(), "boot-b").unwrap();
        spool.install(&c).unwrap();
        assert_eq!(
            spool
                .conn
                .query_row("SELECT count(*) FROM counters", [], |r| r.get::<_, u64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            spool
                .conn
                .query_row("SELECT count(*) FROM sequences", [], |r| r.get::<_, u64>(0))
                .unwrap(),
            1
        );
    }
}

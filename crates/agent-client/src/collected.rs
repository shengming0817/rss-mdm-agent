//! Bounded local projections in the existing communication database.
//! ref: rusqlite 0.32.1 src/transaction.rs (atomic bounded writes).
use crate::{store::decode, wire, Client, Clock, Error, IdentityProvider};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

/// Projection connection lent by the sole communication owner.
pub struct CollectedStore {
    conn: Connection,
    _lease: Option<std::sync::Arc<std::fs::File>>,
    namespace: String,
    bytes: usize,
    records: usize,
}
const PREFIX: &str = "local-data/";
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Retain server-verified registration profiles solely for journal recovery and cleanup.
    /// This history never authorizes a new task or relaxes current runtime identity checks.
    pub fn execution_profiles(&mut self) -> Result<Vec<wire::BindingProfile>, Error> {
        let mut profiles: Vec<wire::BindingProfile> =
            self.store.get("execution-profiles")?.unwrap_or_default();
        if let Ok(current) = self.registration() {
            if !profiles.iter().any(|p| {
                p.registration_id == current.registration_id && p.generation == current.generation
            }) {
                if profiles.len() >= 32 {
                    return Err(Error::Capacity);
                }
                profiles.push(current);
                self.store.put("execution-profiles", &profiles)?;
            }
        }
        Ok(profiles)
    }
    /// Local results bind to the protected installation and remain available before registration.
    pub fn collected_store_for_installation(
        &self,
        installation: uuid::Uuid,
    ) -> Result<CollectedStore, Error> {
        if installation.is_nil() {
            return Err(Error::Configuration);
        }
        let cfg = &self.store.cfg;
        let path = self.store.root.join("communication.sqlite");
        platform_private_storage::validate_sqlite_file(&path)?;
        let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.busy_timeout(std::time::Duration::from_millis(50))?;
        conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL;")?;
        conn.pragma_update(None, "max_page_count", cfg.limits.database_pages)?;
        use sha2::{Digest as _, Sha256};
        let namespace = format!(
            "{PREFIX}installation/{:x}/",
            Sha256::digest(
                serde_json::to_vec(&(cfg.service_id, cfg.tenant, installation))
                    .map_err(|_| Error::Protocol)?
            )
        );
        Ok(CollectedStore {
            _lease: Some(self.store._lock.clone()),
            conn,
            namespace,
            bytes: (cfg.limits.cache_bytes / 4)
                .min(u64::from(cfg.limits.database_pages) * 4096 / 4)
                .min(16 * 1024 * 1024) as usize,
            records: cfg.limits.pending_tasks.saturating_mul(32).clamp(32, 1024),
        })
    }
    /// Open a narrow adapter under the already held namespace lease, without network I/O.
    pub fn collected_store(&self) -> Result<CollectedStore, Error> {
        let cfg = &self.store.cfg;
        let path = self.store.root.join("communication.sqlite");
        platform_private_storage::validate_sqlite_file(&path)?;
        let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.busy_timeout(std::time::Duration::from_millis(50))?;
        conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL;")?;
        conn.pragma_update(None, "max_page_count", cfg.limits.database_pages)?;
        use sha2::{Digest as _, Sha256};
        let registration = self.registration()?;
        let namespace = format!(
            "{PREFIX}enterprise/{:x}/",
            Sha256::digest(
                serde_json::to_vec(&(
                    cfg.tenant,
                    registration.registration_id,
                    registration.generation,
                    &registration.device_id,
                ))
                .map_err(|_| Error::Protocol)?
            )
        );
        conn.execute(
            "DELETE FROM state WHERE key GLOB 'local-data/enterprise/*' AND substr(key,1,length(?1))<>?1",
            [&namespace],
        )?;
        Ok(CollectedStore {
            _lease: Some(self.store._lock.clone()),
            conn,
            namespace,
            bytes: (cfg.limits.cache_bytes / 4)
                .min(u64::from(cfg.limits.database_pages) * 4096 / 4)
                .min(16 * 1024 * 1024) as usize,
            records: cfg.limits.pending_tasks.saturating_mul(32).clamp(32, 1024),
        })
    }
}
impl CollectedStore {
    /// Existing total projection quotas, for bounded owner-side metadata retention.
    pub fn limits(&self) -> (usize, usize) {
        (self.bytes, self.records)
    }

    /// Count records within this registration projection namespace.
    pub fn records(&self) -> Result<usize, Error> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM state WHERE substr(key,1,length(?1))=?1",
            [&self.namespace],
            |r| r.get(0),
        )?)
    }

    fn key(&self, key: &str) -> Result<String, Error> {
        if key.is_empty()
            || key.len() > 256
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-/:.".contains(&b))
        {
            return Err(Error::Protocol);
        }
        Ok(format!("{}{key}", self.namespace))
    }
    /// Read a bounded projection; never starts collection.
    pub fn read(&self, key: &str) -> Result<Option<Value>, Error> {
        let key = self.key(key)?;
        let body: Option<Vec<u8>> = self.conn.query_row(
            "SELECT CASE WHEN typeof(body)='blob' AND length(body)<=?2 THEN body END FROM state WHERE key=?1",
            params![key, self.bytes], |r| r.get(0),
        ).optional()?;
        body.as_deref().map(decode).transpose()
    }
    /// Atomically publish a service-validated projection, independently of upload ACKs.
    pub fn write(&mut self, key: &str, value: &Value) -> Result<(), Error> {
        self.commit(&[(key.to_owned(), value.clone())], &[])
    }
    /// Atomically replace one complete projection batch and remove its recovery inputs.
    /// Capacity eviction never removes a member of the batch being committed.
    pub fn commit(&mut self, values: &[(String, Value)], remove: &[String]) -> Result<(), Error> {
        let mut writes = std::collections::BTreeMap::new();
        for (key, value) in values {
            let body = serde_json::to_vec(value).map_err(|_| Error::Protocol)?;
            if body.len() > self.bytes || writes.insert(self.key(key)?, body).is_some() {
                return Err(Error::Capacity);
            }
        }
        let remove = remove
            .iter()
            .map(|key| self.key(key))
            .collect::<Result<Vec<_>, _>>()?;
        let tx = self.conn.transaction()?;
        for key in remove {
            tx.execute("DELETE FROM state WHERE key=?1", [key])?;
        }
        for (key, body) in &writes {
            tx.execute(
                "INSERT INTO state(key,body) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
                params![key, body],
            )?;
        }
        let (mut count, mut bytes): (usize, usize) = tx.query_row(
            "SELECT count(*),coalesce(sum(length(body)),0) FROM state WHERE substr(key,1,length(?1))=?1",
            [&self.namespace], |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if count > self.records || bytes > self.bytes {
            let mut query = tx.prepare(
                "SELECT key,length(body) FROM state WHERE (key GLOB ?1 OR key GLOB ?2) ORDER BY rowid",
            )?;
            let candidates = query
                .query_map(
                    params![
                        format!("{}latest/*", self.namespace),
                        format!("{}result/*", self.namespace)
                    ],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            drop(query);
            for (key, size) in candidates {
                if count <= self.records && bytes <= self.bytes {
                    break;
                }
                if !writes.contains_key(&key) {
                    tx.execute("DELETE FROM state WHERE key=?1", [key])?;
                    count -= 1;
                    bytes -= size;
                }
            }
            if count > self.records || bytes > self.bytes {
                return Err(Error::Capacity);
            }
        }
        tx.commit()?;
        Ok(())
    }
    /// Read a stable bounded namespace page; prefixes are parameters, never SQL.
    pub fn page(
        &self,
        prefix: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, Value)>, Error> {
        if limit == 0 || limit > 128 {
            return Err(Error::Capacity);
        }
        let prefix = self.key(prefix)?;
        let after = after
            .map(|key| self.key(key))
            .transpose()?
            .unwrap_or_else(|| prefix.clone());
        let mut q = self.conn.prepare(
            "SELECT key,CASE WHEN typeof(body)='blob' AND length(body)<=?4 THEN body END FROM state WHERE key>?1 AND substr(key,1,length(?2))=?2 ORDER BY key LIMIT ?3",
        )?;
        let rows = q.query_map(params![after, prefix, limit, self.bytes], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        rows.map(|row| {
            let (key, body) = row?;
            Ok((key[self.namespace.len()..].to_owned(), decode(&body)?))
        })
        .collect()
    }
    /// Remove only one key in the projection namespace.
    pub fn remove(&mut self, key: &str) -> Result<(), Error> {
        self.conn
            .execute("DELETE FROM state WHERE key=?1", [self.key(key)?])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> CollectedStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE state(key TEXT PRIMARY KEY,body BLOB NOT NULL)")
            .unwrap();
        CollectedStore {
            _lease: None,
            conn,
            namespace: "local-data/test/".into(),
            bytes: 1024,
            records: 4,
        }
    }
    #[test]
    fn bounded_atomic_cache_eviction_preserves_request_identity_and_other_state() {
        let mut s = store();
        s.conn
            .execute(
                "INSERT INTO state VALUES('unrelated',?1)",
                [b"protected".as_slice()],
            )
            .unwrap();
        s.write("request/one", &serde_json::json!({"request":"one"}))
            .unwrap();
        for n in 0..5 {
            s.write(&format!("latest/{n}"), &serde_json::json!({"n":n}))
                .unwrap();
        }
        assert!(s.read("request/one").unwrap().is_some());
        assert!(s.read("latest/0").unwrap().is_none());
        let before = s.read("request/one").unwrap();
        assert_eq!(
            s.write("request/one", &serde_json::json!("x".repeat(1025)))
                .unwrap_err(),
            Error::Capacity
        );
        assert_eq!(s.read("request/one").unwrap(), before);
        assert_eq!(
            s.conn
                .query_row("SELECT body FROM state WHERE key='unrelated'", [], |r| r
                    .get::<_, Vec<u8>>(0))
                .unwrap(),
            b"protected"
        );
        s.namespace = "local-data/other-generation/".into();
        assert!(s.read("request/one").unwrap().is_none());
    }
}

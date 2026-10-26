//! Content consumer controls in the existing communication store. Byte observations are transient.
//! ref: tokio tokio-1.53.1 tokio/src/sync/watch.rs; reqwest v0.13.5 src/async_impl/response.rs.
use crate::{
    store::{decode, encode},
    Error,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

/// Real latest content observation, never an execution or effect receipt.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferObservation {
    /// Exact original task.
    pub task: Uuid,
    /// Exact original attempt.
    pub attempt: Uuid,
    /// Artifact owner key, not a filesystem path or URL.
    pub key: String,
    /// Monotonic consumer control revision.
    pub revision: u64,
    /// Actual successfully written bytes.
    pub bytes: u64,
    /// Signed total when known.
    pub total: Option<u64>,
    /// Current physical producer stage.
    pub stage: String,
    /// This consumer has durably requested pause.
    pub paused: bool,
    /// Actual resumable body support.
    pub resumable: bool,
    /// Another consumer may still be using the producer.
    pub shared_active: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Consumer {
    task: Uuid,
    attempt: Uuid,
    key: String,
    cache: String,
    revision: u64,
    paused: bool,
    cancelled: bool,
    controls: Vec<Control>,
    completed_bytes: Option<u64>,
    checkpoint_bytes: Option<u64>,
    retired: bool,
    total: Option<u64>,
    resumable: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Control {
    id: String,
    pause: bool,
    expected: u64,
    confirmed: bool,
}
struct State {
    conn: Connection,
    consumers: BTreeMap<String, Consumer>,
    latest: BTreeMap<String, (u64, String)>,
    limit: usize,
}
/// Cloneable control access to the original content owner, without another Client or scheduler.
#[derive(Clone)]
pub struct Transfers {
    _lease: Arc<std::fs::File>,
    state: Arc<Mutex<State>>,
    changed: Arc<tokio::sync::Notify>,
}
impl Transfers {
    pub(crate) fn open(
        root: &Path,
        limit: usize,
        lease: Arc<std::fs::File>,
    ) -> Result<Self, Error> {
        let conn = Connection::open_with_flags(
            root.join("communication.sqlite"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;
        conn.busy_timeout(std::time::Duration::from_millis(250))?;
        conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA trusted_schema=OFF;")?;
        let mut consumers = BTreeMap::new();
        {
            let mut q = conn.prepare(
                "SELECT key,body FROM state WHERE key LIKE 'transfer/%' ORDER BY key LIMIT ?1",
            )?;
            for row in q.query_map([limit + 1], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })? {
                let (key, bytes) = row?;
                if consumers.len() >= limit {
                    return Err(Error::Capacity);
                }
                let value: Consumer = decode(&bytes)?;
                consumers.insert(key, value);
            }
        }
        Ok(Self {
            _lease: lease,
            state: Arc::new(Mutex::new(State {
                conn,
                consumers,
                latest: BTreeMap::new(),
                limit,
            })),
            changed: Arc::new(tokio::sync::Notify::new()),
        })
    }
    fn identity(task: Uuid, attempt: Uuid, key: &str) -> String {
        format!("transfer/{task}/{attempt}/{key}")
    }
    pub(crate) fn attach(
        &self,
        task: Uuid,
        attempt: Uuid,
        key: &str,
        cache: &str,
        total: Option<u64>,
        resumable: bool,
    ) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        let id = Self::identity(task, attempt, key);
        if let Some(old) = s.consumers.get(&id) {
            if old.cache != cache || old.total != total || old.resumable != resumable {
                return Err(Error::Untrusted);
            }
            return Ok(());
        }
        if s.consumers.len() >= s.limit {
            let retired = s
                .consumers
                .iter()
                .find(|(_, v)| v.retired)
                .map(|(key, _)| key.clone())
                .ok_or(Error::Capacity)?;
            s.conn
                .execute("DELETE FROM state WHERE key=?1", [&retired])?;
            s.consumers.remove(&retired);
            let retained: std::collections::BTreeSet<_> =
                s.consumers.values().map(|v| v.cache.clone()).collect();
            s.latest.retain(|cache, _| retained.contains(cache));
        }
        let value = Consumer {
            task,
            attempt,
            key: key.into(),
            cache: cache.into(),
            revision: 0,
            paused: false,
            cancelled: false,
            controls: Vec::new(),
            completed_bytes: None,
            checkpoint_bytes: None,
            retired: false,
            total,
            resumable,
        };
        s.save(&id, value)
    }
    /// Current bounded observations for one original owner task/attempt.
    pub fn observations(
        &self,
        task: Uuid,
        attempt: Uuid,
    ) -> Result<Vec<TransferObservation>, Error> {
        let s = self.state.lock().map_err(|_| Error::Storage)?;
        Ok(s.consumers
            .values()
            .filter(|v| v.task == task && v.attempt == attempt)
            .map(|v| {
                let latest = s.latest.get(&v.cache);
                TransferObservation {
                    task,
                    attempt,
                    key: v.key.clone(),
                    revision: v.revision,
                    bytes: latest
                        .map(|v| v.0)
                        .or(v.completed_bytes)
                        .or(v.checkpoint_bytes)
                        .unwrap_or(0),
                    total: v.total,
                    stage: latest.map(|v| v.1.clone()).unwrap_or_else(|| {
                        if v.completed_bytes.is_some() {
                            "complete".into()
                        } else if v.paused {
                            "paused".into()
                        } else {
                            "unobserved".into()
                        }
                    }),
                    paused: v.paused,
                    resumable: v.resumable,
                    shared_active: s.consumers.values().any(|other| {
                        other.cache == v.cache
                            && other.task != task
                            && !other.paused
                            && !other.cancelled
                    }),
                }
            })
            .collect())
    }
    /// Apply one consumer CAS. A pause is confirmed only once physical bytes are safe or another consumer owns the writer.
    pub fn control(
        &self,
        task: Uuid,
        attempt: Uuid,
        id: &str,
        revisions: &[(String, u64)],
        pause: bool,
    ) -> Result<bool, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        let selected: Vec<_> = s
            .consumers
            .iter()
            .filter(|(_, v)| v.task == task && v.attempt == attempt)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let replay_count = selected
            .iter()
            .filter(|(_, v)| v.controls.iter().any(|c| c.id == id))
            .count();
        if replay_count > 0 {
            if replay_count != selected.len() || revisions.len() != selected.len() {
                return Err(Error::Conflict);
            }
            let mut confirmed = true;
            for (_, value) in &selected {
                let original = value
                    .controls
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or(Error::Conflict)?;
                if original.pause != pause
                    || !revisions
                        .iter()
                        .any(|(key, rev)| key == &value.key && *rev == original.expected)
                {
                    return Err(Error::Conflict);
                }
                confirmed &= original.confirmed;
            }
            return Ok(confirmed);
        }
        if selected.is_empty()
            || revisions.len() != selected.len()
            || selected.iter().any(|(_, v)| !v.resumable || v.cancelled)
        {
            return Err(Error::Unsupported);
        }
        let tx = s.conn.unchecked_transaction()?;
        let mut values = Vec::new();
        let mut confirmed = true;
        for (key, mut value) in selected {
            if let Some(original) = value.controls.iter().find(|c| c.id == id) {
                if original.pause != pause
                    || !revisions
                        .iter()
                        .any(|(key, rev)| key == &value.key && *rev == original.expected)
                {
                    return Err(Error::Conflict);
                }
                confirmed &= original.confirmed;
            } else {
                if !revisions
                    .iter()
                    .any(|(key, rev)| key == &value.key && *rev == value.revision)
                {
                    return Err(Error::Conflict);
                }
                if value.controls.len() >= 128 {
                    return Err(Error::Capacity);
                }
                let expected = value.revision;
                value.revision = value.revision.checked_add(1).ok_or(Error::Capacity)?;
                value.paused = pause;
                let other = s
                    .consumers
                    .values()
                    .any(|v| v.cache == value.cache && v.task != task && !v.paused && !v.cancelled);
                let accepted = !pause || other;
                value.controls.push(Control {
                    id: id.into(),
                    pause,
                    expected,
                    confirmed: accepted,
                });
                confirmed &= accepted;
            }
            tx.execute("INSERT INTO state(key,body) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",rusqlite::params![key,encode(&value)?])?;
            values.push((key, value));
        }
        tx.commit()?;
        for (key, value) in values {
            s.consumers.insert(key, value);
        }
        drop(s);
        self.changed.notify_one();
        Ok(confirmed)
    }
    /// Read the original content control receipt; does not change pause intent.
    pub fn receipt(&self, task: Uuid, attempt: Uuid, id: &str) -> Result<Option<bool>, Error> {
        let s = self.state.lock().map_err(|_| Error::Storage)?;
        let values: Vec<_> = s
            .consumers
            .values()
            .filter(|v| v.task == task && v.attempt == attempt)
            .collect();
        if values.is_empty()
            || values
                .iter()
                .any(|v| !v.controls.iter().any(|c| c.id == id))
        {
            return Ok(None);
        }
        Ok(Some(values.iter().all(|v| {
            v.controls.iter().any(|c| c.id == id && c.confirmed)
        })))
    }
    pub(crate) fn consumer_ready(&self, task: Uuid, attempt: Uuid) -> Result<bool, Error> {
        let s = self.state.lock().map_err(|_| Error::Storage)?;
        Ok(s.consumers
            .values()
            .filter(|v| v.task == task && v.attempt == attempt)
            .all(|v| !v.paused && !v.cancelled))
    }
    pub(crate) fn active(&self, cache: &str) -> Result<bool, Error> {
        let s = self.state.lock().map_err(|_| Error::Storage)?;
        Ok(s.consumers
            .values()
            .any(|v| v.cache == cache && !v.paused && !v.cancelled))
    }
    pub(crate) async fn changed(&self) {
        self.changed.notified().await;
    }
    pub(crate) fn publish(&self, cache: &str, bytes: u64, stage: &str) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        s.latest.insert(cache.into(), (bytes, stage.into()));
        if stage == "paused" || stage == "complete" {
            let values: Vec<_> = s
                .consumers
                .iter()
                .filter(|(_, v)| v.cache == cache)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            for (k, mut v) in values {
                if stage == "complete" {
                    v.completed_bytes = Some(bytes);
                } else {
                    v.checkpoint_bytes = Some(bytes);
                }
                for c in &mut v.controls {
                    if c.pause {
                        c.confirmed = true;
                    }
                }
                s.save(&k, v)?;
            }
        }
        Ok(())
    }
    /// Detach only one cancelled consumer; other parents retain their original producer and cache refs.
    pub fn detach(&self, task: Uuid, attempt: Uuid) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        let values: Vec<_> = s
            .consumers
            .iter()
            .filter(|(_, v)| v.task == task && v.attempt == attempt)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, mut value) in values {
            value.cancelled = true;
            s.save(&key, value)?;
        }
        self.changed.notify_one();
        Ok(())
    }
    /// Retain bounded safe terminal progress after transport GC; active cache leases remain separate.
    pub fn release(&self, task: Uuid) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        let values: Vec<_> = s
            .consumers
            .iter()
            .filter(|(_, v)| v.task == task)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, mut value) in values {
            value.retired = true;
            value.cancelled = true;
            s.save(&key, value)?;
        }
        self.changed.notify_one();
        Ok(())
    }
}
impl State {
    fn save(&mut self, key: &str, value: Consumer) -> Result<(), Error> {
        self.conn.execute("INSERT INTO state(key,body) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",rusqlite::params![key,encode(&value)?])?;
        self.consumers.insert(key.into(), value);
        Ok(())
    }
}

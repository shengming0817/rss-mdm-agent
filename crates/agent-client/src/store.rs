use crate::{wire, Config, Error, OpenMode};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
const SCHEMA: &str = "CREATE TABLE metadata(singleton INTEGER PRIMARY KEY CHECK(singleton=1),binding BLOB NOT NULL,sequence INTEGER NOT NULL,watermark INTEGER NOT NULL);
                    CREATE TABLE state(key TEXT PRIMARY KEY,body BLOB NOT NULL);
                    CREATE TABLE reports(id TEXT PRIMARY KEY,body BLOB NOT NULL);
                    CREATE TABLE tasks(id TEXT PRIMARY KEY,body BLOB NOT NULL);
                    CREATE TABLE requests(key TEXT PRIMARY KEY,task TEXT NOT NULL,source TEXT,body BLOB NOT NULL,accepted INTEGER NOT NULL DEFAULT 0 CHECK(accepted IN(0,1)));
                    CREATE TABLE cache_refs(task TEXT NOT NULL,name TEXT NOT NULL,PRIMARY KEY(task,name));
                    PRAGMA application_id=1380008771; PRAGMA user_version=2;";
pub(crate) struct Store {
    pub conn: Connection,
    pub root: PathBuf,
    pub cfg: Config,
    _lock: File,
}
pub(crate) fn encode(v: &impl Serialize) -> Result<Vec<u8>, Error> {
    serde_json_canonicalizer::to_vec(v).map_err(|_| Error::Protocol)
}
pub(crate) fn decode<T: DeserializeOwned>(b: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(b).map_err(|_| Error::Protocol)
}
pub(crate) fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
pub(crate) fn private(p: &Path) -> Result<(), Error> {
    platform_private_storage::validate(p)?;
    Ok(())
}
pub(crate) fn private_root(root: &Path) -> Result<(), Error> {
    if !platform_private_storage::is_canonical_path(root)? {
        return Err(Error::Storage);
    }
    private(root)
}
impl Store {
    pub fn open(root: &Path, cfg: Config, mode: OpenMode) -> Result<Self, Error> {
        cfg.validate()?;
        private_root(root)?;
        if !root.is_dir() {
            return Err(Error::Storage);
        }
        let path = root.join("communication.sqlite");
        if path.exists() {
            drop(read_existing(root, &cfg)?);
            if matches!(mode, OpenMode::Create) {
                return Err(Error::Storage);
            }
        } else if matches!(mode, OpenMode::Existing) {
            return Err(Error::Storage);
        }
        let lock_path = root.join("client.lock");
        let lock = match platform_private_storage::create_new(&lock_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                platform_private_storage::open_existing(&lock_path)?
            }
            Err(_) => return Err(Error::Storage),
        };
        lock.try_lock_exclusive().map_err(|_| Error::Storage)?;
        let binding = encode(&(
            cfg.service_id,
            cfg.tenant,
            cfg.platform,
            cfg.architecture,
            cfg.transport == crate::Transport::TestLoopback,
        ))?;
        match mode {
            OpenMode::Create => {
                drop(platform_private_storage::create_new(&path)?);
                let conn = Connection::open(&path)?;
                conn.execute_batch(
                    "PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE;",
                )?;
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(SCHEMA)?;
                tx.execute("INSERT INTO metadata VALUES(1,?1,0,0)", [binding])?;
                tx.execute(
                    "INSERT INTO state VALUES('execution_context',?1)",
                    [encode(&cfg.execution_context)?],
                )?;
                tx.execute(
                    "INSERT INTO state VALUES('connections',?1)",
                    [crate::connections::initial_state(&cfg)?],
                )?;
                tx.commit()?;
            }
            OpenMode::Existing => {
                drop(read_existing(root, &cfg)?);
            }
        }
        sidecars(&path)?;
        let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.execute_batch(
            "PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;",
        )?;
        sidecars(&path)?;
        conn.pragma_update(None, "max_page_count", cfg.limits.database_pages)?;
        conn.busy_timeout(std::time::Duration::from_millis(250))?;
        Ok(Self {
            conn,
            root: root.to_owned(),
            cfg,
            _lock: lock,
        })
    }
    pub fn time(&self, now: i64) -> Result<i64, Error> {
        let old: i64 = self
            .conn
            .query_row("SELECT watermark FROM metadata", [], |r| r.get(0))?;
        if now < 0 || now < old {
            return Err(Error::Clock);
        }
        self.conn
            .execute("UPDATE metadata SET watermark=?1", [now])?;
        Ok(now)
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        let bytes:Option<Vec<u8>>=self.conn.query_row("SELECT CASE WHEN typeof(body)='blob' AND length(body)<=16777216 THEN body END FROM state WHERE key=?1",[key],|r|r.get(0)).optional()?;
        bytes.as_deref().map(decode).transpose()
    }
    pub fn put(&self, key: &str, value: &impl Serialize) -> Result<(), Error> {
        let body = encode(value)?;
        if body.len() > self.cfg.limits.response_bytes {
            return Err(Error::Capacity);
        }
        self.conn.execute(
            "INSERT INTO state VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
            params![key, body],
        )?;
        Ok(())
    }
    pub fn registration(&self) -> Result<wire::BindingProfile, Error> {
        self.get::<crate::identity::IdentityState>("identity")?
            .and_then(|state| state.active.map(|active| active.receipt.runtime))
            .ok_or(Error::Identity)
    }
}

pub(crate) fn read_existing(root: &Path, cfg: &Config) -> Result<Connection, Error> {
    cfg.validate()?;
    private_root(root)?;
    let path = root.join("communication.sqlite");
    platform_private_storage::validate_sqlite_file(&path)?;
    sidecars(&path)?;
    let reader = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version: u32 = reader.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let app: u32 = reader.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if version != 2 || app != 1380008771 {
        return Err(Error::Schema);
    }
    validate_schema(&reader)?;
    let binding = encode(&(
        cfg.service_id,
        cfg.tenant,
        cfg.platform,
        cfg.architecture,
        cfg.transport == crate::Transport::TestLoopback,
    ))?;
    let actual: Vec<u8> =
        reader.query_row("SELECT binding FROM metadata WHERE singleton=1", [], |r| {
            r.get(0)
        })?;
    if actual != binding {
        return Err(Error::Identity);
    }
    crate::connections::validate_stored(&reader, cfg)?;
    Ok(reader)
}
/// Validate storage format and deployment binding without locks, writes or recovery.
pub fn validate_existing(root: &Path, cfg: &Config) -> Result<(), Error> {
    drop(read_existing(root, cfg)?);
    Ok(())
}
/// Inspect the sole active binding without locks, writes or startup recovery.
pub fn inspect_binding(root: &Path, cfg: &Config) -> Result<wire::BindingProfile, Error> {
    read_identity(root, cfg)?
        .active
        .map(|active| active.receipt.runtime)
        .ok_or(Error::Identity)
}
/// Validate the current runtime identity without locks, writes, recovery or key creation.
/// Historical binding inspection remains independent for local journal recovery.
pub fn validate_identity(
    root: &Path,
    cfg: &Config,
    provider: &impl crate::IdentityProvider,
    clock: &impl crate::Clock,
) -> Result<wire::BindingProfile, Error> {
    let identity = read_identity(root, cfg)?;
    identity.active_identity(clock.now()?)?;
    let reader = read_existing(root, cfg)?;
    let now = clock.now()?;
    let watermark: i64 = reader.query_row(
        "SELECT watermark FROM metadata WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    if now < 0 || now < watermark {
        return Err(Error::Clock);
    }
    let bytes: Option<Vec<u8>> = reader.query_row("SELECT CASE WHEN typeof(body)='blob' AND length(body)<=?1 THEN body END FROM state WHERE key='pki'", [cfg.limits.response_bytes], |r| r.get(0)).optional()?.flatten();
    let state: crate::pki::State = bytes
        .as_deref()
        .map(decode)
        .transpose()?
        .ok_or(Error::Identity)?;
    let trust = state.verified(cfg, now)?;
    Ok(identity
        .validate_runtime(cfg, provider, clock.now()?, &trust)?
        .clone())
}
/// Read installation identity and all retained key references without opening a writer or
/// requiring a currently usable certificate. Expiry/revocation do not permit replacing identity.
pub fn validate_stored_identity(
    root: &Path,
    cfg: &Config,
    provider: &impl crate::IdentityProvider,
) -> Result<Option<wire::BindingProfile>, Error> {
    let identity = read_identity(root, cfg)?;
    if let Some(active) = identity
        .active
        .as_ref()
        .filter(|_| identity.pending.is_none())
    {
        provider.key(&active.key_reference)?;
        provider.resolve(&active.secret_reference)?;
    }
    if let Some(pending) = &identity.pending {
        if pending.csr.is_some() {
            provider.key(&pending.key_reference)?;
        }
        provider.resolve(&pending.secret_reference)?;
    }
    Ok(identity.active.map(|active| active.receipt.runtime))
}
fn read_identity(root: &Path, cfg: &Config) -> Result<crate::identity::IdentityState, Error> {
    let reader = read_existing(root, cfg)?;
    let body: Option<Vec<u8>> = reader
        .query_row(
            "SELECT CASE WHEN length(body)<=?1 THEN body END FROM state WHERE key='identity'",
            [cfg.limits.response_bytes],
            |r| r.get(0),
        )
        .optional()?;
    // Match the communication owner's identity_state: initialization precedes enrollment.
    body.as_deref()
        .map(decode)
        .transpose()
        .map(Option::unwrap_or_default)
}

fn sidecars(path: &Path) -> Result<(), Error> {
    for suffix in ["-journal", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let sidecar = Path::new(&name);
        match std::fs::symlink_metadata(sidecar) {
            Ok(_) => {
                platform_private_storage::validate_sqlite_file(sidecar)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn validate_schema(conn: &Connection) -> Result<(), Error> {
    fn catalog(conn: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
        let mut query = conn.prepare(
            "SELECT name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = query.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(SCHEMA)?;
    if catalog(conn)? != catalog(&expected)? {
        return Err(Error::Schema);
    }
    let integrity: String = conn.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))?;
    if integrity != "ok" || conn.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err(Error::Schema);
    }
    Ok(())
}

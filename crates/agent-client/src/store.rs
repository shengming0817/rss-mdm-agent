use crate::{wire, Config, Error, OpenMode};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
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
    native_process::private_storage::validate(p)?;
    Ok(())
}
impl Store {
    pub fn open(root: &Path, cfg: Config, mode: OpenMode) -> Result<Self, Error> {
        cfg.validate()?;
        if !root.is_absolute() || root.canonicalize()? != root {
            return Err(Error::Storage);
        }
        private(root)?;
        if !root.is_dir() {
            return Err(Error::Storage);
        }
        let lock_path = root.join("client.lock");
        let lock = match native_process::private_storage::create_new(&lock_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                private(&lock_path)?;
                File::open(&lock_path)?
            }
            Err(_) => return Err(Error::Storage),
        };
        lock.try_lock_exclusive().map_err(|_| Error::Storage)?;
        let path = root.join("communication.sqlite");
        let binding = encode(&(
            cfg.origin.as_str(),
            cfg.tenant,
            cfg.platform,
            cfg.architecture,
            cfg.transport == crate::Transport::TestLoopback,
        ))?;
        match mode {
            OpenMode::Create => {
                drop(native_process::private_storage::create_new(&path)?);
                let conn = Connection::open(&path)?;
                conn.execute_batch(
                    "PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE;",
                )?;
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch("CREATE TABLE metadata(singleton INTEGER PRIMARY KEY CHECK(singleton=1),binding BLOB NOT NULL,sequence INTEGER NOT NULL,watermark INTEGER NOT NULL);
                    CREATE TABLE state(key TEXT PRIMARY KEY,body BLOB NOT NULL);
                    CREATE TABLE reports(id TEXT PRIMARY KEY,body BLOB NOT NULL);
                    CREATE TABLE tasks(id TEXT PRIMARY KEY,body BLOB NOT NULL);
                    CREATE TABLE requests(key TEXT PRIMARY KEY,task TEXT NOT NULL,source TEXT,body BLOB NOT NULL,accepted INTEGER NOT NULL DEFAULT 0 CHECK(accepted IN(0,1)));
                    CREATE TABLE cache_refs(task TEXT NOT NULL,name TEXT NOT NULL,PRIMARY KEY(task,name));
                    PRAGMA application_id=1380008771; PRAGMA user_version=1;")?;
                tx.execute("INSERT INTO metadata VALUES(1,?1,0,0)", [binding])?;
                tx.commit()?;
            }
            OpenMode::Existing => {
                private(&path)?;
                let reader =
                    Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let version: u32 = reader.pragma_query_value(None, "user_version", |r| r.get(0))?;
                let app: u32 = reader.pragma_query_value(None, "application_id", |r| r.get(0))?;
                if version != 1 || app != 1380008771 {
                    return Err(Error::Schema);
                }
                let actual: Vec<u8> = reader.query_row(
                    "SELECT binding FROM metadata WHERE singleton=1",
                    [],
                    |r| r.get(0),
                )?;
                if actual != binding {
                    return Err(Error::Identity);
                }
            }
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.execute_batch(
            "PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;",
        )?;
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
    pub fn registration(&self) -> Result<wire::RegistrationReceipt, Error> {
        self.get("registration")?.ok_or(Error::Identity)
    }
    pub fn secret_reference(&self) -> Result<String, Error> {
        self.get("credential")?.ok_or(Error::Identity)
    }
}

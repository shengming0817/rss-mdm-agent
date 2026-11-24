//! A single writer owns the original enrollment and report operations, including across retries.
use super::Backend;
use crate::{
    store::{decode, encode, private_root},
    Error,
};
use fs2::FileExt;
use platform_private_storage as files;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::{fs::File, path::Path};
use uuid::Uuid;

const APP: u32 = 0x52534441;
pub(super) struct Journal {
    pub db: Connection,
    _writer: File,
}

fn connection(root: &Path, write: bool) -> Result<Connection, Error> {
    private_root(root)?;
    let path = root.join("da.sqlite");
    files::validate_sqlite_file(&path)?;
    for suffix in ["da.sqlite-journal", "da.sqlite-wal", "da.sqlite-shm"] {
        let p = root.join(suffix);
        match p.symlink_metadata() {
            Ok(_) if suffix == "da.sqlite-journal" => files::validate_sqlite_file(&p)?,
            Ok(_) => return Err(Error::Schema),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    let flags = if write {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let conn = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NOFOLLOW)?;
    conn.execute_batch("PRAGMA trusted_schema=OFF;")?;
    Ok(conn)
}

fn verify(conn: &Connection, backend: &Backend, installation: Uuid) -> Result<(), Error> {
    if files::sqlite_format(conn)? != Some((APP, 1)) {
        return Err(Error::Schema);
    }
    let saved: (Uuid, Uuid, Uuid) = read(conn, "authority")?.ok_or(Error::Storage)?;
    if saved != (installation, backend.instance, backend.tenant) {
        return Err(Error::Identity);
    }
    Ok(())
}

/// Decide whether DA still holds authority that an MDM retirement cannot release.
pub fn has_live_credentials(root: &Path, installation: Uuid) -> Result<bool, Error> {
    match root.symlink_metadata() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
        Ok(meta) if !meta.is_dir() => return Err(Error::Storage),
        _ => (),
    }
    private_root(root)?;
    if std::fs::read_dir(root)?.next().transpose()?.is_none() {
        return Ok(false);
    }
    if files::read(&root.join("initialized"), 16)? != b"da-v1" {
        return Err(Error::Schema);
    }
    let mut db = connection(root, false)?;
    let tx = db.transaction()?;
    if files::sqlite_format(&tx)? != Some((APP, 1)) {
        return Err(Error::Schema);
    }
    let (owner, _, _): (Uuid, Uuid, Uuid) = read(&tx, "authority")?.ok_or(Error::Storage)?;
    if owner != installation {
        return Err(Error::Identity);
    }
    let active = read::<super::registration::Active>(&tx, "active")?;
    let pending = read::<Option<super::registration::Pending>>(&tx, "enrollment")?.flatten();
    let release: Vec<(String, bool)> = read(&tx, "release")?.unwrap_or_default();
    Ok(active.is_some() || pending.is_some() || !release.is_empty())
}

/// Return only the committed, locally released credential withdrawal for this installed authority.
pub fn retired_receipt(
    root: &Path,
    backend: &Backend,
    installation: Uuid,
    operation: Uuid,
) -> Result<rss_mdm_agent_wire::da::RevocationReceipt, Error> {
    let mut db = inspect(root, backend, installation)?.ok_or(Error::Identity)?;
    let tx = db.transaction()?;
    if read::<super::registration::Active>(&tx, "active")?.is_some()
        || read::<Option<super::registration::Pending>>(&tx, "enrollment")?
            .flatten()
            .is_some()
        || !read::<Vec<(String, bool)>>(&tx, "release")?
            .unwrap_or_default()
            .is_empty()
    {
        return Err(Error::Conflict);
    }
    let receipt: rss_mdm_agent_wire::da::RevocationReceipt =
        read(&tx, "retired-registration")?.ok_or(Error::Denied)?;
    if operation.is_nil()
        || receipt.credential.is_nil()
        || (receipt.operation_id, receipt.service_id, receipt.tenant)
            != (operation, backend.instance, backend.tenant)
        || read::<rss_mdm_agent_wire::da::RevocationReceipt>(&tx, "revocation")?.as_ref()
            != Some(&receipt)
    {
        return Err(Error::Identity);
    }
    Ok(receipt)
}
pub(super) fn inspect(
    root: &Path,
    backend: &Backend,
    installation: Uuid,
) -> Result<Option<Connection>, Error> {
    if !root.try_exists()? {
        return Ok(None);
    }
    private_root(root)?;
    if !root.join("da.sqlite").try_exists()? && !root.join("initialized").try_exists()? {
        return Ok(None);
    }
    let c = connection(root, false)?;
    verify(&c, backend, installation)?;
    Ok(Some(c))
}

impl Journal {
    pub fn open(root: &Path, backend: &Backend, installation: Uuid) -> Result<Self, Error> {
        files::directory(root)?;
        private_root(root)?;
        let lock_path = root.join("owner.lock");
        let lock = match files::create_new(&lock_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                files::open_existing(&lock_path)?
            }
            Err(e) => return Err(e.into()),
        };
        lock.try_lock_exclusive().map_err(|_| Error::Conflict)?;
        let path = root.join("da.sqlite");
        let marker = root.join("initialized");
        let published = marker.try_exists()?;
        if published {
            if files::read(&marker, 16)? != b"da-v1" || !path.try_exists()? {
                return Err(Error::Storage);
            }
        } else {
            match files::create_new(&path) {
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e.into()),
            }
        }
        let mut db = connection(root, true)?;
        db.pragma_update(None, "synchronous", "FULL")?;

        if !published && files::sqlite_format(&db)?.is_none() {
            let t = db.transaction()?;
            t.execute_batch("CREATE TABLE state(name TEXT PRIMARY KEY,body BLOB NOT NULL);
                CREATE TABLE reports(kind TEXT NOT NULL,sequence INTEGER NOT NULL,body BLOB,operation TEXT,receipt BLOB,error TEXT,PRIMARY KEY(kind,sequence)); CREATE INDEX pending_reports ON reports(kind,sequence) WHERE body IS NOT NULL AND error IS NULL;")?;
            write(
                &t,
                "authority",
                &(installation, backend.instance, backend.tenant),
            )?;
            t.pragma_update(None, "application_id", APP)?;
            t.pragma_update(None, "user_version", 1)?;
            t.commit()?;
        }
        verify(&db, backend, installation)?;
        if !published {
            files::write_new(&marker, b"da-v1")?;
        }
        Ok(Self { db, _writer: lock })
    }
}
pub(super) fn read<T: DeserializeOwned>(db: &Connection, name: &str) -> Result<Option<T>, Error> {
    let bytes: Option<Vec<u8>> = db
        .query_row("SELECT body FROM state WHERE name=?1", [name], |r| r.get(0))
        .optional()?;
    bytes.as_deref().map(decode).transpose()
}
pub(super) fn write(db: &Connection, name: &str, value: &impl Serialize) -> Result<(), Error> {
    db.execute(
        "INSERT INTO state VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET body=excluded.body",
        rusqlite::params![name, encode(value)?],
    )?;
    Ok(())
}

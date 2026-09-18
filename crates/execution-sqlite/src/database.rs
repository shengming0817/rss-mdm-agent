use crate::{Error, Limits};
use execution_contract::Authority;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::path::Path;

pub(crate) const SCHEMA_VERSION: u32 = 1;
const APPLICATION_ID: u32 = 0x52534558;

/// Protected synchronous journal. Open separate handles for separate threads; SQLite,
/// not an in-process mutex, arbitrates writers. No runner or background tasks are owned.
pub struct Store {
    pub(crate) conn: Connection,
    pub(crate) limits: Limits,
    pub(crate) authority: Authority,
}
/// Opening a newer database never creates a write-capable handle.
pub enum OpenOutcome {
    /// Validated current database.
    Ready(Box<Store>),
    /// Header diagnostics only; no business queries, migrations or execution methods.
    NewerSchema {
        /// Version found using a read-only SQLite connection.
        found: u32,
        /// Maximum version this binary understands.
        supported: u32,
    },
}
impl Store {
    /// Explicit S1 bootstrap into a precreated private directory. Existing files are never
    /// overwritten. Local/enterprise authority bootstrap requires a later product adapter.
    pub fn initialize_test(
        path: &Path,
        authority: Authority,
        limits: Limits,
    ) -> Result<Self, Error> {
        limits.validate()?;
        if !matches!(authority, Authority::Test { .. }) {
            return Err(Error::Denied);
        }
        protected_path(path, false)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        drop(options.open(path).map_err(|_| Error::Storage)?);
        let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        configure(&conn, limits)?;
        migrate(&mut conn, &authority)?;
        protected_path(path, true)?;
        Ok(Self {
            conn,
            limits,
            authority,
        })
    }
    /// Open ONLY an existing database after read-only identity/version inspection.
    /// Failure retains the original file; no fallback path or implicit authority creation.
    pub fn open(path: &Path, authority: &Authority, limits: Limits) -> Result<OpenOutcome, Error> {
        limits.validate()?;
        protected_path(path, true)?;
        let reader = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let application: u32 = reader.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: u32 = reader.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if application != APPLICATION_ID {
            return Err(Error::Schema);
        }
        if version > SCHEMA_VERSION {
            return Ok(OpenOutcome::NewerSchema {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        if version != SCHEMA_VERSION {
            return Err(Error::Schema);
        }
        let encoded: Vec<u8> = reader.query_row(
            "SELECT authority FROM metadata WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        let stored: Authority = crate::journal::decode(&encoded, limits.max_record_bytes)?;
        if &stored != authority {
            return Err(Error::Denied);
        }
        drop(reader);
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        ensure_current(&conn, authority, limits)?;
        configure(&conn, limits)?;
        protected_path(path, true)?;
        Ok(OpenOutcome::Ready(Box::new(Self {
            conn,
            limits,
            authority: stored,
        })))
    }
}
pub(crate) fn ensure_current(
    conn: &Connection,
    authority: &Authority,
    limits: Limits,
) -> Result<(), Error> {
    let application: u32 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if application != APPLICATION_ID || version != SCHEMA_VERSION {
        return Err(Error::Schema);
    }
    let encoded: Vec<u8> = conn.query_row(
        "SELECT authority FROM metadata WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    let stored: Authority = crate::journal::decode(&encoded, limits.max_record_bytes)?;
    if &stored != authority {
        return Err(Error::Denied);
    }
    Ok(())
}
fn configure(conn: &Connection, limits: Limits) -> Result<(), Error> {
    conn.busy_timeout(std::time::Duration::from_millis(u64::from(
        limits.busy_timeout_ms,
    )))?;
    conn.execute_batch(
        "PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;",
    )?;
    #[cfg(target_os = "macos")]
    conn.execute_batch("PRAGMA fullfsync=ON; PRAGMA checkpoint_fullfsync=ON;")?;
    let mode: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
    let sync: u32 = conn.pragma_query_value(None, "synchronous", |r| r.get(0))?;
    let foreign: bool = conn.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
    if mode != "wal" || sync != 2 || !foreign {
        return Err(Error::Configuration);
    }
    let pages: u32 = conn.query_row(
        &format!("PRAGMA max_page_count={}", limits.max_database_pages),
        [],
        |r| r.get(0),
    )?;
    if pages != limits.max_database_pages {
        return Err(Error::Capacity);
    }
    Ok(())
}
// Initial schema is the only released format. Future forward migrations belong here, in the
// same transaction as their version stamp; no legacy reader or artificial v2 exists.
// ref: rusqlite src/transaction.rs@499cc7bb986e04cc66e6ed762522f7ea449178d1
fn migrate(conn: &mut Connection, authority: &Authority) -> Result<(), Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != 0 {
        return Err(Error::Schema);
    }
    apply_schema(&tx, authority)?;
    tx.commit().map_err(|_| Error::CommitUnknown)
}
fn apply_schema(tx: &rusqlite::Transaction<'_>, authority: &Authority) -> Result<(), Error> {
    tx.execute_batch(include_str!("schema.sql"))?;
    tx.execute(
        "INSERT INTO metadata VALUES(1,?1,0)",
        [crate::journal::encode(authority, 4096)?],
    )?;
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    let mut check = tx.prepare("PRAGMA foreign_key_check")?;
    if check.query([])?.next()?.is_some() {
        return Err(Error::Corrupt);
    }
    Ok(())
}
fn protected_path(path: &Path, existing: bool) -> Result<(), Error> {
    if !path.is_absolute() {
        return Err(Error::Storage);
    }
    let parent = path.parent().ok_or(Error::Storage)?;
    if parent.canonicalize().map_err(|_| Error::Storage)? != parent {
        return Err(Error::Storage);
    }
    check_metadata(parent, true)?;
    if existing {
        check_metadata(path, false)?;
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let sidecar = Path::new(&sidecar);
        match std::fs::symlink_metadata(sidecar) {
            Ok(_) => check_metadata(sidecar, false)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Storage),
        }
    }
    Ok(())
}
fn check_metadata(path: &Path, directory: bool) -> Result<(), Error> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| Error::Storage)?;
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(Error::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(Error::Storage);
        }
    }
    // Windows ACL enforcement is not claimed by S1; only explicit Test bootstrap is supplied.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn authority() -> Authority {
        Authority::Test {
            id: execution_contract::Id::new("migration-test").unwrap(),
        }
    }
    fn temporary(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "execution-migration-{}-{name}.db",
            std::process::id()
        ))
    }
    fn assert_empty(conn: &Connection) {
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='metadata'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
    }
    #[test]
    fn ddl_failure_rolls_back_every_statement_and_version() {
        let path = temporary("ddl");
        let mut conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE attempts(x INTEGER);")
            .unwrap();
        assert!(migrate(&mut conn, &authority()).is_err());
        drop(conn);
        let conn = Connection::open(&path).unwrap();
        assert_empty(&conn);
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn interrupted_migration_never_publishes_partial_schema_or_version() {
        const KEY: &str = "EXECUTION_SQLITE_MIGRATION_TEST";
        if let Some(path) = std::env::var_os(KEY) {
            let mut conn = Connection::open(path).unwrap();
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
                .unwrap();
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            apply_schema(&tx, &authority()).unwrap();
            // Terminate without committing or running Rust/SQLite destructors.
            std::process::exit(0);
        }
        let path = temporary("interrupt");
        assert!(std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "database::tests::interrupted_migration_never_publishes_partial_schema_or_version"
            ])
            .env(KEY, &path)
            .status()
            .unwrap()
            .success());
        let mut conn = Connection::open(&path).unwrap();
        assert_empty(&conn);
        migrate(&mut conn, &authority()).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }
}

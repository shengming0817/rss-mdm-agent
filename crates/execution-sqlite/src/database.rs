use crate::{Error, Limits};
use execution_contract::Authority;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::path::Path;

// Includes persisted lifecycle records and the journal fingerprint domain, not just DDL.
pub(crate) const SCHEMA_VERSION: u32 = 2;
const APPLICATION_ID: u32 = 0x52534558;

// Only internal schema column names are accepted, never caller-provided SQL.
// SQLite reads a BLOB's length from its record header; nested CASE evaluation is lazy.
// Invalid types (including NULL), negative lengths and oversized values become NULL,
// so Vec<u8> decoding fails closed before SQLite or Rust materializes the payload.
// No length-to-usize conversion is needed. All maxima come from validated Limits.
// ref: SQLite src/func.c (lengthFunc), lang_expr.html#the_case_expression.
pub(crate) fn bounded_blob(column: &'static str, max: usize) -> String {
    format!("CASE WHEN typeof({column})='blob' THEN CASE WHEN length({column}) BETWEEN 0 AND {max} THEN {column} END END")
}

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
        if std::fs::symlink_metadata(path).is_ok() {
            return Err(Error::Storage);
        }
        let (staged, mut conn) = staging_connection(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        configure(&conn, limits)?;
        migrate(&mut conn, &authority)?;
        // Publish only a complete standalone file. No WAL frames may be left at the old name.
        let busy: u32 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
        if busy != 0 {
            return Err(Error::Busy);
        }
        conn.close().map_err(|_| Error::Storage)?;
        std::fs::File::open(&staged)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Storage)?;
        // A hard link is an atomic no-replace publication on the same filesystem.
        std::fs::hard_link(&staged, path).map_err(|_| Error::Storage)?;
        sync_parent(path)?;
        std::fs::remove_file(&staged).map_err(|_| Error::Storage)?;
        sync_parent(path)?;
        match Self::open(path, &authority, limits)? {
            OpenOutcome::Ready(store) => Ok(*store),
            OpenOutcome::NewerSchema { .. } => Err(Error::Schema),
        }
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
            &format!(
                "SELECT {} FROM metadata WHERE singleton=1",
                bounded_blob("authority", limits.max_record_bytes)
            ),
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
fn staging_connection(final_path: &Path) -> Result<(std::path::PathBuf, Connection), Error> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Storage)?
        .as_nanos();
    let staged = final_path.parent().ok_or(Error::Storage)?.join(format!(
        ".execution-bootstrap-{}-{nonce}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(unix)]
    drop(options.open(&staged).map_err(|_| Error::Storage)?);
    #[cfg(windows)]
    drop(native_process::private_storage::create_new(&staged).map_err(|_| Error::Storage)?);
    let conn = Connection::open_with_flags(&staged, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    Ok((staged, conn))
}
fn sync_parent(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    std::fs::File::open(path.parent().ok_or(Error::Storage)?)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Storage)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
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
        &format!(
            "SELECT {} FROM metadata WHERE singleton=1",
            bounded_blob("authority", limits.max_record_bytes)
        ),
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
// Bootstrap only the current format, including its lifecycle snapshot and fingerprint encoding.
// Existing versions are rejected by open; no migration or legacy reader is provided.
// ref: rusqlite src/transaction.rs@499cc7bb986e04cc66e6ed762522f7ea449178d1
fn migrate(conn: &mut Connection, authority: &Authority) -> Result<(), Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != 0 {
        return Err(Error::Schema);
    }
    apply_schema(&tx, authority)?;
    tx.commit().map_err(|_| Error::BootstrapUnpublished)
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
    #[cfg(windows)]
    native_process::private_storage::validate(path).map_err(|_| Error::Storage)?;
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
    fn bootstrap_commit_failure_is_unpublished_and_retryable() {
        let path = temporary("commit-failure");
        let mut conn = Connection::open(&path).unwrap();
        conn.busy_timeout(std::time::Duration::ZERO).unwrap();
        conn.execute_batch("CREATE TABLE fixture(id INTEGER)")
            .unwrap();
        let reader = Connection::open(&path).unwrap();
        reader
            .execute_batch("BEGIN; SELECT * FROM fixture;")
            .unwrap();
        // A rollback-journal reader permits schema writes but blocks COMMIT.
        let error = migrate(&mut conn, &authority()).unwrap_err();
        assert_eq!(error, Error::BootstrapUnpublished);
        assert_empty(&conn);
        reader.execute_batch("ROLLBACK").unwrap();
        migrate(&mut conn, &authority()).unwrap();
        drop(reader);
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn bounded_projection_never_returns_oversized_or_non_blob_payloads() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE fixture(payload);
            INSERT INTO fixture VALUES(NULL),(-1),(1.5),('text'),(zeroblob(0)),(zeroblob(16)),(zeroblob(17)),(zeroblob(2097152));").unwrap();
        let mut statement = conn
            .prepare(&format!(
                "SELECT {} FROM fixture ORDER BY rowid",
                bounded_blob("payload", 16)
            ))
            .unwrap();
        let lengths: Vec<_> = statement
            .query_map([], |row| {
                // Inspect the SQL result before any owned blob allocation or decoder runs.
                Ok(match row.get_ref(0)? {
                    rusqlite::types::ValueRef::Null => None,
                    rusqlite::types::ValueRef::Blob(bytes) => Some(bytes.len()),
                    other => panic!("unbounded SQL value: {other:?}"),
                })
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            lengths,
            vec![None, None, None, None, Some(0), Some(16), None, None]
        );
    }
    #[test]
    fn non_durable_sqlite_configuration_is_configuration_error() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(configure(&conn, test_limits()), Err(Error::Configuration));
    }
    fn test_limits() -> Limits {
        Limits {
            plan: execution_contract::PlanLimits {
                max_input_bytes: 65_536,
                max_depth: 32,
                max_nodes: 4096,
                max_string_bytes: 4096,
                max_collection_items: 128,
                max_timeout_ms: 60_000,
                max_output_bytes: 65_536,
                max_stdin_bytes: 65_536,
                max_attempts: 3,
            },
            lifecycle: execution_lifecycle::Limits {
                max_snapshot_bytes: 16_384,
            },
            interaction: execution_interaction::Limits {
                max_snapshot_bytes: 16_384,
                max_lifetime_ms: 60_000,
            },
            max_approvals: 8,
            max_record_bytes: 131_072,
            max_receipts: 1000,
            max_database_pages: 16_384,
            max_consumers: 8,
            max_batch: 64,
            busy_timeout_ms: 1000,
        }
    }
    #[test]
    fn interrupted_bootstrap_recovers_only_through_public_api() {
        const KEY: &str = "EXECUTION_SQLITE_MIGRATION_TEST";
        if let Some(path) = std::env::var_os(KEY) {
            let (_, mut conn) = staging_connection(Path::new(&path)).unwrap();
            let phase = std::env::var("EXECUTION_SQLITE_MIGRATION_PHASE").unwrap();
            if phase == "created" {
                std::process::exit(0);
            }
            conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
            configure(&conn, test_limits()).unwrap();
            if phase == "schema" {
                let tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .unwrap();
                apply_schema(&tx, &authority()).unwrap();
                std::process::exit(0);
            }
            migrate(&mut conn, &authority()).unwrap();
            std::process::exit(0);
        }
        for phase in ["created", "schema", "committed"] {
            let root = temporary(phase);
            std::fs::create_dir(&root).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            let root = root.canonicalize().unwrap();
            let path = root.join("authority.db");
            assert!(std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "database::tests::interrupted_bootstrap_recovers_only_through_public_api"
                ])
                .env(KEY, &path)
                .env("EXECUTION_SQLITE_MIGRATION_PHASE", phase)
                .status()
                .unwrap()
                .success());
            assert!(!path.exists());
            // Consumer recovery never opens/migrates/relabels the abandoned staging file.
            drop(Store::initialize_test(&path, authority(), test_limits()).unwrap());
            assert!(matches!(
                Store::open(&path, &authority(), test_limits()).unwrap(),
                OpenOutcome::Ready(_)
            ));
            assert!(std::fs::read_dir(&root).unwrap().count() >= 2);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

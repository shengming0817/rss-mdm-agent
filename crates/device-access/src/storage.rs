use super::*;
const SCHEMA: &str = "CREATE TABLE installation(id TEXT NOT NULL);
CREATE TABLE registration(channel INTEGER PRIMARY KEY CHECK(channel IN (1,2)),body BLOB NOT NULL);
CREATE TABLE join_epoch(id INTEGER PRIMARY KEY CHECK(id=1),fingerprint TEXT NOT NULL,observed INTEGER NOT NULL);
CREATE TABLE revoked(owner BLOB NOT NULL,binding TEXT NOT NULL,generation INTEGER NOT NULL,revision INTEGER NOT NULL,PRIMARY KEY(owner,binding));
CREATE TABLE candidates(id TEXT PRIMARY KEY,body BLOB NOT NULL);
CREATE TABLE bindings(sid TEXT PRIMARY KEY,body BLOB NOT NULL);
CREATE TABLE hello_bindings(sid TEXT PRIMARY KEY,body BLOB NOT NULL);
CREATE TABLE recovery_uses(id TEXT PRIMARY KEY,owner BLOB NOT NULL,body BLOB NOT NULL);
PRAGMA application_id=1380204865; PRAGMA user_version=4;";
pub(super) fn open(root: &Path, installation: Uuid, create: bool) -> Result<Store, Error> {
    if installation.is_nil() || !platform_private_storage::is_canonical_path(root)? {
        return Err(Error::Invalid);
    }
    if !create {
        validate_existing(root, installation)?;
    }
    let directory = platform_private_storage::PrivateDirectory::open(root)?;
    let path = root.join("device-access.sqlite");
    if create {
        drop(directory.create_file(Path::new("device-access.sqlite"))?);
    }
    check_files(&path)?;
    let mut conn = Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    validate_format(&conn, create)?;
    conn.busy_timeout(std::time::Duration::from_millis(250))?;
    conn.execute_batch("PRAGMA trusted_schema=OFF;")?;
    if !create {
        // SQLite's pager owns the exclusive recovery lock. Validate recovered
        // facts before configuration changes or any factor transaction.
        // ref: SQLite src/pager.c sqlite3PagerSharedLock.
        validate_schema(&conn, installation)?;
    }
    conn.pragma_update(None, "max_page_count", 16384)?;
    conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")?;
    if create {
        conn.execute_batch("PRAGMA journal_mode=DELETE;")?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(SCHEMA)?;
        tx.execute(
            "INSERT INTO installation VALUES(?1)",
            [installation.to_string()],
        )?;
        tx.commit()?;
        directory.sync()?;
        validate_schema(&conn, installation)?;
    }
    check_files(&path)?;
    Ok(Store {
        conn,
        path,
        _directory: directory,
    })
}

fn validate_schema(conn: &Connection, installation: Uuid) -> Result<(), Error> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let app: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let binding: String = conn.query_row("SELECT id FROM installation", [], |r| r.get(0))?;
    let mode: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
    if version != 4 || app != 1380204865 || binding != installation.to_string() || mode != "delete"
    {
        return Err(Error::Invalid);
    }
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(SCHEMA)?;
    fn catalog(conn: &Connection) -> Result<Vec<(String, String)>, Error> {
        let mut query = conn.prepare(
            "SELECT name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = query.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    if catalog(conn)? != catalog(&expected)? {
        return Err(Error::Invalid);
    }
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(Error::Storage);
    }
    Ok(())
}
impl Store {
    pub(super) fn check_files(&self) -> Result<(), Error> {
        check_files(&self.path)
    }
}
fn check_files(path: &Path) -> Result<(), Error> {
    platform_private_storage::validate_sqlite_file(path)?;
    for suffix in ["-journal", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let sidecar = Path::new(&name);
        match std::fs::symlink_metadata(sidecar) {
            Ok(_) => platform_private_storage::validate_sqlite_file(sidecar)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub(super) fn validate_existing(root: &Path, installation: Uuid) -> Result<(), Error> {
    if installation.is_nil() || !platform_private_storage::is_canonical_path(root)? {
        return Err(Error::Invalid);
    }
    let path = root.join("device-access.sqlite");
    check_files(&path)?;
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    validate_format(&conn, false)?;
    match conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)) {
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.extended_code == rusqlite::ffi::SQLITE_READONLY_ROLLBACK =>
        {
            Ok(())
        }
        result => {
            result?;
            validate_schema(&conn, installation)
        }
    }
}

fn validate_format(conn: &Connection, create: bool) -> Result<(), Error> {
    match platform_private_storage::sqlite_format(conn).map_err(|_| Error::Invalid)? {
        None if create => Ok(()),
        Some((1380204865, 4)) => Ok(()),
        _ => Err(Error::Invalid),
    }
}

use execution_contract::ExecutionLimits;
/// Static failure codes; never include SQL, paths, input payloads or provider text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Invalid explicit store limits or SQLite durability configuration.
    #[error("invalid storage configuration")]
    Configuration,
    /// Invalid operation identity, query arguments or new aggregate input; correct the call.
    #[error("invalid storage operation input")]
    InvalidInput,
    /// Authentication, scope, or requested access was rejected.
    #[error("storage access denied")]
    Denied,
    /// Requested record does not exist in the authorized scope.
    #[error("record not found")]
    NotFound,
    /// Operation content, identity or conditional revision conflicts.
    #[error("storage identity or revision conflict")]
    Conflict,
    /// Reliable time is unavailable or moved behind a committed watermark.
    #[error("reliable clock unavailable")]
    Clock,
    /// Protected trust data is missing, expired or invalid.
    #[error("trusted state unavailable")]
    Trust,
    /// Another connection holds a conflicting lock.
    #[error("database busy")]
    Busy,
    /// Logical quota, bounded input, or physical database capacity was exhausted.
    #[error("storage capacity exceeded")]
    Capacity,
    /// Database or bounded stored data is malformed.
    #[error("database corrupt")]
    Corrupt,
    /// Database identity or schema cannot be opened by this writer.
    #[error("unsupported database schema")]
    Schema,
    /// Storage protection or filesystem operation failed.
    #[error("protected storage unavailable")]
    Storage,
    /// Operation commit did not report success; query or resubmit the SAME operation ID.
    /// Never use a new ID or redispatch a runner to recover this error.
    #[error("operation commit outcome unknown")]
    OperationCommitUnknown,
    /// Confirmation commit did not report success; retry the SAME scope/consumer/event.
    /// No operation receipt exists for confirmation; retry is idempotent.
    #[error("confirmation commit outcome unknown")]
    ConfirmationCommitUnknown,
    /// Migration commit failed in an isolated bootstrap file; no database was published.
    /// Retry initialize_test with the same intended path, or diagnose storage first.
    #[error("bootstrap database not published")]
    BootstrapUnpublished,
}
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode::*;
        match value {
            rusqlite::Error::SqliteFailure(e, _) => match e.code {
                DatabaseBusy | DatabaseLocked => Self::Busy,
                DiskFull | TooBig => Self::Capacity,
                ConstraintViolation => Self::Conflict,
                DatabaseCorrupt | NotADatabase => Self::Corrupt,
                _ => Self::Storage,
            },
            rusqlite::Error::QueryReturnedNoRows => Self::NotFound,
            _ => Self::Corrupt,
        }
    }
}
/// Explicit resource bounds, supplied by the product rather than inferred from a database.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Frozen plan decoder limits.
    pub input: ExecutionLimits,
    /// Lifecycle snapshot limit, including terminal-state headroom.
    pub lifecycle: execution_lifecycle::Limits,
    /// Interaction bounds.
    pub interaction: execution_interaction::Limits,
    /// Maximum records/profiles in one trust update or admission.
    pub max_approvals: usize,
    /// Maximum encoded receipt/audit/trust record bytes.
    pub max_record_bytes: usize,
    /// Maximum retained receipts plus reserved terminal receipt slots.
    pub max_receipts: u64,
    /// Per-connection SQLite page ceiling; physical disk exhaustion may occur earlier.
    pub max_database_pages: u32,
    /// Maximum distinct delivery consumers per scope.
    pub max_consumers: u32,
    /// Maximum results returned by one query.
    pub max_batch: usize,
    /// Bounded SQLite lock wait; zero is a valid nonblocking configuration.
    pub busy_timeout_ms: u32,
}
impl Limits {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if self.max_approvals == 0
            || self.max_approvals > 128
            || self.max_batch == 0
            || self.max_batch > 4096
            || self.max_receipts < 9
            || self.max_receipts > i64::MAX as u64
            || self.max_database_pages == 0
            || self.max_consumers == 0
            || self.max_consumers > 128
            || self.max_record_bytes < 65_536
            || self.max_record_bytes > 16 * 1024 * 1024
            || self.lifecycle.max_snapshot_bytes < execution_lifecycle::MIN_SNAPSHOT_BYTES
            || self.lifecycle.max_snapshot_bytes > self.max_record_bytes
            || self.interaction.max_snapshot_bytes == 0
            || self.interaction.max_snapshot_bytes > self.max_record_bytes
            || self.interaction.max_lifetime_ms == 0
            || self.input.max_input_bytes > self.max_record_bytes
            || self.busy_timeout_ms > 60_000
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
}

impl From<execution_app::JournalError> for Error {
    fn from(error: execution_app::JournalError) -> Self {
        match error {
            execution_app::JournalError::Configuration => Self::Configuration,
            execution_app::JournalError::InvalidInput => Self::InvalidInput,
            execution_app::JournalError::Denied => Self::Denied,
            execution_app::JournalError::NotFound => Self::NotFound,
            execution_app::JournalError::Conflict => Self::Conflict,
            execution_app::JournalError::Clock => Self::Clock,
            execution_app::JournalError::Trust => Self::Trust,
            execution_app::JournalError::Busy => Self::Busy,
            execution_app::JournalError::Capacity => Self::Capacity,
            execution_app::JournalError::Corrupt => Self::Corrupt,
            execution_app::JournalError::Schema => Self::Schema,
            execution_app::JournalError::Storage => Self::Storage,
            execution_app::JournalError::OperationCommitUnknown => Self::OperationCommitUnknown,
            execution_app::JournalError::ConfirmationCommitUnknown => {
                Self::ConfirmationCommitUnknown
            }
        }
    }
}
impl From<Error> for execution_app::JournalError {
    fn from(error: Error) -> Self {
        match error {
            Error::Configuration => Self::Configuration,
            Error::InvalidInput => Self::InvalidInput,
            Error::Denied => Self::Denied,
            Error::NotFound => Self::NotFound,
            Error::Conflict => Self::Conflict,
            Error::Clock => Self::Clock,
            Error::Trust => Self::Trust,
            Error::Busy => Self::Busy,
            Error::Capacity => Self::Capacity,
            Error::Corrupt => Self::Corrupt,
            Error::Schema => Self::Schema,
            Error::Storage => Self::Storage,
            Error::OperationCommitUnknown => Self::OperationCommitUnknown,
            Error::ConfirmationCommitUnknown => Self::ConfirmationCommitUnknown,
            Error::BootstrapUnpublished => Self::Storage,
        }
    }
}
impl From<Error> for execution_app::Error {
    fn from(error: Error) -> Self {
        execution_app::JournalError::from(error).into()
    }
}

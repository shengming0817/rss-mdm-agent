use crate::Limits;
/// Fixed S1 protected-storage envelope. These bootstrap values are never hot-replaced.
pub fn test_store_limits() -> Limits {
    Limits {
        input: execution_app::test_execution_limits(),
        lifecycle: execution_lifecycle::Limits {
            max_snapshot_bytes: 16_384,
        },
        interaction: execution_interaction::Limits {
            max_snapshot_bytes: 16_384,
            max_lifetime_ms: 60_000,
        },
        max_approvals: 128,
        max_record_bytes: 131_072,
        max_receipts: 10_000,
        max_database_pages: 32_768,
        max_consumers: 8,
        max_batch: 64,
        busy_timeout_ms: 1000,
    }
}

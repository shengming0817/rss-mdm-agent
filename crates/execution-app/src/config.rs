use crate::Error;
use execution_contract::ExecutionLimits;

/// Explicit application limits fenced by the selected immutable storage envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    /// Current maximum total plan timeout, at most the selected storage bound.
    pub max_timeout_ms: u64,
    /// Current maximum total plan output, at most the selected storage bound.
    pub max_output_bytes: u64,
}
impl AppConfig {
    /// Explicit limits for the test assembly.
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_defaults() -> Self {
        Self {
            max_timeout_ms: 60_000,
            max_output_bytes: 65_536,
        }
    }
    /// Validate static limits against the immutable journal envelope.
    pub fn validate(self, bounds: ExecutionLimits) -> Result<(), Error> {
        if self.max_timeout_ms == 0
            || self.max_timeout_ms > bounds.max_timeout_ms
            || self.max_output_bytes == 0
            || self.max_output_bytes > bounds.max_output_bytes
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
}

/// Explicit S1 execution envelope used by fixture plans and the test runner.
#[cfg(any(test, feature = "test-support"))]
pub fn test_execution_limits() -> ExecutionLimits {
    execution_contract::ExecutionLimits {
        max_input_bytes: 65_536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60_000,
        max_output_bytes: 65_536,
        max_stdin_bytes: 65_536,
        max_attempts: 3,
    }
}

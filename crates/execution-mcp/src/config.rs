use service_catalog::{CatalogLimits, ParameterLimits};
use std::time::Duration;

/// Required host limits. No protocol input can raise them.
#[derive(Debug, Clone)]
pub struct McpLimits {
    /// Maximum complete input frame, excluding its newline, in UTF-8 bytes.
    pub frame_bytes: usize,
    /// Maximum complete output frame, including protocol envelope and newline.
    pub response_bytes: usize,
    /// Maximum JSON nesting (root is one; at most 64).
    pub json_depth: usize,
    /// Maximum JSON values per frame; also bounds object keys and collection lengths.
    pub json_nodes: usize,
    /// Maximum accepted requests awaiting handler completion or response delivery.
    pub in_flight: usize,
    /// Maximum frames per connection, including notifications and rejected requests.
    /// A reconnect uses the same business IDs and does not reset execution budgets.
    pub session_frames: usize,
    /// Maximum duration of initialization and each tool/metadata request.
    pub request_timeout: Duration,
    /// Maximum input idle/partial-frame wait or complete output write.
    pub io_timeout: Duration,
    /// Existing catalog decoding/selection limits.
    pub catalog: CatalogLimits,
    /// Existing shared human/AI parameter limits.
    pub parameters: ParameterLimits,
}

/// Provider-neutral description of a trusted host's stdio launcher.
///
/// Only the host constructs this value; no Deserialize implementation is provided.
/// This is process configuration, never identity evidence. Provider adapters map the
/// same command/arguments into their native SDK configuration without adding credentials.
/// The launcher must obtain its service binding independently of model-editable arguments.
pub struct StdioServiceConfig {
    command: std::path::PathBuf,
    arguments: Vec<String>,
}
impl StdioServiceConfig {
    /// Construct bounded explicit configuration. The host must verify executable provenance.
    pub fn new(
        command: std::path::PathBuf,
        arguments: Vec<String>,
    ) -> Result<Self, crate::ServiceError> {
        if !command.is_absolute()
            || command
                .to_str()
                .is_none_or(|s| s.len() > 4096 || s.contains('\0'))
            || arguments.len() > 128
            || arguments.iter().any(|s| s.len() > 4096 || s.contains('\0'))
            || arguments.iter().map(String::len).sum::<usize>() > 65536
        {
            return Err(crate::ServiceError::InvalidInput);
        }
        Ok(Self { command, arguments })
    }
    /// Stable name used by all native provider adapters.
    pub fn name(&self) -> &'static str {
        "rss_execution"
    }
    /// Trusted executable location; never search a model-controlled PATH.
    pub fn command(&self) -> &std::path::Path {
        &self.command
    }
    /// Host-selected non-secret startup arguments.
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
    /// Sole supported MCP protocol revision.
    pub fn protocol_version(&self) -> &'static str {
        "2025-11-25"
    }
}
impl McpLimits {
    pub(crate) fn validate(&self) -> Result<(), crate::ServiceError> {
        let nonzero = [
            self.frame_bytes,
            self.response_bytes,
            self.json_depth,
            self.json_nodes,
            self.in_flight,
            self.session_frames,
            self.catalog.max_bytes,
            self.catalog.max_depth,
            self.catalog.max_nodes,
            self.catalog.max_string_bytes,
            self.catalog.max_collection_items,
            self.parameters.max_bytes,
            self.parameters.max_string_bytes,
            self.parameters.max_parameters,
        ]
        .into_iter()
        .all(|n| n > 0);
        if !nonzero
            || self.response_bytes < 1024
            || self.json_depth > 64
            || self.catalog.max_depth > 64
            || self.in_flight > 1024
            || self.request_timeout.is_zero()
            || self.io_timeout.is_zero()
            || self.request_timeout > Duration::from_secs(3600)
            || self.io_timeout > Duration::from_secs(3600)
        {
            return Err(crate::ServiceError::InvalidLimits);
        }
        Ok(())
    }
}

use crate::Error;
use execution_contract::ActorId;

/// Explicit S1 application limits. Defaults are fixture bounds, not a platform capacity SLO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    /// Monotonically increasing trusted configuration revision.
    pub revision: u64,
    /// Maximum C07 rules (hard ceiling 1024).
    pub max_rules: usize,
    /// Maximum approval profiles and records (hard ceiling 128).
    pub max_profiles: usize,
    /// Maximum total environment inventory entries (hard ceiling 4096).
    pub max_capability_entries: usize,
    /// Current maximum total plan timeout, at most the S1 storage bound.
    pub max_timeout_ms: u64,
    /// Current maximum total plan output, at most the S1 storage bound.
    pub max_output_bytes: u64,
}
impl AppConfig {
    /// Small explicit fixture limits; callers must select a revision.
    pub fn test_defaults(revision: u64) -> Self {
        Self {
            revision,
            max_rules: 32,
            max_profiles: 16,
            max_capability_entries: 128,
            max_timeout_ms: 60_000,
            max_output_bytes: 65_536,
        }
    }
    fn validate(self) -> Result<(), Error> {
        if self.revision == 0
            || self.max_rules == 0
            || self.max_rules > 1024
            || self.max_profiles == 0
            || self.max_profiles > 128
            || self.max_capability_entries == 0
            || self.max_capability_entries > 4096
            || self.max_timeout_ms == 0
            || self.max_timeout_ms > 60_000
            || self.max_output_bytes == 0
            || self.max_output_bytes > 65_536
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
}

/// Current configuration health; none of these values can change test/production mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigState {
    /// Latest verified configuration was activated.
    Active,
    /// Latest load failed; retained configuration still meets the mandatory revision/bounds.
    LastKnownGood,
    /// Mandatory policy is no longer satisfied; new execution is disabled.
    Degraded,
}
/// Authenticated configuration change submitted before activation; no secret values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChange {
    /// Authenticated operator, obtained from the host binding.
    pub actor: ActorId,
    /// Previously active revision.
    pub previous_revision: u64,
    /// Candidate revision.
    pub next_revision: u64,
    /// Exact before values for audit correlation.
    pub previous: AppConfig,
    /// Exact after values for audit correlation.
    pub next: AppConfig,
}
/// Service-owned atomic configuration snapshot. Mutation requires exclusive service access.
/// The trusted host owns loading/persistence and its audit sink; S1 supplies no production file store.
pub struct Configuration {
    config: AppConfig,
    state: ConfigState,
    minimum_revision: u64,
}
impl Configuration {
    /// Validate the initial trusted snapshot without fallback or implicit defaults.
    pub fn new(config: AppConfig) -> Result<Self, Error> {
        config.validate()?;
        Ok(Self {
            config,
            state: ConfigState::Active,
            minimum_revision: config.revision,
        })
    }
    /// Health remains inspectable while degraded.
    pub fn state(&self) -> ConfigState {
        self.state
    }
    /// Last activated revision, including while degraded.
    pub fn revision(&self) -> u64 {
        self.config.revision
    }
    /// Obtain a conforming snapshot for new execution.
    pub fn active(&self) -> Result<AppConfig, Error> {
        if self.state == ConfigState::Degraded {
            Err(Error::Degraded)
        } else {
            Ok(self.config)
        }
    }
    /// Validate, record the trusted change, then atomically activate it. Audit failure preserves
    /// both the previous values and health; lower or reused revisions cannot replace content.
    pub fn replace(
        &mut self,
        next: AppConfig,
        actor: &ActorId,
        audit: impl FnOnce(&ConfigChange) -> Result<(), Error>,
    ) -> Result<(), Error> {
        next.validate()?;
        if next.revision <= self.config.revision || next.revision < self.minimum_revision {
            return Err(Error::Conflict);
        }
        audit(&ConfigChange {
            actor: actor.clone(),
            previous_revision: self.config.revision,
            next_revision: next.revision,
            previous: self.config,
            next,
        })?;
        self.config = next;
        self.state = ConfigState::Active;
        Ok(())
    }
    /// Record load failure against a trusted mandatory revision. The floor only increases;
    /// it cannot be supplied by UI/model input. Compiled hard bounds are always revalidated.
    pub fn load_failed(&mut self, mandatory_revision: u64) {
        self.minimum_revision = self.minimum_revision.max(mandatory_revision);
        self.state =
            if self.config.validate().is_ok() && self.config.revision >= self.minimum_revision {
                ConfigState::LastKnownGood
            } else {
                ConfigState::Degraded
            };
    }
}

/// Fixed S1 protected-storage envelope. These bootstrap values are never hot-replaced.
pub fn test_store_limits() -> execution_sqlite::Limits {
    execution_sqlite::Limits {
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
        max_approvals: 128,
        max_record_bytes: 131_072,
        max_receipts: 10_000,
        max_database_pages: 32_768,
        max_consumers: 8,
        max_batch: 64,
        busy_timeout_ms: 1000,
    }
}

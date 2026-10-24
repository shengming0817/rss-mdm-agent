//! Code-owned native software operations and their exact per-submission permit.
use crate::local_permissions::{Policy, Restrictions};
use agent_client::Error;
use execution_contract::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
pub(crate) mod compile;
pub mod native;
/// Physical installation scope selected by a code-owned native adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalScope {
    User,
    Device,
}
/// Private per-submission evidence. Serialized intents cannot construct a permit.
pub(crate) struct Permit {
    plan: FrozenExecution,
    policy: Policy,
}
impl Permit {
    pub(crate) fn new(plan: FrozenExecution, policy: Policy) -> Result<Self, Error> {
        policy.current()?;
        if !plan
            .spec()
            .policy
            .id
            .as_str()
            .starts_with("local-software:")
            || matches!(
                plan.spec().request.initiator,
                Initiator::Backend { .. } | Initiator::Policy { .. }
            )
        {
            return Err(Error::Denied);
        }
        Ok(Self { plan, policy })
    }
    pub(crate) fn matches(&self, plan: &FrozenExecution) -> bool {
        self.plan.digest() == plan.digest()
    }
    pub(crate) fn verify(&self, plan: &FrozenExecution, now: u64) -> Result<(), Error> {
        if plan.digest() != self.plan.digest() || now >= plan.spec().validity.expires_at_unix_ms {
            return Err(Error::Denied);
        }
        self.policy.current()
    }
}

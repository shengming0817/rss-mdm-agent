//! Native identity owner: short memory locks, immutable network snapshots, and one refresh flight.
use super::{account::EnterpriseSession, users::Users};
use crate::self_service::{error, Result};
use std::sync::{Arc, Mutex};

pub(super) struct IdentityOwner {
    pub(super) users: Arc<Mutex<Users>>,
    account: Mutex<Option<EnterpriseSession>>,
    pub(super) refresh: tokio::sync::Mutex<Refresh>,
}
impl IdentityOwner {
    pub(super) fn new(users: Arc<Mutex<Users>>) -> Self {
        Self {
            users,
            account: Mutex::new(None),
            refresh: tokio::sync::Mutex::new(Refresh { last: None }),
        }
    }
    pub(super) fn session(&self) -> Result<Option<EnterpriseSession>> {
        Ok(self
            .account
            .lock()
            .map_err(|_| error("account_unavailable", "身份状态不可用"))?
            .clone())
    }
    pub(super) fn set_session(&self, session: Option<EnterpriseSession>) -> Result<()> {
        *self
            .account
            .lock()
            .map_err(|_| error("account_unavailable", "身份状态不可用"))? = session;
        Ok(())
    }
    pub(super) fn take_session(&self) -> Result<Option<EnterpriseSession>> {
        Ok(self
            .account
            .lock()
            .map_err(|_| error("account_unavailable", "身份状态不可用"))?
            .take())
    }
    pub(super) fn snapshot(&self) -> Result<ai_session_contract::UserContext> {
        self.users
            .lock()
            .map_err(|_| error("account_unavailable", "身份状态不可用"))?
            .current()
    }
}

pub(super) struct Refresh {
    pub(super) last: Option<(String, std::time::Instant, Result<()>)>,
}

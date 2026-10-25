//! Apple protocol adapter. Vault remains the only owner of credential material.
mod request;
pub use request::Request;
#[cfg(target_os = "macos")]
pub mod index;
use passkey_vault::{
    ceremony::{Authorizer, Context, Decision},
    Entry, Error, Result, UnlockMethod, Vault,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

/// One system callback, one user decision, one completion. No reusable authorization token.
pub struct Session {
    request: Request,
    interaction: passkey_vault::Interaction,
    vault: Arc<Vault>,
    cancelled: AtomicBool,
    completed: AtomicBool,
    started: Instant,
    verified: AtomicBool,
}
impl Session {
    pub fn new(request: Request, vault: Arc<Vault>) -> Result<Self> {
        request.validate()?;
        Ok(Self {
            request,
            interaction: vault.begin_interaction(),
            vault,
            cancelled: AtomicBool::new(false),
            completed: AtomicBool::new(false),
            started: Instant::now(),
            verified: AtomicBool::new(false),
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire)
            || self.started.elapsed() >= Duration::from_secs(120)
        {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    /// Only the trusted native UI may call this after verification for this system request.
    pub fn unlock(&self, method: UnlockMethod, password: Option<&str>) -> Result<()> {
        self.check()?;
        if self.completed.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        self.vault.unlock(method, password, &self.interaction)?;
        self.check()?;
        self.verified.store(true, Ordering::Release);
        Ok(())
    }
    pub fn accounts(&self) -> Result<Vec<Entry>> {
        self.check()?;
        let status = self.vault.status()?;
        if !status.unlocked {
            return Err(Error::Locked);
        }
        Ok(status
            .entries
            .into_iter()
            .filter(|entry| self.request.matches(entry))
            .collect())
    }
    pub fn execute(&self, selected: Option<Vec<u8>>) -> Result<serde_json::Value> {
        self.check()?;
        if self.completed.swap(true, Ordering::AcqRel) {
            return Err(Error::Cancelled);
        }
        if !self.verified.load(Ordering::Acquire) {
            return Err(Error::Authentication);
        }
        if let Some(id) = &selected {
            if !self.accounts()?.iter().any(|e| &e.credential_id == id) {
                return Err(Error::NotFound);
            }
        }
        let approval = Approval {
            session: self,
            selected,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|_| Error::SystemUnavailable)?;
        let response = runtime.block_on(self.request.execute(&self.vault, &approval))?;
        self.check()?;
        // status rechecks the persisted cancellation generation as well as the local session.
        if !self.vault.status()?.unlocked {
            return Err(Error::Cancelled);
        }
        Ok(response)
    }
    pub fn vault(&self) -> &Arc<Vault> {
        &self.vault
    }
}
struct Approval<'a> {
    session: &'a Session,
    selected: Option<Vec<u8>>,
}
impl Authorizer for Approval<'_> {
    fn authorize(&self, context: &Context<'_>) -> Result<Decision> {
        self.check()?;
        if context.rp_id != self.session.request.rp()
            || context.client_data_hash != self.session.request.hash()
        {
            return Err(Error::Authentication);
        }
        Ok(Decision {
            selected: self.selected.clone(),
            verified: self.session.verified.load(Ordering::Acquire),
        })
    }
    fn check(&self) -> Result<()> {
        self.session.check()
    }
}
#[cfg(test)]
mod tests;

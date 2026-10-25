use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperationKind {
    Create,
    Assert,
}
struct State {
    active: Option<Arc<Operation>>,
    completed: HashSet<([u8; 16], [u8; 32])>,
}
/// One active ceremony per host. Native runtime additionally holds an OS-user file lease.
pub struct Operations {
    state: Mutex<State>,
    stopped: AtomicBool,
}
impl Default for Operations {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                active: None,
                completed: HashSet::new(),
            }),
            stopped: AtomicBool::new(false),
        }
    }
}
pub struct Operation {
    id: [u8; 16],
    digest: [u8; 32],
    kind: OperationKind,
    started: Instant,
    cancelled: AtomicBool,
}
impl Operation {
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire)
            || self.started.elapsed() >= Duration::from_secs(120)
        {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn matches(&self, kind: OperationKind, encoded: &[u8]) -> bool {
        self.kind == kind && self.digest == <[u8; 32]>::from(Sha256::digest(encoded))
    }
}
impl Operations {
    pub fn begin(
        &self,
        id: [u8; 16],
        kind: OperationKind,
        encoded: &[u8],
    ) -> Result<Arc<Operation>> {
        let mut state = self.state.lock().map_err(|_| Error::Busy)?;
        if self.stopped.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        if id == [0; 16] || encoded.is_empty() || encoded.len() > 131072 {
            return Err(Error::Input);
        }
        if state.active.is_some() {
            return Err(Error::Busy);
        }
        let digest = Sha256::digest(encoded).into();
        // No eviction that could make an old transaction reusable within this process.
        if state
            .completed
            .iter()
            .any(|(old, hash)| *old == id || *hash == digest)
        {
            return Err(Error::Cancelled);
        }
        if state.completed.len() >= 4096 {
            return Err(Error::Capacity);
        }
        let operation = Arc::new(Operation {
            id,
            digest,
            kind,
            started: Instant::now(),
            cancelled: AtomicBool::new(false),
        });
        state.active = Some(operation.clone());
        Ok(operation)
    }
    pub fn cancel(&self, id: [u8; 16]) -> Result<()> {
        let state = self.state.lock().map_err(|_| Error::Busy)?;
        let op = state
            .active
            .as_ref()
            .filter(|o| o.id == id)
            .ok_or(Error::NotFound)?;
        op.cancel();
        Ok(())
    }
    pub fn finish(&self, operation: &Arc<Operation>) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Busy)?;
        if !state
            .active
            .as_ref()
            .is_some_and(|o| Arc::ptr_eq(o, operation))
        {
            return Err(Error::Cancelled);
        }
        state.completed.insert((operation.id, operation.digest));
        state.active = None;
        operation.check()
    }
    pub fn is_idle(&self) -> bool {
        self.state.lock().is_ok_and(|s| s.active.is_none())
    }
    pub fn cancel_active(&self) {
        if let Ok(state) = self.state.lock() {
            if let Some(op) = &state.active {
                op.cancel();
            }
        }
    }
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(state) = self.state.lock() {
            if let Some(op) = &state.active {
                op.cancel();
            }
        }
    }
}

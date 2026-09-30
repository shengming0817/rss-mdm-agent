use super::*;
use std::sync::Condvar;

/// The physical owner waits here until the application returns a committed SQLite receipt.
/// Only one boundary is outstanding; memory does not become a second execution journal.
pub(super) struct Progress {
    state: Mutex<(Option<SoftwareProgress>, bool)>,
    committed: Condvar,
}
impl Progress {
    pub(super) fn complete(&self, plan: &FrozenExecution) -> Result<bool, Error> {
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        Ok(state.1 && state.0.as_ref().is_some_and(|p| p.complete(plan)))
    }
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new((None, false)),
            committed: Condvar::new(),
        }
    }
    pub(super) fn pending(&self) -> Result<Option<SoftwareProgress>, Error> {
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        Ok(if state.1 { None } else { state.0.clone() })
    }
    pub(super) fn acknowledge(
        &self,
        receipt: execution_app::CommittedSoftwareProgress,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        if state.0.as_ref() != Some(receipt.facts()) {
            return Err(Error::Conflict);
        }
        state.1 = true;
        self.committed.notify_all();
        Ok(())
    }
    pub(super) fn checkpoint(
        &self,
        facts: SoftwareProgress,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        if state
            .0
            .as_ref()
            .is_some_and(|previous| !facts.extends(previous))
        {
            return Err(Error::Conflict);
        }
        *state = (Some(facts), false);
        loop {
            if state.1 {
                return Ok(());
            }
            if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
                return Err(Error::OutcomeUnknown);
            }
            state = self
                .committed
                .wait_timeout(state, Duration::from_millis(100))
                .map_err(|_| Error::Unavailable)?
                .0;
        }
    }
}

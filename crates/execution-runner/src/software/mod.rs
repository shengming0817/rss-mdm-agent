//! Native read-only software detection and bounded preparation.
pub(crate) mod native_detection;
use execution_app::Error;
pub(crate) use native_detection::detect as native_detection;
use std::sync::Arc;
pub(crate) struct PreparationControl {
    pub deadline: std::time::Instant,
    pub cancelled: Arc<std::sync::atomic::AtomicBool>,
}
impl PreparationControl {
    pub(crate) fn check(&self) -> Result<(), Error> {
        if self.cancelled.load(std::sync::atomic::Ordering::Acquire)
            || std::time::Instant::now() >= self.deadline
        {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn test() -> Self {
        Self {
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

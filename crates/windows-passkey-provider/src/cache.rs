//! Serialize the COM host's cache clear and publication without holding the Vault writer lock.
use crate::{Error, Result};
use std::sync::Mutex;
static PUBLICATION: Mutex<()> = Mutex::new(());
pub(crate) fn update<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let _guard = PUBLICATION.try_lock().map_err(|_| Error::Busy)?;
    operation()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clearing_cannot_interleave_between_cache_publication_and_acknowledgement() {
        let cache = Mutex::new(Vec::new());
        update(|| {
            *cache.lock().unwrap() = vec![1];
            assert_eq!(
                update(|| {
                    cache.lock().unwrap().clear();
                    Ok(())
                }),
                Err(Error::Busy)
            );
            assert_eq!(*cache.lock().unwrap(), vec![1]);
            Ok(())
        })
        .unwrap();
        update(|| {
            cache.lock().unwrap().clear();
            Ok(())
        })
        .unwrap();
        assert!(cache.lock().unwrap().is_empty());
    }
}

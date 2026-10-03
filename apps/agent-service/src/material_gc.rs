//! Disk reclamation belongs to DeviceService between submissions, never network reconciliation.
use crate::DeviceService;
use agent_client::{Error, SecretProvider};
use std::collections::{BTreeMap, BTreeSet};

impl<S: SecretProvider> DeviceService<S> {
    pub(crate) fn reclaim_between_submissions(&mut self) {
        if let Err(error) = self.reclaim_materials() {
            // A closed, path-free diagnostic. Incomplete proof preserves materials; normal
            // communication continues, and every publisher still enforces the physical quota.
            eprintln!("material_reclaim_deferred: {error}");
        }
    }
    pub(crate) fn reclaim_materials(&mut self) -> Result<(), Error> {
        let mut associated = BTreeSet::new();
        for task in self
            .client
            .pending_tasks(self.client.configuration().limits.pending_tasks)?
        {
            if let Some(request) = self.client.bound_request(task)? {
                associated.insert(request);
            }
        }
        let existing = self
            .core
            .material_root
            .existing_files()
            .map_err(crate::error::app_error)?;
        let mut interpreter_digests = BTreeSet::new();
        let consumer = crate::backend::plan::id("backend-results")?;
        let mut retained = BTreeSet::new();
        let mut candidates = BTreeMap::new();
        let mut cursor = None;
        // No mutator runs until every page and every required delivery/reference read succeeds.
        loop {
            let page = self
                .core
                .app
                .service_journal_tasks(cursor.as_ref(), 64)
                .map_err(crate::error::app_error)?;
            for item in page.items {
                let request = &item.status.operation_request_id;
                let input = self
                    .core
                    .app
                    .service_frozen_input(request)
                    .map_err(crate::error::app_error)?;
                let references = crate::recovery::references(&input, &self.core.config)?;
                let communicating = associated.remove(request);
                let settled = crate::backend::settled(&item.status)
                    && !communicating
                    && self
                        .core
                        .app
                        .service_delivery(request, &consumer, 1)
                        .map_err(crate::error::app_error)?
                        .is_empty();
                if settled {
                    match self.core.host.materials.retire(&input) {
                        Ok(()) | Err(execution_app::Error::Conflict) => (),
                        Err(error) => return Err(crate::error::app_error(error)),
                    }
                    for (path, digest) in references
                        .into_iter()
                        .filter(|(path, _)| existing.contains(path))
                    {
                        if candidates
                            .insert(path, digest.clone())
                            .is_some_and(|old| old != digest)
                        {
                            return Err(Error::Untrusted);
                        }
                    }
                } else {
                    interpreter_digests.insert(
                        input
                            .spec()
                            .launch
                            .interpreter
                            .artifact
                            .sha256
                            .as_str()
                            .to_owned(),
                    );
                    if let Some(program) = input.spec().execution.software_program() {
                        for step in &program.steps {
                            for invocation in crate::recovery::software_invocations(step) {
                                interpreter_digests.insert(
                                    invocation
                                        .launch
                                        .interpreter
                                        .artifact
                                        .sha256
                                        .as_str()
                                        .to_owned(),
                                );
                            }
                        }
                    }
                    retained.extend(references.into_keys());
                }
                if retained.len() > 32_768
                    || candidates.len() > 32_768
                    || interpreter_digests.len() > 32_768
                {
                    return Err(Error::Capacity);
                }
            }
            match page.next {
                None => break,
                Some(next) => {
                    if cursor.as_ref().is_some_and(|old| old >= &next) {
                        return Err(Error::Storage);
                    }
                    cursor = Some(next);
                }
            }
        }
        if !associated.is_empty() {
            return Err(Error::Conflict);
        }
        candidates.retain(|_, digest| !interpreter_digests.contains(digest.as_str()));
        retained.extend(
            self.core
                .config
                .interpreters
                .iter()
                .map(|i| i.image.path.clone()),
        );
        retained.extend(
            self.core
                .config
                .managers
                .iter()
                .map(|m| m.image.path.clone()),
        );
        // Native and helper attempts retain their registered Arc recipes until capture settles.
        retained.extend(
            self.core
                .host
                .materials
                .retained_paths()
                .map_err(crate::error::app_error)?,
        );
        let result = self
            .core
            .material_root
            .reclaim(&candidates, &retained)
            .map_err(crate::error::app_error)?;
        if result.files != 0 || result.directories != 0 {
            eprintln!(
                "material_reclaimed: files={} bytes={} directories={}",
                result.files, result.bytes, result.directories
            );
        }
        Ok(())
    }
}

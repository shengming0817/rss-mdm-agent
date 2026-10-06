//! Cooperative reclamation belongs to the existing single device owner.
use crate::DeviceService;
use agent_client::{Error, IdentityProvider};
use execution_contract::{Digest, RequestId};
use execution_runner::{
    staging::{ReclaimPhase, Reclamation},
    RetainedMaterials,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Default)]
pub(crate) struct MaterialGc {
    pass: Option<Pass>,
    completed: Option<Key>,
}
#[cfg(test)]
impl MaterialGc {
    pub(crate) fn idle(&self) -> bool {
        self.pass.is_none()
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Key {
    checkpoint: execution_sqlite::MaterialCheckpoint,
    root: u64,
    registry: u64,
    associated: BTreeSet<RequestId>,
    pins: Vec<PathBuf>,
}
struct Pass {
    key: Key,
    inventory_generation: u64,
    physical: Reclamation,
    cursor: Option<RequestId>,
    scanned: bool,
    associated: BTreeSet<RequestId>,
    candidates: BTreeMap<PathBuf, Digest>,
    retained: BTreeSet<PathBuf>,
    interpreters: BTreeSet<String>,
    retire: BTreeMap<String, Digest>,
    live: Option<RetainedMaterials>,
    filter_cursor: Option<PathBuf>,
    filtered: bool,
    retry: bool,
}
impl<S: IdentityProvider> DeviceService<S> {
    /// One synchronous bounded step, called between owner command opportunities.
    pub(crate) fn reclaim_step(&mut self) -> Result<(), Error> {
        let mut associated = BTreeSet::new();
        for task in self
            .client
            .pending_tasks(self.client.configuration().limits.pending_tasks)?
        {
            if let Some(request) = self.client.bound_request(task)? {
                associated.insert(request);
            }
        }
        let key = Key {
            checkpoint: self
                .core
                .app
                .service_material_checkpoint()
                .map_err(crate::error::app_error)?,
            root: self.core.material_root.generation(),
            registry: self.core.host.materials.generation(),
            associated,
            pins: self
                .core
                .config
                .interpreters
                .iter()
                .map(|i| i.image.path.clone())
                .chain(
                    self.core
                        .config
                        .managers
                        .iter()
                        .map(|m| m.image.path.clone()),
                )
                .collect(),
        };
        if let Some(pass) = &mut self.material_gc.pass {
            if key.root != pass.key.root
                || key.registry != pass.key.registry
                || key.associated != pass.key.associated
                || key.pins != pass.key.pins
                || !self
                    .core
                    .app
                    .service_material_proof_current(pass.key.checkpoint)
                    .map_err(crate::error::app_error)?
            {
                // Publication appends immutable names. Reauthenticate deletion, but retain
                // existing inventory and identity-bound hashes (ref: tokio coop made_progress).
                pass.physical
                    .recheck_proof()
                    .map_err(crate::error::app_error)?;
                pass.associated = key.associated.clone();
                pass.retained = key.pins.iter().cloned().collect();
                pass.key = key.clone();
                pass.cursor = None;
                pass.scanned = false;
                pass.candidates.clear();
                pass.interpreters.clear();
                pass.retire.clear();
                pass.live = None;
                pass.filter_cursor = None;
                pass.filtered = false;
            }
        }
        if self.material_gc.pass.is_none() {
            if self.material_gc.completed.as_ref() == Some(&key) {
                return Ok(());
            }
            self.material_gc.pass = Some(Pass {
                inventory_generation: key.root,
                associated: key.associated.clone(),
                retained: key.pins.iter().cloned().collect(),
                key,
                physical: self.core.material_root.begin_reclamation(),
                cursor: None,
                scanned: false,
                candidates: BTreeMap::new(),
                interpreters: BTreeSet::new(),
                retire: BTreeMap::new(),
                live: None,
                filter_cursor: None,
                filtered: false,
                retry: false,
            });
        }
        let mut pass = self.material_gc.pass.take().unwrap();
        let outcome = self.advance_reclamation(&mut pass);
        match outcome {
            Ok(()) if pass.physical.phase() == ReclaimPhase::Complete => {
                let result = pass
                    .physical
                    .into_result()
                    .map_err(crate::error::app_error)?;
                if result.files != 0 || result.directories != 0 {
                    eprintln!(
                        "material_reclaimed: files={} bytes={} directories={}",
                        result.files, result.bytes, result.directories
                    );
                }
                // New names appended during this pass belong to the next inventory.
                pass.key.root = pass.inventory_generation;
                self.material_gc.completed = if pass.retry { None } else { Some(pass.key) };
                Ok(())
            }
            Ok(()) => {
                self.material_gc.pass = Some(pass);
                Ok(())
            }
            Err(error) => {
                self.material_gc.completed = None;
                Err(error)
            }
        }
    }
    fn advance_reclamation(&mut self, pass: &mut Pass) -> Result<(), Error> {
        if pass.physical.phase() != ReclaimPhase::AwaitingProof {
            // This short writer fence also prevents another SQLite connection from invalidating
            // already authenticated historical rows between the final check and unlink.
            let core = &mut self.core;
            if matches!(
                pass.physical.phase(),
                ReclaimPhase::Files | ReclaimPhase::Directories
            ) {
                return core
                    .app
                    .with_service_material_proof(pass.key.checkpoint, || {
                        core.material_root.reclaim_step(&mut pass.physical)
                    })
                    .map_err(crate::error::app_error);
            }
            return core
                .material_root
                .reclaim_step(&mut pass.physical)
                .map_err(crate::error::app_error);
        }
        if !pass.scanned {
            let consumer = crate::backend::plan::id("backend-results")?;
            let page = self
                .core
                .app
                .service_material_page(pass.cursor.as_ref(), 1, &consumer)
                .map_err(crate::error::app_error)?;
            for item in page.items {
                let input = item.input;
                let communicating = pass.associated.remove(&input.spec().request.request_id);
                let references = crate::recovery::references(&input, &self.core.config)?;
                if item.reclaimable && !communicating {
                    if self
                        .core
                        .host
                        .materials
                        .registered(&input)
                        .map_err(crate::error::app_error)?
                    {
                        pass.retire
                            .insert(input.digest().as_str().to_owned(), input.digest().clone());
                    }
                    for (path, digest) in references
                        .into_iter()
                        .filter(|(p, _)| pass.physical.contains_file(p))
                    {
                        if pass
                            .candidates
                            .insert(path, digest.clone())
                            .is_some_and(|d| d != digest)
                        {
                            return Err(Error::Untrusted);
                        }
                    }
                } else {
                    pass.retained.extend(references.into_keys());
                    pass.interpreters.insert(
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
                                pass.interpreters.insert(
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
                }
                if pass.retained.len() > 32768
                    || pass.candidates.len() > 32768
                    || pass.interpreters.len() > 32768
                {
                    return Err(Error::Capacity);
                }
            }
            match page.next {
                Some(next) => {
                    if pass.cursor.as_ref().is_some_and(|old| old >= &next) {
                        return Err(Error::Storage);
                    }
                    pass.cursor = Some(next);
                }
                None => {
                    if !pass.associated.is_empty() {
                        return Err(Error::Conflict);
                    }
                    pass.scanned = true;
                }
            }
            return Ok(());
        }
        // Retire only after every journal page and delivery/reference read has succeeded.
        if let Some((_, digest)) = pass.retire.pop_first() {
            match self.core.host.materials.retire(&digest) {
                Ok(()) => (),
                Err(execution_app::Error::Conflict) => pass.retry = true,
                Err(error) => return Err(crate::error::app_error(error)),
            }
            pass.key.registry = self.core.host.materials.generation();
            return Ok(());
        }
        if !pass.filtered {
            use std::ops::Bound::{Excluded, Unbounded};
            for _ in 0..64 {
                let next = pass
                    .candidates
                    .range::<PathBuf, _>((
                        pass.filter_cursor.as_ref().map_or(Unbounded, Excluded),
                        Unbounded,
                    ))
                    .next()
                    .map(|(p, d)| (p.clone(), d.clone()));
                let Some((path, digest)) = next else {
                    pass.filtered = true;
                    break;
                };
                pass.filter_cursor = Some(path.clone());
                if pass.interpreters.contains(digest.as_str()) {
                    pass.candidates.remove(&path);
                }
            }
            return Ok(());
        }
        if pass.live.is_none() {
            pass.live = Some(
                self.core
                    .host
                    .materials
                    .retained_materials()
                    .map_err(crate::error::app_error)?,
            );
        }
        let live = pass.live.as_mut().unwrap();
        if !live.complete() {
            pass.retained
                .extend(live.step().map_err(crate::error::app_error)?);
            if pass.retained.len() > 32768 {
                return Err(Error::Capacity);
            }
            return Ok(());
        }
        self.core
            .material_root
            .prove_reclamation(
                &mut pass.physical,
                std::mem::take(&mut pass.candidates),
                std::mem::take(&mut pass.retained),
            )
            .map_err(crate::error::app_error)
    }
    pub(crate) fn reclaim_owner_step(&mut self) {
        if let Err(error) = self.reclaim_step() {
            // Incomplete proof preserves materials; communication and physical quota remain active.
            eprintln!("material_reclaim_deferred: {error}");
        }
    }
}

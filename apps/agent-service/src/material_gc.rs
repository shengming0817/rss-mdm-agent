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
    initial: Key,
    physical: Reclamation,
    cursor: Option<RequestId>,
    scanned: bool,
    associated: BTreeSet<RequestId>,
    candidates: BTreeMap<PathBuf, Digest>,
    retained: BTreeSet<PathBuf>,
    interpreters: BTreeSet<String>,
    retire: BTreeMap<String, Digest>,
    live: Option<RetainedMaterials>,
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
                pass.associated
                    .extend(key.associated.difference(&pass.key.associated).cloned());
                pass.associated
                    .retain(|request| key.associated.contains(request));
                pass.retained.extend(key.pins.iter().cloned());
                if key.registry != pass.key.registry {
                    if let Some(live) = &mut pass.live {
                        self.core
                            .host
                            .materials
                            .refresh_retained(live)
                            .map_err(crate::error::app_error)?;
                    }
                }
                // The journal checkpoint advances only through authenticated receipt deltas.
                pass.key.root = key.root;
                pass.key.registry = key.registry;
                pass.key.associated = key.associated.clone();
                pass.key.pins = key.pins.clone();
            }
        }
        if self.material_gc.pass.is_none() {
            if self.material_gc.completed.as_ref() == Some(&key) {
                return Ok(());
            }
            self.material_gc.pass = Some(Pass {
                initial: key.clone(),
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
                self.material_gc.completed = if pass.retry { None } else { Some(pass.initial) };
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
            let consumer = crate::backend::plan::id(crate::service::RESULT_CONSUMER)?;
            let page = self
                .core
                .app
                .service_material_page(pass.cursor.as_ref(), 1, &consumer)
                .map_err(crate::error::app_error)?;
            for item in page.items {
                self.project_material_fact(pass, item)?;
            }
            match page.next {
                Some(next) => {
                    if pass.cursor.as_ref().is_some_and(|old| old >= &next) {
                        return Err(Error::Storage);
                    }
                    pass.cursor = Some(next);
                }
                None => {
                    pass.scanned = true;
                }
            }
            return Ok(());
        }
        let consumer = crate::backend::plan::id(crate::service::RESULT_CONSUMER)?;
        // A newly associated request can precede the historical request cursor.
        if let Some(request) = pass.associated.pop_first() {
            let fact = self
                .core
                .app
                .service_material_fact(&request, &consumer)
                .map_err(crate::error::app_error)?;
            self.project_material_fact(pass, fact)?;
            return Ok(());
        }
        let changes = self
            .core
            .app
            .service_material_changes(pass.key.checkpoint, 1, &consumer)
            .map_err(crate::error::app_error)?;
        let changed = !changes.items.is_empty();
        for fact in changes.items {
            self.project_material_fact(pass, fact)?;
        }
        pass.key.checkpoint = changes.checkpoint;
        if !changes.complete || changed {
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
            .prove_reclamation(&mut pass.physical, &pass.candidates, &pass.retained)
            .map_err(crate::error::app_error)
    }
    fn project_material_fact(
        &mut self,
        pass: &mut Pass,
        fact: execution_app::ServiceMaterialFact,
    ) -> Result<(), Error> {
        let input = fact.input;
        if let Err(error) = self.core.remember_data(&input) {
            eprintln!("agent_collected_data: {error}");
        }
        let request = &input.spec().request.request_id;
        pass.associated.remove(request);
        let communicating = pass.key.associated.contains(request);
        let references = crate::recovery::references(&input, &self.core.config)?;
        if fact.reclaimable && !communicating {
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
                if pass.interpreters.contains(digest.as_str()) {
                    pass.retained.insert(path.clone());
                }
                if pass
                    .candidates
                    .insert(path, digest.clone())
                    .is_some_and(|old| old != digest)
                {
                    return Err(Error::Untrusted);
                }
            }
        } else {
            // Within one pass references only strengthen. A later pass may release them.
            pass.retire.remove(input.digest().as_str());
            pass.retained.extend(references.into_keys());
            let mut new_interpreters = BTreeSet::new();
            new_interpreters.insert(
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
                        new_interpreters.insert(
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
            new_interpreters.retain(|digest| !pass.interpreters.contains(digest));
            if !new_interpreters.is_empty() {
                // This is the bounded physical projection, never a full-history rescan.
                pass.retained.extend(
                    pass.candidates
                        .iter()
                        .filter(|(_, digest)| new_interpreters.contains(digest.as_str()))
                        .map(|(path, _)| path.clone()),
                );
                pass.interpreters.extend(new_interpreters);
            }
        }
        if pass.retained.len() > 32768
            || pass.candidates.len() > 32768
            || pass.interpreters.len() > 32768
        {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    pub(crate) fn reclaim_owner_step(&mut self) {
        if let Err(error) = self.reclaim_step() {
            // Incomplete proof preserves materials; communication and physical quota remain active.
            eprintln!("material_reclaim_deferred: {error}");
        }
    }
}

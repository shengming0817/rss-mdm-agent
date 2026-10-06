//! Reclamation reads and short deletion fences owned by the authoritative journal.
use crate::{database::bounded_blob, execution::load_state, journal::*, *};
use execution_contract::{FrozenExecution, Id, RequestId};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::sync::atomic::{AtomicU64, Ordering};

/// Connection-local checkpoint. It is neither persisted nor transferable between stores.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MaterialCheckpoint {
    owner: u64,
    safety: u64,
    external: u64,
    changes: u64,
    receipt: u64,
}
pub(crate) fn owner_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
impl Store {
    pub(crate) fn sql_material_checkpoint(&self) -> Result<MaterialCheckpoint, Error> {
        crate::database::ensure_current(&self.conn, &self.authority, self.limits)?;
        Ok(MaterialCheckpoint {
            owner: self.material_owner,
            safety: self.material_safety.get(),
            external: self
                .conn
                .pragma_query_value(None, "data_version", |r| r.get(0))?,
            changes: self.conn.total_changes(),
            receipt: self.conn.query_row(
                "SELECT COALESCE(MAX(sequence),0) FROM receipts",
                [],
                |r| r.get(0),
            )?,
        })
    }
    pub(crate) fn sql_material_proof_current(
        &self,
        expected: MaterialCheckpoint,
    ) -> Result<bool, Error> {
        let current = self.sql_material_checkpoint()?;
        Ok(current.owner == expected.owner
            && current.safety == expected.safety
            && current.external == expected.external)
    }
    pub(crate) fn sql_with_material_proof<T>(
        &self,
        expected: MaterialCheckpoint,
        action: impl FnOnce() -> Result<T, execution_app::Error>,
    ) -> Result<T, execution_app::Error> {
        // ref: rusqlite src/transaction.rs@499cc7bb986e04cc66e6ed762522f7ea449178d1.
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(|e| execution_app::Error::from(Error::from(e)))?;
        if !self.sql_material_proof_current(expected)? {
            return Err(execution_app::Error::Conflict);
        }
        let result = action();
        // No DB mutation; rollback/drop releases the writer fence, including on callback failure.
        drop(tx);
        result
    }
    pub(crate) fn sql_material_page(
        &self,
        after: Option<&RequestId>,
        limit: usize,
        consumer: &Id,
        host: &impl JournalHost,
    ) -> Result<Vec<ServiceMaterialFact>, Error> {
        if !(1..=128).contains(&limit) {
            return Err(Error::InvalidInput);
        }
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        let mut statement = tx.prepare(&format!(
            "SELECT request_id,scope,{},digest FROM executions WHERE request_id>?1 ORDER BY request_id LIMIT ?2",
            bounded_blob("plan", self.limits.input.max_input_bytes)))?;
        let rows = statement.query_map(
            params![after.map_or("", RequestId::as_str), limit as i64],
            material_row,
        )?;
        let mut facts = Vec::new();
        for row in rows {
            facts.push(self.material_fact_row(&tx, row?, consumer, host)?);
        }
        Ok(facts)
    }
    pub(crate) fn sql_material_fact(
        &self,
        request: &RequestId,
        consumer: &Id,
        host: &impl JournalHost,
    ) -> Result<ServiceMaterialFact, Error> {
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        let row = tx.query_row(
            &format!(
                "SELECT request_id,scope,{},digest FROM executions WHERE request_id=?1",
                bounded_blob("plan", self.limits.input.max_input_bytes)
            ),
            [request.as_str()],
            material_row,
        )?;
        self.material_fact_row(&tx, row, consumer, host)
    }
    pub(crate) fn sql_material_changes(
        &self,
        checkpoint: MaterialCheckpoint,
        limit: usize,
        consumer: &Id,
        host: &impl JournalHost,
    ) -> Result<ServiceMaterialChanges<MaterialCheckpoint>, Error> {
        if !(1..=128).contains(&limit) {
            return Err(Error::InvalidInput);
        }
        let tx = self.conn.unchecked_transaction()?;
        let current = self.sql_material_checkpoint()?;
        if current.owner != checkpoint.owner
            || current.external != checkpoint.external
            || current.receipt < checkpoint.receipt
        {
            return Err(Error::Conflict);
        }
        // Receipts commit atomically with every new/widened execution. Read by the indexed
        // immutable sequence, including non-material receipts so skipping is also bounded.
        let mut statement = tx.prepare("SELECT sequence,scope,kind FROM receipts WHERE sequence>?1 AND sequence<=?2 ORDER BY sequence LIMIT ?3")?;
        let rows = statement.query_map(
            params![checkpoint.receipt, current.receipt, limit as i64],
            |r| {
                Ok((
                    r.get::<_, u64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )?;
        let mut next = checkpoint;
        let mut items = Vec::new();
        for row in rows {
            let (sequence, scope, kind) = row?;
            next.receipt = sequence;
            if kind == "trust" {
                continue;
            }
            let row = tx
                .query_row(
                    &format!(
                        "SELECT request_id,scope,{},digest FROM executions WHERE scope=?1",
                        bounded_blob("plan", self.limits.input.max_input_bytes)
                    ),
                    [scope],
                    material_row,
                )
                .optional()?;
            if let Some(row) = row {
                items.push(self.material_fact_row(&tx, row, consumer, host)?);
            }
        }
        let complete = next.receipt == current.receipt;
        Ok(ServiceMaterialChanges {
            items,
            checkpoint: if complete { current } else { next },
            complete,
        })
    }
    fn material_fact_row(
        &self,
        conn: &rusqlite::Connection,
        row: (String, String, Vec<u8>, String),
        consumer: &Id,
        host: &impl JournalHost,
    ) -> Result<ServiceMaterialFact, Error> {
        let (request, key, bytes, digest) = row;
        let spec = execution_contract::decode_execution(&bytes, &self.limits.input)
            .map_err(|_| Error::Corrupt)?;
        let input =
            FrozenExecution::freeze(spec, &self.limits.input).map_err(|_| Error::Corrupt)?;
        let scope = Scope::from_input(&input);
        self.check_scope(&scope)?;
        if input.spec().request.request_id.as_str() != request
            || scope.interaction_subject().as_str() != key
            || hash(input.digest())? != digest
        {
            return Err(Error::Corrupt);
        }
        authorize(host, Access::RunnerFact, &scope, None)?;
        authorize(host, Access::Deliver, &scope, Some(consumer))?;
        let execution = load_state(conn, &scope, input.clone(), self.limits)?.0;
        let admission = latest_admission(conn, &scope, self.limits)?;
        // Preserve the existing evidence integrity checks, without another plan restoration.
        if let Some(attempt) = &execution.snapshot().attempt {
            let process = crate::process::capture(conn, &attempt.id, self.limits)?;
            if process.is_some_and(|p| p.content_digest != *input.digest()) {
                return Err(Error::Corrupt);
            }
            let progress = crate::software_progress::read(conn, &attempt.id, self.limits)?;
            if progress.is_some_and(|p| !p.valid_for(&input)) {
                return Err(Error::Corrupt);
            }
        }
        let pending: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM receipts r WHERE r.scope=?1 AND r.kind!='trust' AND NOT EXISTS(SELECT 1 FROM confirmations c WHERE c.scope=r.scope AND c.consumer=?2 AND c.sequence=r.sequence))",
                params![key, consumer.as_str()], |r| r.get(0))?;
        let reclaimable = material_settled(&execution, admission) && !pending;
        Ok(ServiceMaterialFact { input, reclaimable })
    }
}
fn material_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, Vec<u8>, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}
pub(crate) fn latest_admission(
    conn: &rusqlite::Connection,
    scope: &Scope,
    limits: Limits,
) -> Result<Option<AdmissionStatus>, Error> {
    let receipt: Option<Vec<u8>> = conn.query_row(
        &format!("SELECT {} FROM receipts WHERE scope=?1 AND kind='admission' ORDER BY sequence DESC LIMIT 1", bounded_blob("body", limits.max_record_bytes)),
        [scope.interaction_subject().as_str()], |r| r.get(0)).optional()?;
    receipt
        .map(|b| {
            let receipt: Receipt = decode(&b, limits.max_record_bytes)?;
            if receipt.scope != *scope {
                return Err(Error::Corrupt);
            }
            Ok(receipt.admission)
        })
        .transpose()
        .map(Option::flatten)
}
fn material_settled(
    execution: &execution_lifecycle::Execution,
    admission: Option<AdmissionStatus>,
) -> bool {
    use execution_lifecycle::{EffectAssessment, Observation, Phase};
    let snapshot = execution.snapshot();
    let assessment = snapshot
        .attempt
        .as_ref()
        .and_then(|a| a.assessment.as_ref())
        .and_then(|a| match a.observation {
            Observation::Effect { assessment } => Some(assessment),
            _ => None,
        });
    (snapshot.cancel_requested
        && (snapshot.attempt.is_none() || assessment == Some(EffectAssessment::NoEffect)))
        || matches!(
            execution.phase(),
            Phase::Cancelled | Phase::FailedBeforeDispatch
        )
        || (execution.phase() == Phase::Verified
            && matches!(
                assessment,
                Some(EffectAssessment::Satisfied | EffectAssessment::NoEffect)
            ))
        || (snapshot.attempt.is_none() && admission == Some(AdmissionStatus::Denied))
}

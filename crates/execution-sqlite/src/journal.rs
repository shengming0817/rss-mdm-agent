use crate::*;
use execution_contract::{AttemptId, EventId, Id};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest as _, Sha256};

pub(crate) fn encode(value: &impl Serialize, max: usize) -> Result<Vec<u8>, Error> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Corrupt)?;
    if bytes.len() > max {
        return Err(Error::Capacity);
    }
    Ok(bytes)
}
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8], max: usize) -> Result<T, Error> {
    if bytes.len() > max {
        return Err(Error::Corrupt);
    }
    serde_json::from_slice(bytes).map_err(|_| Error::Corrupt)
}
pub(crate) fn hash(value: &impl Serialize) -> Result<String, Error> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Corrupt)?;
    let digest = Sha256::digest([b"execution-sqlite/v1\0".as_slice(), &bytes].concat());
    Ok(format!("{digest:x}"))
}
pub(crate) fn integer(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::Capacity)
}
pub(crate) fn authorize(
    host: &impl Host,
    access: Access,
    scope: &Scope,
    consumer: Option<&Id>,
) -> Result<(), Error> {
    host.authorize(AccessRequest {
        access,
        scope,
        consumer,
        interaction: None,
    })
}
pub(crate) enum Start<'a> {
    Replay(Receipt),
    New(Write<'a>),
}
pub(crate) struct Write<'a> {
    pub tx: Transaction<'a>,
    pub scope: Scope,
    pub op: OperationRequestId,
    pub kind: OperationKind,
    fingerprint: String,
    pub now: u64,
    pub limits: Limits,
}
impl Store {
    pub(crate) fn check_scope(&self, scope: &Scope) -> Result<(), Error> {
        if scope.authority != self.authority {
            return Err(Error::Denied);
        }
        Ok(())
    }
    pub(crate) fn read(
        &self,
        scope: &Scope,
        access: Access,
        consumer: Option<&Id>,
        host: &impl Host,
    ) -> Result<Transaction<'_>, Error> {
        self.check_scope(scope)?;
        authorize(host, access, scope, consumer)?;
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        Ok(tx)
    }
    pub(crate) fn start<'a>(
        &'a mut self,
        op: &OperationRequestId,
        scope: &Scope,
        kind: OperationKind,
        content: &impl Serialize,
        access: Access,
        host: &impl Host,
    ) -> Result<Start<'a>, Error> {
        self.check_scope(scope)?;
        authorize(host, Access::ReadResult, scope, None)?;
        // Hash normalized intent, not timestamps, verification responses or mutable state.
        let fingerprint = hash(&(scope, kind, content))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        if let Some((old_scope, old_hash, bytes)) = tx
            .query_row(
                "SELECT scope,fingerprint,body FROM receipts WHERE operation_id=?1",
                [op.as_str()],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()?
        {
            if old_scope != scope.key() || old_hash != fingerprint {
                return Err(Error::Conflict);
            }
            return Ok(Start::Replay(decode(&bytes, self.limits.max_record_bytes)?));
        }
        authorize(host, access, scope, None)?;
        let now = host.reliable_now()?;
        let watermark: u64 = tx.query_row(
            "SELECT clock_watermark FROM metadata WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        if now < watermark {
            return Err(Error::Clock);
        }
        integer(now)?;
        Ok(Start::New(Write {
            tx,
            scope: scope.clone(),
            op: op.clone(),
            kind,
            fingerprint,
            now,
            limits: self.limits,
        }))
    }
    /// Read the original receipt under current result-read authorization, without reauthorizing execution.
    pub fn receipt(
        &self,
        scope: &Scope,
        operation: &OperationRequestId,
        host: &impl Host,
    ) -> Result<Option<Receipt>, Error> {
        let tx = self.read(scope, Access::ReadResult, None, host)?;
        let bytes: Option<Vec<u8>> = tx
            .query_row(
                "SELECT body FROM receipts WHERE operation_id=?1 AND scope=?2",
                params![operation.as_str(), scope.key()],
                |r| r.get(0),
            )
            .optional()?;
        bytes
            .map(|b| decode(&b, self.limits.max_record_bytes))
            .transpose()
    }
    /// Pull immutable, unconfirmed results for one authorized scope/consumer. Zero starts at the beginning.
    /// A lost response is recovered by repeating the same cursor; query receipts independently as needed.
    pub fn pull_results(
        &self,
        scope: &Scope,
        consumer: &Id,
        after: u64,
        limit: usize,
        host: &impl Host,
    ) -> Result<Vec<Receipt>, Error> {
        let tx = self.read(scope, Access::Deliver, Some(consumer), host)?;
        if limit == 0 || limit > self.limits.max_batch {
            return Err(Error::Configuration);
        }
        let mut statement = tx.prepare(
            "SELECT r.body FROM receipts r WHERE r.scope=?1 AND r.sequence>?2 AND r.kind!='trust'
             AND NOT EXISTS(SELECT 1 FROM confirmations c WHERE c.scope=r.scope AND c.consumer=?3 AND c.sequence=r.sequence)
             ORDER BY r.sequence LIMIT ?4")?;
        let rows = statement.query_map(
            params![
                scope.key(),
                integer(after)?,
                consumer.as_str(),
                limit as i64
            ],
            |r| r.get::<_, Vec<u8>>(0),
        )?;
        rows.map(|r| decode(&r?, self.limits.max_record_bytes))
            .collect()
    }
    /// Idempotently confirm exactly one delivered event. Confirmation creates no delivery event.
    /// Consumers persist their own effect/result before acknowledging; delivery is at least once.
    pub fn confirm(
        &mut self,
        scope: &Scope,
        consumer: &Id,
        event: &EventId,
        host: &impl Host,
    ) -> Result<(), Error> {
        self.check_scope(scope)?;
        authorize(host, Access::Deliver, scope, Some(consumer))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        let sequence: i64 = tx.query_row(
            "SELECT sequence FROM receipts WHERE scope=?1 AND event_id=?2 AND kind!='trust'",
            params![scope.key(), event.as_str()],
            |r| r.get(0),
        )?;
        let known: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM confirmations WHERE scope=?1 AND consumer=?2)",
            params![scope.key(), consumer.as_str()],
            |r| r.get(0),
        )?;
        let consumers: u32 = tx.query_row(
            "SELECT count(DISTINCT consumer) FROM confirmations WHERE scope=?1",
            [scope.key()],
            |r| r.get(0),
        )?;
        if !known && consumers >= self.limits.max_consumers {
            return Err(Error::Capacity);
        }
        tx.execute("INSERT INTO confirmations(scope,consumer,sequence) VALUES(?1,?2,?3) ON CONFLICT(scope,consumer,sequence) DO NOTHING",
            params![scope.key(), consumer.as_str(), sequence])?;
        tx.commit().map_err(|_| Error::CommitUnknown)
    }
    /// Read full audit using a separate current authorization; ordinary result access is insufficient.
    pub fn audit(
        &self,
        scope: &Scope,
        operation: &OperationRequestId,
        host: &impl Host,
    ) -> Result<AuditRecord, Error> {
        let tx = self.read(scope, Access::ReadAudit, None, host)?;
        let bytes: Vec<u8> = tx.query_row(
            "SELECT a.body FROM audits a JOIN receipts r ON r.sequence=a.sequence WHERE r.scope=?1 AND r.operation_id=?2",
            params![scope.key(), operation.as_str()], |r| r.get(0))?;
        decode(&bytes, self.limits.max_record_bytes)
    }
}
impl Write<'_> {
    pub fn refresh_time(&mut self, host: &impl Host) -> Result<(), Error> {
        let now = host.reliable_now()?;
        if now < self.now {
            return Err(Error::Clock);
        }
        integer(now)?;
        self.now = now;
        Ok(())
    }
    pub fn bounded(&self, value: &impl Serialize) -> Result<Vec<u8>, Error> {
        encode(value, self.limits.max_record_bytes)
    }
    pub fn ensure_capacity(&self) -> Result<(), Error> {
        let used: u64 = self
            .tx
            .query_row("SELECT count(*) FROM receipts", [], |r| r.get(0))?;
        let reserved: u64 = self.tx.query_row(
            "SELECT COALESCE((SELECT sum((reserve&1)+((reserve>>1)&1)+((reserve>>2)&1)+((reserve>>3)&1)) FROM executions),0)
             + COALESCE((SELECT sum(reserve) FROM interactions),0)", [], |r| r.get(0))?;
        if used
            .checked_add(reserved)
            .and_then(|n| n.checked_add(1))
            .is_none_or(|n| n > self.limits.max_receipts)
        {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    pub fn prepare(
        &self,
        outcome: Outcome,
        revision: u64,
        attempt: Option<AttemptId>,
        mut audit: AuditRecord,
    ) -> Result<Receipt, Error> {
        self.ensure_capacity()?;
        let event_id = EventId::new(hash(&(self.op.as_str(), &self.fingerprint))?)
            .map_err(|_| Error::Corrupt)?;
        self.tx.execute("INSERT INTO receipts(operation_id,event_id,scope,fingerprint,kind,body) VALUES(?1,?2,?3,?4,?5,X'')",
            params![self.op.as_str(), event_id.as_str(), self.scope.key(), self.fingerprint, match self.kind { OperationKind::Trust => "trust", _ => "result" }])?;
        let sequence = u64::try_from(self.tx.last_insert_rowid()).map_err(|_| Error::Capacity)?;
        let receipt = Receipt {
            sequence,
            event_id: event_id.clone(),
            operation_id: self.op.clone(),
            scope: self.scope.clone(),
            kind: self.kind,
            outcome,
            revision,
            attempt_id: attempt,
            occurred_at_unix_ms: self.now,
        };
        if let Some(event) = &mut audit.event {
            event.event_id = event_id;
            event
                .validate(&self.limits.plan)
                .map_err(|_| Error::Capacity)?;
        }
        self.tx.execute(
            "UPDATE receipts SET body=?1 WHERE sequence=?2",
            params![self.bounded(&receipt)?, integer(sequence)?],
        )?;
        self.tx.execute(
            "INSERT INTO audits VALUES(?1,?2)",
            params![integer(sequence)?, self.bounded(&audit)?],
        )?;
        self.tx.execute(
            "UPDATE metadata SET clock_watermark=?1 WHERE singleton=1",
            [integer(self.now)?],
        )?;
        Ok(receipt)
    }
    pub fn finish(
        self,
        outcome: Outcome,
        revision: u64,
        attempt: Option<AttemptId>,
        audit: AuditRecord,
    ) -> Result<CommitOutcome, Error> {
        let receipt = self.prepare(outcome, revision, attempt, audit)?;
        self.commit()?;
        Ok(CommitOutcome::Applied {
            receipt,
            first_dispatch: None,
        })
    }
    pub fn commit(self) -> Result<(), Error> {
        self.tx.commit().map_err(|_| Error::CommitUnknown)
    }
}
pub(crate) fn empty_audit(reason: &str) -> AuditRecord {
    AuditRecord {
        event: None,
        attempt_id: None,
        rule_ids: Vec::new(),
        reason: reason.into(),
        authorization_revision: None,
        consumptions: Vec::new(),
    }
}
pub(crate) fn load_plan(
    conn: &Connection,
    scope: &Scope,
    limits: Limits,
) -> Result<execution_contract::FrozenPlan, Error> {
    let (bytes, digest): (Vec<u8>, String) = conn.query_row(
        "SELECT plan,digest FROM executions WHERE scope=?1",
        [scope.key()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let spec = execution_contract::decode_plan(&bytes, &limits.plan).map_err(|_| Error::Corrupt)?;
    let plan =
        execution_contract::FrozenPlan::freeze(spec, &limits.plan).map_err(|_| Error::Corrupt)?;
    if hash(plan.digest())? != digest || Scope::from_plan(&plan) != *scope {
        return Err(Error::Corrupt);
    }
    Ok(plan)
}

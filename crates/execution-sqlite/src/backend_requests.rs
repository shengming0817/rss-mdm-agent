use crate::{
    database::bounded_blob,
    journal::{authorize, decode, encode, integer},
    *,
};
use execution_contract::BackendRequest;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
impl Store {
    /// List bounded preparation records without creating another task queue.
    pub(crate) fn sql_backend_requests(
        &self,
        authority: &execution_contract::Authority,
        actor: &execution_contract::ActorId,
        host: &impl JournalHost,
    ) -> Result<Vec<BackendRequest>, Error> {
        crate::database::ensure_current(&self.conn, &self.authority, self.limits)?;
        let mut query = self.conn.prepare(
            "SELECT request_id FROM backend_requests WHERE actor=?1 ORDER BY rowid DESC LIMIT ?2",
        )?;
        let ids = query
            .query_map(params![actor.as_str(), self.limits.max_batch], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                let scope = Scope {
                    authority: authority.clone(),
                    actor: actor.clone(),
                    request_id: execution_contract::RequestId::new(id)
                        .map_err(|_| Error::Corrupt)?,
                };
                self.sql_backend_request(&scope, host)?
                    .ok_or(Error::Corrupt)
            })
            .collect()
    }
    /// Read a pre-execution intent from the same protected journal and actor namespace.
    pub(crate) fn sql_backend_request(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Option<BackendRequest>, Error> {
        let tx = self.read(scope, Access::ReadResult, None, host)?;
        read(&tx, scope.interaction_subject().as_str(), self.limits)
    }
    /// Compare-and-append a preparation transition; it creates no execution attempt or grant.
    pub(crate) fn sql_record_backend_request(
        &mut self,
        scope: &Scope,
        expected: Option<&BackendRequest>,
        next: &BackendRequest,
        host: &impl JournalHost,
    ) -> Result<(), Error> {
        self.check_scope(scope)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        authorize(host, Access::Execute, scope, None)?;
        reserve_request(&tx, scope)?;
        use execution_contract::BackendRequestState as S;
        let transition = match expected.map(|p| p.state) {
            None => matches!(
                next.state,
                S::Draft | S::AwaitingConfirmation | S::Ready | S::Failed
            ),
            Some(S::Draft) => matches!(next.state, S::Ready | S::Cancelled | S::Failed),
            Some(S::AwaitingConfirmation) => {
                matches!(next.state, S::Ready | S::Cancelled | S::Failed)
            }
            Some(S::Ready) => matches!(next.state, S::Submitting | S::Cancelled | S::Failed),
            Some(S::Submitting) => matches!(
                next.state,
                S::Submitting | S::Observed | S::Cancelled | S::Failed
            ),
            Some(S::Observed | S::Cancelled | S::Failed) => false,
        };
        if (next.state == S::Observed)
            != matches!(
                &next.action,
                execution_contract::PreparedAction::SoftwareRead {
                    observation: Some(_),
                    ..
                }
            )
        {
            return Err(Error::InvalidInput);
        }
        if !transition
            || !next.valid_product_gate()
            || next
                .action
                .offer()
                .is_some_and(|offer| !offer.user_initiated)
            || matches!(
                next.trigger,
                execution_contract::BackendTrigger::Automatic {}
            )
            || (next.state == S::Failed) != next.failure.is_some() && next.state != S::Cancelled
        {
            return Err(Error::InvalidInput);
        }
        if next.action.request() != &scope.request_id
            || next.revision != expected.map_or(1, |p| p.revision.saturating_add(1))
        {
            return Err(Error::InvalidInput);
        }
        let key = scope.interaction_subject();
        let previous = read(&tx, key.as_str(), self.limits)?;
        if previous.as_ref() != expected {
            return Err(Error::Conflict);
        }
        if expected.is_some_and(|p| {
            p.recorded_at_unix_ms != next.recorded_at_unix_ms
                || p.retry_of != next.retry_of
                || p.retry_expected_revision != next.retry_expected_revision
        }) {
            return Err(Error::Conflict);
        }
        if next.retry_of.is_some() != next.retry_expected_revision.is_some()
            || next.cancellation_control.is_some() != next.cancellation_expected_revision.is_some()
        {
            return Err(Error::InvalidInput);
        }
        if next.cancellation_control.is_some() && (next.state != S::Cancelled || expected.is_none())
        {
            return Err(Error::InvalidInput);
        }
        if expected.is_some_and(|p| {
            p.cancellation_control.is_some()
                && (p.cancellation_control != next.cancellation_control
                    || p.cancellation_expected_revision != next.cancellation_expected_revision)
        }) {
            return Err(Error::Conflict);
        }
        if expected.is_some_and(|p| {
            !(p.action.same_choice(&next.action) || p.prepares_call(next))
                || p.trigger != next.trigger
                || (p.risk != next.risk && !p.prepares_call(next))
                || (p.confirmation != next.confirmation
                    && !(matches!(p.state, S::Draft | S::AwaitingConfirmation)
                        && next.state == S::Ready
                        && p.confirmation.is_none()
                        && next.confirmation.is_some()))
        }) {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO backend_requests VALUES(?1,?2,?3,?4) ON CONFLICT(scope) DO UPDATE SET body=excluded.body", params![key.as_str(), scope.actor.as_str(), scope.request_id.as_str(), encode(next, self.limits.max_record_bytes)?])?;
        tx.commit().map_err(|_| Error::OperationCommitUnknown)
    }
}
fn read(
    conn: &rusqlite::Connection,
    key: &str,
    limits: Limits,
) -> Result<Option<BackendRequest>, Error> {
    let body: Option<Vec<u8>> = conn
        .query_row(
            &format!(
                "SELECT {} FROM backend_requests WHERE scope=?1",
                bounded_blob("body", limits.max_record_bytes)
            ),
            [key],
            |r| r.get(0),
        )
        .optional()?;
    body.map(|bytes| decode(&bytes, limits.max_record_bytes))
        .transpose()
}

/// One physical journal reserves each UUID across preparation and execution atomically.
pub(crate) fn reserve_request(conn: &rusqlite::Connection, scope: &Scope) -> Result<(), Error> {
    let collision: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM backend_requests WHERE request_id=?1 AND scope<>?2 UNION ALL SELECT 1 FROM executions WHERE request_id=?1 AND scope<>?2)",
        params![scope.request_id.as_str(), scope.interaction_subject().as_str()],
        |row| row.get(0),
    )?;
    if collision {
        return Err(Error::Conflict);
    }
    Ok(())
}

impl Store {
    pub(crate) fn sql_task_history_page(
        &self,
        authority: &execution_contract::Authority,
        actor: &execution_contract::ActorId,
        query: &TaskHistoryQuery,
        host: &impl JournalHost,
    ) -> Result<Vec<TaskHistoryPosition>, Error> {
        if !(1..=128).contains(&query.limit) {
            return Err(Error::InvalidInput);
        }
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        // ref: sqlite.org/rowvalue.html#scrolling_window_queries. Both coordinates are
        // immutable. UNION reads original tables; no materialized history or migration.
        let sql = format!(
            "WITH history AS (
                SELECT request_id, scope, json_extract(CAST({} AS TEXT),'$.recordedAtUnixMs') AS at, 0 AS lane
                FROM backend_requests WHERE actor=?1 AND rowid<=?2
                UNION ALL
                SELECT e.request_id,e.scope,json_extract(CAST({} AS TEXT),'$.occurredAtUnixMs') AS at,1 AS lane
                FROM receipts r JOIN executions e ON e.scope=r.scope
                WHERE r.sequence<=?3
                  AND r.sequence=(SELECT MIN(first.sequence) FROM receipts first WHERE first.scope=r.scope)
                  AND NOT EXISTS(SELECT 1 FROM backend_requests b WHERE b.scope=e.scope AND b.actor=?1)
            ) SELECT request_id,scope,at,lane FROM history
              WHERE at<=?4 AND (?5 IS NULL OR (at,request_id)<(?5,?6))
              ORDER BY at DESC,request_id DESC LIMIT ?7",
            bounded_blob("body", self.limits.max_record_bytes),
            bounded_blob("r.body", self.limits.max_record_bytes));
        let mut statement = tx.prepare(&sql)?;
        let rows = statement.query_map(
            params![
                actor.as_str(),
                integer(query.preparations)?,
                integer(query.receipts)?,
                integer(query.as_of)?,
                query
                    .after
                    .as_ref()
                    .map(|p| integer(p.recorded_at_unix_ms))
                    .transpose()?,
                query.after.as_ref().map(|p| p.request.as_str()),
                query.limit as i64,
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u64>(2)?,
                    r.get::<_, u8>(3)?,
                ))
            },
        )?;
        let mut result = Vec::new();
        for row in rows {
            let (request, key, at, lane) = row?;
            let request =
                execution_contract::RequestId::new(request).map_err(|_| Error::Corrupt)?;
            if lane == 0 {
                let scope = Scope {
                    authority: authority.clone(),
                    actor: actor.clone(),
                    request_id: request.clone(),
                };
                self.check_scope(&scope)?;
                if scope.interaction_subject().as_str() != key {
                    return Err(Error::Corrupt);
                }
                authorize(host, Access::ReadResult, &scope, None)?;
                let record = read(&tx, &key, self.limits)?.ok_or(Error::Corrupt)?;
                if record.action.request() != &request || record.recorded_at_unix_ms != at {
                    return Err(Error::Corrupt);
                }
            } else {
                let (plan_bytes,receipt_bytes):(Vec<u8>,Vec<u8>)=tx.query_row(&format!(
                    "SELECT {},{} FROM executions e JOIN receipts r ON r.scope=e.scope WHERE e.scope=?1 ORDER BY r.sequence LIMIT 1",
                    bounded_blob("e.plan",self.limits.input.max_input_bytes),bounded_blob("r.body",self.limits.max_record_bytes)),[&key],|r|Ok((r.get(0)?,r.get(1)?)))?;
                let spec = execution_contract::decode_execution(&plan_bytes, &self.limits.input)
                    .map_err(|_| Error::Corrupt)?;
                let input = execution_contract::FrozenExecution::freeze(spec, &self.limits.input)
                    .map_err(|_| Error::Corrupt)?;
                let scope = Scope::from_input(&input);
                let receipt: Receipt = decode(&receipt_bytes, self.limits.max_record_bytes)?;
                self.check_scope(&scope)?;
                if scope.request_id != request
                    || scope.interaction_subject().as_str() != key
                    || receipt.scope != scope
                    || receipt.occurred_at_unix_ms != at
                {
                    return Err(Error::Corrupt);
                }
                authorize(host, Access::RunnerFact, &scope, None)?;
            }
            result.push(TaskHistoryPosition {
                request,
                recorded_at_unix_ms: at,
            });
        }
        Ok(result)
    }
}

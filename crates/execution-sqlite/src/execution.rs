use crate::database::bounded_blob;
use crate::{journal::*, *};
use execution_contract::{AuditEvent, Decision, EventId, EvidenceRefs, FrozenExecution, Id, V1};
use execution_lifecycle::{
    self as lifecycle, Command, CommandEvent, EventRecord, Execution, ObservationEvent,
    ObservationVerifier,
};
use rusqlite::{params, Connection, OptionalExtension};

impl Store {
    /// Restore by stable business request identity after reconnect or restart. The stored scope
    /// is authenticated before any plan or state is returned. No dispatch action is recoverable.
    pub(crate) fn sql_execution_by_request(
        &self,
        request: &execution_contract::RequestId,
        access: ExecutionAccess<'_>,
        host: &impl JournalHost,
    ) -> Result<ExecutionRecord, Error> {
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        let (key, bytes, digest): (String, Vec<u8>, String) = tx.query_row(
            &format!(
                "SELECT scope,{},digest FROM executions WHERE request_id=?1",
                bounded_blob("plan", self.limits.input.max_input_bytes)
            ),
            [request.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let spec = execution_contract::decode_execution(&bytes, &self.limits.input)
            .map_err(|_| Error::Corrupt)?;
        let plan = FrozenExecution::freeze(spec, &self.limits.input).map_err(|_| Error::Corrupt)?;
        let scope = Scope::from_input(&plan);
        self.check_scope(&scope)?;
        access.authorize(&scope, host)?;
        if &plan.spec().request.request_id != request
            || scope.interaction_subject().as_str() != key
            || hash(plan.digest())? != digest
        {
            return Err(Error::Corrupt);
        }
        let admission = crate::material::latest_admission(&tx, &scope, self.limits)?;
        let execution = load_state(&tx, &scope, plan.clone(), self.limits)?.0;
        let process = if matches!(
            access,
            ExecutionAccess::Result | ExecutionAccess::Audit | ExecutionAccess::RunnerFact
        ) {
            execution
                .snapshot()
                .attempt
                .as_ref()
                .map(|a| crate::process::summary(&tx, &a.id, self.limits))
                .transpose()?
                .flatten()
        } else {
            None
        };
        let software = if matches!(
            access,
            ExecutionAccess::Result | ExecutionAccess::Audit | ExecutionAccess::RunnerFact
        ) {
            crate::software::diagnostic(
                &tx,
                &plan,
                execution.snapshot().attempt.as_ref().map(|a| &a.id),
                self.limits,
            )?
        } else {
            None
        };
        Ok(ExecutionRecord {
            software,
            execution,
            admission,
            process,
        })
    }

    /// Retrieve an execution command's safe receipt under current Execute permission, independent
    /// of general result reading. Does not authorize a new attempt or re-run admission.
    pub(crate) fn sql_execution_receipt(
        &self,
        scope: &Scope,
        op: &OperationRequestId,
        host: &impl JournalHost,
    ) -> Result<Option<Receipt>, Error> {
        let tx = self.read(scope, Access::Execute, None, host)?;
        let bytes: Option<Vec<u8>> = tx
            .query_row(
                &format!(
                    "SELECT {} FROM receipts WHERE scope=?1 AND operation_id=?2",
                    bounded_blob("body", self.limits.max_record_bytes)
                ),
                params![scope.interaction_subject().as_str(), op.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        let receipt = bytes
            .map(|b| decode::<Receipt>(&b, self.limits.max_record_bytes))
            .transpose()?;
        if receipt
            .as_ref()
            .is_some_and(|r| r.kind != OperationKind::Execution)
        {
            return Err(Error::Conflict);
        }
        Ok(receipt)
    }

    /// Read whether an exact execution command was durably received, under current result access.
    /// Presence is submission evidence, never dispatch or effect evidence.
    pub fn has_execution_receipt(
        &self,
        scope: &Scope,
        op: &OperationRequestId,
        access: ExecutionAccess<'_>,
        host: &impl JournalHost,
    ) -> Result<bool, Error> {
        self.check_scope(scope)?;
        access.authorize(scope, host)?;
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        Ok(tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM receipts WHERE scope=?1 AND operation_id=?2)",
            params![scope.interaction_subject().as_str(), op.as_str()],
            |row| row.get(0),
        )?)
    }

    /// Register one immutable bounded plan. No preparation, approval or runner action is implied.
    pub(crate) fn sql_open_execution(
        &mut self,
        op: &OperationRequestId,
        plan: &FrozenExecution,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, Error> {
        let scope = Scope::from_input(plan);
        // Revalidate against the store bounds even if another caller froze with looser limits.
        let bytes = encode(plan.spec(), self.limits.input.max_input_bytes)?;
        execution_contract::decode_execution(&bytes, &self.limits.input)
            .map_err(|_| Error::Capacity)?;
        let w = match self.start(
            op,
            &scope,
            OperationKind::OpenExecution,
            plan.spec(),
            Access::Create,
            host,
        )? {
            Start::Replay(r) => return Ok(CommitOutcome::AlreadyCommitted(r)),
            Start::New(w) => w,
        };
        crate::backend_requests::reserve_request(&w.tx, &scope)?;
        // A preview and a submission register the same immutable aggregate under distinct
        // operation IDs. Only the first call for each operation returns Applied. The receipt
        // remains the durable acceptance boundary even if the later initial attempt fails.
        let existing: Option<(Vec<u8>, u64)> =
            w.tx.query_row(
                &format!(
                    "SELECT {},revision FROM executions WHERE request_id=?1",
                    bounded_blob("plan", w.limits.input.max_input_bytes)
                ),
                [plan.spec().request.request_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((original, revision)) = existing {
            if original != bytes {
                return Err(Error::Conflict);
            }
            let audit = plan_audit(
                &w,
                plan,
                Decision::Proposed {},
                AuditReason::ExecutionOpened,
            );
            return w.finish(Outcome::Duplicate, revision, audit);
        }
        let state = Execution::open(plan.clone(), w.now, w.limits.lifecycle)
            .map_err(|_| Error::InvalidInput)?;
        w.tx.execute(
            "INSERT INTO executions VALUES(?1,?2,?3,?4,?5,0,31)",
            params![
                scope.interaction_subject().as_str(),
                plan.spec().request.request_id.as_str(),
                bytes,
                hash(plan.digest())?,
                w.bounded(state.snapshot())?
            ],
        )?;
        let audit = plan_audit(
            &w,
            plan,
            Decision::Proposed {},
            AuditReason::ExecutionOpened,
        );
        w.finish(Outcome::Changed, 0, audit)
    }
    /// Device service recovery scan. Every row requires independent RunnerFact authorization.
    /// This does not grant ordinary callers a cross-actor listing endpoint.
    pub(crate) fn sql_device_execution_requests(
        &self,
        device: &execution_contract::DeviceId,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<ExecutionRequestPage, Error> {
        if !(1..=128).contains(&limit) {
            return Err(Error::InvalidInput);
        }
        crate::database::ensure_current(&self.conn, &self.authority, self.limits)?;
        let mut statement = self.conn.prepare("SELECT request_id FROM executions WHERE request_id>?1 AND json_extract(CAST(plan AS TEXT),'$.request.target.device')=?2 ORDER BY request_id LIMIT ?3")?;
        let rows = statement.query_map(
            params![
                after.map_or("", |id| id.as_str()),
                device.as_str(),
                limit + 1
            ],
            |r| r.get::<_, String>(0),
        )?;
        let mut requests = rows
            .map(|r| execution_contract::RequestId::new(r?).map_err(|_| Error::Corrupt))
            .collect::<Result<Vec<_>, _>>()?;
        let more = requests.len() > limit;
        requests.truncate(limit);
        for request in &requests {
            self.sql_execution_by_request(request, ExecutionAccess::RunnerFact, host)?;
        }
        let next = more.then(|| requests.last().expect("nonempty page").clone());
        Ok(ExecutionRequestPage { requests, next })
    }
    /// Read a bounded page only for the selected actor/device; each row requires result access.
    pub(crate) fn sql_execution_requests(
        &self,
        actor: &execution_contract::ActorId,
        device: &execution_contract::DeviceId,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<ExecutionRequestPage, Error> {
        if !(1..=128).contains(&limit) {
            return Err(Error::InvalidInput);
        }
        crate::database::ensure_current(&self.conn, &self.authority, self.limits)?;
        let mut statement = self.conn.prepare("SELECT request_id FROM executions WHERE request_id>?1 AND json_extract(CAST(plan AS TEXT),'$.request.actor')=?2 AND json_extract(CAST(plan AS TEXT),'$.request.target.device')=?3 ORDER BY request_id LIMIT ?4")?;
        let rows = statement.query_map(
            params![
                after.map_or("", |id| id.as_str()),
                actor.as_str(),
                device.as_str(),
                limit + 1
            ],
            |r| r.get::<_, String>(0),
        )?;
        let mut requests = rows
            .map(|r| execution_contract::RequestId::new(r?).map_err(|_| Error::Corrupt))
            .collect::<Result<Vec<_>, _>>()?;
        let more = requests.len() > limit;
        requests.truncate(limit);
        for request in &requests {
            self.sql_execution_by_request(request, ExecutionAccess::Result, host)?;
        }
        let next = more.then(|| requests.last().expect("nonempty page").clone());
        Ok(ExecutionRequestPage { requests, next })
    }
    /// Apply a host command through historical deduplication and atomic persistence.
    /// BeginAttempt obtains decisions lazily after receipt replay is ruled out.
    /// Never reconstruct the returned dispatch action after a lost response.
    pub(crate) fn sql_apply_command(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &CommandEvent,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, Error> {
        self.apply_input(op, scope, Input::Command(event), host)
    }
    /// Verify and record an observation under RunnerFact authorization.
    /// Replay, revision and attempt checks precede verification; no approval inputs apply.
    pub(crate) fn sql_apply_observation(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &ObservationEvent,
        host: &impl JournalHost,
        verifier: &dyn ObservationVerifier,
    ) -> Result<CommitOutcome, Error> {
        self.apply_input(op, scope, Input::Observation(event, verifier), host)
    }
    fn apply_input(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        input: Input<'_>,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, Error> {
        let event = match &input {
            Input::Command(e) => EventRecord::Command((*e).clone()),
            Input::Observation(e, _) => EventRecord::Observation((*e).clone()),
        };
        let access = match &event {
            EventRecord::Observation(_) => Access::RunnerFact,
            EventRecord::Command(e) => match e.command {
                Command::Dispatched { .. }
                | Command::DispatchUnconfirmed { .. }
                | Command::StopReported { .. }
                | Command::Output { .. }
                | Command::Recover => Access::RunnerFact,
                Command::Prepare
                | Command::Wait
                | Command::BeginAttempt { .. }
                | Command::Cancel => Access::Execute,
            },
        };
        let mut w = match self.start(op, scope, OperationKind::Execution, &event, access, host)? {
            Start::Replay(r) => return Ok(CommitOutcome::AlreadyCommitted(r)),
            Start::New(w) => w,
        };
        let (plan, current, reserve) = load_execution(&w.tx, scope, w.limits)?;
        // The unique historical key remains after last_event changes and after log retention.
        w.tx.execute(
            "INSERT INTO event_keys VALUES(?1,?2,?3)",
            params![
                event.id().as_str(),
                scope.interaction_subject().as_str(),
                op.as_str()
            ],
        )?;
        let mut audit = plan_audit(
            &w,
            &plan,
            Decision::Proposed {},
            AuditReason::ExecutionEvent,
        );
        audit.attempt_id = match &event {
            EventRecord::Observation(e) => Some(e.attempt_id.clone()),
            EventRecord::Command(e) => match &e.command {
                Command::BeginAttempt { attempt_id, .. }
                | Command::Dispatched { attempt_id }
                | Command::DispatchUnconfirmed { attempt_id, .. }
                | Command::StopReported { attempt_id, .. }
                | Command::Output { attempt_id, .. } => Some(attempt_id.clone()),
                Command::Prepare | Command::Wait | Command::Cancel | Command::Recover => None,
            },
        };
        if let EventRecord::Command(CommandEvent {
            command: Command::DispatchUnconfirmed { cause, .. },
            ..
        }) = &event
        {
            audit.dispatch_cause = Some(*cause);
        }
        if let EventRecord::Command(CommandEvent {
            command: Command::StopReported { outcome, .. },
            ..
        }) = &event
        {
            audit.stop_outcome = Some(*outcome);
        }
        let gate = if let EventRecord::Command(CommandEvent {
            command: Command::BeginAttempt { attempt_id, .. },
            ..
        }) = &event
        {
            let gate = host.admit(&plan, attempt_id, w.now);
            w.refresh_time(host)?;
            audit.occurred_at(w.now);
            audit.reason = AuditReason::AdmissionEvaluated;
            let gate = match gate {
                Ok(gate) => gate,
                Err(JournalError::Denied) => {
                    audit.admission = Some(false);
                    audit.reason = AuditReason::CommitGateRejected;
                    audit.event.as_mut().expect("plan audit").decision = Decision::Denied {};
                    return w.finish(Outcome::Rejected, current.snapshot().revision, audit);
                }
                Err(error) => return Err(error.into()),
            };
            let confirmation_ok = match gate.execution_gate {
                ExecutionGate::Direct => true,
                ExecutionGate::Blocked => false,
                ExecutionGate::Confirmation => {
                    crate::interaction::confirmed(&w.tx, &plan, w.limits, w.now)?
                }
            };
            let allowed = confirmation_ok
                && w.now < gate.valid_until_unix_ms
                && w.now >= plan.spec().validity.not_before_unix_ms
                && w.now < plan.spec().validity.expires_at_unix_ms;
            audit.admission = Some(allowed);
            if !allowed {
                audit.reason = AuditReason::CommitGateRejected;
                audit.event.as_mut().expect("plan audit").decision = Decision::Denied {};
                return w.finish(Outcome::Rejected, current.snapshot().revision, audit);
            }
            Some(gate)
        } else {
            None
        };
        let result = match input {
            Input::Command(event) => current.evaluate(event.clone(), w.now),
            Input::Observation(event, verifier) => {
                current.evaluate_observation(event.clone(), w.now, verifier)
            }
        };
        let evaluation = match result {
            Ok(value) => value,
            Err(error) => {
                audit.reason = AuditReason::LifecycleError(error);
                audit.event.as_mut().expect("plan audit").decision = Decision::Denied {};
                return w.finish(Outcome::Rejected, current.snapshot().revision, audit);
            }
        };
        let Some(transition) = evaluation.transition else {
            audit.reason = AuditReason::Lifecycle(evaluation.directive);
            let outcome = match evaluation.outcome {
                lifecycle::EventOutcome::Duplicate => Outcome::Duplicate,
                _ => Outcome::Stale,
            };
            return w.finish(outcome, current.snapshot().revision, audit);
        };
        let next = transition.next().snapshot();
        let attempt = next.attempt.as_ref().map(|a| a.id.clone());
        if gate.is_some() {
            let attempt_id = attempt.as_ref().ok_or(Error::Corrupt)?;
            w.tx.execute(
                "INSERT INTO attempts VALUES(?1,?2,?3)",
                params![
                    attempt_id.as_str(),
                    scope.interaction_subject().as_str(),
                    op.as_str()
                ],
            )?;
            crate::resources::claim(&w.tx, &plan, attempt_id)?;
            audit.event.as_mut().expect("plan audit").decision = Decision::Admitted {};
        }
        if let EventRecord::Observation(ObservationEvent {
            attempt_id,
            evidence,
            ..
        }) = &event
        {
            audit.event.as_mut().expect("plan audit").decision = Decision::Observed {
                attempt_id: attempt_id.clone(),
                evidence: EvidenceRefs::new(vec![evidence.clone()]).map_err(|_| Error::Corrupt)?,
            };
        }
        if !matches!(
            event,
            EventRecord::Command(CommandEvent {
                command: Command::BeginAttempt { .. },
                ..
            })
        ) {
            audit.reason = AuditReason::Lifecycle(evaluation.directive);
        }
        crate::resources::settle(&w.tx, &plan, next, w.limits)?;
        let reserve = terminal_reserve(current.snapshot(), next, reserve, &event);
        let changed = w.tx.execute("UPDATE executions SET snapshot=?1,revision=?2,reserve=?3 WHERE scope=?4 AND revision=?5",
            params![w.bounded(next)?, integer(next.revision)?, reserve, scope.interaction_subject().as_str(), integer(transition.expected_revision())?])?;
        if changed != 1 {
            return Err(Error::Conflict);
        }
        let receipt = w.prepare(Outcome::Changed, next.revision, audit)?;
        let first_dispatch = transition.commit(move |_| {
            w.commit()?;
            Ok::<_, Error>(lifecycle::CommitStatus::Applied)
        })?;
        Ok(CommitOutcome::Applied {
            receipt,
            first_dispatch,
        })
    }
    /// Restore the current execution facts under result-read access. Restoration never releases an action.
    pub fn execution(&self, scope: &Scope, host: &impl JournalHost) -> Result<Execution, Error> {
        let tx = self.read(scope, Access::ReadResult, None, host)?;
        Ok(load_execution(&tx, scope, self.limits)?.1)
    }
}
enum Input<'a> {
    Command(&'a CommandEvent),
    Observation(&'a ObservationEvent, &'a dyn ObservationVerifier),
}
pub(crate) fn load_execution(
    conn: &Connection,
    scope: &Scope,
    limits: Limits,
) -> Result<(FrozenExecution, Execution, u8), Error> {
    let plan = load_plan(conn, scope, limits)?;
    let (state, reserve) = load_state(conn, scope, plan.clone(), limits)?;
    Ok((plan, state, reserve))
}
pub(crate) fn load_state(
    conn: &Connection,
    scope: &Scope,
    plan: FrozenExecution,
    limits: Limits,
) -> Result<(Execution, u8), Error> {
    let (bytes, revision, reserve): (Vec<u8>, u64, u8) = conn.query_row(
        &format!(
            "SELECT {},revision,reserve FROM executions WHERE scope=?1",
            bounded_blob("snapshot", limits.lifecycle.max_snapshot_bytes)
        ),
        [scope.interaction_subject().as_str()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let state = Execution::decode(plan, &bytes, limits.lifecycle).map_err(|_| Error::Corrupt)?;
    if state.snapshot().revision != revision {
        return Err(Error::Corrupt);
    }
    Ok((state, reserve))
}
fn terminal_reserve(
    old: &lifecycle::Snapshot,
    next: &lifecycle::Snapshot,
    mut reserve: u8,
    event: &EventRecord,
) -> u8 {
    // Five bounded obligations: cancel, uncertainty, termination, initial and final assessment.
    // Repeated reports cannot spend the same reservation twice. New attempts must reserve again.
    if matches!(
        event,
        EventRecord::Command(CommandEvent {
            command: Command::BeginAttempt { .. },
            ..
        })
    ) {
        return 31;
    }
    if !old.cancel_requested && next.cancel_requested {
        reserve &= !1;
    }
    if next
        .attempt
        .as_ref()
        .is_some_and(|a| matches!(a.dispatch, lifecycle::DispatchState::Unknown { .. }))
        && old
            .attempt
            .as_ref()
            .is_some_and(|a| !matches!(a.dispatch, lifecycle::DispatchState::Unknown { .. }))
    {
        reserve &= !2;
    }
    if let Some(a) = &next.attempt {
        if a.assessment.is_some() && old.attempt.as_ref().is_some_and(|a| a.assessment.is_none()) {
            reserve &= !8;
        }
        if a.termination.is_some()
            && old
                .attempt
                .as_ref()
                .is_some_and(|a| a.termination.is_none())
        {
            reserve &= !4;
        }
        if a.assessment.as_ref().is_some_and(|o| {
            matches!(
                o.observation,
                lifecycle::Observation::Effect {
                    assessment: lifecycle::EffectAssessment::Satisfied
                        | lifecycle::EffectAssessment::NoEffect
                }
            )
        }) {
            return 0;
        }
    } else if next.cancel_requested {
        return 0;
    }
    reserve
}
pub(crate) fn plan_audit(
    w: &Write<'_>,
    plan: &FrozenExecution,
    decision: Decision,
    reason: AuditReason,
) -> AuditRecord {
    let p = plan.spec();
    let mut audit = empty_audit(reason);
    audit.event = Some(AuditEvent {
        schema_version: V1,
        event_id: EventId::new("pending").expect("static ID"),
        authority: p.request.authority.clone(),
        request_id: p.request.request_id.clone(),
        content_digest: plan.digest().clone(),
        actor: p.request.actor.clone(),
        initiator: p.request.initiator.clone(),
        operation: p.request.operation.clone(),
        target: p.request.target.clone(),
        decision,
        reason: Id::new("execution-journal").expect("static ID"),
        delegation: p.request.delegation.clone(),
        policy: p.policy.clone(),
        occurred_at_unix_ms: w.now,
    });
    audit
}

impl Store {
    /// Bounded owner scan, authenticating every scope before returning a request identifier.
    pub(crate) fn sql_service_requests(
        &self,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<Vec<execution_contract::RequestId>, Error> {
        if limit == 0 || limit > 128 {
            return Err(Error::InvalidInput);
        }
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        let mut statement=tx.prepare("SELECT request_id,scope FROM executions WHERE request_id>?1 ORDER BY request_id LIMIT ?2")?;
        let rows = statement.query_map(
            params![after.map_or("", |id| id.as_str()), limit as i64],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )?;
        let mut requests = Vec::new();
        for row in rows {
            let (request, key) = row?;
            // Decode only bounded plans; never treat the serialized scope claim as authorization.
            let bytes: Vec<u8> = tx.query_row(
                &format!(
                    "SELECT {} FROM executions WHERE scope=?1",
                    bounded_blob("plan", self.limits.input.max_input_bytes)
                ),
                [&key],
                |r| r.get(0),
            )?;
            let spec = execution_contract::decode_execution(&bytes, &self.limits.input)
                .map_err(|_| Error::Corrupt)?;
            let plan =
                FrozenExecution::freeze(spec, &self.limits.input).map_err(|_| Error::Corrupt)?;
            let scope = Scope::from_input(&plan);
            self.check_scope(&scope)?;
            if scope.interaction_subject().as_str() != key {
                return Err(Error::Corrupt);
            }
            authorize(host, Access::RunnerFact, &scope, None)?;
            requests.push(execution_contract::RequestId::new(request).map_err(|_| Error::Corrupt)?);
        }
        Ok(requests)
    }
}

impl Store {}

impl Store {
    pub(crate) fn sql_task_requests(
        &self,
        before: Option<u64>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<RecentRequestPage, Error> {
        if limit == 0 || limit > 128 {
            return Err(Error::InvalidInput);
        }
        let before = before.map(integer).transpose()?.unwrap_or(i64::MAX);
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        // Scan a fixed receipt window before filtering or de-duplicating: hot tasks and historical
        // refreshes cannot turn a page read into an unbounded history walk.
        let sql = format!("SELECT r.sequence,{},e.request_id,e.scope FROM receipts r LEFT JOIN executions e ON e.scope=r.scope WHERE r.sequence<?1 AND r.sequence=(SELECT MIN(first.sequence) FROM receipts first WHERE first.scope=r.scope) ORDER BY r.sequence DESC LIMIT ?2", bounded_blob("r.body", self.limits.max_record_bytes));
        let mut statement = tx.prepare(&sql)?;
        let rows = statement.query_map(params![before, limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut scanned = 0;
        let mut last = None;
        for row in rows {
            let (sequence, bytes, request, key) = row?;
            scanned += 1;
            last = Some(u64::try_from(sequence).map_err(|_| Error::Corrupt)?);
            let (Some(request), Some(key)) = (request, key) else {
                continue;
            };
            if !seen.insert(request.clone()) {
                continue;
            }
            let receipt: Receipt = decode(&bytes, self.limits.max_record_bytes)?;
            let plan_bytes: Vec<u8> = tx.query_row(
                &format!(
                    "SELECT {} FROM executions WHERE scope=?1",
                    bounded_blob("plan", self.limits.input.max_input_bytes)
                ),
                [&key],
                |row| row.get(0),
            )?;
            let spec = execution_contract::decode_execution(&plan_bytes, &self.limits.input)
                .map_err(|_| Error::Corrupt)?;
            let plan =
                FrozenExecution::freeze(spec, &self.limits.input).map_err(|_| Error::Corrupt)?;
            let scope = Scope::from_input(&plan);
            self.check_scope(&scope)?;
            if receipt.scope != scope
                || receipt.sequence != sequence as u64
                || scope.interaction_subject().as_str() != key
                || plan.spec().request.request_id.as_str() != request
            {
                return Err(Error::Corrupt);
            }
            authorize(host, Access::RunnerFact, &scope, None)?;
            items.push(RecentRequest {
                request: plan.spec().request.request_id.clone(),
                sequence: receipt.sequence,
                recorded_at_unix_ms: receipt.occurred_at_unix_ms,
            });
        }
        Ok(RecentRequestPage {
            items,
            next: (scanned == limit).then_some(last).flatten(),
        })
    }
}

impl Store {
    pub(crate) fn sql_remote_request(
        &self,
        task: &execution_contract::Id,
        attempt: &execution_contract::Id,
        host: &impl JournalHost,
    ) -> Result<Option<execution_contract::RequestId>, Error> {
        crate::database::ensure_current(&self.conn, &self.authority, self.limits)?;
        let mut query=self.conn.prepare("SELECT request_id FROM executions WHERE CASE WHEN json_valid(CAST(plan AS TEXT)) THEN json_extract(CAST(plan AS TEXT),'$.request.initiator.task') END=?1 AND CASE WHEN json_valid(CAST(plan AS TEXT)) THEN json_extract(CAST(plan AS TEXT),'$.request.initiator.attempt') END=?2 ORDER BY request_id LIMIT 2")?;
        let ids = query
            .query_map(params![task.as_str(), attempt.as_str()], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if ids.len() > 1 {
            return Err(Error::Corrupt);
        }
        let Some(id) = ids.into_iter().next() else {
            return Ok(None);
        };
        let request = execution_contract::RequestId::new(id).map_err(|_| Error::Corrupt)?;
        let record = self.sql_execution_by_request(&request, ExecutionAccess::RunnerFact, host)?;
        if !matches!(&record.execution.input().spec().request.initiator,execution_contract::Initiator::Backend{task:t,attempt:a,..} if t==task&&a==attempt)
        {
            return Err(Error::Corrupt);
        }
        Ok(Some(request))
    }
}

impl Store {
    pub(crate) fn sql_task_watermarks(&self) -> Result<(u64, u64), Error> {
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        let (preparations, receipts): (i64, i64) = tx.query_row("SELECT (SELECT COALESCE(MAX(rowid),0) FROM backend_requests),(SELECT COALESCE(MAX(sequence),0) FROM receipts)", [], |r| Ok((r.get(0)?,r.get(1)?)))?;
        Ok((
            u64::try_from(preparations).map_err(|_| Error::Corrupt)?,
            u64::try_from(receipts).map_err(|_| Error::Corrupt)?,
        ))
    }
}

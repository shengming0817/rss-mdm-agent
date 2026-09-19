use crate::database::bounded_blob;
use crate::{journal::*, trust::*, *};
use execution_approval::ProfileApproval;
use execution_contract::{AuditEvent, Decision, EventId, EvidenceRefs, FrozenPlan, Id, V1};
use execution_lifecycle::{
    self as lifecycle, Command, CommandEvent, EventRecord, Execution, ObservationEvent,
    ObservationVerifier,
};
use rusqlite::{params, Connection, OptionalExtension};

impl Store {
    /// Restore by stable business request identity after reconnect or restart. The stored scope
    /// is authenticated before any plan or state is returned. No dispatch action is recoverable.
    pub fn execution_by_request(
        &self,
        request: &execution_contract::RequestId,
        access: ExecutionAccess<'_>,
        host: &impl Host,
    ) -> Result<ExecutionRecord, Error> {
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        let bytes: Vec<u8> = tx.query_row(
            &format!(
                "SELECT {} FROM executions WHERE request_id=?1",
                bounded_blob("plan", self.limits.plan.max_input_bytes)
            ),
            [request.as_str()],
            |row| row.get(0),
        )?;
        let spec = execution_contract::decode_plan(&bytes, &self.limits.plan)
            .map_err(|_| Error::Corrupt)?;
        let plan = FrozenPlan::freeze(spec, &self.limits.plan).map_err(|_| Error::Corrupt)?;
        let scope = Scope::from_plan(&plan);
        self.check_scope(&scope)?;
        access.authorize(&scope, host)?;
        if &plan.spec().request.request_id != request {
            return Err(Error::Corrupt);
        }
        let receipt: Option<Vec<u8>> = tx.query_row(
            &format!("SELECT {} FROM receipts WHERE scope=?1 AND kind='admission' ORDER BY sequence DESC LIMIT 1", bounded_blob("body", self.limits.max_record_bytes)),
            [scope.key()], |r| r.get(0),
        ).optional()?;
        let admission = receipt
            .map(|b| decode::<Receipt>(&b, self.limits.max_record_bytes))
            .transpose()?
            .and_then(|r| r.admission);
        Ok(ExecutionRecord {
            execution: load_execution(&tx, &scope, self.limits)?.1,
            admission,
        })
    }

    /// Retrieve an execution command's safe receipt under current Execute permission, independent
    /// of general result reading. Does not authorize a new attempt or re-run admission.
    pub fn execution_receipt(
        &self,
        scope: &Scope,
        op: &OperationRequestId,
        host: &impl Host,
    ) -> Result<Option<Receipt>, Error> {
        let tx = self.read(scope, Access::Execute, None, host)?;
        let bytes: Option<Vec<u8>> = tx
            .query_row(
                &format!(
                    "SELECT {} FROM receipts WHERE scope=?1 AND operation_id=?2",
                    bounded_blob("body", self.limits.max_record_bytes)
                ),
                params![scope.key(), op.as_str()],
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
        host: &impl Host,
    ) -> Result<bool, Error> {
        self.check_scope(scope)?;
        access.authorize(scope, host)?;
        let tx = self.conn.unchecked_transaction()?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        Ok(tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM receipts WHERE scope=?1 AND operation_id=?2)",
            params![scope.key(), op.as_str()],
            |row| row.get(0),
        )?)
    }

    /// Register one immutable bounded plan. No preparation, approval or runner action is implied.
    pub fn open_execution(
        &mut self,
        op: &OperationRequestId,
        plan: &FrozenPlan,
        host: &impl Host,
    ) -> Result<CommitOutcome, Error> {
        let scope = Scope::from_plan(plan);
        // Revalidate against the store bounds even if another caller froze with looser limits.
        let bytes = encode(plan.spec(), self.limits.plan.max_input_bytes)?;
        execution_contract::decode_plan(&bytes, &self.limits.plan).map_err(|_| Error::Capacity)?;
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
        // A preview and a submission register the same immutable aggregate under distinct
        // operation IDs. Only the first call for each operation returns Applied. The receipt
        // remains the durable acceptance boundary even if the later initial attempt fails.
        let existing: Option<(Vec<u8>, u64)> =
            w.tx.query_row(
                &format!(
                    "SELECT {},revision FROM executions WHERE request_id=?1",
                    bounded_blob("plan", w.limits.plan.max_input_bytes)
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
            "INSERT INTO executions VALUES(?1,?2,?3,?4,?5,?6,0,31)",
            params![
                scope.key(),
                plan.spec().request.request_id.as_str(),
                plan.spec().plan_id.as_str(),
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
    /// Read a bounded page only for the selected actor/device; each row requires result access.
    pub fn execution_requests(
        &self,
        actor: &execution_contract::ActorId,
        device: &execution_contract::DeviceId,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl Host,
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
            self.execution_by_request(request, ExecutionAccess::Result, host)?;
        }
        let next = more.then(|| requests.last().expect("nonempty page").clone());
        Ok(ExecutionRequestPage { requests, next })
    }
    /// Apply a host command through historical deduplication and atomic persistence.
    /// BeginAttempt obtains decisions lazily after receipt replay is ruled out.
    /// Never reconstruct the returned dispatch action after a lost response.
    /// ```compile_fail
    /// use execution_sqlite::{Store, Scope, OperationRequestId, Host};
    /// use execution_lifecycle::ObservationEvent;
    /// fn wrong(store: &mut Store, op: &OperationRequestId, scope: &Scope, event: &ObservationEvent, host: &impl Host) {
    ///     store.apply_command(op, scope, event, &[], host);
    /// }
    /// ```
    pub fn apply_command(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &CommandEvent,
        bindings: &[ProfileApproval],
        host: &impl Host,
    ) -> Result<CommitOutcome, Error> {
        self.apply_input(op, scope, Input::Command(event, bindings), host)
    }
    /// Verify and record an observation under RunnerFact authorization.
    /// Replay, revision and attempt checks precede verification; no approval inputs apply.
    /// ```compile_fail
    /// use execution_sqlite::{Store, Scope, OperationRequestId, Host};
    /// use execution_lifecycle::{CommandEvent, ObservationVerifier};
    /// fn wrong(store: &mut Store, op: &OperationRequestId, scope: &Scope, event: &CommandEvent, host: &impl Host, verifier: &dyn ObservationVerifier) {
    ///     store.apply_observation(op, scope, event, host, verifier);
    /// }
    /// ```
    /// ```compile_fail
    /// use execution_sqlite::{Store, Scope, OperationRequestId, Host};
    /// use execution_lifecycle::ObservationEvent;
    /// fn missing(store: &mut Store, op: &OperationRequestId, scope: &Scope, event: &ObservationEvent, host: &impl Host) {
    ///     store.apply_observation(op, scope, event, host);
    /// }
    /// ```
    pub fn apply_observation(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &ObservationEvent,
        host: &impl Host,
        verifier: &dyn ObservationVerifier,
    ) -> Result<CommitOutcome, Error> {
        self.apply_input(op, scope, Input::Observation(event, verifier), host)
    }
    fn apply_input(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        input: Input<'_>,
        host: &impl Host,
    ) -> Result<CommitOutcome, Error> {
        let (event, bindings) = match &input {
            Input::Command(e, bindings) => (EventRecord::Command((*e).clone()), *bindings),
            Input::Observation(e, _) => (EventRecord::Observation((*e).clone()), &[][..]),
        };
        if bindings.len() > self.limits.max_approvals {
            return Err(Error::Capacity);
        }
        let mut refs: Vec<_> = bindings.iter().map(|b| (&b.profile, &b.record)).collect();
        refs.sort_by(|a, b| {
            (&a.0.id, &a.0.revision, &a.1.id, &a.1.revision).cmp(&(
                &b.0.id,
                &b.0.revision,
                &b.1.id,
                &b.1.revision,
            ))
        });
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
        let mut w = match self.start(
            op,
            scope,
            OperationKind::Execution,
            &(&event, refs),
            access,
            host,
        )? {
            Start::Replay(r) => return Ok(CommitOutcome::AlreadyCommitted(r)),
            Start::New(w) => w,
        };
        let (plan, current, reserve) = load_execution(&w.tx, scope, w.limits)?;
        // The unique historical key remains after last_event changes and after log retention.
        w.tx.execute(
            "INSERT INTO event_keys VALUES(?1,?2,?3)",
            params![event.id().as_str(), scope.key(), op.as_str()],
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
            audit.submitted_approvals = bindings.iter().map(ApprovalBindingAudit::from).collect();
            let Some(h) = head(&w.tx, scope, w.limits)? else {
                audit.reason = AuditReason::TrustUnavailable;
                audit.event.as_mut().expect("plan audit").decision = Decision::Denied {};
                return w.finish(Outcome::Rejected, current.snapshot().revision, audit);
            };
            let approvals = StoredApprovals {
                conn: &w.tx,
                scope,
                limits: w.limits,
                now: w.now,
                clock: &|| host.reliable_now(),
                head: &h,
            };
            audit.trust = Some(TrustAudit {
                authorization_revision: h.authorization.clone(),
                approval_revision: h.approval.clone(),
                fresh_until_unix_ms: h.until,
            });
            audit.protected_approvals = approvals.audit_records(bindings)?;
            let gate = host.admit(&plan, attempt_id, bindings, &approvals, w.now)?;
            w.refresh_time(host)?;
            audit.occurred_at(w.now);
            audit.reason = AuditReason::AdmissionEvaluated;
            audit.admission = Some(admission_audit(&gate.admission));
            audit.approval = Some(approval_audit(&gate.approval));
            if !check_gate(&w, &plan, attempt_id, bindings, &gate, &h) {
                audit.reason = AuditReason::CommitGateRejected;
                audit.event.as_mut().expect("plan audit").decision = Decision::Denied {};
                return w.finish(Outcome::Rejected, current.snapshot().revision, audit);
            }
            Some((gate, h))
        } else {
            if !bindings.is_empty() {
                return Err(Error::Conflict);
            }
            None
        };
        let result = match input {
            Input::Command(event, _) => current.evaluate(event.clone(), w.now),
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
        if let Some((gate, h)) = gate {
            let attempt_id = attempt.as_ref().ok_or(Error::Corrupt)?;
            w.tx.execute(
                "INSERT INTO attempts VALUES(?1,?2,?3)",
                params![attempt_id.as_str(), scope.key(), op.as_str()],
            )?;
            audit.consumptions = consume(&w, &plan, attempt_id, &gate, &h)?;
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
        let reserve = terminal_reserve(current.snapshot(), next, reserve, &event);
        let changed = w.tx.execute("UPDATE executions SET snapshot=?1,revision=?2,reserve=?3 WHERE scope=?4 AND revision=?5",
            params![w.bounded(next)?, integer(next.revision)?, reserve, scope.key(), integer(transition.expected_revision())?])?;
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
    pub fn execution(&self, scope: &Scope, host: &impl Host) -> Result<Execution, Error> {
        let tx = self.read(scope, Access::ReadResult, None, host)?;
        Ok(load_execution(&tx, scope, self.limits)?.1)
    }
}
enum Input<'a> {
    Command(&'a CommandEvent, &'a [ProfileApproval]),
    Observation(&'a ObservationEvent, &'a dyn ObservationVerifier),
}
fn load_execution(
    conn: &Connection,
    scope: &Scope,
    limits: Limits,
) -> Result<(FrozenPlan, Execution, u8), Error> {
    let plan = load_plan(conn, scope, limits)?;
    let (bytes, revision, reserve): (Vec<u8>, u64, u8) = conn.query_row(
        &format!(
            "SELECT {},revision,reserve FROM executions WHERE scope=?1",
            bounded_blob("snapshot", limits.lifecycle.max_snapshot_bytes)
        ),
        [scope.key()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let state =
        Execution::decode(plan.clone(), &bytes, limits.lifecycle).map_err(|_| Error::Corrupt)?;
    if state.snapshot().revision != revision {
        return Err(Error::Corrupt);
    }
    Ok((plan, state, reserve))
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
    plan: &FrozenPlan,
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
        plan_id: p.plan_id.clone(),
        plan_digest: plan.digest().clone(),
        actor: p.request.actor.clone(),
        initiator: p.request.initiator.clone(),
        approver: None,
        operation: p.request.operation.clone(),
        target: p.request.target.clone(),
        decision,
        reason: Id::new("execution-journal").expect("static ID"),
        delegation: p.request.delegation.clone(),
        policy: p.policy.clone(),
        approval: None,
        occurred_at_unix_ms: w.now,
    });
    audit
}
impl AuditRecord {
    fn occurred_at(&mut self, now: u64) {
        if let Some(event) = &mut self.event {
            event.occurred_at_unix_ms = now;
        }
    }
}

fn admission_audit(a: &execution_admission::AdmissionDecision) -> AdmissionAudit {
    AdmissionAudit {
        plan_id: a.plan_id().clone(),
        plan_digest: a.plan_digest().clone(),
        attempt_id: a.attempt_id().clone(),
        policy: a.policy().clone(),
        delegation: a.delegation().cloned(),
        outcome: a.outcome().clone(),
        reason: a.reason(),
        rule_ids: a.rule_ids().to_vec(),
        validity: a.validity().map(DecisionValidity::from),
    }
}
fn approval_audit(p: &execution_approval::ApprovalDecision) -> ApprovalAudit {
    ApprovalAudit {
        plan_id: p.plan_id().clone(),
        plan_digest: p.plan_digest().clone(),
        attempt_id: p.attempt_id().clone(),
        outcome: p.outcome().clone(),
        bindings: p
            .bindings()
            .iter()
            .map(ApprovalBindingAudit::from)
            .collect(),
        pending_consumptions: p
            .consumptions()
            .iter()
            .map(|c| PendingConsumptionAudit {
                approval: c.approval().clone(),
                expected_uses: c.expected_uses(),
                expected_consumption_revision: c.expected_consumption_revision(),
                verification_revision: c.verification_revision().clone(),
                valid_until_unix_ms: c.valid_until_unix_ms(),
            })
            .collect(),
        admission_validity: p.admission_validity().map(DecisionValidity::from),
    }
}

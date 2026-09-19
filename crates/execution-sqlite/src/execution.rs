use crate::database::bounded_blob;
use crate::{journal::*, trust::*, *};
use execution_approval::ProfileApproval;
use execution_contract::{AuditEvent, Decision, EventId, EvidenceRefs, FrozenPlan, Id, V1};
use execution_lifecycle::{self as lifecycle, Command, Execution};
use rusqlite::{params, Connection};

impl Store {
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
    /// Apply one event through historical deduplication, core evaluation and atomic persistence.
    /// BeginAttempt obtains decisions lazily from Host only after receipt replay is ruled out.
    /// Never pass the returned action to an outbox or reconstruct it after a lost response.
    pub fn apply_execution(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &lifecycle::Event,
        bindings: &[ProfileApproval],
        host: &impl Host,
    ) -> Result<CommitOutcome, Error> {
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
        let access = match event.command {
            Command::Dispatched { .. }
            | Command::Observe { .. }
            | Command::Output { .. }
            | Command::Recover => Access::RunnerFact,
            _ => Access::Execute,
        };
        let mut w = match self.start(
            op,
            scope,
            OperationKind::Execution,
            &(event, refs),
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
            params![event.id.as_str(), scope.key(), op.as_str()],
        )?;
        let mut audit = plan_audit(
            &w,
            &plan,
            Decision::Proposed {},
            AuditReason::ExecutionEvent,
        );
        audit.attempt_id = match &event.command {
            Command::BeginAttempt { attempt_id, .. }
            | Command::Dispatched { attempt_id }
            | Command::Observe { attempt_id, .. }
            | Command::Output { attempt_id, .. } => Some(attempt_id.clone()),
            Command::Prepare | Command::Wait | Command::Cancel | Command::Recover => None,
        };
        let gate = if let Command::BeginAttempt { attempt_id, .. } = &event.command {
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
        let evaluation = match current.evaluate(event.clone(), w.now, host) {
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
        if let Command::Observe {
            attempt_id,
            evidence,
        } = &event.command
        {
            audit.event.as_mut().expect("plan audit").decision = Decision::Observed {
                attempt_id: attempt_id.clone(),
                evidence: EvidenceRefs::new(vec![evidence.clone()]).map_err(|_| Error::Corrupt)?,
            };
        }
        if !matches!(event.command, Command::BeginAttempt { .. }) {
            audit.reason = AuditReason::Lifecycle(evaluation.directive);
        }
        let reserve = terminal_reserve(current.snapshot(), next, reserve, &event.command);
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
    command: &Command,
) -> u8 {
    // Five bounded obligations: cancel, uncertainty, termination, initial and final assessment.
    // Repeated reports cannot spend the same reservation twice. New attempts must reserve again.
    if matches!(command, Command::BeginAttempt { .. }) {
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

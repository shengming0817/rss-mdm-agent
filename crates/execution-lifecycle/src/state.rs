use crate::*;
use execution_contract::{Authority, FrozenPlan};

/// Validated lifecycle state, not an executable permit.
/// Restore validates protected journal data; it does not authenticate arbitrary JSON.
/// ~~~compile_fail
/// let _: execution_lifecycle::Execution = serde_json::from_str("{}").unwrap();
/// ~~~
#[derive(Debug, Clone)]
pub struct Execution {
    plan: FrozenPlan,
    snapshot: Snapshot,
    limits: Limits,
}
impl Execution {
    /// Create received state. Time may precede the plan window; attempts cannot.
    pub fn open(plan: FrozenPlan, now: u64, limits: Limits) -> Result<Self, LifecycleError> {
        let snapshot = Snapshot {
            version: 1,
            plan_id: plan.spec().plan_id.clone(),
            plan_digest: plan.digest().clone(),
            opened_at_unix_ms: now,
            updated_at_unix_ms: now,
            revision: 0,
            preparation: Preparation::Received,
            cancel_requested: false,
            first_attempt_at_unix_ms: None,
            attempts: 0,
            prior_output_bytes: 0,
            attempt: None,
            last_event: None,
        };
        Self::restore(plan, snapshot, limits)
    }
    /// Decode the sole bounded format; storage authenticity must be established by C18.
    pub fn decode(plan: FrozenPlan, bytes: &[u8], limits: Limits) -> Result<Self, LifecycleError> {
        validate_limits(limits)?;
        if bytes.len() > limits.max_snapshot_bytes {
            return Err(LifecycleError::Limit);
        }
        let s = serde_json::from_slice(bytes).map_err(|_| LifecycleError::Snapshot)?;
        Self::restore(plan, s, limits)
    }
    /// Restore protected storage after checking all structural and plan invariants.
    pub fn restore(
        plan: FrozenPlan,
        snapshot: Snapshot,
        limits: Limits,
    ) -> Result<Self, LifecycleError> {
        validate_limits(limits)?;
        crate::validation::validate(&plan, &snapshot, limits)?;
        Ok(Self {
            plan,
            snapshot,
            limits,
        })
    }
    /// Read the sole persistence representation.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    /// Borrow the exact immutable plan restored with these facts; this grants no dispatch authority.
    pub fn plan(&self) -> &FrozenPlan {
        &self.plan
    }
    /// Cumulative output across attempts, saturating at the integer ceiling.
    /// Saturation can only exhaust a budget, never replenish it.
    pub fn total_output_bytes(&self) -> u64 {
        self.snapshot
            .prior_output_bytes
            .saturating_add(self.snapshot.attempt.as_ref().map_or(0, |a| a.output_bytes))
    }
    fn limit_reason(&self, now: u64) -> Option<LimitReason> {
        let p = self.plan.spec();
        let s = &self.snapshot;
        if now < p.validity.not_before_unix_ms {
            Some(LimitReason::NotYetValid)
        } else if now >= p.validity.expires_at_unix_ms {
            Some(LimitReason::Expired)
        } else if self.total_output_bytes() >= p.budget.total_output_bytes {
            Some(LimitReason::Output)
        } else if s
            .first_attempt_at_unix_ms
            .is_some_and(|start| now.saturating_sub(start) >= p.budget.total_timeout_ms)
        {
            Some(LimitReason::Timeout)
        } else {
            None
        }
    }
    /// Derive phase from facts without storing parallel status fields.
    pub fn phase(&self) -> Phase {
        let s = &self.snapshot;
        let Some(a) = &s.attempt else {
            return if s.cancel_requested {
                Phase::Cancelled
            } else {
                match s.preparation {
                    Preparation::Received => Phase::Received,
                    Preparation::Prepared => Phase::Prepared,
                    Preparation::Waiting => Phase::Waiting,
                }
            };
        };
        if a.assessment.is_some() {
            return Phase::Verified;
        }
        if let Some(t) = &a.termination {
            return if matches!(t.observation, Observation::NeverDispatched { .. }) {
                Phase::FailedBeforeDispatch
            } else {
                Phase::ExecutionEnded
            };
        }
        match a.dispatch {
            DispatchState::Starting => Phase::Starting,
            DispatchState::Dispatched => Phase::Running,
            DispatchState::Unknown { .. } => Phase::OutcomeUnknown,
        }
    }
    /// Derive bounded recovery/continuation advice at reliable host time.
    /// Expired budgets stop new work but never suppress observation recording.
    /// Cause priority: cancellation, validity, output, timeout, then attempt count.
    /// C18/C19 retain the reason and decision time in their audit; this is not termination.
    pub fn directive(&self, now: u64) -> Result<Directive, LifecycleError> {
        if now < self.snapshot.updated_at_unix_ms {
            return Err(LifecycleError::Clock);
        }
        let s = &self.snapshot;
        let Some(a) = &s.attempt else {
            return Ok(if s.cancel_requested {
                Directive::Done
            } else if let Some(reason) = self.limit_reason(now) {
                Directive::BudgetExhausted(reason)
            } else {
                match s.preparation {
                    Preparation::Received => Directive::Prepare,
                    Preparation::Prepared => Directive::Ready,
                    Preparation::Waiting => Directive::Wait,
                }
            });
        };
        if a.termination.is_none() {
            return Ok(if s.cancel_requested {
                Directive::StopRunner(StopReason::Cancelled)
            } else if let Some(reason) = self.limit_reason(now) {
                Directive::StopRunner(StopReason::Limit(reason))
            } else if a.dispatch == DispatchState::Dispatched {
                Directive::Wait
            } else {
                Directive::Reconcile
            });
        }
        let assessment = a.assessment.as_ref().and_then(|o| match o.observation {
            Observation::Effect { assessment } => Some(assessment),
            _ => None,
        });
        let no_effect = assessment == Some(EffectAssessment::NoEffect)
            || (assessment.is_none()
                && a.termination
                    .as_ref()
                    .is_some_and(|t| matches!(t.observation, Observation::NeverDispatched { .. })));
        Ok(
            if assessment == Some(EffectAssessment::Satisfied) || (s.cancel_requested && no_effect)
            {
                Directive::Done
            } else if no_effect {
                if let Some(reason) = self.limit_reason(now) {
                    Directive::BudgetExhausted(reason)
                } else if s.attempts >= self.plan.spec().budget.max_attempts {
                    Directive::BudgetExhausted(LimitReason::Attempts)
                } else {
                    Directive::RetryEligible
                }
            } else if assessment.is_some() {
                Directive::ManualReview
            } else {
                Directive::VerifyTarget
            },
        )
    }
    /// Evaluate a host command without an observation verifier.
    /// Caller commits the candidate atomically before acting.
    ///
    /// Observation inputs cannot enter this path:
    /// ```compile_fail
    /// use execution_lifecycle::{Execution, ObservationEvent};
    /// fn wrong(state: &Execution, event: ObservationEvent) {
    ///     state.evaluate(event, 0);
    /// }
    /// ```
    pub fn evaluate(&self, event: CommandEvent, now: u64) -> Result<Evaluation, LifecycleError> {
        self.evaluate_input(Input::Command(event), now)
    }
    /// Resolve a trusted observation after duplicate, revision and attempt checks.
    /// A verifier is mandatory and ordinary commands cannot enter this path.
    /// ```compile_fail
    /// use execution_lifecycle::{Execution, ObservationEvent};
    /// fn missing(state: &Execution, event: ObservationEvent) {
    ///     state.evaluate_observation(event, 0);
    /// }
    /// ```
    /// ```compile_fail
    /// use execution_lifecycle::{CommandEvent, Execution, ObservationVerifier};
    /// fn wrong(state: &Execution, event: CommandEvent, verifier: &dyn ObservationVerifier) {
    ///     state.evaluate_observation(event, 0, verifier);
    /// }
    /// ```
    pub fn evaluate_observation(
        &self,
        event: ObservationEvent,
        now: u64,
        verifier: &dyn ObservationVerifier,
    ) -> Result<Evaluation, LifecycleError> {
        self.evaluate_input(Input::Observation(event, verifier), now)
    }
    // ref: raft-rs src/raw_node.rs@10c6e9db6792b85c81784e44fc278f895d5f0ab0
    fn evaluate_input(&self, input: Input<'_>, now: u64) -> Result<Evaluation, LifecycleError> {
        let event = match &input {
            Input::Command(e) => EventRecord::Command(e.clone()),
            Input::Observation(e, _) => EventRecord::Observation(e.clone()),
        };
        let directive = self.directive(now)?;
        let s = &self.snapshot;
        let unchanged = |outcome| {
            Ok(Evaluation {
                outcome,
                directive,
                transition: None,
            })
        };
        if let Some(last) = &s.last_event {
            if last.id() == event.id() {
                return if last == &event {
                    unchanged(EventOutcome::Duplicate)
                } else {
                    Err(LifecycleError::IdempotencyConflict)
                };
            }
        }
        if event.expected_revision() < s.revision {
            return unchanged(EventOutcome::Stale);
        }
        if event.expected_revision() != s.revision || s.revision == u64::MAX {
            return Err(LifecycleError::Revision);
        }
        let mut next = s.clone();
        match input {
            Input::Command(event) => {
                self.apply_command(&event.command, directive, now, &mut next)?
            }
            Input::Observation(event, verifier) => {
                self.apply_observation(&event, now, verifier, &mut next)?
            }
        }
        next.updated_at_unix_ms = now;
        next.revision = s.revision + 1;
        next.last_event = Some(event);
        let next = Self::restore(self.plan.clone(), next, self.limits)?;
        Ok(Evaluation {
            outcome: EventOutcome::Applied,
            directive: next.directive(now)?,
            transition: Some(Transition {
                expected_revision: s.revision,
                next,
            }),
        })
    }
    fn apply_command(
        &self,
        command: &Command,
        directive: Directive,
        now: u64,
        next: &mut Snapshot,
    ) -> Result<(), LifecycleError> {
        let s = &self.snapshot;
        match command {
            Command::Prepare | Command::Wait => {
                if s.attempt.is_some() || s.cancel_requested {
                    return Err(LifecycleError::Transition);
                }
                next.preparation = if matches!(command, Command::Prepare) {
                    Preparation::Prepared
                } else {
                    Preparation::Waiting
                };
            }
            Command::BeginAttempt {
                attempt_id,
                runner,
                mode,
            } => {
                if !matches!(directive, Directive::Ready | Directive::RetryEligible) {
                    return Err(LifecycleError::Transition);
                }
                if (matches!(self.plan.spec().request.authority, Authority::Test { .. })
                    && *mode != ExecutionMode::Test)
                    || s.attempt.as_ref().is_some_and(|a| {
                        &a.id == attempt_id || &a.runner != runner || a.mode != *mode
                    })
                {
                    return Err(LifecycleError::Attempt);
                }
                next.attempts = s.attempts.checked_add(1).ok_or(LifecycleError::Attempt)?;
                next.first_attempt_at_unix_ms = Some(s.first_attempt_at_unix_ms.unwrap_or(now));
                next.prior_output_bytes = self.total_output_bytes();
                next.attempt = Some(AttemptSnapshot {
                    id: attempt_id.clone(),
                    number: next.attempts,
                    accepted_at_unix_ms: now,
                    runner: runner.clone(),
                    mode: *mode,
                    dispatch: DispatchState::Starting,
                    dispatch_cause: None,
                    stop_outcome: None,
                    termination: None,
                    assessment: None,
                    output_bytes: 0,
                });
            }
            Command::Dispatched { attempt_id } => {
                let a = current_attempt(next, attempt_id)?;
                if a.dispatch != DispatchState::Starting || a.termination.is_some() {
                    return Err(LifecycleError::Transition);
                }
                a.dispatch = DispatchState::Dispatched;
            }
            Command::Cancel => next.cancel_requested = true,
            Command::DispatchUnconfirmed { attempt_id, cause } => {
                let a = current_attempt(next, attempt_id)?;
                a.dispatch_cause = Some(*cause);
                if a.termination.is_none() {
                    a.dispatch = a.dispatch.uncertain();
                }
            }
            Command::StopReported {
                attempt_id,
                outcome,
            } => {
                current_attempt(next, attempt_id)?.stop_outcome = Some(*outcome);
            }
            Command::Recover => {
                if let Some(a) = &mut next.attempt {
                    if a.termination.is_none() {
                        a.dispatch = a.dispatch.uncertain();
                    }
                }
            }
            Command::Output {
                attempt_id,
                total_bytes,
            } => {
                let a = current_attempt(next, attempt_id)?;
                if (a.termination.is_none() && *total_bytes < a.output_bytes)
                    || (a.termination.is_some() && *total_bytes > a.output_bytes)
                {
                    return Err(LifecycleError::Accounting);
                }
                if a.termination.is_none() {
                    a.output_bytes = *total_bytes;
                }
            }
        }
        Ok(())
    }
    fn apply_observation(
        &self,
        event: &ObservationEvent,
        now: u64,
        verifier: &dyn ObservationVerifier,
        next: &mut Snapshot,
    ) -> Result<(), LifecycleError> {
        let s = &self.snapshot;

        let ObservationEvent {
            attempt_id,
            evidence,
            ..
        } = event;
        let a = current_attempt(next, attempt_id)?;
        let facts = verifier
            .verify(&self.plan, attempt_id, evidence, now)
            .map_err(LifecycleError::ObservationVerification)?;
        if facts.plan_id != s.plan_id
            || facts.plan_digest != s.plan_digest
            || &facts.attempt_id != attempt_id
            || &facts.evidence != evidence
            || evidence.runner != a.runner
            || facts.observed_at_unix_ms < a.accepted_at_unix_ms
            || facts.observed_at_unix_ms > now
            || !crate::validation::valid_evidence(a.mode, evidence.kind, &facts.observation)
        {
            return Err(LifecycleError::Observation);
        }
        let recorded = RecordedObservation {
            evidence: evidence.clone(),
            observed_at_unix_ms: facts.observed_at_unix_ms,
            observation: facts.observation,
        };
        match &recorded.observation {
            Observation::Exited {
                total_output_bytes, ..
            }
            | Observation::NeverDispatched { total_output_bytes } => {
                if a.termination.is_some()
                    || (matches!(recorded.observation, Observation::NeverDispatched { .. })
                        && a.dispatch.was_dispatched())
                {
                    return Err(LifecycleError::Transition);
                }
                if *total_output_bytes < a.output_bytes {
                    return Err(LifecycleError::Accounting);
                }
                a.output_bytes = *total_output_bytes;
                a.termination = Some(recorded);
            }
            Observation::Effect { assessment: _ } => {
                let t = a.termination.as_ref().ok_or(LifecycleError::Transition)?;
                if recorded.observed_at_unix_ms < t.observed_at_unix_ms
                    || a.assessment.as_ref().is_some_and(|o| {
                        recorded.observed_at_unix_ms < o.observed_at_unix_ms
                            || matches!(
                                o.observation,
                                Observation::Effect {
                                    assessment: EffectAssessment::Satisfied
                                        | EffectAssessment::NoEffect
                                }
                            )
                    })
                {
                    return Err(LifecycleError::Transition);
                }
                a.assessment = Some(recorded);
            }
            Observation::Uncertain => {
                if a.termination.is_some() {
                    return Err(LifecycleError::Transition);
                }
                a.dispatch = a.dispatch.uncertain();
            }
        }

        Ok(())
    }
}
enum Input<'a> {
    Command(CommandEvent),
    Observation(ObservationEvent, &'a dyn ObservationVerifier),
}
fn current_attempt<'a>(
    s: &'a mut Snapshot,
    id: &execution_contract::AttemptId,
) -> Result<&'a mut AttemptSnapshot, LifecycleError> {
    s.attempt
        .as_mut()
        .filter(|a| &a.id == id)
        .ok_or(LifecycleError::Attempt)
}
fn validate_limits(limits: Limits) -> Result<(), LifecycleError> {
    if limits.max_snapshot_bytes < MIN_SNAPSHOT_BYTES {
        Err(LifecycleError::Configuration)
    } else {
        Ok(())
    }
}

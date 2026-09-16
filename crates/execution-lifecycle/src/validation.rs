use crate::*;
use execution_contract::{Authority, EvidenceKind, FrozenPlan};

pub(crate) fn valid_evidence(
    mode: ExecutionMode,
    kind: EvidenceKind,
    observation: &Observation,
) -> bool {
    if mode == ExecutionMode::Test {
        return kind == EvidenceKind::TestResult;
    }
    match observation {
        Observation::Exited { .. } => kind == EvidenceKind::ProcessExited,
        _ => kind == EvidenceKind::StateObserved,
    }
}
pub(crate) fn validate(p: &FrozenPlan, s: &Snapshot, limits: Limits) -> Result<(), LifecycleError> {
    let bad = || LifecycleError::Snapshot;
    if s.version != 1
        || s.plan_id != p.spec().plan_id
        || &s.plan_digest != p.digest()
        || s.updated_at_unix_ms < s.opened_at_unix_ms
        || s.attempts > p.spec().budget.max_attempts
        || match &s.last_event {
            None => s.revision != 0,
            Some(e) => e.expected_revision.checked_add(1) != Some(s.revision),
        }
    {
        return Err(bad());
    }
    if s.revision == 0
        && (s.preparation != Preparation::Received
            || s.cancel_requested
            || s.attempt.is_some()
            || s.opened_at_unix_ms != s.updated_at_unix_ms)
    {
        return Err(bad());
    }
    match (&s.attempt, s.first_attempt_at_unix_ms) {
        (None, None) if s.attempts == 0 && s.prior_output_bytes == 0 => {}
        (Some(a), Some(first)) => {
            let w = p.spec().validity;
            if s.preparation != Preparation::Prepared
                || a.number != s.attempts
                || a.number == 0
                || first < s.opened_at_unix_ms
                || first < w.not_before_unix_ms
                || a.accepted_at_unix_ms < first
                || a.accepted_at_unix_ms >= w.expires_at_unix_ms
                || a.accepted_at_unix_ms > s.updated_at_unix_ms
                || a.accepted_at_unix_ms - first >= p.spec().budget.total_timeout_ms
                || (a.number == 1 && (first != a.accepted_at_unix_ms || s.prior_output_bytes != 0))
                || s.prior_output_bytes >= p.spec().budget.total_output_bytes
                || (matches!(p.spec().request.authority, Authority::Test { .. })
                    && a.mode != ExecutionMode::Test)
            {
                return Err(bad());
            }
            for o in [&a.termination, &a.assessment].into_iter().flatten() {
                if o.evidence.runner != a.runner
                    || !valid_evidence(a.mode, o.evidence.kind, &o.observation)
                    || o.observed_at_unix_ms < a.accepted_at_unix_ms
                    || o.observed_at_unix_ms > s.updated_at_unix_ms
                {
                    return Err(bad());
                }
            }
            if let Some(t) = &a.termination {
                if !matches!(
                    t.observation,
                    Observation::Exited { .. } | Observation::NeverDispatched
                ) || (matches!(t.observation, Observation::NeverDispatched)
                    && a.dispatch == DispatchState::Dispatched)
                {
                    return Err(bad());
                }
            }
            if let Some(o) = &a.assessment {
                if !matches!(o.observation, Observation::Effect { .. })
                    || a.termination
                        .as_ref()
                        .is_none_or(|t| o.observed_at_unix_ms < t.observed_at_unix_ms)
                {
                    return Err(bad());
                }
            }
        }
        _ => return Err(bad()),
    }
    if serde_json::to_vec(s).map_err(|_| bad())?.len() > limits.max_snapshot_bytes {
        return Err(LifecycleError::Limit);
    }
    Ok(())
}

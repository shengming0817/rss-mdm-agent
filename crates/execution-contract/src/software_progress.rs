//! Append-only checkpoints of one ordered software attempt.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One physical invocation inside the original intent, never a new business attempt.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum SoftwarePhase {
    /// Independent observation before mutation.
    Before,
    /// Exact requested installer or removal.
    Mutation,
    /// Explicit removal before installation, in the original attempt.
    Removal,
    /// Independent proof that the explicit removal ended in absence.
    RemovalAfter,
    /// Distinct approved update command.
    Upgrade,
    /// Acquire the exact read-only image mount and record its ownership.
    Attach,
    /// Validate and copy the selected payload into private staging.
    Stage,
    /// Close only resources belonging to this attempt.
    Cleanup,
    /// Independent observation after mutation.
    After,
}
/// Durable phase boundaries. A begun invocation without an end remains unknown after a crash.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SoftwareCheckpoint {
    /// Must be committed before creating a process or sending a helper dispatch.
    Begin {
        /// Zero-based backend step.
        step: u32,
        /// Exact operation within this step.
        phase: SoftwarePhase,
    },
    /// Physical facts, independent of effect and delivery.
    End {
        /// Actual duration of this physical operation, retained across recovery.
        duration_ms: u64,
        /// Zero-based backend step.
        step: u32,
        /// Matches the preceding Begin exactly.
        phase: SoftwarePhase,
        /// Real process evidence; native in-process detectors need not have a process.
        process: Option<Box<ProcessEvidence>>,
        /// Independent detection, never inferred from installer exit zero.
        detected: Option<SoftwareState>,
        /// All installation activity for this invocation is independently accounted for.
        quiescent: bool,
    },
    /// All required phase facts were durably committed and the step reached its desired state.
    Complete {
        /// Zero-based backend step.
        step: u32,
    },
}
/// Cumulative progress persisted alongside the original attempt in its execution journal.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareProgress {
    /// Original attempt identity.
    pub attempt_id: AttemptId,
    /// Exact frozen program.
    pub content_digest: Digest,
    /// Original runner identity.
    pub runner: Id,
    /// Append-only phase history, at most seventeen records per backend step.
    pub checkpoints: Vec<SoftwareCheckpoint>,
    /// Cumulative elapsed execution time, including preparation and checkpoint waits.
    pub elapsed_ms: u64,
    /// Cumulative observed bytes, including detector output.
    pub output_bytes: u64,
}
impl std::fmt::Debug for SoftwareProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftwareProgress")
            .field("attempt", &self.attempt_id)
            .field("checkpoints", &self.checkpoints.len())
            .field("elapsed_ms", &self.elapsed_ms)
            .field("output_bytes", &self.output_bytes)
            .finish_non_exhaustive()
    }
}
impl SoftwareProgress {
    /// Validate sequencing, identity, per-invocation accounting and cumulative bounds.
    pub fn valid_for(&self, plan: &FrozenExecution) -> bool {
        let Some(program) = plan.spec().execution.software_program() else {
            return false;
        };
        if self.content_digest != *plan.digest()
            || self.checkpoints.len() > program.steps.len() * 17
        {
            return false;
        }
        let mut index = 0usize;
        let mut pending = None;
        let mut phases = vec![SoftwarePhase::Before];
        let mut position = 0;
        let mut can_complete = false;
        let mut quiet = true;
        let mut succeeded = true;
        let mut output = 0u64;
        let mut duration = 0u64;
        for checkpoint in &self.checkpoints {
            let Some(step) = program.steps.get(index) else {
                return false;
            };
            match checkpoint {
                SoftwareCheckpoint::Begin { step: i, phase } => {
                    if !succeeded && matches!(phase, SoftwarePhase::Cleanup | SoftwarePhase::After)
                    {
                        while phases
                            .get(position)
                            .is_some_and(|p| !p.is_observation() && *p != SoftwarePhase::Cleanup)
                        {
                            position += 1;
                        }
                    }
                    if *i as usize != index
                        || pending.is_some()
                        || can_complete
                        || phases.get(position) != Some(phase)
                    {
                        return false;
                    }
                    pending = Some(*phase);
                }
                SoftwareCheckpoint::End {
                    step: i,
                    phase,
                    process,
                    detected,
                    quiescent,
                    duration_ms,
                } => {
                    if *i as usize != index || pending != Some(*phase) {
                        return false;
                    }
                    let Some(total) = duration.checked_add(*duration_ms) else {
                        return false;
                    };
                    duration = total;
                    if duration > self.elapsed_ms {
                        return false;
                    }
                    let invocation = program.invocation(index, *phase);
                    if invocation.is_some() != process.is_some()
                        || process.as_ref().is_some_and(|p| p.quiescent != *quiescent)
                    {
                        return false;
                    }
                    pending = None;
                    position += 1;
                    quiet &= *quiescent;
                    if let Some(p) = process {
                        if p.attempt_id != self.attempt_id
                            || p.content_digest != self.content_digest
                            || p.runner != self.runner
                            || !p.finished
                            || (p.stdout.len() as u64).saturating_add(p.stderr.len() as u64)
                                > invocation.unwrap().output_bytes
                            || (p.stdout.len() as u64).saturating_add(p.stderr.len() as u64)
                                > p.total_output_bytes
                        {
                            return false;
                        }
                        let Some(sum) = output.checked_add(p.total_output_bytes) else {
                            return false;
                        };
                        output = sum;
                    }
                    if phase.is_observation() {
                        let Some(state) = detected else { return false };
                        can_complete = *phase != SoftwarePhase::RemovalAfter
                            && quiet
                            && succeeded
                            && program.satisfied(step, state);
                        if *phase == SoftwarePhase::RemovalAfter
                            && !matches!(state, SoftwareState::Absent {})
                        {
                            return std::ptr::eq(checkpoint, self.checkpoints.last().unwrap())
                                && output == self.output_bytes;
                        }
                        if *phase == SoftwarePhase::Before && !can_complete {
                            phases.extend(program.mutation_phases(step, state));
                            if program.intent != SoftwareOperation::Detect {
                                phases.push(SoftwarePhase::After);
                            }
                        }
                        if matches!(state, SoftwareState::Unknown { .. }) {
                            return std::ptr::eq(checkpoint, self.checkpoints.last().unwrap())
                                && output == self.output_bytes;
                        }
                    } else {
                        if detected.is_some() || process.is_none() {
                            return false;
                        }
                        succeeded &= invocation
                            .unwrap()
                            .succeeded(process.as_deref().unwrap(), step.allow_reboot);
                    }
                    // A non-quiescent operation cannot dispatch another physical mutation.
                    // An independent observation can still be retained by an existing invocation.
                    if !quiescent && !phase.is_observation() {
                        return std::ptr::eq(checkpoint, self.checkpoints.last().unwrap())
                            && output == self.output_bytes;
                    }
                }
                SoftwareCheckpoint::Complete { step: i } => {
                    if *i as usize != index || pending.is_some() || !can_complete {
                        return false;
                    }
                    index += 1;
                    phases = vec![SoftwarePhase::Before];
                    position = 0;
                    can_complete = false;
                    quiet = true;
                    succeeded = true;
                }
            }
        }
        output == self.output_bytes
    }
    /// All begun physical work has ended and acquired resources were verifiably closed.
    /// Desired-state detection alone never releases serialization claims.
    pub fn closed(&self, plan: &FrozenExecution) -> bool {
        if !self.valid_for(plan) {
            return false;
        }
        let mut pending = false;
        let mut mounts = std::collections::BTreeSet::new();
        for checkpoint in &self.checkpoints {
            match checkpoint {
                SoftwareCheckpoint::Begin { step, phase } => {
                    pending = true;
                    if *phase == SoftwarePhase::Attach {
                        mounts.insert(*step);
                    }
                }
                SoftwareCheckpoint::End {
                    step,
                    phase,
                    quiescent,
                    process,
                    ..
                } => {
                    if !quiescent {
                        return false;
                    }
                    pending = false;
                    if *phase == SoftwarePhase::Cleanup
                        && process.as_ref().is_some_and(|p| {
                            p.end == ProcessEnd::Exited
                                && p.exit_code == Some(0)
                                && p.failure_kind == ProcessFailureKind::None
                        })
                    {
                        mounts.remove(step);
                    }
                }
                SoftwareCheckpoint::Complete { .. } => (),
            }
        }
        !pending && mounts.is_empty()
    }
    /// Resume only across durably ended physical operations. Pending or unknown work is never replayed.
    pub fn resumable(&self, plan: &FrozenExecution) -> bool {
        self.valid_for(plan)
            && !self.complete(plan)
            && self.checkpoints.iter().all(|c| {
                !matches!(
                    c,
                    SoftwareCheckpoint::End {
                        quiescent: false,
                        ..
                    } | SoftwareCheckpoint::End {
                        detected: Some(SoftwareState::Unknown { .. }),
                        ..
                    }
                )
            })
            && matches!(
                self.checkpoints.last(),
                Some(SoftwareCheckpoint::Complete { .. } | SoftwareCheckpoint::End { .. })
            )
    }
    /// A replacement may append evidence or increase elapsed accounting, never rewrite history.
    pub fn extends(&self, previous: &Self) -> bool {
        self.attempt_id == previous.attempt_id
            && self.content_digest == previous.content_digest
            && self.runner == previous.runner
            && self.checkpoints.starts_with(&previous.checkpoints)
            && self.elapsed_ms >= previous.elapsed_ms
            && self.output_bytes >= previous.output_bytes
    }
    /// Whether every ordered step has an independently verified completion checkpoint.
    pub fn complete(&self, plan: &FrozenExecution) -> bool {
        self.valid_for(plan)
            && plan.spec().execution.software_program().is_some_and(|p| {
                self.checkpoints
                    .iter()
                    .filter(|c| matches!(c, SoftwareCheckpoint::Complete { .. }))
                    .count()
                    == p.steps.len()
            })
    }
}

impl SoftwarePhase {
    /// Whether this phase independently observes installed state without mutation.
    pub fn is_observation(self) -> bool {
        matches!(self, Self::Before | Self::RemovalAfter | Self::After)
    }
}

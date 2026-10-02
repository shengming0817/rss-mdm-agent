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
    /// A separately numbered resource-only recovery, committed before invoking frozen Cleanup.
    CleanupBegin {
        /// Original backend step.
        step: u32,
        /// Monotonic recovery number, bounded to three.
        sequence: u32,
        /// Slice of the original reserved closure time, never a new business grant.
        timeout_ms: u64,
        /// Slice of the original reserved closure output.
        output_bytes: u64,
    },
    /// Cleanup facts never rewrite a pending or unknown installer operation.
    CleanupEnd {
        /// Original backend step.
        step: u32,
        /// Matches the preceding recovery begin.
        sequence: u32,
        /// Actual measured duration.
        duration_ms: u64,
        /// Real fixed cleanup worker evidence.
        process: Box<ProcessEvidence>,
        /// Owned image/staging resources were verifiably closed.
        resources_closed: bool,
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
            || self.checkpoints.len() > program.steps.len() * 17 + 6
        {
            return false;
        }
        if let Some(start) = self.checkpoints.iter().position(|checkpoint| {
            matches!(
                checkpoint,
                SoftwareCheckpoint::CleanupBegin { .. } | SoftwareCheckpoint::CleanupEnd { .. }
            )
        }) {
            return self.valid_cleanup_tail(plan, start);
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
                SoftwareCheckpoint::CleanupBegin { .. } | SoftwareCheckpoint::CleanupEnd { .. } => {
                    return false
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
                        pending = true;
                    } else {
                        pending = false;
                    }
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
                SoftwareCheckpoint::CleanupBegin { .. } => pending = true,
                SoftwareCheckpoint::CleanupEnd {
                    step,
                    resources_closed,
                    ..
                } => {
                    if *resources_closed {
                        mounts.remove(step);
                        pending = !self.original_activity_ended();
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
            && !self.checkpoints.iter().any(|checkpoint| {
                matches!(
                    checkpoint,
                    SoftwareCheckpoint::CleanupBegin { .. } | SoftwareCheckpoint::CleanupEnd { .. }
                )
            })
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
    /// Remaining reserved cleanup grant. Pending calls debit their full granted slice.
    pub fn cleanup_allowance(&self, plan: &FrozenExecution) -> Option<(u32, u32, u64, u64)> {
        let program = plan.spec().execution.software_program()?;
        let step = self
            .checkpoints
            .iter()
            .rev()
            .find_map(|checkpoint| match checkpoint {
                SoftwareCheckpoint::Begin {
                    step,
                    phase: SoftwarePhase::Attach,
                } => Some(*step),
                _ => None,
            })?;
        let command = program.invocation(step as usize, SoftwarePhase::Cleanup)?;
        let mut elapsed = 0u64;
        let mut output = 0u64;
        let mut pending = None;
        let mut sequence = 0;
        for checkpoint in &self.checkpoints {
            match checkpoint {
                SoftwareCheckpoint::Begin {
                    step: i,
                    phase: SoftwarePhase::Cleanup,
                } if *i == step => {
                    pending = Some((command.timeout_ms / 3, command.output_bytes / 3));
                }
                SoftwareCheckpoint::End {
                    step: i,
                    phase: SoftwarePhase::Cleanup,
                    duration_ms,
                    process,
                    ..
                } if *i == step => {
                    pending = None;
                    elapsed = elapsed.saturating_add(*duration_ms);
                    output = output.saturating_add(
                        process
                            .as_ref()
                            .map_or(0, |process| process.total_output_bytes),
                    );
                }
                SoftwareCheckpoint::CleanupBegin {
                    timeout_ms,
                    output_bytes,
                    sequence: n,
                    ..
                } => {
                    if let Some((time, bytes)) = pending.take() {
                        elapsed = elapsed.saturating_add(time);
                        output = output.saturating_add(bytes);
                    }
                    pending = Some((*timeout_ms, *output_bytes));
                    sequence = *n;
                }
                SoftwareCheckpoint::CleanupEnd {
                    duration_ms,
                    process,
                    ..
                } => {
                    pending = None;
                    elapsed = elapsed.saturating_add(*duration_ms);
                    output = output.saturating_add(process.total_output_bytes);
                }
                _ => (),
            }
        }
        if let Some((time, bytes)) = pending {
            elapsed = elapsed.saturating_add(time);
            output = output.saturating_add(bytes);
        }
        if sequence >= 3 || self.closed(plan) || self.complete(plan) {
            return None;
        }
        let time = command
            .timeout_ms
            .saturating_sub(elapsed)
            .min(command.timeout_ms / 3);
        let bytes = command
            .output_bytes
            .saturating_sub(output)
            .min(command.output_bytes / 3);
        (time > 0 && bytes >= 1024).then_some((step, sequence + 1, time, bytes))
    }
    /// Only ended business operations can be released after resource-only recovery.
    fn original_activity_ended(&self) -> bool {
        let mut pending = None;
        for checkpoint in &self.checkpoints {
            match checkpoint {
                SoftwareCheckpoint::Begin { phase, .. } => pending = Some(*phase),
                SoftwareCheckpoint::End {
                    phase, quiescent, ..
                } => {
                    if !quiescent && *phase != SoftwarePhase::Cleanup {
                        return false;
                    }
                    pending = None;
                }
                SoftwareCheckpoint::CleanupBegin { .. } | SoftwareCheckpoint::CleanupEnd { .. } => {
                    break
                }
                _ => (),
            }
        }
        pending.is_none() || pending == Some(SoftwarePhase::Cleanup)
    }
    fn valid_cleanup_tail(&self, plan: &FrozenExecution, start: usize) -> bool {
        let mut prefix = self.clone();
        prefix.checkpoints.truncate(start);
        let added_output = self.checkpoints[start..]
            .iter()
            .filter_map(|checkpoint| match checkpoint {
                SoftwareCheckpoint::CleanupEnd { process, .. } => Some(process.total_output_bytes),
                _ => None,
            })
            .try_fold(0u64, |sum, bytes| sum.checked_add(bytes));
        let Some(added_output) = added_output else {
            return false;
        };
        let Some(prefix_output) = self.output_bytes.checked_sub(added_output) else {
            return false;
        };
        prefix.output_bytes = prefix_output;
        if !prefix.valid_for(plan) || prefix.complete(plan) {
            return false;
        }
        let added_duration = self.checkpoints[start..]
            .iter()
            .filter_map(|checkpoint| match checkpoint {
                SoftwareCheckpoint::CleanupEnd { duration_ms, .. } => Some(*duration_ms),
                _ => None,
            })
            .try_fold(0u64, |sum, duration| sum.checked_add(duration));
        let Some(added_duration) = added_duration else {
            return false;
        };
        let Some(prefix_elapsed) = self.elapsed_ms.checked_sub(added_duration) else {
            return false;
        };
        prefix.elapsed_ms = prefix_elapsed;
        if !prefix.valid_for(plan) {
            return false;
        }
        let mut pending = None;
        for checkpoint in &self.checkpoints[start..] {
            match checkpoint {
                SoftwareCheckpoint::CleanupBegin {
                    step,
                    sequence,
                    timeout_ms,
                    output_bytes,
                } => {
                    if prefix.cleanup_allowance(plan)
                        != Some((*step, *sequence, *timeout_ms, *output_bytes))
                    {
                        return false;
                    }
                    pending = Some((*step, *sequence, *timeout_ms, *output_bytes));
                }
                SoftwareCheckpoint::CleanupEnd {
                    step,
                    sequence,
                    duration_ms,
                    process,
                    resources_closed,
                } => {
                    let Some((i, n, _, cap)) = pending.take() else {
                        return false;
                    };
                    if *step != i
                        || *sequence != n
                        || process.attempt_id != self.attempt_id
                        || process.content_digest != self.content_digest
                        || process.runner != self.runner
                        || !process.finished
                        || process.total_output_bytes > cap
                        || (process.stdout.len() as u64).saturating_add(process.stderr.len() as u64)
                            > process.total_output_bytes
                        || *duration_ms > self.elapsed_ms
                        || (*resources_closed
                            && !(process.quiescent
                                && process.end == ProcessEnd::Exited
                                && process.exit_code == Some(0)
                                && process.failure_kind == ProcessFailureKind::None))
                    {
                        return false;
                    }
                    prefix.output_bytes = prefix
                        .output_bytes
                        .saturating_add(process.total_output_bytes);
                    prefix.elapsed_ms = prefix.elapsed_ms.saturating_add(*duration_ms);
                }
                _ => return false,
            }
            prefix.checkpoints.push(checkpoint.clone());
        }
        true
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

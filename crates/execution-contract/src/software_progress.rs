//! Append-only checkpoints of one ordered software attempt.
use crate::*;
use serde::{Deserialize, Serialize};

/// One physical invocation inside the original intent, never a new business attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SoftwarePhase {
    /// Independent observation before mutation.
    Before,
    /// Exact requested installer or removal.
    Mutation,
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
    /// Append-only phase history, at most seven records per backend step.
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
        if self.content_digest != *plan.digest() || self.checkpoints.len() > program.steps.len() * 7
        {
            return false;
        }
        let mut step = 0usize;
        let mut pending = None;
        let mut next = SoftwarePhase::Before;
        let mut observed = None;
        let mut can_complete = false;
        let mut closed = false;
        let mut step_quiescent = true;
        let mut mutation_succeeded = true;
        let mut output = 0u64;
        for checkpoint in &self.checkpoints {
            let Some(spec) = program.steps.get(step) else {
                return false;
            };
            match checkpoint {
                SoftwareCheckpoint::Begin { step: index, phase } => {
                    if *index as usize != step
                        || pending.is_some()
                        || *phase != next
                        || can_complete
                        || closed
                    {
                        return false;
                    }
                    pending = Some(*phase);
                }
                SoftwareCheckpoint::End {
                    step: index,
                    phase,
                    process,
                    detected,
                    quiescent,
                } => {
                    if *index as usize != step || pending != Some(*phase) {
                        return false;
                    }
                    let invocation = program.invocation(step, *phase);
                    if invocation.is_some() != process.is_some()
                        || process.as_ref().is_some_and(|p| p.quiescent != *quiescent)
                    {
                        return false;
                    }
                    pending = None;
                    step_quiescent &= *quiescent;
                    if let Some(facts) = process {
                        if facts.attempt_id != self.attempt_id
                            || facts.content_digest != self.content_digest
                            || facts.runner != self.runner
                            || !facts.finished
                            || (facts.stdout.len() as u64).saturating_add(facts.stderr.len() as u64)
                                > invocation.unwrap().output_bytes
                            || (facts.stdout.len() as u64).saturating_add(facts.stderr.len() as u64)
                                > facts.total_output_bytes
                        {
                            return false;
                        }
                        let Some(sum) = output.checked_add(facts.total_output_bytes) else {
                            return false;
                        };
                        output = sum;
                    }
                    match phase {
                        SoftwarePhase::Before | SoftwarePhase::After => {
                            let Some(state) = detected else { return false };
                            observed = Some(state);
                            can_complete = step_quiescent
                                && mutation_succeeded
                                && match program.intent {
                                    SoftwareOperation::Install => {
                                        matches!(state, SoftwareState::Present { version } if *version == spec.version)
                                    }
                                    SoftwareOperation::Uninstall => {
                                        matches!(state, SoftwareState::Absent {})
                                    }
                                    SoftwareOperation::Detect => {
                                        !matches!(state, SoftwareState::Unknown { .. })
                                    }
                                };
                            next = SoftwarePhase::Mutation;
                            closed = *phase == SoftwarePhase::After
                                || program.intent == SoftwareOperation::Detect;
                        }
                        SoftwarePhase::Mutation => {
                            if detected.is_some() || process.is_none() {
                                return false;
                            }
                            next = SoftwarePhase::After;
                            mutation_succeeded =
                                process.as_ref().is_some_and(|p| spec.mutation_succeeded(p));
                        }
                    }
                    // Unknown activity or detection is a hard boundary: the history may end
                    // here, but cannot start another invocation or skip to the next step.
                    if (*phase != SoftwarePhase::Mutation && !quiescent)
                        || matches!(observed, Some(SoftwareState::Unknown { .. }))
                    {
                        return std::ptr::eq(checkpoint, self.checkpoints.last().unwrap())
                            && output == self.output_bytes;
                    }
                }
                SoftwareCheckpoint::Complete { step: index } => {
                    if *index as usize != step || pending.is_some() || !can_complete {
                        return false;
                    }
                    step += 1;
                    next = SoftwarePhase::Before;
                    observed = None;
                    can_complete = false;
                    closed = false;
                    step_quiescent = true;
                    mutation_succeeded = true;
                }
            }
        }
        output == self.output_bytes
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

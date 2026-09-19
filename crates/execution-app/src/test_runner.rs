use crate::*;
use execution_contract::{
    AttemptId, Authority, EvidenceKind, EvidenceRef, FrozenPlan, Id, VersionedRef,
};
use execution_lifecycle::{EffectAssessment, ExecutionMode, Observation, ObservationFacts};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// Explicit deterministic fixtures; none performs target filesystem, process or network work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestScenario {
    /// Quiescent test result followed by a separate satisfied-effect test result.
    Complete,
    /// Remain active until stop is requested; then report quiescence and no effect.
    Wait,
    /// Authoritatively reject before any test work starts.
    RejectBeforeDispatch,
    /// Lose the delivery acknowledgement and retain uncertainty.
    Unknown,
    /// Quiescent no-effect result, eligible for a separately authorized attempt.
    NoEffect,
}
struct Record {
    plan: FrozenPlan,
    scenario: TestScenario,
    cancelled: bool,
    termination: Option<ObservationFacts>,
    assessment: Option<ObservationFacts>,
}
/// Bounded explicit Test runner. Clones share live fixture facts; constructing a new runner loses
/// those facts and must return None on recovery. It never fabricates history from a stored plan.
#[derive(Clone)]
pub struct DeterministicTestRunner {
    id: Id,
    scenario: TestScenario,
    max_records: usize,
    records: Arc<Mutex<BTreeMap<AttemptId, Record>>>,
}
impl DeterministicTestRunner {
    /// Create an empty fixture runner with a nonzero hard bound (at most 4096 attempts).
    pub fn new(id: Id, scenario: TestScenario, max_records: usize) -> Result<Self, Error> {
        if max_records == 0 || max_records > 4096 {
            return Err(Error::Configuration);
        }
        Ok(Self {
            id,
            scenario,
            max_records,
            records: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }
    /// Number of unique first deliveries, including uncertain/rejected fixture deliveries.
    pub fn dispatch_count(&self) -> usize {
        self.records.lock().expect("fixture mutex").len()
    }
}
impl RunnerPort for DeterministicTestRunner {
    fn id(&self) -> Id {
        self.id.clone()
    }
    fn mode(&self) -> ExecutionMode {
        ExecutionMode::Test
    }
    fn dispatch(&self, permit: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
        permit.dispatch(|plan, action| {
            if action.mode() != ExecutionMode::Test
                || !matches!(plan.spec().request.authority, Authority::Test { .. })
                || action.runner() != &self.id
                || action.plan_id() != &plan.spec().plan_id
                || action.plan_digest() != plan.digest()
            {
                return Err(Error::Denied);
            }
            let mut records = self.records.lock().map_err(|_| Error::Unavailable)?;
            if records.contains_key(action.attempt_id()) {
                return Err(Error::Conflict);
            }
            if records.len() >= self.max_records {
                return Err(Error::Capacity);
            }
            records.insert(
                action.attempt_id().clone(),
                Record {
                    plan: plan.clone(),
                    scenario: self.scenario,
                    cancelled: false,
                    termination: None,
                    assessment: None,
                },
            );
            Ok(match self.scenario {
                TestScenario::RejectBeforeDispatch => DispatchOutcome::NeverDispatched,
                TestScenario::Unknown => DispatchOutcome::OutcomeUnknown,
                _ => DispatchOutcome::Accepted,
            })
        })
    }
    fn stop(&self, plan: &FrozenPlan, attempt: &AttemptId) -> Result<(), Error> {
        let mut records = self.records.lock().map_err(|_| Error::Unavailable)?;
        if let Some(record) = records.get_mut(attempt) {
            if record.plan.digest() != plan.digest() {
                return Err(Error::Conflict);
            }
            record.cancelled = true;
        }
        Ok(())
    }
    fn observe(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        stage: ObservationStage,
        now: u64,
    ) -> Result<Option<ObservationFacts>, Error> {
        let mut records = self.records.lock().map_err(|_| Error::Unavailable)?;
        let Some(record) = records.get_mut(attempt) else {
            return Ok(None);
        };
        if record.plan.digest() != plan.digest() {
            return Err(Error::Conflict);
        }
        if record.scenario == TestScenario::Unknown
            || (record.scenario == TestScenario::Wait && !record.cancelled)
        {
            return Ok(None);
        }
        let slot = match stage {
            ObservationStage::Termination => &mut record.termination,
            ObservationStage::Assessment => &mut record.assessment,
        };
        if let Some(facts) = slot {
            return Ok(Some(facts.clone()));
        }
        let observation = match stage {
            ObservationStage::Termination
                if record.scenario == TestScenario::RejectBeforeDispatch =>
            {
                Observation::NeverDispatched {
                    total_output_bytes: 0,
                }
            }
            ObservationStage::Termination => Observation::Exited {
                exit_code: if record.scenario == TestScenario::Complete {
                    0
                } else {
                    1
                },
                total_output_bytes: 0,
            },
            ObservationStage::Assessment => Observation::Effect {
                assessment: if record.scenario == TestScenario::Complete {
                    EffectAssessment::Satisfied
                } else {
                    EffectAssessment::NoEffect
                },
            },
        };
        let facts = ObservationFacts {
            plan_id: plan.spec().plan_id.clone(),
            plan_digest: plan.digest().clone(),
            attempt_id: attempt.clone(),
            evidence: EvidenceRef {
                kind: EvidenceKind::TestResult,
                reference: VersionedRef {
                    id: Id::new(crate::service::key(
                        plan,
                        "test-evidence",
                        &format!("{}:{stage:?}", attempt.as_str()),
                    )?)
                    .map_err(|_| Error::InvalidInput)?,
                    revision: Id::new("1").expect("static ID"),
                },
                runner: self.id.clone(),
            },
            observed_at_unix_ms: now,
            observation,
        };
        *slot = Some(facts.clone());
        Ok(Some(facts))
    }
}

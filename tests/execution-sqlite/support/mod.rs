use execution_admission::Rule;
use execution_admission::*;
use execution_approval::{evaluate, ApprovalLimits, ApprovalVerifier, ProfileApproval};
use execution_contract::*;
use execution_lifecycle::{
    self as lifecycle, Observation, ObservationError, ObservationFacts, ObservationVerifier,
};
use execution_sqlite::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
pub fn reference(s: &str) -> VersionedRef {
    VersionedRef {
        id: id(s),
        revision: id("1"),
    }
}
pub fn operation(s: &str) -> OperationRequestId {
    OperationRequestId::new(s).unwrap()
}
pub fn limits() -> Limits {
    Limits {
        plan: PlanLimits {
            max_input_bytes: 65_536,
            max_depth: 32,
            max_nodes: 4096,
            max_string_bytes: 4096,
            max_collection_items: 128,
            max_timeout_ms: 60_000,
            max_output_bytes: 65_536,
            max_stdin_bytes: 65_536,
            max_attempts: 3,
        },
        lifecycle: lifecycle::Limits {
            max_snapshot_bytes: 16_384,
        },
        interaction: execution_interaction::Limits {
            max_snapshot_bytes: 16_384,
            max_lifetime_ms: 60_000,
        },
        max_approvals: 16,
        max_record_bytes: 131_072,
        max_receipts: 10_000,
        max_database_pages: 32_768,
        max_consumers: 8,
        max_batch: 64,
        busy_timeout_ms: 1000,
    }
}
pub fn plan() -> FrozenPlan {
    FrozenPlan::freeze(
        decode_plan(
            include_bytes!("../../../crates/execution-contract/tests/fixtures/plan.json"),
            &limits().plan,
        )
        .unwrap(),
        &limits().plan,
    )
    .unwrap()
}
pub struct Database {
    pub root: PathBuf,
    pub path: PathBuf,
}
impl Database {
    pub fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "execution-sqlite-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            path: root.join("execution.db"),
            root,
        }
    }
    pub fn create(&self) -> Store {
        Store::initialize_test(
            &self.path,
            plan().spec().request.authority.clone(),
            limits(),
        )
        .unwrap()
    }
    pub fn open(&self) -> Store {
        self.open_with(limits())
    }
    pub fn open_with(&self, limits: Limits) -> Store {
        match Store::open(&self.path, &plan().spec().request.authority, limits).unwrap() {
            OpenOutcome::Ready(s) => *s,
            _ => panic!("current schema"),
        }
    }
    pub fn sql(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.path).unwrap()
    }
    pub fn count(&self, table: &str) -> u64 {
        self.sql()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[derive(Clone)]
pub struct TestHost {
    pub plan: FrozenPlan,
    pub now: Cell<u64>,
    pub calls: Cell<u32>,
    pub entries: Vec<TrustedApproval>,
    pub authorization: VersionedRef,
    pub approval: VersionedRef,
    pub read: bool,
    pub denied: Vec<Access>,
    pub consumer: Option<Id>,
    pub access_calls: RefCell<Vec<(Access, Option<Id>, bool)>>,
    pub write: bool,
    pub audit: bool,
    pub expire_during_admission: bool,
    pub snapshot_clock: Option<u64>,
    pub answer_clock: Option<u64>,
}
impl TestHost {
    pub fn new(records: usize) -> Self {
        let plan = plan();
        let entries = (0..records)
            .map(|n| TrustedApproval {
                state: ApprovalState::Active,
                definition: ApprovalDefinition {
                    reference: reference(&format!("approval-{n}")),
                    approver: ActorId::new(format!("approver-{n}")).unwrap(),
                    plan_id: plan.spec().plan_id.clone(),
                    plan_digest: plan.digest().clone(),
                    profiles: vec![reference(&format!("profile-{n}"))],
                    validity: plan.spec().validity,
                    max_uses: 1,
                },
            })
            .collect();
        Self {
            plan,
            now: Cell::new(1000),
            calls: Cell::new(0),
            entries,
            authorization: reference("authority-epoch"),
            approval: reference("approval-epoch"),
            read: true,
            denied: vec![],
            consumer: None,
            access_calls: RefCell::new(vec![]),
            write: true,
            audit: true,
            expire_during_admission: false,
            snapshot_clock: None,
            answer_clock: None,
        }
    }
    pub fn scope(&self) -> Scope {
        Scope::from_plan(&self.plan)
    }
    pub fn bindings(&self) -> Vec<ProfileApproval> {
        self.entries
            .iter()
            .flat_map(|e| {
                e.definition.profiles.iter().map(|p| ProfileApproval {
                    profile: p.clone(),
                    record: e.definition.reference.clone(),
                })
            })
            .collect()
    }
    pub fn prepare(&self, store: &mut Store) {
        store
            .refresh_trust(&operation("trust"), &self.scope(), None, self)
            .unwrap();
        store
            .open_execution(&operation("open"), &self.plan, self)
            .unwrap();
        store
            .apply_command(
                &operation("prepare"),
                &self.scope(),
                &event("prepare", 0, lifecycle::Command::Prepare),
                &[],
                self,
            )
            .unwrap();
    }
    pub fn begin(&self) -> lifecycle::CommandEvent {
        event(
            "begin",
            1,
            lifecycle::Command::BeginAttempt {
                attempt_id: AttemptId::new("attempt-1").unwrap(),
                runner: id("test-runner"),
                mode: lifecycle::ExecutionMode::Test,
            },
        )
    }
}
impl Host for TestHost {
    fn authorize(&self, request: AccessRequest<'_>) -> Result<(), Error> {
        self.access_calls.borrow_mut().push((
            request.access,
            request.consumer.cloned(),
            request.interaction.is_some(),
        ));
        if self.denied.contains(&request.access) {
            return Err(Error::Denied);
        }
        if request.access == Access::Deliver
            && self
                .consumer
                .as_ref()
                .is_some_and(|c| request.consumer != Some(c))
        {
            return Err(Error::Denied);
        }
        if *request.scope != self.scope() {
            return Err(Error::Denied);
        }
        if request.interaction.is_some() {
            if let Some(now) = self.answer_clock {
                self.now.set(now);
            }
        }
        match request.access {
            Access::ReadResult | Access::Deliver if !self.read => Err(Error::Denied),
            Access::ReadAudit if !self.audit => Err(Error::Denied),
            Access::Create
            | Access::Execute
            | Access::RunnerFact
            | Access::Interact
            | Access::ManageTrust
                if !self.write =>
            {
                Err(Error::Denied)
            }
            _ => Ok(()),
        }
    }
    fn reliable_now(&self) -> Result<u64, Error> {
        Ok(self.now.get())
    }
    fn trusted_snapshot(&self, _: &Scope) -> Result<TrustSnapshot, Error> {
        if let Some(now) = self.snapshot_clock {
            self.now.set(now);
        }
        Ok(TrustSnapshot {
            authorization_revision: self.authorization.clone(),
            approval_revision: self.approval.clone(),
            fresh_until_unix_ms: 2000,
            approvals: self.entries.clone(),
        })
    }
    fn admit(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        bindings: &[ProfileApproval],
        approvals: &dyn ApprovalVerifier,
        _: u64,
    ) -> Result<AdmissionGate, Error> {
        self.calls.set(self.calls.get() + 1);
        let admission = decide(plan, attempt, self, AdmissionLimits { max_rules: 32 });
        let approval = evaluate(
            plan,
            &admission,
            bindings,
            approvals,
            ApprovalLimits {
                max_profiles: 16,
                max_records: 16,
            },
        );
        if self.expire_during_admission {
            self.now.set(2000);
        }
        Ok(AdmissionGate {
            admission,
            approval,
        })
    }
}
impl AuthorityVerifier for TestHost {
    fn verify(&self, _: &FrozenPlan, _: &AttemptId) -> Result<AuthorityFacts, VerificationError> {
        let p = self.plan.spec();
        let rules = if self.entries.is_empty() {
            vec![Rule {
                id: id("allow-rule"),
                template: self.plan.clone(),
                effect: RuleEffect::Allow,
            }]
        } else {
            self.entries
                .iter()
                .flat_map(|e| {
                    e.definition.profiles.iter().map(|profile| Rule {
                        id: profile.id.clone(),
                        template: self.plan.clone(),
                        effect: RuleEffect::ApprovalRequired {
                            profile: profile.clone(),
                        },
                    })
                })
                .collect()
        };
        Ok(AuthorityFacts {
            subject: SubjectFacts {
                authority: p.request.authority.clone(),
                actor: p.request.actor.clone(),
                validity: p.validity,
                budget: p.budget,
            },
            delegation: p.request.delegation.as_ref().map(|r| DelegationFacts {
                reference: r.clone(),
                scope: self.plan.clone(),
            }),
            policy: p.policy.clone(),
            rules,
            now_unix_ms: self.now.get(),
            verification_revision: self.authorization.clone(),
            fresh_until_unix_ms: 2000,
        })
    }
}
pub struct TestEvidence {
    pub observation: Observation,
    pub calls: Cell<u32>,
}
impl TestEvidence {
    pub fn new(observation: Observation) -> Self {
        Self {
            observation,
            calls: Cell::new(0),
        }
    }
}
impl ObservationVerifier for TestEvidence {
    fn verify(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        evidence: &EvidenceRef,
        now: u64,
    ) -> Result<ObservationFacts, ObservationError> {
        self.calls.set(self.calls.get() + 1);
        Ok(ObservationFacts {
            plan_id: plan.spec().plan_id.clone(),
            plan_digest: plan.digest().clone(),
            attempt_id: attempt.clone(),
            evidence: evidence.clone(),
            observed_at_unix_ms: now,
            observation: self.observation.clone(),
        })
    }
}
pub fn event(name: &str, revision: u64, command: lifecycle::Command) -> lifecycle::CommandEvent {
    lifecycle::CommandEvent {
        id: EventId::new(name).unwrap(),
        expected_revision: revision,
        command,
    }
}
pub fn observation_event(
    name: &str,
    revision: u64,
    attempt_id: AttemptId,
    evidence: EvidenceRef,
) -> lifecycle::ObservationEvent {
    lifecycle::ObservationEvent {
        id: EventId::new(name).unwrap(),
        expected_revision: revision,
        attempt_id,
        evidence,
    }
}
pub fn spec(host: &TestHost) -> execution_interaction::Spec {
    use execution_interaction::*;
    Spec {
        id: Reference::new("interaction-1").unwrap(),
        subject: host.scope().interaction_subject(),
        kind: Kind::UserConfirmation {
            purpose: ConfirmationPurpose::Continue,
        },
        expires_at_unix_ms: 1800,
    }
}

use execution_admission::{self as admission, AuthorityVerifier};
use execution_approval::{self as approval, ApprovalVerifier, ProfileApproval};
use execution_contract::*;
use execution_lifecycle as lifecycle;
use execution_sqlite::*;
use std::path::PathBuf;

struct TestHost(FrozenPlan);
fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
fn version() -> VersionedRef {
    VersionedRef {
        id: id("test-authority-epoch"),
        revision: id("1"),
    }
}
impl AuthorityVerifier for TestHost {
    fn verify(
        &self,
        _: &FrozenPlan,
        _: &AttemptId,
    ) -> Result<admission::AuthorityFacts, admission::VerificationError> {
        let p = self.0.spec();
        Ok(admission::AuthorityFacts {
            subject: admission::SubjectFacts {
                authority: p.request.authority.clone(),
                actor: p.request.actor.clone(),
                validity: p.validity,
                budget: p.budget,
            },
            delegation: p
                .request
                .delegation
                .as_ref()
                .map(|r| admission::DelegationFacts {
                    reference: r.clone(),
                    scope: self.0.clone(),
                }),
            policy: p.policy.clone(),
            rules: vec![admission::Rule {
                id: id("test-allow"),
                template: self.0.clone(),
                effect: admission::RuleEffect::Allow,
            }],
            now_unix_ms: p.validity.not_before_unix_ms,
            fresh_until_unix_ms: p.validity.expires_at_unix_ms,
            verification_revision: version(),
        })
    }
}
impl Host for TestHost {
    fn authorize(&self, request: AccessRequest<'_>) -> Result<(), Error> {
        // Explicit test subject only; this is not a production authentication adapter.
        if request.scope == &Scope::from_plan(&self.0)
            && matches!(request.scope.authority, Authority::Test { .. })
        {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
    fn reliable_now(&self) -> Result<u64, Error> {
        Ok(self.0.spec().validity.not_before_unix_ms)
    }
    fn trusted_snapshot(&self, _: &Scope) -> Result<TrustSnapshot, Error> {
        Ok(TrustSnapshot {
            authorization_revision: version(),
            approval_revision: version(),
            fresh_until_unix_ms: self.0.spec().validity.expires_at_unix_ms,
            approvals: vec![],
        })
    }
    fn admit(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        bindings: &[ProfileApproval],
        verifier: &dyn ApprovalVerifier,
        _: u64,
    ) -> Result<AdmissionGate, Error> {
        let admission = admission::decide(
            plan,
            attempt,
            self,
            admission::AdmissionLimits { max_rules: 8 },
        );
        let approval = approval::evaluate(
            plan,
            &admission,
            bindings,
            verifier,
            approval::ApprovalLimits {
                max_profiles: 8,
                max_records: 8,
            },
        );
        Ok(AdmissionGate {
            admission,
            approval,
        })
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_limits = PlanLimits {
        max_input_bytes: 65_536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60_000,
        max_output_bytes: 65_536,
        max_stdin_bytes: 65_536,
        max_attempts: 3,
    };
    let bytes = std::fs::read(
        std::env::args()
            .nth(1)
            .ok_or("test plan fixture required")?,
    )?;
    let plan = FrozenPlan::freeze(decode_plan(&bytes, &plan_limits)?, &plan_limits)?;
    let scope = Scope::from_plan(&plan);
    let host = TestHost(plan.clone());
    let limits = Limits {
        plan: plan_limits,
        lifecycle: lifecycle::Limits {
            max_snapshot_bytes: 16_384,
        },
        interaction: execution_interaction::Limits {
            max_snapshot_bytes: 16_384,
            max_lifetime_ms: 60_000,
        },
        max_approvals: 8,
        max_record_bytes: 131_072,
        max_receipts: 1000,
        max_database_pages: 16_384,
        max_consumers: 8,
        max_batch: 64,
        busy_timeout_ms: 1000,
    };
    let root = std::env::temp_dir()
        .canonicalize()?
        .join(format!("execution-sqlite-consumer-{}", std::process::id()));
    std::fs::create_dir(&root)?;
    let temporary = Temporary(root);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary.0, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = temporary.0.join("execution.db");
    let mut store = Store::initialize_test(&path, scope.authority.clone(), limits)?;
    let op = |name: &str| OperationRequestId::new(name).unwrap();
    store.refresh_trust(&op("trust"), &scope, None, &host)?;
    store.open_execution(&op("open"), &plan, &host)?;
    store.apply_command(
        &op("prepare"),
        &scope,
        &lifecycle::CommandEvent {
            id: EventId::new("prepare")?,
            expected_revision: 0,
            command: lifecycle::Command::Prepare,
        },
        &[],
        &host,
    )?;
    let begin = lifecycle::CommandEvent {
        id: EventId::new("begin")?,
        expected_revision: 1,
        command: lifecycle::Command::BeginAttempt {
            attempt_id: AttemptId::new("test-attempt")?,
            runner: id("test-runner"),
            mode: lifecycle::ExecutionMode::Test,
        },
    };
    let receipt = match store.apply_command(&op("begin"), &scope, &begin, &[], &host)? {
        CommitOutcome::Applied {
            receipt,
            first_dispatch: Some(action),
        } => {
            action.dispatch(|a| assert_eq!(a.mode(), lifecycle::ExecutionMode::Test));
            receipt
        }
        _ => panic!("first successful commit must provide one test action"),
    };
    drop(store);
    let mut store = match Store::open(&path, &scope.authority, limits)? {
        OpenOutcome::Ready(store) => store,
        _ => panic!("current schema expected"),
    };
    let replay = store.apply_command(&op("begin"), &scope, &begin, &[], &host)?;
    assert!(matches!(replay, CommitOutcome::AlreadyCommitted(_)));
    assert_eq!(replay.receipt(), &receipt);
    assert_eq!(
        store
            .execution(&scope, &host)?
            .directive(host.reliable_now()?)?,
        lifecycle::Directive::Reconcile
    );
    let consumer = id("test-consumer");
    assert_eq!(OperationRequestId::new(""), Err(Error::InvalidInput));
    assert_eq!(
        store.pull_results(&scope, &consumer, 0, &host),
        Err(Error::InvalidInput)
    );
    let events = store.pull_results(&scope, &consumer, 64, &host)?;
    assert_eq!(events.len(), 3);
    // Confirm a later event first; an earlier pending result must still be delivered.
    store.confirm(&scope, &consumer, &events[2].event_id, &host)?;
    assert_eq!(
        store.pull_results(&scope, &consumer, 64, &host)?,
        events[..2]
    );
    for result in &events[..2] {
        store.confirm(&scope, &consumer, &result.event_id, &host)?;
    }
    assert!(store.pull_results(&scope, &consumer, 64, &host)?.is_empty());
    assert_eq!(store.receipt(&scope, &op("begin"), &host)?, Some(receipt));
    println!("execution-sqlite: real file transaction, test-only authorization/action, restart replay and result confirmation");
    Ok(())
}

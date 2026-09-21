//! Standalone S1 fixture owner. The only filesystem changes are a private temporary SQLite DB.
use execution_admission::{
    AdmissionLimits, AuthorityFacts, AuthorityVerifier, DelegationFacts, Rule, RuleEffect,
    SubjectFacts, VerificationError,
};
use execution_app::*;
use execution_approval::ProfileApproval;
use execution_capability::*;
use execution_contract::*;
use execution_sqlite::{AccessRequest, TrustSnapshot};

fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
fn reference(value: &str) -> VersionedRef {
    VersionedRef {
        id: id(value),
        revision: id("1"),
    }
}
// This immutable test policy is provisioned by the fixture owner, independently of submissions.
#[derive(Clone)]
struct FixtureHost(FrozenPlan);
impl AuthorityVerifier for FixtureHost {
    fn verify(&self, _: &FrozenPlan, _: &AttemptId) -> Result<AuthorityFacts, VerificationError> {
        let p = self.0.spec();
        Ok(AuthorityFacts {
            subject: SubjectFacts {
                authority: p.request.authority.clone(),
                actor: p.request.actor.clone(),
                validity: p.validity,
                budget: p.budget,
            },
            delegation: p
                .request
                .delegation
                .clone()
                .map(|reference| DelegationFacts {
                    reference,
                    scope: self.0.clone(),
                }),
            policy: p.policy.clone(),
            rules: vec![Rule {
                id: id("fixture-rule"),
                template: self.0.clone(),
                effect: RuleEffect::Allow,
            }],
            now_unix_ms: 1000,
            verification_revision: reference("fixture-policy"),
            fresh_until_unix_ms: 2000,
        })
    }
}
impl AppHost for FixtureHost {
    fn service_binding(&self) -> Result<ServiceBinding, Error> {
        let p = &self.0.spec().request;
        Ok(ServiceBinding {
            authority: p.authority.clone(),
            device: p.target.device.clone(),
        })
    }
    fn authorize(
        &self,
        caller: &RequestContext,
        request: AccessRequest<'_>,
    ) -> Result<(), execution_sqlite::Error> {
        if caller.actor != request.scope.actor {
            return Err(execution_sqlite::Error::Denied);
        }
        self.authorize_service(request)
    }
    fn authorize_service(&self, request: AccessRequest<'_>) -> Result<(), execution_sqlite::Error> {
        let p = &self.0.spec().request;
        if request.scope.authority != p.authority || request.scope.actor != p.actor {
            return Err(execution_sqlite::Error::Denied);
        }
        Ok(())
    }
    fn reliable_now(&self) -> Result<u64, execution_sqlite::Error> {
        Ok(1000)
    }
    fn capabilities(&self, _: &FrozenPlan) -> Result<CapabilitySnapshot, Error> {
        fn inventory<T>(values: Vec<T>) -> Inventory<T> {
            Inventory {
                complete: true,
                entries: values
                    .into_iter()
                    .map(|capability| Entry {
                        capability,
                        availability: Availability::Available,
                    })
                    .collect(),
            }
        }
        let p = self.0.spec();
        Ok(CapabilitySnapshot {
            verified_at_unix_ms: 1000,
            fresh_until_unix_ms: 2000,
            environment: EnvironmentSnapshot {
                authority: p.request.authority.clone(),
                device: p.request.target.device.clone(),
                source: reference("fixture-capabilities"),
                platform: Some(p.request.target.platform),
                interpreters: inventory(vec![p.launch.interpreter.clone()]),
                launch_io: inventory(vec![LaunchIoCapability::CapturedText(TextEncoding::Utf8)]),
                run_as: inventory(vec![p.run_as.clone()]),
                user_sessions: inventory(vec![]),
                isolation: inventory(vec![
                    Isolation::NetworkDenied,
                    Isolation::ReadPaths,
                    Isolation::WritePaths,
                    Isolation::ChildProcessesDenied,
                    Isolation::Sandbox,
                ]),
            },
        })
    }
    fn trusted_snapshot(&self, _: &FrozenPlan) -> Result<TrustSnapshot, execution_sqlite::Error> {
        Ok(TrustSnapshot {
            authorization_revision: reference("fixture-policy"),
            approval_revision: reference("fixture-approval"),
            fresh_until_unix_ms: 2000,
            approvals: vec![],
        })
    }
    fn approval_bindings(&self, _: &FrozenPlan) -> Result<Vec<ProfileApproval>, Error> {
        Ok(vec![])
    }
    fn configuration_change(&self, _: &ConfigChange) -> Result<(), Error> {
        Err(Error::Denied)
    }
}
struct Directory(std::path::PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() {
    let input = std::env::args_os().nth(1).expect("fixture path");
    let limits = test_store_limits();
    let plan = FrozenPlan::freeze(
        decode_plan(&std::fs::read(input).unwrap(), &limits.plan).unwrap(),
        &limits.plan,
    )
    .unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let host = FixtureHost(plan.clone());
    // Establish that the provisioned policy is a real C07 consumer, not a parallel allow decision.
    let admission = execution_admission::decide(
        &plan,
        &AttemptId::new("probe").unwrap(),
        &host,
        AdmissionLimits { max_rules: 8 },
    );
    assert!(matches!(
        admission.outcome(),
        execution_admission::DecisionOutcome::Allowed
    ));
    let path = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("execution-app-consumer-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let directory = Directory(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory.0, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let db = directory.0.join("execution.db");
    let runner =
        DeterministicTestRunner::new(id("fixture-runner"), TestScenario::Complete, 4).unwrap();
    let (response, disconnected_client) = std::sync::mpsc::channel();
    drop(disconnected_client);
    let worker_host = host.clone();
    let worker_runner = runner.clone();
    let worker_db = db.clone();
    let worker_plan = plan.clone();
    let worker_caller = caller.clone();
    std::thread::spawn(move || {
        let mut app = ExecutionApp::start(
            &worker_db,
            Startup::CreateTest,
            worker_host,
            worker_runner,
            AppConfig::test_defaults(1),
        )
        .unwrap();
        let request = &worker_plan.spec().request.request_id;
        let accepted = app.submit(&worker_caller, request, &worker_plan).unwrap();
        assert!(response.send(accepted).is_err()); // transport loss does not own/cancel execution
        assert_eq!(
            app.reconcile(request).unwrap().phase,
            TaskPhase::TestCompleted
        );
    })
    .join()
    .unwrap();
    let mut app = ExecutionApp::start(
        &db,
        Startup::OpenTest,
        host,
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let request = &plan.spec().request.request_id;
    assert_eq!(
        app.submit(&caller, request, &plan).unwrap().phase,
        TaskPhase::TestCompleted
    );
    let results = app
        .pull_results(&caller, request, &id("consumer"), 32)
        .unwrap();
    for result in results {
        app.confirm(&caller, request, &id("consumer"), &result.event_id)
            .unwrap();
    }
    assert!(app
        .pull_results(&caller, request, &id("consumer"), 32)
        .unwrap()
        .is_empty());
    assert_eq!(runner.dispatch_count(), 1);
    println!("execution-app: Test-only submit/reconnect/reconcile/delivery passed");
}

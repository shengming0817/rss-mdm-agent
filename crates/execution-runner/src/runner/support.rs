//! Runner-owned test assembly. No fixture authority is compiled into production.
use super::*;
use execution_admission::{
    AuthorityFacts, AuthorityVerifier, DelegationFacts, Rule, RuleEffect, SubjectFacts,
    VerificationError,
};
use execution_app::{AppHost, CapabilitySnapshot, RequestContext, ServiceBinding};
use execution_capability::{
    Availability, Entry, EnvironmentSnapshot, Inventory, Isolation, LaunchIoCapability,
};
use execution_sqlite::{AccessRequest, TrustSnapshot};
pub fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
fn reference(value: &str) -> VersionedRef {
    VersionedRef {
        id: id(value),
        revision: id("1"),
    }
}
pub fn plan() -> FrozenExecution {
    let original = FrozenExecution::freeze(
        decode_execution(
            include_bytes!("../../../execution-contract/tests/fixtures/plan.json"),
            &execution_app::test_store_limits().plan,
        )
        .unwrap(),
        &execution_app::test_store_limits().plan,
    )
    .unwrap();
    let mut spec = original.spec().clone();
    spec.request.initiator = execution_contract::Initiator::Policy {
        policy: spec.policy.clone(),
    };
    FrozenExecution::freeze(spec, &execution_app::test_store_limits().plan).unwrap()
}
pub fn caller() -> RequestContext {
    RequestContext {
        actor: plan().spec().request.actor.clone(),
    }
}
pub struct Database {
    pub path: std::path::PathBuf,
    root: std::path::PathBuf,
}
impl Database {
    pub fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "runner-app-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        native_process::private_storage::directory(&root).unwrap();
        Self {
            path: root.join("execution.sqlite"),
            root,
        }
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[derive(Clone)]
pub struct TestHost {
    pub template: FrozenExecution,
}
impl TestHost {
    pub fn new() -> Self {
        Self { template: plan() }
    }
    fn now(&self) -> u64 {
        self.template
            .spec()
            .validity
            .not_before_unix_ms
            .saturating_add(1)
            .max(1000)
    }
}
impl AuthorityVerifier for TestHost {
    fn verify(
        &self,
        _: &FrozenExecution,
        _: &AttemptId,
    ) -> Result<AuthorityFacts, VerificationError> {
        let p = self.template.spec();
        Ok(AuthorityFacts {
            verified_origin: p.request.initiator.clone(),
            risk: Some(execution_admission::RiskLevel::One),
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
                    scope: self.template.clone(),
                }),
            policy: p.policy.clone(),
            rules: vec![Rule {
                id: id("runner-test-rule"),
                template: self.template.clone(),
                effect: RuleEffect::Allow,
            }],
            now_unix_ms: self.now(),
            verification_revision: reference("runner-policy"),
            fresh_until_unix_ms: self.now() + 60000,
        })
    }
}
fn inventory<T>(items: Vec<T>) -> Inventory<T> {
    Inventory {
        complete: true,
        entries: items
            .into_iter()
            .map(|capability| Entry {
                capability,
                availability: Availability::Available,
            })
            .collect(),
    }
}
impl AppHost for TestHost {
    fn service_binding(&self) -> Result<ServiceBinding, Error> {
        Ok(ServiceBinding {
            authority: self.template.spec().request.authority.clone(),
            device: self.template.spec().request.target.device.clone(),
        })
    }
    fn authorize(
        &self,
        caller: &RequestContext,
        r: AccessRequest<'_>,
    ) -> Result<(), execution_sqlite::Error> {
        if caller.actor != r.scope.actor {
            return Err(execution_sqlite::Error::Denied);
        }
        self.authorize_service(r)
    }
    fn authorize_service(&self, r: AccessRequest<'_>) -> Result<(), execution_sqlite::Error> {
        if r.scope.authority != self.template.spec().request.authority
            || r.scope.actor != self.template.spec().request.actor
        {
            Err(execution_sqlite::Error::Denied)
        } else {
            Ok(())
        }
    }
    fn reliable_now(&self) -> Result<u64, execution_sqlite::Error> {
        Ok(self.now())
    }
    fn capabilities(&self, _: &FrozenExecution) -> Result<CapabilitySnapshot, Error> {
        let p = self.template.spec();
        Ok(CapabilitySnapshot {
            verified_at_unix_ms: self.now(),
            fresh_until_unix_ms: self.now() + 60000,
            environment: EnvironmentSnapshot {
                authority: p.request.authority.clone(),
                device: p.request.target.device.clone(),
                source: reference("runner-test-capabilities"),
                platform: Some(p.request.target.platform),
                software: execution_capability::Inventory {
                    complete: true,
                    entries: vec![],
                },
                interpreters: inventory(vec![p.launch.interpreter.clone()]),
                launch_io: inventory(vec![
                    LaunchIoCapability::ControlledStdin(TextEncoding::Utf8),
                    LaunchIoCapability::CapturedText(TextEncoding::Utf8),
                ]),
                run_as: inventory(vec![p.run_as.clone()]),
                user_sessions: inventory(match &p.run_as {
                    RunAs::User { account } => vec![account.clone()],
                    _ => vec![],
                }),
                isolation: inventory(vec![
                    Isolation::NetworkDenied,
                    Isolation::NetworkAllowlist,
                    Isolation::ReadPaths,
                    Isolation::WritePaths,
                    Isolation::ChildProcessesDenied,
                    Isolation::Sandbox,
                ]),
            },
        })
    }
    fn trusted_snapshot(
        &self,
        _: &FrozenExecution,
    ) -> Result<TrustSnapshot, execution_sqlite::Error> {
        Ok(TrustSnapshot {
            authorization_revision: reference("runner-policy"),
            approval_revision: reference("runner-approval"),
            fresh_until_unix_ms: self.now() + 60000,
            approvals: vec![],
        })
    }
    fn approval_bindings(
        &self,
        _: &FrozenExecution,
    ) -> Result<Vec<execution_approval::ProfileApproval>, Error> {
        Ok(vec![])
    }
    fn configuration_change(&self, _: &execution_app::ConfigChange) -> Result<(), Error> {
        Ok(())
    }
}
// Explicit fixture-only carrier: retain production's Real/Test authority separation.
#[derive(Clone)]
pub struct TestCarrier(
    pub Arc<NativeRunner>,
    pub Arc<std::sync::atomic::AtomicUsize>,
);
impl RunnerPort for TestCarrier {
    fn software_evidence(
        &self,
        p: &FrozenExecution,
        a: &AttemptId,
        deadline: execution_app::SoftwareObservation<'_>,
    ) -> Result<Option<SoftwareEvidence>, Error> {
        self.0.software_evidence(p, a, deadline)
    }

    fn id(&self) -> Id {
        self.0.id()
    }
    fn mode(&self) -> ExecutionMode {
        ExecutionMode::Test
    }
    fn dispatch(&self, p: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
        let allowance = p.allowance();
        let ownership = p.software_ownership();
        p.dispatch(|plan, action| {
            if action.mode() != ExecutionMode::Test
                || !matches!(plan.spec().request.authority, Authority::Test { .. })
                || action.runner() != &self.0.id
                || action.content_digest() != plan.digest()
                || action.request_id() != &plan.spec().request.request_id
            {
                return Err(Error::Denied);
            }
            self.1.fetch_add(1, Ordering::SeqCst);
            self.0
                .launch(plan, action.attempt_id(), allowance, ownership)
        })
    }
    fn stop(&self, p: &FrozenExecution, a: &AttemptId) -> Result<(), Error> {
        self.0.stop(p, a)
    }
    fn evidence(
        &self,
        p: &FrozenExecution,
        a: &AttemptId,
    ) -> Result<Option<ProcessEvidence>, Error> {
        self.0.evidence(p, a)
    }
    fn acknowledge_capture(&self, p: &FrozenExecution, f: &ProcessEvidence) -> Result<(), Error> {
        self.0.acknowledge_capture(p, f)
    }
    fn observe(
        &self,
        p: &FrozenExecution,
        a: &AttemptId,
        s: ObservationStage,
        n: u64,
    ) -> Result<Option<ObservationFacts>, Error> {
        self.0.observe(p, a, s, n)
    }
}

#![allow(dead_code)]
use execution_app::*;
use execution_contract::*;

use std::sync::{Arc, Mutex};

pub fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
pub fn request(value: &str) -> RequestId {
    RequestId::new(value).unwrap()
}
pub fn reference(value: &str) -> VersionedRef {
    VersionedRef {
        id: id(value),
        revision: id("1"),
    }
}
pub fn plan() -> FrozenExecution {
    let original = FrozenExecution::freeze(
        decode_execution(
            include_bytes!("../../../execution-contract/tests/fixtures/plan.json"),
            &test_execution_limits(),
        )
        .unwrap(),
        &test_execution_limits(),
    )
    .unwrap();
    let mut spec = original.spec().clone();
    spec.request.initiator = execution_contract::Initiator::Policy {
        policy: spec.policy.clone(),
    };
    FrozenExecution::freeze(spec, &test_execution_limits()).unwrap()
}
pub struct Database {
    pub path: std::path::PathBuf,
    root: std::path::PathBuf,
}
impl Database {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "execution-app-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        platform_private_storage::directory(&root).unwrap();
        Self {
            path: root.join("execution.db"),
            root,
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

pub struct State {
    pub risk: Option<RiskLevel>,
    pub accesses: Option<Vec<Access>>,
    pub consumer: Option<Id>,
    pub read_hook_access: Option<Access>,
    pub read_hook: Option<Arc<dyn Fn() + Send + Sync>>,
    pub runner_facts: bool,
    pub bound: bool,
    pub actor: ActorId,
    pub now: u64,
    pub clock_error: bool,
    pub allow: bool,
    pub read: bool,
    pub audit: bool,

    pub admissions: usize,
}
#[derive(Clone)]
pub struct TestHost {
    pub template: FrozenExecution,
    pub state: Arc<Mutex<State>>,
}
impl TestHost {
    pub fn new() -> Self {
        let template = plan();
        Self {
            state: Arc::new(Mutex::new(State {
                risk: Some(RiskLevel::One),
                accesses: None,
                consumer: None,
                read_hook_access: None,
                read_hook: None,
                runner_facts: true,
                bound: true,
                actor: template.spec().request.actor.clone(),
                now: 1000,
                clock_error: false,
                allow: true,
                read: true,
                audit: true,

                admissions: 0,
            })),
            template,
        }
    }
}

impl AppHost for TestHost {
    fn service_binding(&self) -> Result<ServiceBinding, execution_app::Error> {
        let s = self.state.lock().unwrap();
        if !s.bound {
            return Err(execution_app::Error::Unbound);
        }
        Ok(ServiceBinding {
            authority: self.template.spec().request.authority.clone(),
            device: self.template.spec().request.target.device.clone(),
        })
    }
    fn authorize(
        &self,
        caller: &RequestContext,
        r: AccessRequest<'_>,
    ) -> Result<(), execution_app::JournalError> {
        if caller.actor != r.scope.actor {
            return Err(execution_app::JournalError::Denied);
        }
        self.authorize_service(r)
    }
    fn authorize_service(&self, r: AccessRequest<'_>) -> Result<(), execution_app::JournalError> {
        let mut s = self.state.lock().unwrap();
        if !s.bound
            || r.scope.authority != self.template.spec().request.authority
            || (r.access == Access::ReadResult && !s.read)
            || (r.access == Access::ReadAudit && !s.audit)
            || (r.access == Access::RunnerFact && !s.runner_facts)
            || s.accesses
                .as_ref()
                .is_some_and(|grants| !grants.contains(&r.access))
            || (r.access == Access::Deliver
                && s.consumer.as_ref().is_some_and(|id| r.consumer != Some(id)))
        {
            return Err(execution_app::JournalError::Denied);
        }
        let hook = if s.read_hook_access.map_or(
            matches!(
                r.access,
                Access::ReadResult | Access::Execute | Access::RunnerFact
            ),
            |access| access == r.access,
        ) {
            s.read_hook.take()
        } else {
            None
        };
        drop(s);
        if let Some(hook) = hook {
            hook();
        }
        Ok(())
    }
    fn reliable_now(&self) -> Result<u64, execution_app::JournalError> {
        let state = self.state.lock().unwrap();
        if state.clock_error {
            Err(execution_app::JournalError::Clock)
        } else {
            Ok(state.now)
        }
    }

    fn admit(
        &self,
        plan: &FrozenExecution,
        _: &AttemptId,
        _now: u64,
    ) -> Result<execution_app::AdmissionGate, execution_app::JournalError> {
        let mut s = self.state.lock().unwrap();
        s.admissions += 1;
        if !s.allow {
            return Err(execution_app::JournalError::Denied);
        }
        Ok(execution_app::AdmissionGate {
            execution_gate: if s.actor != plan.spec().request.actor
                || plan.digest() != self.template.digest()
            {
                ExecutionGate::Blocked
            } else if matches!(plan.spec().request.initiator, Initiator::Ai { .. }) {
                ai_execution_gate(s.risk)
            } else {
                ExecutionGate::Direct
            },
            valid_until_unix_ms: 2000.min(plan.spec().validity.expires_at_unix_ms),
        })
    }
}

pub fn caller() -> RequestContext {
    RequestContext {
        actor: plan().spec().request.actor.clone(),
    }
}

/// Create a fixture journal, then attach it through the real application port.
pub fn create_app<H: AppHost, R: RunnerPort>(
    path: &std::path::Path,
    host: H,
    runner: R,
    config: AppConfig,
) -> Result<ExecutionApp<H, R, execution_sqlite::Store>, execution_app::Error> {
    let binding = host.service_binding()?;
    let journal = execution_sqlite::Store::initialize_test(
        path,
        binding.authority,
        execution_sqlite::test_store_limits(),
    )?;
    ExecutionApp::new(journal, host, runner, config)
}
/// Reopen a fixture journal without create fallback or first-dispatch reconstruction.
pub fn reopen_app<H: AppHost, R: RunnerPort>(
    path: &std::path::Path,
    host: H,
    runner: R,
    config: AppConfig,
) -> Result<ExecutionApp<H, R, execution_sqlite::Store>, execution_app::Error> {
    let binding = host.service_binding()?;
    let journal = match execution_sqlite::Store::open(
        path,
        &binding.authority,
        execution_sqlite::test_store_limits(),
    )? {
        execution_sqlite::OpenOutcome::Ready(journal) => *journal,
        execution_sqlite::OpenOutcome::UnsupportedSchema { found, supported } => {
            return Err(execution_app::Error::UnsupportedSchema { found, supported });
        }
    };
    ExecutionApp::new(journal, host, runner, config)
}

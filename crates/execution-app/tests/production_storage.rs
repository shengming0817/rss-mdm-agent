//! Storage-boundary tests only; these hosts do not prove OS identity or execution.
mod support;
use execution_app::*;
use execution_contract::*;
use execution_lifecycle::{ExecutionMode, ObservationFacts};
use support::*;

struct NoDispatch;
impl RunnerPort for NoDispatch {
    fn id(&self) -> Id {
        id("storage-boundary-only")
    }
    fn mode(&self) -> ExecutionMode {
        ExecutionMode::Real
    }
    fn dispatch(&self, _: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
        panic!("storage tests must never dispatch")
    }
    fn evidence(
        &self,
        _: &FrozenExecution,
        _: &AttemptId,
    ) -> Result<Option<ProcessEvidence>, Error> {
        Err(Error::Unsupported)
    }
    fn acknowledge_capture(&self, _: &FrozenExecution, _: &ProcessEvidence) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn stop(&self, _: &FrozenExecution, _: &AttemptId) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn observe(
        &self,
        _: &FrozenExecution,
        _: &AttemptId,
        _: ObservationStage,
        _: u64,
    ) -> Result<Option<ObservationFacts>, Error> {
        Err(Error::Unsupported)
    }
}
fn host(tenant: &str) -> TestHost {
    let mut host = TestHost::new();
    let mut spec = host.template.spec().clone();
    spec.request.authority = Authority::Enterprise {
        id: id("registered-device-authority"),
        tenant: id(tenant),
    };
    host.template = FrozenExecution::freeze(spec, &test_store_limits().input).unwrap();
    host
}
fn start(
    db: &Database,
    mode: ProductionStartup,
    tenant: &str,
) -> Result<ExecutionApp<TestHost, NoDispatch>, Error> {
    ExecutionApp::start_production(
        &db.path,
        mode,
        host(tenant),
        NoDispatch,
        AppConfig::test_defaults(1),
        test_store_limits(),
    )
}

#[test]
fn production_journal_requires_explicit_create_and_preserves_identity_on_reopen() {
    let db = Database::new();
    assert!(start(&db, ProductionStartup::Open, "tenant-a").is_err());
    assert!(!db.path.exists());
    drop(start(&db, ProductionStartup::Create, "tenant-a").unwrap());
    let original = std::fs::read(&db.path).unwrap();
    assert!(start(&db, ProductionStartup::Create, "tenant-a").is_err());
    assert!(start(&db, ProductionStartup::Open, "tenant-b").is_err());
    assert_eq!(original, std::fs::read(&db.path).unwrap());
    drop(start(&db, ProductionStartup::Open, "tenant-a").unwrap());
}

#[test]
fn production_rejects_test_identity_and_test_runner_before_touching_storage() {
    let db = Database::new();
    assert!(matches!(
        ExecutionApp::start_production(
            &db.path,
            ProductionStartup::Create,
            TestHost::new(),
            NoDispatch,
            AppConfig::test_defaults(1),
            test_store_limits()
        ),
        Err(Error::Unbound)
    ));
    let runner = DeterministicTestRunner::new(id("fixture"), TestScenario::Complete, 1).unwrap();
    assert!(matches!(
        ExecutionApp::start_production(
            &db.path,
            ProductionStartup::Create,
            host("tenant-a"),
            runner,
            AppConfig::test_defaults(1),
            test_store_limits()
        ),
        Err(Error::Unbound)
    ));
    assert!(!db.path.exists());
}

#[test]
fn production_cannot_open_or_relabel_existing_test_journal() {
    let db = Database::new();
    let h = TestHost::new();
    drop(
        execution_sqlite::Store::initialize_test(
            &db.path,
            h.template.spec().request.authority.clone(),
            test_store_limits(),
        )
        .unwrap(),
    );
    let original = std::fs::read(&db.path).unwrap();
    assert!(start(&db, ProductionStartup::Open, "tenant-a").is_err());
    assert_eq!(original, std::fs::read(&db.path).unwrap());
}

#[test]
fn production_preserves_unsupported_schema_without_migration_or_recreation() {
    let db = Database::new();
    drop(start(&db, ProductionStartup::Create, "tenant-a").unwrap());
    db.sql().pragma_update(None, "user_version", 999).unwrap();
    let original = std::fs::read(&db.path).unwrap();
    assert!(matches!(
        start(&db, ProductionStartup::Open, "tenant-a"),
        Err(Error::UnsupportedSchema { found: 999, .. })
    ));
    assert_eq!(original, std::fs::read(&db.path).unwrap());
}

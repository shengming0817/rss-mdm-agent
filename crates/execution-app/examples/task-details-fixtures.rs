//! Regenerable UI evidence through the real S1 application, deterministic runner and SQLite.
#[path = "../tests/support/mod.rs"]
mod support;
use execution_app::*;
use support::*;
fn main() {
    let mut fixtures = serde_json::Map::new();
    for (name, scenario, approval, reconcile, cancel) in [
        ("software", TestScenario::Wait, false, false, false),
        ("running", TestScenario::Wait, false, false, false),
        ("approvalRequired", TestScenario::Wait, true, false, false),
        ("outcomeUnknown", TestScenario::Unknown, false, true, false),
        ("verified", TestScenario::Complete, false, true, false),
        ("cancelled", TestScenario::Wait, false, true, true),
    ] {
        let db = Database::new();
        let mut host = TestHost::new();
        let p = if name == "software" {
            execution_contract::FrozenExecution::freeze(
                execution_contract::decode_execution(
                    include_bytes!("../../execution-contract/tests/fixtures/software.json"),
                    &test_store_limits().plan,
                )
                .unwrap(),
                &test_store_limits().plan,
            )
            .unwrap()
        } else {
            plan()
        };
        host.template = p.clone();
        host.state.lock().unwrap().approval = approval;
        let runner = DeterministicTestRunner::new(id("test-runner"), scenario, 16).unwrap();
        let mut app = ExecutionApp::start(
            &db.path,
            Startup::CreateTest,
            host,
            runner,
            AppConfig::test_defaults(1),
        )
        .unwrap();
        let request = &p.spec().request.request_id;
        app.request_execution(&caller(), request, &p).unwrap();
        if cancel {
            app.cancel(&caller(), request).unwrap();
        }
        if reconcile {
            app.reconcile(request).unwrap();
        }
        fixtures.insert(
            name.into(),
            serde_json::to_value(app.task_details(&caller(), request).unwrap()).unwrap(),
        );
    }
    println!("{}", serde_json::to_string_pretty(&fixtures).unwrap());
}

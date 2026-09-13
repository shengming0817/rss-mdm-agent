use execution_capability::*;
use execution_contract::*;
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(std::env::args().nth(1).ok_or("test plan path required")?)?;
    let limits = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_attempts: 3,
    };
    let plan = FrozenPlan::freeze(decode_plan(&bytes, &limits)?, &limits)?;
    let spec = plan.spec();
    let mut snapshot = EnvironmentSnapshot {
        device: spec.request.target.device.clone(),
        source: VersionedRef {
            id: Id::new("fixed-test-snapshot")?,
            revision: Id::new("1")?,
        },
        platform: Some(spec.request.target.platform),
        interpreters: inventory(vec![spec.launch.interpreter.clone()]),
        run_as: inventory(vec![spec.run_as.clone()]),
        user_sessions: inventory(match &spec.session_requirement {
            SessionRequirement::NotRequired {} => vec![],
            SessionRequirement::ActiveUser { account } => vec![account.clone()],
        }),
        isolation: inventory(vec![
            Isolation::NetworkDenied,
            Isolation::NetworkAllowlist,
            Isolation::ReadPaths,
            Isolation::WritePaths,
            Isolation::ChildProcessesDenied,
            Isolation::Sandbox,
        ]),
    };
    let bounds = MatchLimits { max_entries: 64 };
    assert_eq!(
        match_capabilities(&plan, &snapshot, bounds)?.status,
        MatchStatus::Supported
    );
    snapshot.interpreters.entries.clear();
    snapshot.interpreters.complete = false;
    assert_eq!(
        match_capabilities(&plan, &snapshot, bounds)?.status,
        MatchStatus::Unknown
    );
    snapshot.interpreters.complete = true;
    assert_eq!(
        match_capabilities(&plan, &snapshot, bounds)?.status,
        MatchStatus::Unsupported
    );
    println!("execution-capability: fixed test snapshots only; no platform probing, runner or OS isolation proof");
    Ok(())
}

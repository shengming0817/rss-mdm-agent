use execution_contract::{decode_plan, Digest, FrozenPlan, PlanLimits};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let bytes = std::fs::read(args.next().ok_or("plan fixture path required")?)?;
    let expected = std::fs::read_to_string(args.next().ok_or("expected digest path required")?)?;
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
    assert!(plan.matches_digest(&Digest::new(expected.trim())?));
    let mut changed = plan.spec().clone();
    changed.budget.timeout_ms += 1;
    assert_ne!(
        FrozenPlan::freeze(changed, &limits)?.digest(),
        plan.digest()
    );
    println!("execution-contract: independent consumer verified plan digest and budget binding; no runner invoked");
    Ok(())
}

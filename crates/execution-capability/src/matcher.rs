use crate::*;
use execution_contract::{
    Constraints, ExecutionRequest, FrozenPlan, LaunchSpec, NetworkAccess, OutputSpec, PlanSpec,
    SessionRequirement, StandardInput, Target,
};

fn validate<T: PartialEq>(
    inventory: &Inventory<T>,
    dimension: Dimension,
) -> Result<(), MatchError> {
    for (index, entry) in inventory.entries.iter().enumerate() {
        if inventory.entries[..index]
            .iter()
            .any(|previous| previous.capability == entry.capability)
        {
            return Err(MatchError::Duplicate(dimension));
        }
    }
    Ok(())
}
fn lookup<T: PartialEq>(inventory: &Inventory<T>, required: &T) -> MatchStatus {
    match inventory.entries.iter().find(|e| &e.capability == required) {
        Some(Entry {
            availability: Availability::Available,
            ..
        }) => MatchStatus::Supported,
        Some(Entry {
            availability: Availability::Blocked,
            ..
        }) => MatchStatus::Blocked,
        None if inventory.complete => MatchStatus::Unsupported,
        None => MatchStatus::Unknown,
    }
}
// Every inventory enters through this one path for budgeting, validation and lookup.
fn checked_inventory<'a, T: PartialEq>(
    inventory: &'a Inventory<T>,
    dimension: Dimension,
    remaining: &mut usize,
) -> Result<impl Fn(&T) -> MatchStatus + 'a, MatchError> {
    *remaining = remaining
        .checked_sub(inventory.entries.len())
        .ok_or(MatchError::Limit)?;
    validate(inventory, dimension)?;
    Ok(move |required: &T| lookup(inventory, required))
}
/// Check every requirement from the immutable plan. There is no caller override for session
/// or isolation requirements. Invalid snapshots return an error instead of a partial success.
pub fn match_capabilities(
    plan: &FrozenPlan,
    snapshot: &EnvironmentSnapshot,
    limits: MatchLimits,
) -> Result<MatchReport, MatchError> {
    if limits.max_entries == 0 {
        return Err(MatchError::Configuration);
    }
    // No rest patterns: future snapshot/plan/constraint fields require an explicit
    // capability decision. ref: Rust Reference, patterns.html#struct-patterns
    let EnvironmentSnapshot {
        authority,
        device,
        source,
        platform,
        interpreters,
        launch_io,
        run_as: identities,
        user_sessions,
        isolation,
    } = snapshot;
    let PlanSpec {
        schema_version: _,
        plan_id: _, // Validated version and correlation, not capabilities.
        request,
        launch,
        run_as,
        session_requirement,
        constraints,
        budget: _,
        validity: _,
        policy: _, // Admission and lifecycle own these limits.
    } = plan.spec();
    let ExecutionRequest {
        schema_version: _,
        request_id: _, // Validated version and correlation.
        actor: _,
        initiator: _,
        delegation: _, // Authentication/admission own authority proof.
        operation: _,
        parameters: _, // Catalog/runner own operation semantics.
        authority: required_authority,
        target,
    } = request;
    let Target {
        device: required_device,
        platform: required_platform,
        scope: _,
    } = target;
    // Scope consistency is frozen; runtime identity/session support is checked below.
    let LaunchSpec {
        interpreter,
        artifact: _,
        argv: _,
        cwd: _,
        env: _,
        artifact_encoding: _, // Exact byte verification belongs to artifact materialization.
        stdin,
        output,
    } = launch;
    // Artifact and process inputs are enforced by the runner, not inventory facts.
    let Constraints {
        network,
        read_paths: _,
        write_paths: _,
        allow_child_processes,
        require_sandbox,
    } = constraints;
    // Path contents are enforced by the runner; both confinement mechanisms are mandatory.
    let mut remaining = limits.max_entries;
    let interpreters = checked_inventory(interpreters, Dimension::Interpreter, &mut remaining)?;
    let launch_io = checked_inventory(launch_io, Dimension::LaunchIo, &mut remaining)?;
    let identities = checked_inventory(identities, Dimension::RunAs, &mut remaining)?;
    let user_sessions = checked_inventory(user_sessions, Dimension::UserSession, &mut remaining)?;
    let isolation = checked_inventory(isolation, Dimension::Isolation, &mut remaining)?;
    let target_matches = authority == required_authority && device == required_device;
    let mut checks = Vec::new();
    let mut push = |dimension, status| {
        checks.push(CapabilityCheck {
            dimension,
            status: if target_matches {
                status
            } else {
                MatchStatus::Unknown
            },
        })
    };
    push(Dimension::Target, MatchStatus::Supported);
    push(
        Dimension::Platform,
        match platform {
            Some(p) if p == required_platform => MatchStatus::Supported,
            Some(_) => MatchStatus::Unsupported,
            None => MatchStatus::Unknown,
        },
    );
    push(Dimension::Interpreter, interpreters(interpreter));
    match stdin {
        StandardInput::Closed {} => {}
        StandardInput::Controlled {
            reference: _,
            encoding,
            max_bytes: _,
        } => {
            push(
                Dimension::StandardInput,
                launch_io(&LaunchIoCapability::ControlledStdin(*encoding)),
            );
        }
    }
    let OutputSpec { stdout, stderr } = output;
    push(
        Dimension::StandardOutput,
        launch_io(&LaunchIoCapability::CapturedText(*stdout)),
    );
    push(
        Dimension::StandardError,
        launch_io(&LaunchIoCapability::CapturedText(*stderr)),
    );
    push(Dimension::RunAs, identities(run_as));
    match session_requirement {
        SessionRequirement::NotRequired {} => {}
        SessionRequirement::ActiveUser { account } => {
            push(Dimension::UserSession, user_sessions(account))
        }
    }
    let network = match network {
        NetworkAccess::Denied {} => Isolation::NetworkDenied,
        NetworkAccess::Allowlist { destinations: _ } => Isolation::NetworkAllowlist,
    };
    for (dimension, required) in [
        (Dimension::Network, network),
        (Dimension::ReadPaths, Isolation::ReadPaths),
        (Dimension::WritePaths, Isolation::WritePaths),
    ] {
        push(dimension, isolation(&required));
    }
    if !*allow_child_processes {
        push(
            Dimension::ChildProcesses,
            isolation(&Isolation::ChildProcessesDenied),
        );
    }
    if *require_sandbox {
        push(Dimension::Sandbox, isolation(&Isolation::Sandbox));
    }
    let status = checks
        .iter()
        .map(|c| c.status)
        .max()
        .unwrap_or(MatchStatus::Unknown);
    Ok(MatchReport {
        plan_digest: plan.digest().clone(),
        snapshot: source.clone(),
        status,
        checks,
    })
}

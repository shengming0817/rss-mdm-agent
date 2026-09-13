use crate::*;
use execution_contract::{FrozenPlan, NetworkAccess, SessionRequirement};

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
    let counts = [
        snapshot.interpreters.entries.len(),
        snapshot.run_as.entries.len(),
        snapshot.user_sessions.entries.len(),
        snapshot.isolation.entries.len(),
    ];
    if counts
        .into_iter()
        .try_fold(0usize, |sum, n| sum.checked_add(n))
        .is_none_or(|n| n > limits.max_entries)
    {
        return Err(MatchError::Limit);
    }
    validate(&snapshot.interpreters, Dimension::Interpreter)?;
    validate(&snapshot.run_as, Dimension::RunAs)?;
    validate(&snapshot.user_sessions, Dimension::UserSession)?;
    validate(&snapshot.isolation, Dimension::Isolation)?;
    let spec = plan.spec();
    let target_matches = snapshot.device == spec.request.target.device;
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
        match snapshot.platform {
            Some(p) if p == spec.request.target.platform => MatchStatus::Supported,
            Some(_) => MatchStatus::Unsupported,
            None => MatchStatus::Unknown,
        },
    );
    push(
        Dimension::Interpreter,
        lookup(&snapshot.interpreters, &spec.launch.interpreter),
    );
    push(Dimension::RunAs, lookup(&snapshot.run_as, &spec.run_as));
    if let SessionRequirement::ActiveUser { account } = &spec.session_requirement {
        push(
            Dimension::UserSession,
            lookup(&snapshot.user_sessions, account),
        );
    }
    let network = match spec.constraints.network {
        NetworkAccess::Denied {} => Isolation::NetworkDenied,
        NetworkAccess::Allowlist { .. } => Isolation::NetworkAllowlist,
    };
    for (dimension, isolation) in [
        (Dimension::Network, network),
        (Dimension::ReadPaths, Isolation::ReadPaths),
        (Dimension::WritePaths, Isolation::WritePaths),
    ] {
        push(dimension, lookup(&snapshot.isolation, &isolation));
    }
    if !spec.constraints.allow_child_processes {
        push(
            Dimension::ChildProcesses,
            lookup(&snapshot.isolation, &Isolation::ChildProcessesDenied),
        );
    }
    if spec.constraints.require_sandbox {
        push(
            Dimension::Sandbox,
            lookup(&snapshot.isolation, &Isolation::Sandbox),
        );
    }
    let status = checks
        .iter()
        .map(|c| c.status)
        .max()
        .unwrap_or(MatchStatus::Unknown);
    Ok(MatchReport {
        plan_digest: plan.digest().clone(),
        snapshot: snapshot.source.clone(),
        status,
        checks,
    })
}

use crate::*;
use execution_contract::{Authority, EvidenceKind, EvidenceRef, TargetScope};
use std::ops::ControlFlow::{self, Break, Continue};

fn evidence(authority: &Authority, fact: &EvidenceRef, state: bool) -> Result<(), DecisionError> {
    let valid = match authority {
        Authority::Test { .. } => fact.kind == EvidenceKind::TestResult,
        Authority::Local { .. } | Authority::Enterprise { .. } => {
            fact.kind == EvidenceKind::StateObserved
                || (!state && fact.kind == EvidenceKind::ProcessExited)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(DecisionError::Evidence)
    }
}
fn bounds(
    i: &SoftwareIntent,
    s: &PlanningSnapshot,
    limits: PlanningLimits,
) -> Result<(), DecisionError> {
    if limits.max_text_bytes == 0 || limits.max_capabilities == 0 {
        return Err(DecisionError::Limits);
    }
    let bounded = |v: &PackageValue| {
        if v.as_str().len() <= limits.max_text_bytes {
            Ok(())
        } else {
            Err(DecisionError::Bound)
        }
    };
    for package in [&i.package, &s.package] {
        for v in [&package.package, &package.architecture, &package.variant] {
            bounded(v)?;
        }
    }
    if let DesiredState::Present { version, .. } = &i.desired {
        bounded(version)?;
    }
    if let Detection::Present { version, .. } = &s.detection {
        bounded(version)?;
    }
    if let Some(c) = &s.comparison {
        bounded(&c.installed)?;
        bounded(&c.desired)?;
    }
    if s.installer.operations.len() > limits.max_capabilities {
        return Err(DecisionError::Bound);
    }
    for (index, kind) in s.installer.operations.iter().enumerate() {
        if s.installer.operations[..index].contains(kind) {
            return Err(DecisionError::Duplicate);
        }
    }
    Ok(())
}
fn validate(
    i: &SoftwareIntent,
    s: &PlanningSnapshot,
    limits: PlanningLimits,
) -> Result<(), DecisionError> {
    bounds(i, s, limits)?;
    for (mismatch, reason) in [
        (i.authority != s.authority, DecisionError::Authority),
        (i.target != s.target, DecisionError::Target),
        (i.policy != s.policy, DecisionError::Policy),
        (i.package != s.package, DecisionError::Package),
        (
            s.installer.manager != i.package.manager,
            DecisionError::Manager,
        ),
    ] {
        if mismatch {
            return Err(reason);
        }
    }
    if let TargetScope::User { account } = &i.target.scope {
        if account.platform != i.target.platform {
            return Err(DecisionError::TargetPlatform);
        }
    }
    match &s.detection {
        Detection::Absent { evidence: fact } | Detection::Present { evidence: fact, .. } => {
            evidence(&s.authority, fact, true)?
        }
        Detection::Needed(_) | Detection::Indeterminate => (),
    }
    if let Readiness::PendingRestart { evidence: fact } = &s.readiness {
        evidence(&s.authority, fact, false)?;
    }
    if let Some(c) = &s.comparison {
        let (
            Detection::Present {
                version: installed, ..
            },
            DesiredState::Present {
                version: desired, ..
            },
        ) = (&s.detection, &i.desired)
        else {
            return Err(DecisionError::ComparisonContext);
        };
        if &c.installed != installed
            || &c.desired != desired
            || (installed == desired && c.relation != VersionRelation::Equal)
        {
            return Err(DecisionError::ComparisonOperands);
        }
    }
    Ok(())
}
fn block(reason: BlockReason) -> DecisionOutcome {
    DecisionOutcome::Blocked(reason)
}
fn next_change(
    i: &SoftwareIntent,
    s: &PlanningSnapshot,
) -> ControlFlow<DecisionOutcome, MutationKind> {
    if s.management.required && i.desired == DesiredState::Absent {
        return Break(block(BlockReason::Required));
    }
    if let Readiness::PendingRestart { evidence } = &s.readiness {
        return Break(if s.management.allow_restart {
            DecisionOutcome::RequireRestart {
                evidence: evidence.clone(),
            }
        } else {
            block(BlockReason::Restart)
        });
    }
    match (&s.detection, &i.desired) {
        (Detection::Needed(cause), _) => Break(if s.installer.can_detect {
            DecisionOutcome::Detect(*cause)
        } else {
            block(BlockReason::DetectionUnavailable)
        }),
        (Detection::Indeterminate, _) => Break(block(BlockReason::Indeterminate)),
        (Detection::Absent { evidence }, DesiredState::Absent) => {
            Break(DecisionOutcome::Satisfied {
                evidence: evidence.clone(),
                ownership: None,
            })
        }
        (Detection::Absent { .. }, DesiredState::Present { .. }) => Continue(MutationKind::Install),
        (Detection::Present { .. }, DesiredState::Absent) => Continue(MutationKind::Uninstall),
        (
            Detection::Present {
                version: installed,
                evidence,
                ownership,
                ..
            },
            DesiredState::Present {
                version: desired, ..
            },
        ) => {
            let relation = if installed == desired {
                Some(VersionRelation::Equal)
            } else {
                s.comparison.as_ref().map(|c| c.relation)
            };
            match relation {
                Some(VersionRelation::Equal) => Break(DecisionOutcome::Satisfied {
                    evidence: evidence.clone(),
                    ownership: Some(*ownership),
                }),
                Some(VersionRelation::Older) => Continue(MutationKind::Upgrade),
                Some(VersionRelation::Newer) => Continue(MutationKind::Downgrade),
                Some(VersionRelation::Incomparable) => {
                    Break(block(BlockReason::VersionIncomparable))
                }
                None => Break(block(BlockReason::VersionUnknown)),
            }
        }
    }
}
fn removal(kind: MutationKind, s: &PlanningSnapshot) -> bool {
    kind == MutationKind::Uninstall
        || (matches!(kind, MutationKind::Upgrade | MutationKind::Downgrade)
            && s.installer.upgrade_strategy == UpgradeStrategy::UninstallThenInstall)
}
fn installed_guards(kind: MutationKind, s: &PlanningSnapshot) -> Result<(), BlockReason> {
    let Detection::Present {
        ownership,
        dependencies,
        ..
    } = &s.detection
    else {
        return Ok(());
    };
    match ownership {
        Ownership::Unknown => return Err(BlockReason::OwnershipUnknown),
        Ownership::UserExisting if !s.management.allow_modify_user_owned => {
            return Err(BlockReason::UserOwned)
        }
        _ => (),
    }
    if removal(kind, s) {
        if !s.management.allow_remove {
            return Err(BlockReason::Removal);
        }
        if *dependencies != DependencyUse::Unused {
            return Err(BlockReason::DependencyUse);
        }
    }
    if kind == MutationKind::Downgrade && !s.management.allow_downgrade {
        return Err(BlockReason::Downgrade);
    }
    Ok(())
}
fn mutation_guards(kind: MutationKind, s: &PlanningSnapshot) -> Result<(), BlockReason> {
    if !s.installer.can_detect {
        return Err(BlockReason::DetectionUnavailable);
    }
    installed_guards(kind, s)?;
    let supported = |op| s.installer.operations.contains(&op);
    if !supported(kind)
        || (kind != MutationKind::Uninstall
            && removal(kind, s)
            && (!supported(MutationKind::Install) || !supported(MutationKind::Uninstall)))
    {
        return Err(BlockReason::UnsupportedOperation);
    }
    match s.installer.restart {
        RestartBehavior::Automatic => return Err(BlockReason::Restart),
        RestartBehavior::MayRequire if !s.management.allow_restart => {
            return Err(BlockReason::Restart)
        }
        _ => (),
    }
    match s.installer.dependency_impact {
        DependencyImpact::Unknown => Err(BlockReason::DependencyImpact),
        DependencyImpact::Declared if !s.management.allow_dependency_changes => {
            Err(BlockReason::DependencyImpact)
        }
        _ => Ok(()),
    }
}
fn outcome(i: &SoftwareIntent, s: &PlanningSnapshot) -> DecisionOutcome {
    let kind = match next_change(i, s) {
        Continue(kind) => kind,
        Break(outcome) => return outcome,
    };
    if let Err(reason) = mutation_guards(kind, s) {
        return block(reason);
    }
    if let Readiness::Waiting(reason) = s.readiness {
        return DecisionOutcome::Wait(reason);
    }
    let installed_version = match &s.detection {
        Detection::Present { version, .. } => Some(version.clone()),
        _ => None,
    };
    DecisionOutcome::Mutate(Mutation {
        kind,
        upgrade_strategy: matches!(kind, MutationKind::Upgrade | MutationKind::Downgrade)
            .then_some(s.installer.upgrade_strategy),
        installer: s.installer.artifact.clone(),
        installed_version,
        post_detection: i.desired.clone(),
    })
}
/// Pure single-step decision from a coherent host snapshot; no I/O or execution authority.
/// State authenticity/freshness are host responsibilities. Invalid binding is an error;
/// a coherent but prohibited/unknown change returns Blocked without retry or fallback.
pub fn decide(
    intent: &SoftwareIntent,
    snapshot: &PlanningSnapshot,
    limits: PlanningLimits,
) -> Result<SoftwareDecision, DecisionError> {
    validate(intent, snapshot, limits)?;
    Ok(SoftwareDecision {
        intent: intent.clone(),
        snapshot: snapshot.revision.clone(),
        outcome: outcome(intent, snapshot),
    })
}

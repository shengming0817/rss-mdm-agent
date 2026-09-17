use execution_contract::*;
use software_plan::*;
#[allow(dead_code)]
#[path = "../examples/software-plan-consumer.rs"]
mod fixture;
use fixture::*;
fn outcome(i: &SoftwareIntent, s: &PlanningSnapshot) -> DecisionOutcome {
    decide(i, s, limits()).unwrap().outcome().clone()
}
fn present(
    s: &mut PlanningSnapshot,
    version: &str,
    ownership: Ownership,
    dependencies: DependencyUse,
) {
    s.detection = Detection::Present {
        version: value(version),
        ownership,
        dependencies,
        evidence: evidence(),
    };
}
fn compare(i: &SoftwareIntent, s: &mut PlanningSnapshot, relation: VersionRelation) {
    let DesiredState::Present {
        version: desired, ..
    } = &i.desired
    else {
        panic!()
    };
    let Detection::Present {
        version: installed, ..
    } = &s.detection
    else {
        panic!()
    };
    s.comparison = Some(VersionComparison {
        comparator: reference("ecosystem-comparator"),
        installed: installed.clone(),
        desired: desired.clone(),
        relation,
    });
}
fn blocked(i: &SoftwareIntent, s: &PlanningSnapshot, reason: BlockReason) {
    assert_eq!(outcome(i, s), DecisionOutcome::Blocked(reason));
}
#[test]
fn install_and_satisfied_preserve_exact_context_and_ownership() {
    let (i, mut s) = scenario();
    let decision = decide(&i, &s, limits()).unwrap();
    assert_eq!(decision.intent(), &i);
    assert_eq!(decision.snapshot(), &s.revision);
    let DecisionOutcome::Mutate(m) = decision.outcome() else {
        panic!()
    };
    assert_eq!(m.kind(), MutationKind::Install);
    assert_eq!(m.post_detection(), &i.desired);
    assert_eq!(m.installer(), &s.installer.artifact);
    assert_eq!(m.installed_version(), None);
    let DesiredState::Present { version, .. } = &i.desired else {
        panic!()
    };
    present(
        &mut s,
        version.as_str(),
        Ownership::UserExisting,
        DependencyUse::Unknown,
    );
    s.readiness = Readiness::Waiting(WaitReason::ResourceBusy);
    assert_eq!(
        outcome(&i, &s),
        DecisionOutcome::Satisfied {
            evidence: evidence(),
            ownership: Some(Ownership::UserExisting)
        }
    );
}
#[test]
fn unknown_effect_exit_and_ambiguous_detection_never_imply_success_or_retry() {
    let (i, mut s) = scenario();
    for cause in [
        DetectionCause::NotObserved,
        DetectionCause::AfterMutation,
        DetectionCause::UnknownEffect,
        DetectionCause::AfterRestart,
    ] {
        s.detection = Detection::Needed(cause);
        assert_eq!(outcome(&i, &s), DecisionOutcome::Detect(cause));
    }
    s.installer.can_detect = false;
    blocked(&i, &s, BlockReason::DetectionUnavailable);
    s.installer.can_detect = true;
    s.detection = Detection::Indeterminate;
    blocked(&i, &s, BlockReason::Indeterminate);
    let mut e = evidence();
    e.kind = EvidenceKind::ProcessExited;
    s.detection = Detection::Absent { evidence: e };
    assert_eq!(decide(&i, &s, limits()), Err(DecisionError::Evidence));
}
#[test]
fn ecosystem_ordering_is_required_bound_and_never_parsed_as_semver() {
    let (i, mut s) = scenario();
    present(
        &mut s,
        "99.9+vendor",
        Ownership::OrganizationManaged,
        DependencyUse::Unused,
    );
    blocked(&i, &s, BlockReason::VersionUnknown);
    compare(&i, &mut s, VersionRelation::Older);
    assert!(
        matches!(outcome(&i,&s), DecisionOutcome::Mutate(m) if m.kind()==MutationKind::Upgrade && m.installed_version()==Some(&value("99.9+vendor")))
    );
    compare(&i, &mut s, VersionRelation::Newer);
    blocked(&i, &s, BlockReason::Downgrade);
    s.management.allow_downgrade = true;
    assert!(
        matches!(outcome(&i,&s), DecisionOutcome::Mutate(m) if m.kind()==MutationKind::Downgrade)
    );
    compare(&i, &mut s, VersionRelation::Incomparable);
    blocked(&i, &s, BlockReason::VersionIncomparable);
    compare(&i, &mut s, VersionRelation::Equal);
    assert!(matches!(outcome(&i, &s), DecisionOutcome::Satisfied { .. }));
    s.comparison.as_mut().unwrap().installed = value("other");
    assert_eq!(decide(&i, &s, limits()), Err(DecisionError::Binding));
}
#[test]
fn removal_and_uninstall_before_upgrade_obey_ownership_and_dependencies() {
    let (mut i, mut s) = scenario();
    present(&mut s, "old", Ownership::Unknown, DependencyUse::Unused);
    compare(&i, &mut s, VersionRelation::Older);
    blocked(&i, &s, BlockReason::OwnershipUnknown);
    present(
        &mut s,
        "old",
        Ownership::UserExisting,
        DependencyUse::Unused,
    );
    blocked(&i, &s, BlockReason::UserOwned);
    s.management.allow_modify_user_owned = true;
    s.installer.upgrade_strategy = UpgradeStrategy::UninstallThenInstall;
    s.management.allow_remove = false;
    blocked(&i, &s, BlockReason::Removal);
    s.management.allow_remove = true;
    for dependency in [DependencyUse::InUse, DependencyUse::Unknown] {
        present(&mut s, "old", Ownership::DependencyIntroduced, dependency);
        blocked(&i, &s, BlockReason::DependencyUse);
    }
    present(
        &mut s,
        "old",
        Ownership::DependencyIntroduced,
        DependencyUse::Unused,
    );
    s.installer
        .operations
        .retain(|op| *op != MutationKind::Uninstall);
    blocked(&i, &s, BlockReason::UnsupportedOperation);
    s.installer.operations.push(MutationKind::Uninstall);
    assert!(
        matches!(outcome(&i, &s), DecisionOutcome::Mutate(m) if m.upgrade_strategy() == Some(UpgradeStrategy::UninstallThenInstall))
    );
    i.desired = DesiredState::Absent;
    s.comparison = None;
    assert!(
        matches!(outcome(&i,&s),DecisionOutcome::Mutate(m) if m.kind()==MutationKind::Uninstall && m.post_detection()==&DesiredState::Absent)
    );
    s.management.required = true;
    blocked(&i, &s, BlockReason::Required);
    s.detection = Detection::Absent {
        evidence: evidence(),
    };
    blocked(&i, &s, BlockReason::Required);
    s.management.required = false;
    assert!(matches!(
        outcome(&i, &s),
        DecisionOutcome::Satisfied {
            ownership: None,
            ..
        }
    ));
}
#[test]
fn readiness_restart_and_dependency_effects_are_explicit() {
    let (i, mut s) = scenario();
    for reason in [
        WaitReason::ResourceBusy,
        WaitReason::PackageManagerBusy,
        WaitReason::MaintenanceWindow,
        WaitReason::ApplicationBusy,
    ] {
        s.readiness = Readiness::Waiting(reason);
        assert_eq!(outcome(&i, &s), DecisionOutcome::Wait(reason));
    }
    s.readiness = Readiness::PendingRestart {
        evidence: evidence(),
    };
    blocked(&i, &s, BlockReason::Restart);
    s.management.allow_restart = true;
    assert_eq!(
        outcome(&i, &s),
        DecisionOutcome::RequireRestart {
            evidence: evidence()
        }
    );
    s.readiness = Readiness::Ready;
    s.installer.restart = RestartBehavior::Automatic;
    blocked(&i, &s, BlockReason::Restart);
    s.installer.restart = RestartBehavior::MayRequire;
    s.management.allow_restart = false;
    blocked(&i, &s, BlockReason::Restart);
    s.management.allow_restart = true;
    s.installer.dependency_impact = DependencyImpact::Unknown;
    blocked(&i, &s, BlockReason::DependencyImpact);
    s.installer.dependency_impact = DependencyImpact::Declared;
    blocked(&i, &s, BlockReason::DependencyImpact);
    s.management.allow_dependency_changes = true;
    assert!(matches!(outcome(&i, &s), DecisionOutcome::Mutate(_)));
    s.installer.can_detect = false;
    blocked(&i, &s, BlockReason::DetectionUnavailable);
}
#[test]
fn snapshots_cannot_cross_identity_policy_scope_or_source() {
    let (i, s) = scenario();
    for n in 0..10 {
        let mut s = s.clone();
        match n {
            0 => {
                s.authority = Authority::Local {
                    id: Id::new("fixture").unwrap(),
                }
            }
            1 => s.target.device = DeviceId::new("other").unwrap(),
            2 => s.target.platform = Platform::Macos,
            3 => s.policy.revision = Id::new("2").unwrap(),
            4 => s.package.source.revision = Id::new("2").unwrap(),
            5 => s.package.package = value("other"),
            6 => s.package.architecture = value("x64"),
            7 => s.package.variant = value("preview"),
            8 => s.installer.manager = reference("other-manager"),
            _ => s.package.manager.revision = Id::new("2").unwrap(),
        };
        assert_eq!(
            decide(&i, &s, limits()),
            Err(DecisionError::Binding),
            "case {n}"
        );
    }
    let (mut i, mut s) = scenario();
    i.target.scope = TargetScope::User {
        account: OsAccountRef {
            platform: Platform::Linux,
            subject: Id::new("uid:1").unwrap(),
        },
    };
    s.target = i.target.clone();
    assert_eq!(decide(&i, &s, limits()), Err(DecisionError::Binding));
}
#[test]
fn text_work_bounds_duplicate_capabilities_and_evidence_namespaces() {
    for text in ["", "a\n", "a\0"] {
        assert_eq!(PackageValue::new(text), Err(DecisionError::Value));
    }
    assert_eq!(
        PackageValue::new("x".repeat(1025)),
        Err(DecisionError::Value)
    );
    let (mut i, mut s) = scenario();
    assert_eq!(
        decide(
            &i,
            &s,
            PlanningLimits {
                max_text_bytes: 0,
                max_capabilities: 4
            }
        ),
        Err(DecisionError::Limits)
    );
    assert_eq!(
        decide(
            &i,
            &s,
            PlanningLimits {
                max_text_bytes: 2,
                max_capabilities: 4
            }
        ),
        Err(DecisionError::Bound)
    );
    assert_eq!(
        decide(
            &i,
            &s,
            PlanningLimits {
                max_text_bytes: 1024,
                max_capabilities: 3
            }
        ),
        Err(DecisionError::Bound)
    );
    s.installer.operations[3] = MutationKind::Install;
    assert_eq!(decide(&i, &s, limits()), Err(DecisionError::Duplicate));
    s.installer.operations.pop();
    i.authority = Authority::Enterprise {
        id: Id::new("authority").unwrap(),
        tenant: Id::new("tenant").unwrap(),
    };
    s.authority = i.authority.clone();
    assert_eq!(decide(&i, &s, limits()), Err(DecisionError::Evidence));
    if let Detection::Absent { evidence } = &mut s.detection {
        evidence.kind = EvidenceKind::StateObserved;
    }
    assert!(matches!(outcome(&i, &s), DecisionOutcome::Mutate(_)));
}

use execution_contract::*;
use software_plan::*;

pub fn value(text: &str) -> PackageValue {
    PackageValue::new(text).unwrap()
}
pub fn reference(text: &str) -> VersionedRef {
    VersionedRef {
        id: Id::new(text).unwrap(),
        revision: Id::new("1").unwrap(),
    }
}
pub fn evidence() -> EvidenceRef {
    EvidenceRef {
        reference: reference("test-observation"),
        kind: EvidenceKind::TestResult,
        runner: Id::new("fixture-runner").unwrap(),
    }
}
pub fn limits() -> PlanningLimits {
    PlanningLimits {
        max_text_bytes: 1024,
        max_capabilities: 4,
    }
}
/// Synthetic facts only; never an installer or platform claim.
pub fn scenario() -> (SoftwareIntent, PlanningSnapshot) {
    let payload_artifact = ExactArtifactRef {
        resource: reference("test-payload"),
        sha256: Digest::new("ab".repeat(32)).unwrap(),
    };
    let intent = SoftwareIntent {
        authority: Authority::Test {
            id: Id::new("fixture").unwrap(),
        },
        target: Target {
            device: DeviceId::new("test-device").unwrap(),
            platform: Platform::Windows,
            scope: TargetScope::Device {},
        },
        policy: reference("policy"),
        package: PackageIdentity {
            manager: reference("manager"),
            source: reference("source"),
            package: value("vendor.package"),
            architecture: value("arm64"),
            variant: value("stable"),
        },
        desired: DesiredState::Present {
            version: value("2:1.0~rc1+vendor"),
            artifact: payload_artifact,
        },
    };
    let snapshot = PlanningSnapshot {
        revision: reference("snapshot"),
        authority: intent.authority.clone(),
        target: intent.target.clone(),
        policy: intent.policy.clone(),
        package: intent.package.clone(),
        detection: Detection::Absent {
            evidence: evidence(),
        },
        comparison: None,
        installer: InstallerCapabilities {
            artifact: ExactArtifactRef {
                resource: reference("test-installer"),
                sha256: Digest::new("cd".repeat(32)).unwrap(),
            },
            manager: intent.package.manager.clone(),
            can_detect: true,
            operations: vec![
                MutationKind::Install,
                MutationKind::Upgrade,
                MutationKind::Downgrade,
                MutationKind::Uninstall,
            ],
            upgrade_strategy: UpgradeStrategy::InPlace,
            restart: RestartBehavior::Never,
            dependency_impact: DependencyImpact::None,
        },
        management: ManagementConstraints {
            required: false,
            allow_modify_user_owned: false,
            allow_remove: true,
            allow_downgrade: false,
            allow_restart: false,
            allow_dependency_changes: false,
        },
        readiness: Readiness::Ready,
    };
    (intent, snapshot)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (intent, snapshot) = scenario();
    let decision = decide(&intent, &snapshot, limits())?;
    assert_eq!(decision.snapshot(), &snapshot.revision);
    let DecisionOutcome::Mutate(change) = decision.outcome() else {
        panic!("expected install")
    };
    let DesiredState::Present {
        version,
        artifact: payload,
    } = &intent.desired
    else {
        unreachable!()
    };
    assert_eq!(change.kind(), MutationKind::Install);
    assert_eq!(change.installer(), &snapshot.installer.artifact);
    assert_ne!(change.installer(), payload);
    assert_eq!(change.post_detection(), &intent.desired);

    let unknown_snapshot = PlanningSnapshot {
        revision: reference("snapshot-unknown-effect"),
        detection: Detection::Needed(DetectionCause::UnknownEffect),
        ..snapshot.clone()
    };
    let unknown = decide(&intent, &unknown_snapshot, limits())?;
    assert_eq!(unknown.snapshot(), &unknown_snapshot.revision);
    assert_ne!(unknown.snapshot(), decision.snapshot());
    assert_eq!(
        unknown.outcome(),
        &DecisionOutcome::Detect(DetectionCause::UnknownEffect)
    );

    let observed_snapshot = PlanningSnapshot {
        revision: reference("snapshot-observed-present"),
        detection: Detection::Present {
            version: version.clone(),
            ownership: Ownership::UserExisting,
            dependencies: DependencyUse::Unknown,
            evidence: EvidenceRef {
                reference: reference("test-observed-present"),
                ..evidence()
            },
        },
        ..unknown_snapshot
    };
    let observed = decide(&intent, &observed_snapshot, limits())?;
    assert_eq!(observed.snapshot(), &observed_snapshot.revision);
    assert_ne!(observed.snapshot(), unknown.snapshot());
    assert!(matches!(
        observed.outcome(),
        DecisionOutcome::Satisfied {
            ownership: Some(Ownership::UserExisting),
            ..
        }
    ));
    println!("software-plan: synthetic facts; install -> unknown-effect detection -> independent state observation; no OS execution");
    Ok(())
}

use execution_capability::*;
use execution_contract::*;
fn limits() -> PlanLimits {
    PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_stdin_bytes: 65536,
        max_attempts: 3,
    }
}
fn plan(platform: Platform) -> FrozenPlan {
    let mut p = decode_plan(
        include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
        &limits(),
    )
    .unwrap();
    p.request.target.platform = platform;
    let account = OsAccountRef {
        platform,
        subject: Id::new("user-1").unwrap(),
    };
    p.request.target.scope = TargetScope::User {
        account: account.clone(),
    };
    p.run_as = RunAs::User {
        account: account.clone(),
    };
    p.session_requirement = SessionRequirement::ActiveUser { account };
    p.launch.cwd = if platform == Platform::Windows {
        "C:\\workspace"
    } else {
        "/workspace"
    }
    .into();
    p.constraints.read_paths = vec![p.launch.cwd.clone()];
    FrozenPlan::freeze(p, &limits()).unwrap()
}
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
fn snapshot(p: &FrozenPlan) -> EnvironmentSnapshot {
    let s = p.spec();
    EnvironmentSnapshot {
        authority: s.request.authority.clone(),
        device: s.request.target.device.clone(),
        source: VersionedRef {
            id: Id::new("snapshot").unwrap(),
            revision: Id::new("1").unwrap(),
        },
        platform: Some(s.request.target.platform),
        interpreters: inventory(vec![s.launch.interpreter.clone()]),
        launch_io: inventory(vec![LaunchIoCapability::CapturedText(TextEncoding::Utf8)]),
        run_as: inventory(vec![s.run_as.clone()]),
        user_sessions: inventory(match &s.session_requirement {
            SessionRequirement::ActiveUser { account } => vec![account.clone()],
            _ => vec![],
        }),
        isolation: inventory(vec![
            Isolation::NetworkDenied,
            Isolation::NetworkAllowlist,
            Isolation::ReadPaths,
            Isolation::WritePaths,
            Isolation::ChildProcessesDenied,
            Isolation::Sandbox,
        ]),
    }
}
fn check(p: &FrozenPlan, s: &EnvironmentSnapshot) -> MatchReport {
    match_capabilities(p, s, MatchLimits { max_entries: 64 }).unwrap()
}
#[test]
fn every_platform_and_required_dimension_is_explicit() {
    for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
        let p = plan(platform);
        let s = snapshot(&p);
        assert_eq!(check(&p, &s).status, MatchStatus::Supported);
        for index in 0..s.isolation.entries.len() {
            let mut blocked = s.clone();
            blocked.isolation.entries[index].availability = Availability::Blocked;
            let result = check(&p, &blocked);
            if s.isolation.entries[index].capability != Isolation::NetworkAllowlist {
                assert_eq!(result.status, MatchStatus::Blocked);
            }
        }
        let mut missing = s.clone();
        missing.user_sessions.entries.clear();
        assert_eq!(check(&p, &missing).status, MatchStatus::Unsupported);
        missing.user_sessions.complete = false;
        assert_eq!(check(&p, &missing).status, MatchStatus::Unknown);
        missing = s.clone();
        missing.interpreters.entries[0]
            .capability
            .artifact
            .resource
            .revision = Id::new("other").unwrap();
        assert_eq!(check(&p, &missing).status, MatchStatus::Unsupported);
        missing.interpreters.complete = false;
        assert_eq!(check(&p, &missing).status, MatchStatus::Unknown);
    }
}
#[test]
fn disabling_sandbox_does_not_disable_other_constraints() {
    let p = plan(Platform::Linux);
    let mut spec = p.spec().clone();
    spec.constraints.require_sandbox = false;
    spec.constraints.allow_child_processes = true;
    spec.session_requirement = SessionRequirement::NotRequired {};
    let p = FrozenPlan::freeze(spec, &limits()).unwrap();
    let mut s = snapshot(&p);
    s.isolation.entries.retain(|e| {
        !matches!(
            e.capability,
            Isolation::Sandbox | Isolation::ChildProcessesDenied
        )
    });
    s.user_sessions.complete = false;
    assert_eq!(check(&p, &s).status, MatchStatus::Supported);
    s.isolation
        .entries
        .retain(|e| e.capability != Isolation::NetworkDenied);
    assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
}
#[test]
fn failures_are_complete_order_stable_and_deterministic() {
    let p = plan(Platform::Linux);
    let mut s = snapshot(&p);
    s.platform = None;
    s.interpreters.entries.clear();
    s.run_as.entries[0].availability = Availability::Blocked;
    let result = check(&p, &s);
    assert_eq!(result.status, MatchStatus::Unsupported);
    assert!(result
        .checks
        .iter()
        .any(|c| c.dimension == Dimension::Platform && c.status == MatchStatus::Unknown));
    assert!(result
        .checks
        .iter()
        .any(|c| c.dimension == Dimension::RunAs && c.status == MatchStatus::Blocked));
    s.isolation.entries.reverse();
    assert_eq!(check(&p, &s), result);
    s.interpreters.complete = false;
    assert_eq!(check(&p, &s).status, MatchStatus::Unknown);
    s.device = DeviceId::new("different-device").unwrap();
    let wrong_target = check(&p, &s);
    assert_eq!(wrong_target.status, MatchStatus::Unknown);
    assert!(wrong_target
        .checks
        .iter()
        .all(|c| c.status == MatchStatus::Unknown));
}
#[test]
fn malformed_or_over_budget_snapshots_fail_closed() {
    let p = plan(Platform::Linux);
    let mut s = snapshot(&p);
    s.interpreters
        .entries
        .push(s.interpreters.entries[0].clone());
    assert_eq!(
        match_capabilities(&p, &s, MatchLimits { max_entries: 64 }).unwrap_err(),
        MatchError::Duplicate(Dimension::Interpreter)
    );
    assert_eq!(
        match_capabilities(&p, &s, MatchLimits { max_entries: 1 }).unwrap_err(),
        MatchError::Limit
    );
    assert_eq!(
        match_capabilities(&p, &s, MatchLimits { max_entries: 0 }).unwrap_err(),
        MatchError::Configuration
    );
}

#[test]
fn exact_identity_and_every_inventory_fail_closed() {
    let p = plan(Platform::Linux);
    for axis in 0..4 {
        let mut s = snapshot(&p);
        match axis {
            0 => s.interpreters.entries.clear(),
            1 => s.run_as.entries.clear(),
            2 => s.user_sessions.entries.clear(),
            _ => s.isolation.entries.clear(),
        };
        assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
        match axis {
            0 => s.interpreters.complete = false,
            1 => s.run_as.complete = false,
            2 => s.user_sessions.complete = false,
            _ => s.isolation.complete = false,
        };
        assert_eq!(check(&p, &s).status, MatchStatus::Unknown);
    }
    let mut s = snapshot(&p);
    s.interpreters.entries[0].capability.artifact.sha256 = Digest::new("45".repeat(32)).unwrap();
    assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
    let mut s = snapshot(&p);
    s.platform = Some(Platform::Windows);
    assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
    let mut s = snapshot(&p);
    s.user_sessions.entries[0].capability.subject = Id::new("wrong-user").unwrap();
    assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
    let mut s = snapshot(&p);
    s.run_as.entries[0].capability = RunAs::System {
        platform: Platform::Linux,
    };
    assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
    let mut spec = p.spec().clone();
    spec.constraints.network = NetworkAccess::Allowlist {
        destinations: vec![NetworkDestination {
            scheme: NetworkScheme::Https,
            host: NetworkHost::new("example.invalid").unwrap(),
            port: std::num::NonZeroU16::new(443).unwrap(),
        }],
    };
    let p = FrozenPlan::freeze(spec, &limits()).unwrap();
    let mut s = snapshot(&p);
    assert_eq!(check(&p, &s).status, MatchStatus::Supported);
    s.isolation
        .entries
        .retain(|e| e.capability != Isolation::NetworkAllowlist);
    assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
}

#[test]
fn identical_device_ids_cannot_replay_snapshots_across_authorities_or_tenants() {
    let authorities = vec![
        Authority::Enterprise {
            id: Id::new("authority").unwrap(),
            tenant: Id::new("tenant-a").unwrap(),
        },
        Authority::Enterprise {
            id: Id::new("authority").unwrap(),
            tenant: Id::new("tenant-b").unwrap(),
        },
        Authority::Local {
            id: Id::new("authority").unwrap(),
        },
        Authority::Test {
            id: Id::new("authority").unwrap(),
        },
    ];
    for source in &authorities {
        let mut spec = plan(Platform::Linux).spec().clone();
        spec.request.authority = source.clone();
        let p = FrozenPlan::freeze(spec, &limits()).unwrap();
        let s = snapshot(&p);
        assert_eq!(check(&p, &s).status, MatchStatus::Supported);
        for target in &authorities {
            if target == source {
                continue;
            }
            let mut other = p.spec().clone();
            other.request.authority = target.clone();
            let other = FrozenPlan::freeze(other, &limits()).unwrap();
            let result = check(&other, &s);
            assert_eq!(result.status, MatchStatus::Unknown);
            assert!(result
                .checks
                .iter()
                .all(|c| c.status == MatchStatus::Unknown));
        }
    }
}

#[test]
fn profile_and_each_stream_requirement_need_exact_positive_facts() {
    let base = plan(Platform::Linux);
    let mut spec = base.spec().clone();
    spec.launch.stdin = StandardInput::Controlled {
        reference: spec.policy.clone(),
        encoding: TextEncoding::Utf16Le,
        max_bytes: 64,
    };
    spec.launch.output.stderr = TextEncoding::Utf16Le;
    let p = FrozenPlan::freeze(spec, &limits()).unwrap();
    let mut s = snapshot(&p);
    s.launch_io.complete = false;
    assert_eq!(check(&p, &s).status, MatchStatus::Unknown);
    s.launch_io.complete = true;
    assert_eq!(check(&p, &s).status, MatchStatus::Unsupported);
    s.launch_io = inventory(vec![
        LaunchIoCapability::ControlledStdin(TextEncoding::Utf16Le),
        LaunchIoCapability::CapturedText(TextEncoding::Utf8),
        LaunchIoCapability::CapturedText(TextEncoding::Utf16Le),
    ]);
    assert_eq!(check(&p, &s).status, MatchStatus::Supported);
    for index in 0..3 {
        let mut other = s.clone();
        other.launch_io.entries[index].availability = Availability::Blocked;
        assert_eq!(check(&p, &other).status, MatchStatus::Blocked);
    }
    let mut other = s.clone();
    other.interpreters.entries[0].capability.profile.revision = Id::new("2").unwrap();
    assert_eq!(check(&p, &other).status, MatchStatus::Unsupported);
    s.launch_io.entries.push(s.launch_io.entries[0].clone());
    assert_eq!(
        match_capabilities(&p, &s, MatchLimits { max_entries: 64 }),
        Err(MatchError::Duplicate(Dimension::LaunchIo))
    );
}

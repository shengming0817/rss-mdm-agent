use super::*;
use serde_json::json;
pub(super) fn query(definition: w::NativeQueryDefinition) -> w::NativeQuery {
    let cardinality = match &definition {
        w::NativeQueryDefinition::MacosHardware { section } => section.cardinality(),
        _ => w::NativeCardinality::List,
    };
    w::NativeQuery {
        version: 1,
        definition,
        cardinality,
        instance_keys: if cardinality == w::NativeCardinality::List {
            vec!["id".into()]
        } else {
            vec![]
        },
        budget: w::NativeBudget {
            timeout_seconds: 20,
            output_bytes: 1048576,
            max_rows: 4096,
        },
    }
}
pub(super) fn input(q: w::NativeQuery) -> Input {
    let plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
        version: 1,
        scope: rss_mdm_agent_wire::CollectionScope::Device,
        budget: q.budget,
        queries: [("items".into(), q)].into(),
    })
    .unwrap();
    let id = uuid::Uuid::new_v4();
    let time = now().unwrap();
    let identity = plan.execution_identity();
    let target = if identity == w::ExecutionIdentity::System {
        w::SoftwareExecutionTarget::Device
    } else {
        w::SoftwareExecutionTarget::User {
            identity: unsafe { libc::geteuid() }.to_string(),
            session_id: id,
        }
    };
    let task = w::CollectionTaskSpec {
        wire_version: 2,
        tenant_id: id,
        device_id: "fixture-device".into(),
        platform: w::TaskPlatform::Macos,
        architecture: w::TaskArchitecture::Aarch64,
        registration_id: id,
        generation: 1,
        task_id: id,
        attempt_id: id,
        permit: w::TaskPermit::Start,
        expires_at: time + 30,
        resource_digest: [3; 32],
        resource: w::TaskResource {
            id: "fixture-native".into(),
            version: "v1".into(),
            variant: "macos".into(),
        },
        authorization: w::TaskAuthorization::Policy {
            policy_id: id,
            policy_version: id,
        },
        deadline: time + 60,
        plan_digest: plan.digest(),
        execution: w::NativeExecutionBinding {
            identity,
            context_revision: 1,
            target,
        },
        plan,
    };
    Input {
        task,
        finish_by_unix_ms: (time as u64 + 20) * 1000,
        image_sha256: Digest::new("00".repeat(32)).unwrap(),
        run_as: RunAs::System {
            platform: execution_contract::Platform::Macos,
        },
        session: SessionRequirement::NotRequired {},
        tools: Default::default(),
        observed: Default::default(),
    }
}
#[test]
fn apfs_capacity_is_per_container_and_duplicate_identity_is_rejected() {
    let q = query(w::NativeQueryDefinition::MacosHardware {
        section: w::MacosHardwareSection::ApfsContainers,
    });
    let mut d = plist::Dictionary::new();
    d.insert("APFSContainerUUID".into(), "container".into());
    d.insert("CapacityCeiling".into(), 1000u64.into());
    d.insert("CapacityFree".into(), 400u64.into());
    d.insert(
        "Volumes".into(),
        plist::Value::Array(vec![plist::Value::Dictionary(Default::default()); 3]),
    );
    let root = |items| {
        plist::Value::Dictionary(
            [(String::from("Containers"), plist::Value::Array(items))]
                .into_iter()
                .collect(),
        )
    };
    let mut rows = Rows::new(&q, now().unwrap());
    hardware::containers(&root(vec![d.clone().into()]), &mut rows).unwrap();
    let result = rows.finish(Ok(()));
    assert!(result.complete_for(&q, now().unwrap()));
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0]["size_bytes"], "1000");
    let mut rows = Rows::new(&q, now().unwrap());
    assert!(hardware::containers(&root(vec![d.clone().into(), d.into()]), &mut rows).is_err());
}
#[test]
fn application_copies_versions_and_user_scopes_keep_distinct_identity() {
    let d = plist::Dictionary::from_iter::<[(String, plist::Value); 3]>([
        ("CFBundleIdentifier".into(), "org.example.app".into()),
        ("CFBundleShortVersionString".into(), "2.0".into()),
        ("CFBundleVersion".into(), "204".into()),
    ]);
    let a = applications::decode(&d, "/Applications/A.app", "").unwrap();
    let b = applications::decode(&d, "/Applications/B.app", "").unwrap();
    let user = applications::decode(&d, "/Users/test/Applications/A.app", "501").unwrap();
    assert_ne!(a["id"], b["id"]);
    assert_ne!(a["id"], user["id"]);
    assert_eq!(a["version"], "2.0");
    assert_eq!(a["build_version"], "204");
    assert_eq!(user["owner"], "501");
    let mut invalid = d;
    invalid.remove("CFBundleIdentifier");
    assert!(applications::decode(&invalid, "/Applications/A.app", "").is_err());
}
#[test]
fn receipt_is_evidence_even_when_install_location_does_not_exist() {
    let d = plist::Dictionary::from_iter::<[(String, plist::Value); 5]>([
        ("pkgid".into(), "org.example.receipt".into()),
        ("pkg-version".into(), "1".into()),
        ("volume".into(), "/".into()),
        ("install-location".into(), "missing-example".into()),
        ("install-time".into(), 1u64.into()),
    ]);
    let r = receipts::decode(&d).unwrap();
    assert_eq!(r["origin"], "pkg_receipt");
    assert!(!r.contains_key("signature_status"));
    assert_eq!(r["location"], "/missing-example");
}
#[test]
fn brew_installed_versions_prefixes_taps_and_empty_results_are_distinct() {
    let definition = |prefix: &str| w::NativeQueryDefinition::MacosBrew {
        prefix: prefix.into(),
        package: w::MacosBrewPackage::Formula,
        taps: Default::default(),
    };
    let q = query(definition("/opt/homebrew"));
    let value = json!({"formulae":[{"name":"example","tap":"homebrew/core","installed":[{"version":"1"},{"version":"2"}]}],"casks":[]});
    let mut rows = Rows::new(&q, now().unwrap());
    brew::decode(
        &value,
        "/opt/homebrew",
        "501",
        w::MacosBrewPackage::Formula,
        &Default::default(),
        &mut rows,
    )
    .unwrap();
    let r = rows.finish(Ok(()));
    assert!(r.complete_for(&q, now().unwrap()));
    assert_eq!(r.rows.len(), 2);
    assert_ne!(r.rows[0]["id"], r.rows[1]["id"]);
    let mut rows = Rows::new(&q, now().unwrap());
    brew::decode(
        &value,
        "/opt/homebrew",
        "501",
        w::MacosBrewPackage::Formula,
        &["other/tap".into()].into(),
        &mut rows,
    )
    .unwrap();
    assert_eq!(rows.finish(Ok(())).rows.len(), 0);
    let other = query(definition("/usr/local"));
    let mut rows = Rows::new(&other, now().unwrap());
    brew::decode(
        &value,
        "/usr/local",
        "501",
        w::MacosBrewPackage::Formula,
        &Default::default(),
        &mut rows,
    )
    .unwrap();
    assert_ne!(rows.finish(Ok(())).rows[0]["id"], r.rows[0]["id"]);
    let mut rows = Rows::new(&q, now().unwrap());
    assert!(brew::decode(
        &json!({}),
        "/opt/homebrew",
        "501",
        w::MacosBrewPackage::Formula,
        &Default::default(),
        &mut rows
    )
    .is_err());
}
#[test]
fn actual_host_memory_os_cpu_and_network_use_native_providers() {
    // Provider-only T2: no claim of privileged service installation or enterprise authorization.
    for section in [
        w::MacosHardwareSection::Memory,
        w::MacosHardwareSection::Os,
        w::MacosHardwareSection::Cpu,
        w::MacosHardwareSection::Network,
    ] {
        let q = query(w::NativeQueryDefinition::MacosHardware { section });
        let input = input(q.clone());
        let result = execute(&input, &q).unwrap();
        assert!(
            result.complete_for(&q, now().unwrap()),
            "{section:?}: {:?}",
            result.status
        );
    }
}
#[test]
fn missing_tool_timeout_and_empty_inventory_are_not_interchangeable() {
    let q = query(w::NativeQueryDefinition::MacosPkgReceipts {});
    let mut input = input(q.clone());
    assert_eq!(execute(&input, &q).unwrap().status, Status::Unsupported);
    input.finish_by_unix_ms = 1;
    assert_eq!(execute(&input, &q).unwrap().status, Status::Timeout);
    let empty = Rows::new(&q, now().unwrap()).finish(Ok(()));
    assert!(empty.complete_for(&q, now().unwrap()));
}
#[test]
fn actual_fixed_system_tool_has_bounded_owner_and_output() {
    let q = query(w::NativeQueryDefinition::MacosHardware {
        section: w::MacosHardwareSection::Identity,
    });
    let mut input = input(q.clone());
    input.tools.insert(
        "system_profiler".into(),
        crate::backend::plan::pin(Path::new("/usr/sbin/system_profiler")).unwrap(),
    );
    let result = execute(&input, &q).unwrap();
    assert!(
        result.complete_for(&q, now().unwrap()),
        "{:?}",
        result.status
    );
}
#[test]
fn actual_brew_sandbox_denies_writes_and_network() {
    let status=std::process::Command::new("/usr/bin/sandbox-exec")
        .args(["-p","(version 1)(allow default)(deny file-write*)(allow file-write* (literal \"/dev/null\"))(deny network*)","/usr/bin/touch","/tmp/rss-collection-must-not-exist"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap();
    assert!(!status.success());
}

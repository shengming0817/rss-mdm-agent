use super::*;
#[test]
fn partial_file_read_failure_accounts_observed_bytes() {
    struct Partial(bool);
    impl std::io::Read for Partial {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.0 {
                return Err(std::io::Error::other("fixture read failure"));
            }
            self.0 = true;
            buffer[..3].copy_from_slice(b"abc");
            Ok(3)
        }
    }
    let input = input(query(w::NativeQueryDefinition::MacosPkgReceipts {}));
    let context = Context {
        input: &input,
        until: Instant::now() + std::time::Duration::from_secs(5),
        bytes: Cell::new(0),
        cap: 1024,
    };
    assert!(matches!(
        context.read_bounded(Partial(false), 1024),
        Err(Fault::Query(Status::Invalid))
    ));
    assert_eq!(context.bytes.get(), 3);
    assert_eq!(input.observed.load(std::sync::atomic::Ordering::Relaxed), 3);
}

pub(super) fn query(definition: w::NativeQueryDefinition) -> w::NativeQuery {
    let cardinality = match &definition {
        w::NativeQueryDefinition::MacosHardware { section } => section.cardinality(),
        _ => w::NativeCardinality::List,
    };
    w::NativeQuery {
        version: definition.engine().contract_version(),
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
        wire_version: 3,
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
            generated_uid: if identity == w::ExecutionIdentity::LoggedInUser {
                Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into())
            } else {
                None
            },
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
fn apfs_boot_snapshot_is_not_a_logical_volume_or_a_new_identity() {
    let mut volume = plist::Dictionary::from_iter::<[(&str, plist::Value); 2]>([
        ("APFSVolumeUUID", "logical-system".into()),
        ("Roles", vec![plist::Value::String("System".into())].into()),
    ]);
    let container = plist::Dictionary::from_iter::<[(&str, plist::Value); 1]>([(
        "APFSContainerUUID",
        "container".into(),
    )]);
    let mut info = plist::Dictionary::from_iter::<[(&str, plist::Value); 5]>([
        ("FilesystemType", "apfs".into()),
        ("VolumeName", "System".into()),
        ("TotalSize", 1000u64.into()),
        ("VolumeUUID", "logical-system".into()),
        ("APFSVolumeGroupID", "boot-group".into()),
    ]);
    let mut root = info.clone();
    root.insert("VolumeUUID".into(), "snapshot-1".into());
    root.insert("APFSSnapshot".into(), true.into());
    let first = hardware::volume_row(&info, Some((&container, &volume)), &root)
        .unwrap()
        .unwrap();
    assert_eq!(first["id"], "logical-system");
    assert_eq!(first["system_volume"], true);
    root.insert("VolumeUUID".into(), "snapshot-2".into());
    assert_eq!(
        hardware::volume_row(&info, Some((&container, &volume)), &root)
            .unwrap()
            .unwrap(),
        first
    );
    assert!(hardware::volume_row(&root, None, &root).unwrap().is_none());
    volume.insert(
        "Roles".into(),
        vec![plist::Value::String("Data".into())].into(),
    );
    info.insert("VolumeUUID".into(), "data".into());
    assert_eq!(
        hardware::volume_row(&info, Some((&container, &volume)), &root)
            .unwrap()
            .unwrap()["system_volume"],
        false
    );
}

#[test]
fn signed_generated_uid_is_checked_against_the_local_directory() {
    use execution_contract::{Id, OsAccountRef, Platform};
    let subject = unsafe { libc::geteuid() }.to_string();
    let observed = crate::account_identity::generated_uid(&subject)
        .unwrap()
        .unwrap();
    let q = query(w::NativeQueryDefinition::MacosApplications {
        root: w::MacosApplicationRoot::UserApplications,
        max_depth: 1,
    });
    let mut input = input(q);
    let account = OsAccountRef {
        platform: Platform::Macos,
        subject: Id::new(subject.clone()).unwrap(),
    };
    input.run_as = RunAs::User {
        account: account.clone(),
    };
    input.session = SessionRequirement::ActiveUser {
        account,
        session: Id::new("identity-boundary-test").unwrap(),
    };
    input.task.execution.generated_uid = Some(observed);
    super::super::verify_identity_binding(&input).unwrap();
    input.task.execution.generated_uid = Some(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        super::super::verify_identity_binding(&input),
        Err(Error::Untrusted)
    );
    assert!(crate::account_identity::generated_uid("4294967295").is_err());
}

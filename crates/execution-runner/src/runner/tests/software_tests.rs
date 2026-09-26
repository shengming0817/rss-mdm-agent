use super::*;
use crate::software::{SoftwareArtifacts, SoftwareProbe};
use std::io::Write;
struct Probe;
impl SoftwareProbe for Probe {
    fn dependency_use(&self, _: &FrozenPlan) -> Result<DependencyUse, Error> {
        Ok(DependencyUse::Unused)
    }
    fn comparison(
        &self,
        plan: &FrozenPlan,
        installed: &PackageValue,
    ) -> Result<Option<VersionComparison>, Error> {
        let s = plan.spec().execution.software().unwrap();
        Ok(match &s.desired {
            DesiredState::Present { version, .. }
                if installed.as_str() == "1.0" && version.as_str() == "2.0" =>
            {
                Some(VersionComparison {
                    comparator: s.snapshot.clone(),
                    installed: installed.clone(),
                    desired: version.clone(),
                    relation: VersionRelation::Older,
                })
            }
            _ => None,
        })
    }
    fn quiescence(&self, _: &FrozenPlan, _: &AttemptId) -> Result<Option<EvidenceRef>, Error> {
        Ok(None)
    }
}
fn software_fixture(script: &str) -> Fixture {
    let mut f = fixture(script, vec![LaunchArg::ArtifactPath {}], 4096, 10000);
    let old = f.plan.digest().as_str().to_owned();
    let mut source = Arc::try_unwrap(f.runner.artifacts.remove(&old).unwrap())
        .ok()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../execution-contract/tests/fixtures/software.json"
    ))
    .unwrap();
    let mut software: SoftwareSpec =
        serde_json::from_value(value["execution"]["software"].clone()).unwrap();
    software.adapter = SoftwareKind::MacosBundle;
    software.detection.path = f.root.join("installed").to_str().unwrap().into();
    software.detection.versions[0].sha256 =
        Digest::new(format!("{:x}", Sha256::digest(b"v1"))).unwrap();
    software.manager = f.plan.spec().launch.interpreter.artifact.clone();
    software.bundle = Some(BundleLimits {
        archive_bytes: 1024 * 1024,
        files: 8,
        file_bytes: 8192,
        expanded_bytes: 16384,
        depth: 8,
    });
    let manifest = BundleManifest {
        package: software.package.clone(),
        version: PackageValue::new("1.0").unwrap(),
        files: vec![BundleFile {
            path: "install.sh".into(),
            sha256: f.plan.spec().launch.artifact.sha256.clone(),
        }],
        install: "install.sh".into(),
        uninstall: None,
        detection: software.detection.clone(),
    };
    let payload = f.root.join("payload.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&payload).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    zip.start_file("install.sh", options).unwrap();
    zip.write_all(script.as_bytes()).unwrap();
    zip.finish().unwrap();
    software.payload.sha256 = Digest::new(format!(
        "{:x}",
        Sha256::digest(std::fs::read(&payload).unwrap())
    ))
    .unwrap();
    software.desired = DesiredState::Present {
        version: PackageValue::new("1.0").unwrap(),
        artifact: software.payload.clone(),
    };
    let mut spec = f.plan.spec().clone();
    spec.request.parameters.clear();
    spec.request.operation.action = Id::new("software.install").unwrap();
    spec.execution = ExecutionSpec::Software {
        software: Box::new(software),
    };
    f.plan = FrozenPlan::freeze(spec, &execution_app::test_store_limits().plan).unwrap();
    source.software = Some(SoftwareArtifacts {
        payload,
        manager: "/bin/sh".into(),
        lock_root: f.root.clone(),
        probe: Arc::new(Probe),
        fixture_owned: true,
    });
    f.runner
        .artifacts
        .insert(f.plan.digest().as_str().into(), Arc::new(source));
    f
}
fn start_software(f: &Fixture) -> AttemptId {
    let attempt = AttemptId::new("software-attempt").unwrap();
    f.runner
        .launch(
            &f.plan,
            &attempt,
            DispatchAllowance {
                deadline_unix_ms: now().unwrap() + 10000,
                remaining_timeout_ms: 10000,
                remaining_output_bytes: 4096,
            },
            Some(SoftwareProvenance {
                ownership: Ownership::UserExisting,
                state: None,
            }),
        )
        .unwrap();
    attempt
}
#[test]
fn real_bundle_effect_is_independent_and_not_a_quiescence_claim() {
    let f = software_fixture("printf v1 > installed\n");
    let id = start_software(&f);
    let process = finish(&f, &id);
    assert_eq!(process.exit_code, Some(0));
    assert!(!process.quiescent);
    let detected = f.runner.software_evidence(&f.plan, &id).unwrap().unwrap();
    assert_eq!(detected.before, Some(SoftwareState::Absent {}));
    assert_eq!(
        detected.detected,
        SoftwareState::Present {
            version: PackageValue::new("1.0").unwrap()
        }
    );
    assert!(f
        .runner
        .observe(&f.plan, &id, ObservationStage::Termination, now().unwrap())
        .unwrap()
        .is_none());
    f.runner.acknowledge_capture(&f.plan, &process).unwrap();
    let recovered = f.runner.software_evidence(&f.plan, &id).unwrap().unwrap();
    assert_eq!(recovered.before, None);
    assert_eq!(recovered.detected, detected.detected);
}
#[test]
fn zero_exit_without_installed_effect_is_not_satisfied() {
    let f = software_fixture("exit 0\n");
    let id = start_software(&f);
    let process = finish(&f, &id);
    assert_eq!(process.exit_code, Some(0));
    let facts = f.runner.software_evidence(&f.plan, &id).unwrap().unwrap();
    assert_eq!(facts.detected, SoftwareState::Absent {});
    assert!(!f
        .plan
        .spec()
        .execution
        .software()
        .unwrap()
        .satisfied(&facts.detected));
}
#[test]
fn existing_user_software_is_never_overwritten_by_install() {
    let f = software_fixture("printf v1 > installed\n");
    std::fs::write(f.root.join("installed"), b"user-owned").unwrap();
    let id = start_software(&f);
    let process = finish(&f, &id);
    assert!(matches!(process.scope, ProcessScope::NotStarted {}));
    assert_eq!(
        std::fs::read(f.root.join("installed")).unwrap(),
        b"user-owned"
    );
}

fn change_bundle(f: &mut Fixture, mutation: MutationKind, script: &str) {
    let old = f.plan.digest().as_str().to_owned();
    let mut source = Arc::try_unwrap(f.runner.artifacts.remove(&old).unwrap())
        .ok()
        .unwrap();
    let mut p = f.plan.spec().clone();
    let ExecutionSpec::Software { software: s } = &mut p.execution else {
        panic!("software")
    };
    let uninstall = b"rm -f installed\n";
    s.uninstall = Some(ExactArtifactRef {
        resource: s.snapshot.clone(),
        sha256: Digest::new(format!("{:x}", Sha256::digest(uninstall))).unwrap(),
    });
    s.detection.versions.push(SoftwareVersionProof {
        version: PackageValue::new("2.0").unwrap(),
        sha256: Digest::new(format!("{:x}", Sha256::digest(b"v2"))).unwrap(),
    });
    s.detection.versions.dedup_by(|a, b| a.version == b.version);
    let install_hash = Digest::new(format!("{:x}", Sha256::digest(script.as_bytes()))).unwrap();
    let manifest = BundleManifest {
        package: s.package.clone(),
        version: PackageValue::new("2.0").unwrap(),
        files: vec![
            BundleFile {
                path: "install.sh".into(),
                sha256: install_hash.clone(),
            },
            BundleFile {
                path: "uninstall.sh".into(),
                sha256: s.uninstall.as_ref().unwrap().sha256.clone(),
            },
        ],
        install: "install.sh".into(),
        uninstall: Some("uninstall.sh".into()),
        detection: s.detection.clone(),
    };
    let payload = &source.software.as_ref().unwrap().payload;
    let mut zip = zip::ZipWriter::new(std::fs::File::create(payload).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("install.sh", script.as_bytes().to_vec()),
        ("uninstall.sh", uninstall.to_vec()),
    ] {
        zip.start_file(name, options).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
    s.payload.sha256 = Digest::new(format!(
        "{:x}",
        Sha256::digest(std::fs::read(payload).unwrap())
    ))
    .unwrap();
    s.mutation = mutation;
    if mutation == MutationKind::Uninstall {
        s.desired = DesiredState::Absent;
        s.comparison = None;
        p.launch.artifact = s.uninstall.clone().unwrap();
        p.request.operation.action = Id::new("software.uninstall").unwrap();
    } else {
        s.desired = DesiredState::Present {
            version: PackageValue::new("2.0").unwrap(),
            artifact: s.payload.clone(),
        };
        s.comparison = Some(VersionComparison {
            comparator: s.snapshot.clone(),
            installed: PackageValue::new("1.0").unwrap(),
            desired: PackageValue::new("2.0").unwrap(),
            relation: VersionRelation::Older,
        });
        p.launch.artifact.sha256 = install_hash;
        p.request.operation.action = Id::new("software.upgrade").unwrap();
    }
    // Bundle entries are resolved from the exact archive, not this fallback path.
    source.content = f.root.join("unused-entry");
    f.plan = FrozenPlan::freeze(p, &execution_app::test_store_limits().plan).unwrap();
    f.runner
        .artifacts
        .insert(f.plan.digest().as_str().into(), Arc::new(source));
}
#[test]
fn real_bundle_upgrade_and_declared_uninstall_recheck_owned_version() {
    let mut f = software_fixture("printf v1 > installed\n");
    let attempt = start_software(&f);
    let facts = finish(&f, &attempt);
    assert_eq!(facts.exit_code, Some(0));
    f.runner.acknowledge_capture(&f.plan, &facts).unwrap();
    for (mutation, version, expected) in [
        (
            MutationKind::Upgrade,
            "1.0",
            SoftwareState::Present {
                version: PackageValue::new("2.0").unwrap(),
            },
        ),
        (MutationKind::Uninstall, "2.0", SoftwareState::Absent {}),
    ] {
        change_bundle(&mut f, mutation, "printf v2 > installed\n");
        f.runner
            .launch(
                &f.plan,
                &attempt,
                DispatchAllowance {
                    deadline_unix_ms: now().unwrap() + 10000,
                    remaining_timeout_ms: 10000,
                    remaining_output_bytes: 4096,
                },
                Some(SoftwareProvenance {
                    ownership: Ownership::OrganizationManaged,
                    state: Some(SoftwareState::Present {
                        version: PackageValue::new(version).unwrap(),
                    }),
                }),
            )
            .unwrap();
        let facts = finish(&f, &attempt);
        assert_eq!(facts.exit_code, Some(0), "{:?}", facts.failure_kind);
        assert_eq!(
            f.runner
                .software_evidence(&f.plan, &attempt)
                .unwrap()
                .unwrap()
                .detected,
            expected
        );
        f.runner.acknowledge_capture(&f.plan, &facts).unwrap();
    }
}

#[test]
fn manager_lock_blocks_a_second_attempt_and_cancel_keeps_effect_separate() {
    let f = software_fixture("/bin/sleep 2; printf v1 > installed\n");
    let first = start_software(&f);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if f.runner
            .evidence(&f.plan, &first)
            .unwrap()
            .is_some_and(|p| matches!(p.scope, ProcessScope::ProcessGroup { .. }))
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let second = AttemptId::new("second-attempt").unwrap();
    f.runner
        .launch(
            &f.plan,
            &second,
            DispatchAllowance {
                deadline_unix_ms: now().unwrap() + 10000,
                remaining_timeout_ms: 10000,
                remaining_output_bytes: 4096,
            },
            Some(SoftwareProvenance {
                ownership: Ownership::UserExisting,
                state: None,
            }),
        )
        .unwrap();
    let rejected = finish(&f, &second);
    assert!(matches!(rejected.scope, ProcessScope::NotStarted {}));
    assert_eq!(rejected.failure_kind, ProcessFailureKind::Conflict);
    f.runner.stop(&f.plan, &first).unwrap();
    let stopped = finish(&f, &first);
    assert_eq!(stopped.end, ProcessEnd::Cancelled);
    assert!(!stopped.quiescent);
    assert_eq!(
        f.runner
            .software_evidence(&f.plan, &first)
            .unwrap()
            .unwrap()
            .detected,
        SoftwareState::Absent {}
    );
}

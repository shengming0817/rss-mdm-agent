use execution_contract::{decode_execution, ExecutionLimits};

fn limits() -> ExecutionLimits {
    ExecutionLimits {
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

#[test]
fn v6_requires_explicit_execution_kind_and_rejects_old_plans() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 6.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_ok());
    value["schemaVersion"] = 2.into();
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
    value["schemaVersion"] = 6.into();
    value.as_object_mut().unwrap().remove("execution");
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn process_cannot_impersonate_software_operation() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 6.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    value["request"]["operation"]["action"] = "software.install".into();
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn software_steps_bind_order_materials_and_context_without_legacy_parser() {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/software.json")).unwrap();
    let freeze = |v: &serde_json::Value| {
        execution_contract::FrozenExecution::freeze(
            decode_execution(&serde_json::to_vec(v).unwrap(), &limits()).unwrap(),
            &limits(),
        )
        .unwrap()
    };
    let original = freeze(&value);
    for field in ["package", "architecture"] {
        let mut changed = value.clone();
        changed["execution"]["program"]["steps"][0][field] = if field == "architecture" {
            "x86_64"
        } else {
            "another-package"
        }
        .into();
        assert_ne!(original.digest(), freeze(&changed).digest());
    }
    let mut changed = value.clone();
    changed["execution"]["program"]["steps"][0]["install"]["launch"]["artifact"]["sha256"] =
        "ab".repeat(32).into();
    assert_ne!(original.digest(), freeze(&changed).digest());
    let steps = changed["execution"]["program"]["steps"]
        .as_array_mut()
        .unwrap();
    let mut other = steps[0].clone();
    other["package"] = "second-package".into();
    steps.push(other);
    let ordered = freeze(&changed);
    changed["execution"]["program"]["steps"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_ne!(ordered.digest(), freeze(&changed).digest());
    assert!(
        decode_execution(include_bytes!("fixtures/software-obsolete.json"), &limits()).is_err()
    );
    for replacement in [serde_json::json!([]), serde_json::json!([{}])] {
        let mut changed = value.clone();
        changed["execution"]["program"]["steps"] = replacement;
        assert!(decode_execution(&serde_json::to_vec(&changed).unwrap(), &limits()).is_err());
    }
}
#[test]
fn journal_history_cannot_skip_unknown_steps_or_rewrite_exits() {
    use execution_contract::*;
    let plan = FrozenExecution::freeze(
        decode_execution(include_bytes!("fixtures/software.json"), &limits()).unwrap(),
        &limits(),
    )
    .unwrap();
    let mut progress = SoftwareProgress {
        attempt_id: AttemptId::new("attempt").unwrap(),
        content_digest: plan.digest().clone(),
        runner: Id::new("runner").unwrap(),
        checkpoints: vec![],
        elapsed_ms: 1,
        output_bytes: 0,
    };
    progress.checkpoints.push(SoftwareCheckpoint::Begin {
        step: 0,
        phase: SoftwarePhase::Before,
    });
    assert!(progress.valid_for(&plan));
    let begun = progress.clone();
    progress.checkpoints.push(SoftwareCheckpoint::End {
        duration_ms: 0,
        step: 0,
        phase: SoftwarePhase::Before,
        process: None,
        detected: Some(SoftwareState::Unknown {
            reason: SoftwareDetectionFailure::Unavailable,
        }),
        quiescent: false,
    });
    assert!(progress.valid_for(&plan));
    assert!(progress.extends(&begun));
    assert!(!progress.complete(&plan));
    progress
        .checkpoints
        .push(SoftwareCheckpoint::Complete { step: 0 });
    assert!(!progress.valid_for(&plan));
    progress.checkpoints.clear();
    assert!(!progress.extends(&begun));
}

#[test]
fn msi_reboot_completion_requires_permission_detection_and_quiescence() {
    use execution_contract::*;
    for adapter in [SoftwareKind::Msi, SoftwareKind::Pkg] {
        for allow_reboot in [false, true] {
            for code in [0, 3010, 1641, 1603] {
                for observed in [false, true] {
                    for quiet in [false, true] {
                        let mut input =
                            decode_execution(include_bytes!("fixtures/software.json"), &limits())
                                .unwrap();
                        let platform = adapter.platform();
                        input.request.target.platform = platform;
                        input.run_as = RunAs::System { platform };
                        if platform == Platform::Windows {
                            input.launch.cwd = r"C:\workspace".into();
                        }
                        let ExecutionSpec::SoftwareProgram { program } = &mut input.execution
                        else {
                            unreachable!()
                        };
                        program.steps.truncate(1);
                        let step = &mut program.steps[0];
                        step.format = if adapter == SoftwareKind::Msi {
                            SoftwareFormat::Msi {}
                        } else {
                            SoftwareFormat::Pkg {}
                        };
                        step.install.exit_codes.reboot = if adapter == SoftwareKind::Msi {
                            [1641, 3010].into_iter().collect()
                        } else {
                            Default::default()
                        };
                        step.upgrade = SoftwareUpgrade::Deny {};
                        step.allow_reboot = allow_reboot;
                        step.install.run_as = RunAs::System { platform };
                        if platform == Platform::Windows {
                            step.install.launch.cwd = r"C:\workspace".into();
                        }
                        if adapter == SoftwareKind::Msi {
                            step.detection = SoftwareDetector::MsiProduct {
                                product_code: "{12345678-1234-1234-1234-123456789ABC}".into(),
                                version: step.version.clone(),
                            };
                        }
                        let version = step.version.clone();
                        let plan = FrozenExecution::freeze(input, &limits()).unwrap();
                        let facts = ProcessEvidence {
                            content_digest: plan.digest().clone(),
                            attempt_id: AttemptId::new("attempt").unwrap(),
                            runner: Id::new("runner").unwrap(),
                            scope: ProcessScope::NotStarted {},
                            finished: true,
                            exit_code: Some(code),
                            end: ProcessEnd::Exited,
                            failure_kind: ProcessFailureKind::None,
                            quiescent: quiet,
                            stdout: vec![],
                            stderr: vec![],
                            total_output_bytes: 0,
                            quality: OutputQuality::Complete,
                        };
                        let mut progress = SoftwareProgress {
                            attempt_id: facts.attempt_id.clone(),
                            content_digest: plan.digest().clone(),
                            runner: facts.runner.clone(),
                            elapsed_ms: 1,
                            output_bytes: 0,
                            checkpoints: vec![
                                SoftwareCheckpoint::Begin {
                                    step: 0,
                                    phase: SoftwarePhase::Before,
                                },
                                SoftwareCheckpoint::End {
                                    duration_ms: 0,
                                    step: 0,
                                    phase: SoftwarePhase::Before,
                                    process: None,
                                    detected: Some(SoftwareState::Absent {}),
                                    quiescent: true,
                                },
                                SoftwareCheckpoint::Begin {
                                    step: 0,
                                    phase: SoftwarePhase::Mutation,
                                },
                                SoftwareCheckpoint::End {
                                    duration_ms: 0,
                                    step: 0,
                                    phase: SoftwarePhase::Mutation,
                                    process: Some(Box::new(facts)),
                                    detected: None,
                                    quiescent: quiet,
                                },
                                SoftwareCheckpoint::Begin {
                                    step: 0,
                                    phase: SoftwarePhase::After,
                                },
                                SoftwareCheckpoint::End {
                                    duration_ms: 0,
                                    step: 0,
                                    phase: SoftwarePhase::After,
                                    process: None,
                                    detected: Some(if observed {
                                        SoftwareState::Present { version }
                                    } else {
                                        SoftwareState::Absent {}
                                    }),
                                    quiescent: true,
                                },
                            ],
                        };
                        assert_eq!(progress.valid_for(&plan), quiet);
                        if !quiet {
                            continue;
                        }
                        assert_eq!(progress.closed(&plan), quiet);
                        progress
                            .checkpoints
                            .push(SoftwareCheckpoint::Complete { step: 0 });
                        let success = code == 0
                            || (adapter == SoftwareKind::Msi
                                && allow_reboot
                                && matches!(code, 3010 | 1641));
                        assert_eq!(progress.valid_for(&plan), success && observed && quiet);
                    }
                }
            }
        }
    }
}

#[test]
fn approved_exit_policy_and_update_invocation_are_frozen_and_closed() {
    use execution_contract::*;
    let input = decode_execution(include_bytes!("fixtures/software.json"), &limits()).unwrap();
    let original = FrozenExecution::freeze(input.clone(), &limits()).unwrap();
    let mut changed = input;
    let ExecutionSpec::SoftwareProgram { program } = &mut changed.execution else {
        unreachable!()
    };
    let step = &mut program.steps[0];
    step.install.exit_codes.success.insert(42);
    let mut update = step.install.clone();
    update.launch.argv.push(LaunchArg::Literal {
        value: "--approved-update".into(),
    });
    step.upgrade = SoftwareUpgrade::InPlace {
        invocation: Box::new(update),
    };
    let changed = FrozenExecution::freeze(changed, &limits()).unwrap();
    assert_ne!(original.digest(), changed.digest());
    let mut invalid = changed.spec().clone();
    let ExecutionSpec::SoftwareProgram { program } = &mut invalid.execution else {
        unreachable!()
    };
    program.steps[0].install.exit_codes.reboot.insert(42);
    assert!(FrozenExecution::freeze(invalid, &limits()).is_err());
    let mut old: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/software.json")).unwrap();
    old["schemaVersion"] = 5.into();
    assert!(decode_execution(&serde_json::to_vec(&old).unwrap(), &limits()).is_err());
    old["schemaVersion"] = 6.into();
    old["execution"]["program"]["steps"][0]["adapter"] = "exe".into();
    assert!(decode_execution(&serde_json::to_vec(&old).unwrap(), &limits()).is_err());
}

#[test]
fn removal_updates_require_independent_absence_and_keep_failed_work_closed() {
    use execution_contract::*;
    let mut input = decode_execution(include_bytes!("fixtures/software.json"), &limits()).unwrap();
    let ExecutionSpec::SoftwareProgram { program } = &mut input.execution else {
        unreachable!()
    };
    program.steps.truncate(1);
    let step = &mut program.steps[0];
    step.uninstall = Some(step.install.clone());
    step.upgrade = SoftwareUpgrade::UninstallThenInstall {};
    let before = SoftwareState::Present {
        version: PackageValue::new("0.9").unwrap(),
    };
    assert_eq!(
        program.mutation_phases(&program.steps[0], &before),
        vec![
            SoftwarePhase::Removal,
            SoftwarePhase::RemovalAfter,
            SoftwarePhase::Mutation
        ]
    );
    let plan = FrozenExecution::freeze(input, &limits()).unwrap();
    let attempt = AttemptId::new("same-attempt").unwrap();
    let runner = Id::new("native").unwrap();
    let facts = |code| ProcessEvidence {
        content_digest: plan.digest().clone(),
        attempt_id: attempt.clone(),
        runner: runner.clone(),
        scope: ProcessScope::NotStarted {},
        finished: true,
        exit_code: Some(code),
        end: ProcessEnd::Exited,
        failure_kind: ProcessFailureKind::None,
        quiescent: true,
        stdout: vec![],
        stderr: vec![],
        total_output_bytes: 0,
        quality: OutputQuality::Complete,
    };
    let mut progress = SoftwareProgress {
        attempt_id: attempt.clone(),
        content_digest: plan.digest().clone(),
        runner: runner.clone(),
        elapsed_ms: 40,
        output_bytes: 0,
        checkpoints: vec![
            SoftwareCheckpoint::Begin {
                step: 0,
                phase: SoftwarePhase::Before,
            },
            SoftwareCheckpoint::End {
                step: 0,
                phase: SoftwarePhase::Before,
                process: None,
                detected: Some(before),
                quiescent: true,
                duration_ms: 5,
            },
            SoftwareCheckpoint::Begin {
                step: 0,
                phase: SoftwarePhase::Removal,
            },
            SoftwareCheckpoint::End {
                step: 0,
                phase: SoftwarePhase::Removal,
                process: Some(Box::new(facts(0))),
                detected: None,
                quiescent: true,
                duration_ms: 10,
            },
        ],
    };
    assert!(progress.valid_for(&plan) && progress.resumable(&plan));
    let durable = progress.clone();
    // Successful removal exit cannot dispatch installation without an absence observation.
    progress.checkpoints.push(SoftwareCheckpoint::Begin {
        step: 0,
        phase: SoftwarePhase::Mutation,
    });
    assert!(!progress.valid_for(&plan));
    progress = durable;
    progress.checkpoints.extend([
        SoftwareCheckpoint::Begin {
            step: 0,
            phase: SoftwarePhase::RemovalAfter,
        },
        SoftwareCheckpoint::End {
            step: 0,
            phase: SoftwarePhase::RemovalAfter,
            process: None,
            detected: Some(SoftwareState::Absent {}),
            quiescent: true,
            duration_ms: 5,
        },
        SoftwareCheckpoint::Begin {
            step: 0,
            phase: SoftwarePhase::Mutation,
        },
        SoftwareCheckpoint::End {
            step: 0,
            phase: SoftwarePhase::Mutation,
            process: Some(Box::new(facts(7))),
            detected: None,
            quiescent: true,
            duration_ms: 10,
        },
        SoftwareCheckpoint::Begin {
            step: 0,
            phase: SoftwarePhase::After,
        },
        SoftwareCheckpoint::End {
            step: 0,
            phase: SoftwarePhase::After,
            process: None,
            detected: Some(SoftwareState::Absent {}),
            quiescent: true,
            duration_ms: 5,
        },
    ]);
    assert!(progress.valid_for(&plan));
    assert!(progress.closed(&plan));
    assert!(!progress.complete(&plan));
    let known_failure = progress.clone();
    progress.checkpoints.pop();
    assert!(progress.valid_for(&plan));
    assert!(!progress.closed(&plan));
    assert!(!progress.resumable(&plan));
    progress = known_failure;
    progress.elapsed_ms = 34;
    assert!(
        !progress.valid_for(&plan),
        "recovery cannot drop already consumed operation time"
    );
}

#[test]
fn cleanup_recovery_closes_resources_without_replaying_unknown_business_work() {
    use execution_contract::*;
    let mut input = decode_execution(include_bytes!("fixtures/software.json"), &limits()).unwrap();
    let ExecutionSpec::SoftwareProgram { program } = &mut input.execution else {
        unreachable!()
    };
    let step = &mut program.steps[0];
    step.format = SoftwareFormat::DmgPkg {
        volume: PackageValue::new("Approved").unwrap(),
        path: "fixed.pkg".into(),
        length: 1,
        sha256: step.payload.sha256.clone(),
        receipt: PackageValue::new("org.rss.fixture").unwrap(),
    };
    let mut cleanup = step.install.clone();
    cleanup.launch.interpreter.profile.id = Id::new("native-software-worker").unwrap();
    cleanup.timeout_ms = 900;
    cleanup.output_bytes = 6144;
    step.auxiliary
        .insert(SoftwarePhase::Attach, step.install.clone());
    step.auxiliary.insert(SoftwarePhase::Cleanup, cleanup);
    let plan = FrozenExecution::freeze(input, &limits()).unwrap();
    for unknown_mutation in [false, true] {
        let attempt = AttemptId::new("original-attempt").unwrap();
        let runner = Id::new("native").unwrap();
        let facts = |code| {
            Box::new(ProcessEvidence {
                content_digest: plan.digest().clone(),
                attempt_id: attempt.clone(),
                runner: runner.clone(),
                scope: ProcessScope::ProcessGroup { owner: 1, group: 2 },
                finished: true,
                exit_code: Some(code),
                end: ProcessEnd::Exited,
                failure_kind: ProcessFailureKind::None,
                quiescent: true,
                stdout: vec![],
                stderr: vec![],
                total_output_bytes: 0,
                quality: OutputQuality::Complete,
            })
        };
        let mut progress = SoftwareProgress {
            content_digest: plan.digest().clone(),
            attempt_id: attempt.clone(),
            runner: runner.clone(),
            elapsed_ms: 100,
            output_bytes: 0,
            checkpoints: vec![
                SoftwareCheckpoint::Begin {
                    step: 0,
                    phase: SoftwarePhase::Before,
                },
                SoftwareCheckpoint::End {
                    step: 0,
                    phase: SoftwarePhase::Before,
                    duration_ms: 5,
                    process: None,
                    detected: Some(SoftwareState::Absent {}),
                    quiescent: true,
                },
                SoftwareCheckpoint::Begin {
                    step: 0,
                    phase: SoftwarePhase::Attach,
                },
                SoftwareCheckpoint::End {
                    step: 0,
                    phase: SoftwarePhase::Attach,
                    duration_ms: 10,
                    process: Some(facts(0)),
                    detected: None,
                    quiescent: true,
                },
                SoftwareCheckpoint::Begin {
                    step: 0,
                    phase: SoftwarePhase::Mutation,
                },
            ],
        };
        if !unknown_mutation {
            progress.checkpoints.extend([
                SoftwareCheckpoint::End {
                    step: 0,
                    phase: SoftwarePhase::Mutation,
                    duration_ms: 10,
                    process: Some(facts(7)),
                    detected: None,
                    quiescent: true,
                },
                SoftwareCheckpoint::Begin {
                    step: 0,
                    phase: SoftwarePhase::Cleanup,
                },
                SoftwareCheckpoint::End {
                    step: 0,
                    phase: SoftwarePhase::Cleanup,
                    duration_ms: 10,
                    process: Some(facts(1)),
                    detected: None,
                    quiescent: true,
                },
            ]);
        }
        assert!(progress.valid_for(&plan) && !progress.closed(&plan));
        let original = progress.clone();
        let (step, sequence, timeout_ms, output_bytes) = progress.cleanup_allowance(&plan).unwrap();
        progress.checkpoints.push(SoftwareCheckpoint::CleanupBegin {
            step,
            sequence,
            timeout_ms,
            output_bytes,
        });
        assert!(progress.valid_for(&plan) && !progress.resumable(&plan));
        // Losing an acknowledgement burns the whole granted slice; restart cannot refill it.
        let next = progress.cleanup_allowance(&plan).unwrap();
        assert_eq!(next.1, sequence + 1);
        progress.checkpoints.push(SoftwareCheckpoint::CleanupEnd {
            step,
            sequence,
            duration_ms: 15,
            process: facts(0),
            resources_closed: true,
        });
        assert!(progress.valid_for(&plan) && progress.extends(&original));
        assert_eq!(progress.closed(&plan), !unknown_mutation);
        assert!(!progress.complete(&plan) && !progress.resumable(&plan));
        progress.checkpoints.push(SoftwareCheckpoint::Begin {
            step: 0,
            phase: SoftwarePhase::Mutation,
        });
        assert!(
            !progress.valid_for(&plan),
            "cleanup never authorizes business redispatch"
        );
    }
}

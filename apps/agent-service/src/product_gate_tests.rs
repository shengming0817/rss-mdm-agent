//! Real signed offers/Start and SQLite, with risk available only in this test assembly.
use super::*;
use crate::{
    backend::gate::{ProductGateProof, TrustedRisk},
    protocol_test_support as protocol, SoftwareManager, SoftwareManagerKind,
};
use agent_api::runtime as wire;
use agent_client::{Clock, OpenMode};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply};
use sha2::{Digest as _, Sha256};

struct Case {
    _server: protocol::Server,
    _root: protocol::Root,
    service: DeviceService<protocol::Secrets>,
    offer: Offer,
    selection: Selection,
}
impl Case {
    async fn new(level: Option<RiskLevel>) -> Self {
        Self::with_offer_lifetime(level, 60).await
    }
    async fn with_offer_lifetime(level: Option<RiskLevel>, seconds: i64) -> Self {
        let server = protocol::Server::new().await;
        let root = protocol::Root::new();
        let clock = SystemClock::new().unwrap();
        server.time.set(clock.now().unwrap());
        {
            let mut data = server.data.lock().unwrap();
            data.software(1, true);
            let wire::TaskPayload::SoftwareTaskSpec(mut spec) =
                data.offer.as_ref().unwrap().payload.clone()
            else {
                panic!("software")
            };
            spec.expires_at = clock.now().unwrap() + seconds;
            data.offer = Some(data.signed(wire::TaskPayload::SoftwareTaskSpec(spec)));
            data.explicit_offers = true;
        }
        let mut client = server.client(&root, OpenMode::Create);
        server.register(&mut client).await;
        drop(client);
        let image = installation_security::Artifact {
            path: "/bin/sh".into(),
            sha256: format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())),
            cdhash: None,
        };
        let client = Client::open(
            &root.path,
            server.config(),
            OpenMode::Existing,
            server.secrets.clone(),
            clock.clone(),
        )
        .unwrap();
        platform_private_storage::directory(&root.path.join("materials")).unwrap();
        let mut service = DeviceService::assemble(
            client,
            &root.path.join("execution.sqlite"),
            crate::ProductionStartup::Create,
            ExecutionConfig {
                work_root: root.path.clone(),
                material_root: root.path.join("materials"),
                interpreters: vec![Interpreter {
                    profile: wire::ExecutorProfile::PosixSh,
                    image: image.clone(),
                }],
                managers: vec![],
                processes: 1,
            },
            clock.clone(),
            UserResources {
                image,
                work_roots: Default::default(),
            },
            execution_runner::staging::MaterialRoot::open_fixture,
        )
        .unwrap();
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        service.drive(&mut commands).await.unwrap();
        let offer = service
            .client
            .retained_offer(server.data.lock().unwrap().task)
            .unwrap();
        let view = offered(&offer).unwrap();
        assert!(service.waiting.is_some());
        service.core.available_risk = level.map(|level| {
            TrustedRisk::fixture(
                &offer,
                ExecutionRisk {
                    level,
                    source: execution_contract::RiskSource::Policy {
                        policy: plan::reference("backend-risk-policy", "1").unwrap(),
                    },
                    expires_at_unix_ms: clock.millis().unwrap() + 120_000,
                },
            )
        });
        let selection = Selection {
            request: view.request,
            task: view.task,
            attempt: view.attempt,
            revision: view.revision,
            subject: execution_ipc::host::current_subject().unwrap(),
            session: 42,
            binding: plan::id("user/42/test-login").unwrap(),
            origin: ClientOrigin::Ai {
                context: execution_contract::NativeContextRef {
                    id: plan::id("pending").unwrap(),
                    generation: plan::id("initial").unwrap(),
                },
                config: plan::reference("ai", "1").unwrap(),
                conversation: plan::id("conversation").unwrap(),
                tool_call: plan::id("tool-call").unwrap(),
            },
        };
        let mut case = Self {
            _server: server,
            _root: root,
            service,
            offer,
            selection,
        };
        case.employee("initial").await;
        case
    }
    fn selection(&self, human: bool) -> Selection {
        let mut selection = Selection::from_record(&BackendRequest {
            action: execution_contract::PreparedAction::Offered {
                offer: offered(&self.offer).unwrap(),
            },
            trigger: self
                .selection
                .trigger(&self.service.core.host.binding.device)
                .unwrap(),
            risk: None,
            confirmation: None,
            revision: 1,
            state: BackendRequestState::Failed,
            failure: Some(BackendRequestFailure::RiskUnknown),
        })
        .unwrap();
        if human {
            selection.origin = ClientOrigin::Desktop {};
        }
        selection
    }
    fn record(&self) -> BackendRequest {
        self.service
            .core
            .app
            .backend_request(&self.service.core.caller(), &self.selection.request)
            .unwrap()
            .unwrap()
    }
}

#[tokio::test]
async fn terminal_waiting_releases_capacity_without_changing_the_original_request() {
    for cause in 0..4 {
        let mut c = Case::new(match cause {
            0 => None,
            1 => Some(RiskLevel::Three),
            _ => Some(RiskLevel::Two),
        })
        .await;
        if cause == 2 {
            c.service.core.available_risk = Some(TrustedRisk::fixture(
                &c.offer,
                ExecutionRisk {
                    level: RiskLevel::Two,
                    source: execution_contract::RiskSource::Policy {
                        policy: plan::reference("backend-risk-policy", "1").unwrap(),
                    },
                    expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() - 1,
                },
            ));
        }
        let ai = c.selection(false);
        c.service.core.start_task(ai).unwrap();
        if cause == 3 {
            c.service
                .core
                .transition(
                    &c.selection.request,
                    BackendRequestState::Cancelled,
                    Some(BackendRequestFailure::Interrupted),
                )
                .unwrap();
        }
        let original = c.record();
        assert!(matches!(
            original.state,
            BackendRequestState::Failed | BackendRequestState::Cancelled
        ));
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        c.service.drive(&mut commands).await.unwrap();
        assert!(c.service.waiting.is_none());
        assert!(c.service.core.available.is_none());
        assert!(c.service.core.available_risk.is_none());
        assert!(c.service.core.selected.is_none());
        assert_eq!(c.record(), original);
        assert!(c
            .service
            .client
            .pending_tasks(c.service.client.configuration().limits.pending_tasks)
            .unwrap()
            .is_empty());
        assert!(c
            .service
            .core
            .app
            .service_tasks(None, 64)
            .unwrap()
            .items
            .is_empty());
        assert!(c._server.data.lock().unwrap().start_ops.is_empty());
        let replay = c.selection(false);
        c.service.core.start_task(replay).unwrap();
        assert_eq!(c.record(), original);
        assert!(c.service.core.selected.is_none());

        {
            let mut data = c._server.data.lock().unwrap();
            data.task = uuid::Uuid::new_v4();
            data.attempt = uuid::Uuid::new_v4();
            data.software(1, true);
        }
        c.service.next_network = std::time::Instant::now();
        c.service.drive(&mut commands).await.unwrap();
        let next = c.service.core.available.as_ref().unwrap().clone();
        assert_ne!(next.request, original.action.offer().unwrap().request);
        assert!(c.service.waiting.is_some());
        let mut human = c.selection(true);
        human.request = next.request;
        human.task = next.task;
        human.attempt = next.attempt;
        human.revision = next.revision;
        assert!(matches!(
            c.service.core.start_task(human).unwrap(),
            Reply::Queued {
                confirmation_required: false,
                ..
            }
        ));
        assert_eq!(c.record(), original);
        assert!(c._server.data.lock().unwrap().start_ops.is_empty());
    }
}

#[tokio::test]
async fn failed_abandonment_retries_the_same_attempt_after_releasing_waiting() {
    let mut c = Case::new(None).await;
    let ai = c.selection(false);
    c.service.core.start_task(ai).unwrap();
    let original = c.record();
    c._server.data.lock().unwrap().result_failure = true;
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    assert_eq!(
        c.service.drive(&mut commands).await,
        Err(Error::Unavailable)
    );
    assert!(c.service.waiting.is_none());
    assert!(c.service.core.available.is_none());
    assert!(c.service.core.available_risk.is_none());
    assert!(c.service.core.selected.is_none());
    assert_eq!(c.record(), original);
    assert_eq!(
        c.service
            .client
            .pending_tasks(c.service.client.configuration().limits.pending_tasks)
            .unwrap(),
        vec![c.offer.task_id()]
    );
    let results = c._server.data.lock().unwrap().results.clone();
    assert_eq!(results.len(), 1);
    assert_eq!(
        results.values().next().unwrap()["attemptId"],
        c.offer.attempt_id().to_string()
    );
    c.service.next_network = std::time::Instant::now();
    c.service.drive(&mut commands).await.unwrap();
    assert!(c
        .service
        .client
        .pending_tasks(c.service.client.configuration().limits.pending_tasks)
        .unwrap()
        .is_empty());
    assert!(c.service.waiting.is_none());
    assert_eq!(c.record(), original);
    assert!(c
        .service
        .core
        .app
        .service_tasks(None, 64)
        .unwrap()
        .items
        .is_empty());
    let data = c._server.data.lock().unwrap();
    assert_eq!(data.result_calls, 2);
    assert_eq!(data.results, results);
    assert!(data.start_ops.is_empty());
}

#[tokio::test]
async fn production_preparation_applies_the_risk_matrix_and_human_has_no_gate() {
    for level in [
        Some(RiskLevel::Zero),
        Some(RiskLevel::One),
        Some(RiskLevel::Two),
        Some(RiskLevel::Three),
        None,
    ] {
        let mut c = Case::new(level).await;
        let selection = c.selection(false);
        c.service.core.start_task(selection).unwrap();
        let r = c.record();
        let expected = match level {
            Some(RiskLevel::One) => BackendRequestState::Ready,
            Some(RiskLevel::Two) => BackendRequestState::AwaitingConfirmation,
            _ => BackendRequestState::Failed,
        };
        assert_eq!(r.state, expected);
        assert!(matches!(r.trigger, BackendTrigger::Ai { .. }));
        assert!(r.confirmation.is_none());
        assert_eq!(
            c.service.core.selected.is_some(),
            expected == BackendRequestState::Ready
        );
        assert!(c._server.data.lock().unwrap().start_ops.is_empty());
        if expected == BackendRequestState::Failed {
            assert_eq!(
                r.failure,
                Some(if level.is_none() {
                    BackendRequestFailure::RiskUnknown
                } else {
                    BackendRequestFailure::RiskBlocked
                })
            );
        }
    }
    let mut c = Case::new(None).await;
    let human = c.selection(true);
    c.service.core.start_task(human).unwrap();
    let r = c.record();
    assert_eq!(r.state, BackendRequestState::Ready);
    assert!(matches!(r.trigger, BackendTrigger::Human { .. }));
    assert!(r.risk.is_none() && r.confirmation.is_none());
    assert!(c.service.core.selected.is_some());
}

#[tokio::test]
async fn one_desktop_confirmation_retains_ai_origin_and_cannot_be_replaced_by_start() {
    let mut c = Case::new(Some(RiskLevel::Two)).await;
    let ai = c.selection(false);
    c.service.core.start_task(ai).unwrap();
    let original = c.record();
    let replay = c.selection(false);
    assert!(matches!(
        c.service.core.start_task(replay).unwrap(),
        Reply::Pending { .. }
    ));
    let human = c.selection(true);
    assert!(c.service.core.start_task(human).is_err());
    assert_eq!(c.record(), original);
    for mismatch in 0..4 {
        let mut human = c.selection(true);
        match mismatch {
            0 => human.subject = "another-user".into(),
            1 => human.binding = plan::id("user/43/other-login").unwrap(),
            2 => human.revision = Digest::new("b".repeat(64)).unwrap(),
            _ => human.attempt = plan::id("other-attempt").unwrap(),
        }
        assert!(c.service.core.confirm_task(human).is_err());
        assert_eq!(c.record(), original);
    }
    let human = c.selection(true);
    assert!(matches!(
        c.service.core.confirm_task(human).unwrap(),
        Reply::Queued {
            confirmation_required: false,
            ..
        }
    ));
    let confirmed = c.record();
    assert_eq!(confirmed.trigger, original.trigger);
    assert_eq!(confirmed.state, BackendRequestState::Ready);
    assert_eq!(confirmed.revision, 2);
    let human = c.selection(true);
    c.service.core.confirm_task(human).unwrap();
    assert_eq!(c.record(), confirmed);
    let mut forged = confirmed.clone();
    forged.revision += 1;
    forged.risk.as_mut().unwrap().level = RiskLevel::Zero;
    assert!(c
        .service
        .core
        .app
        .record_backend_request(&c.service.core.caller(), Some(&confirmed), &forged)
        .is_err());
    forged = confirmed.clone();
    forged.revision += 1;
    forged.confirmation = None;
    assert!(c
        .service
        .core
        .app
        .record_backend_request(&c.service.core.caller(), Some(&confirmed), &forged)
        .is_err());
}

#[tokio::test]
async fn signed_start_needs_the_current_confirmed_request_and_frozen_action() {
    let mut c = Case::new(Some(RiskLevel::Two)).await;
    let ai = c.selection(false);
    c.service.core.start_task(ai).unwrap();
    let human = c.selection(true);
    c.service.core.confirm_task(human).unwrap();
    c.service
        .core
        .transition(&c.selection.request, BackendRequestState::Submitting, None)
        .unwrap();
    let record = c.record();
    let materials = c.service.client.prepare(&c.offer).await.unwrap();
    c.service.client.received(&c.offer).await.unwrap();
    let start = c
        .service
        .client
        .start_user_initiated(&c.offer, &materials)
        .await
        .unwrap();
    // Test the proof's binding independently of native package staging/physical execution.
    let mut spec: ExecutionInput = serde_json::from_str(include_str!(
        "../../../crates/execution-contract/tests/fixtures/plan.json"
    ))
    .unwrap();
    spec.request.request_id = record.action.offer().unwrap().request.clone();
    spec.request.initiator = Initiator::Backend {
        task: record.action.offer().unwrap().task.clone(),
        attempt: record.action.offer().unwrap().attempt.clone(),
        trigger: record.trigger.clone(),
    };
    let frozen = FrozenExecution::freeze(spec, &plan::storage_limits().input).unwrap();
    let now = c.service.core.host.clock.millis().unwrap();
    let risk = c.service.core.available_risk.as_ref();
    let proof = ProductGateProof::verify_start(&c.offer, &start, &frozen, Some(&record), risk, now)
        .unwrap();
    assert!(proof.verify(&frozen, now).is_ok());
    assert!(proof.verify(&frozen, proof.until()).is_err());
    assert!(
        ProductGateProof::verify_start(&c.offer, &start, &frozen, Some(&record), None, now)
            .is_err()
    );
    let mut cancelled = record.clone();
    cancelled.state = BackendRequestState::Cancelled;
    assert!(
        ProductGateProof::verify_start(&c.offer, &start, &frozen, Some(&cancelled), risk, now)
            .is_err()
    );
    let mut changed = frozen.spec().clone();
    changed.budget.total_timeout_ms -= 1;
    let changed = FrozenExecution::freeze(changed, &plan::storage_limits().input).unwrap();
    assert!(proof.verify(&changed, now).is_err());
    assert!(ProductGateProof::verify_start(&c.offer, &start, &frozen, None, risk, now).is_err());
    assert!(ProductGateProof::verify_start(
        &c.offer,
        &start,
        &frozen,
        Some(&record),
        risk,
        record.confirmation.as_ref().unwrap().expires_at_unix_ms
    )
    .is_err());
}

#[tokio::test]
async fn material_gc_scans_every_journal_page_and_waits_for_delivery_confirmation() {
    // The real signed offer must remain valid throughout the large historical-journal scan.
    let mut c = Case::with_offer_lifetime(None, 600).await;
    // This case proves GC across 32,768 historical materials, not short Start expiry.
    // Owner reconciliation may scan that history while the HTTP response is pending.
    c._server.data.lock().unwrap().start_validity_seconds = 600;
    let bytes = b"original journal material";
    let digest = execution_contract::Digest::new(format!("{:x}", Sha256::digest(bytes))).unwrap();
    let source = c
        .service
        .core
        .material_root
        .publish(None, bytes.as_slice(), &digest, bytes.len() as u64)
        .unwrap();
    let caller = c.service.core.caller();
    let consumer = plan::id("backend-results").unwrap();
    let mut requests = Vec::new();
    for index in 0..65 {
        let mut spec: ExecutionInput = serde_json::from_str(include_str!(
            "../../../crates/execution-contract/tests/fixtures/plan.json"
        ))
        .unwrap();
        spec.request.authority = c.service.core.host.binding.authority.clone();
        spec.request.actor = caller.actor.clone();
        spec.request.target.device = c.service.core.host.binding.device.clone();
        spec.request.request_id = RequestId::new(format!("gc-page-{index:03}")).unwrap();
        spec.request.initiator = Initiator::Backend {
            task: plan::id("gc-task").unwrap(),
            attempt: plan::id("gc-attempt").unwrap(),
            trigger: BackendTrigger::Automatic {},
        };
        spec.request.delegation = None;
        spec.launch.artifact.sha256 = digest.clone();
        // Register the original input without bound dispatch material, then cancel through
        // the actual journal API. No attempt starts; delivery still requires confirmation.
        spec.run_as = RunAs::System {
            platform: spec.request.target.platform,
        };
        spec.session_requirement = SessionRequirement::NotRequired {};
        if index < 64 {
            // More than 32,768 distinct historical paths are already absent. They must
            // not consume the bounded deletion-candidate budget of this small live root.
            spec.request.target.platform = Platform::Macos;
            spec.request.target.scope = TargetScope::Device {};
            spec.run_as = RunAs::System {
                platform: Platform::Macos,
            };
            let invocation = SoftwareInvocation {
                launch: spec.launch.clone(),
                run_as: spec.run_as.clone(),
                session_requirement: spec.session_requirement.clone(),
                timeout_ms: 1000,
                output_bytes: 1024,
                exit_codes: SoftwareExitCodes {
                    success: [0].into(),
                    reboot: Default::default(),
                },
            };
            let materials = (0..512)
                .map(|member| {
                    let sha =
                        execution_contract::Digest::new(format!("{:064x}", index * 512 + member))
                            .unwrap();
                    SoftwareMaterial {
                        path: c
                            .service
                            .core
                            .config
                            .material_root
                            .join("system")
                            .join(sha.as_str())
                            .to_str()
                            .unwrap()
                            .into(),
                        artifact: ExactArtifactRef {
                            resource: plan::reference("historical-material", "1").unwrap(),
                            sha256: sha,
                        },
                    }
                })
                .collect();
            spec.request.parameters.clear();
            spec.request.operation.action = plan::id("software.install").unwrap();
            spec.launch.interpreter.profile =
                plan::reference("native-software-sequence", "1").unwrap();
            spec.execution = ExecutionSpec::SoftwareProgram {
                program: Box::new(SoftwareProgram {
                    definition_digest: digest.clone(),
                    intent: SoftwareOperation::Install,
                    steps: vec![SoftwareProgramStep {
                        format: SoftwareFormat::Pkg {},
                        package: PackageValue::new("historical").unwrap(),
                        version: PackageValue::new("1").unwrap(),
                        architecture: PackageValue::new("aarch64").unwrap(),
                        payload: invocation.launch.artifact.clone(),
                        materials,
                        signatures: vec![],
                        upgrade: SoftwareUpgrade::Deny {},
                        install: invocation,
                        auxiliary: Default::default(),
                        uninstall: None,
                        detection: SoftwareDetector::PkgReceipt {
                            receipt: "example.receipt".into(),
                            version: PackageValue::new("1").unwrap(),
                        },
                        allow_downgrade: false,
                        allow_reboot: false,
                    }],
                }),
            };
        }
        let input = FrozenExecution::freeze(spec, &plan::storage_limits().input).unwrap();
        assert!(matches!(
            c.service.core.app.request_execution(&caller, &input),
            Err(execution_app::Error::Unbound)
        ));
        let status = c
            .service
            .core
            .app
            .cancel(&caller, &input.spec().request.request_id)
            .unwrap();
        assert!(crate::backend::settled(&status));
        requests.push(input.spec().request.request_id.clone());
        if index < 64 {
            for item in c
                .service
                .core
                .app
                .service_delivery(&input.spec().request.request_id, &consumer, 64)
                .unwrap()
            {
                c.service
                    .core
                    .app
                    .service_confirm(
                        &input.spec().request.request_id,
                        &consumer,
                        &item.receipt.event_id,
                    )
                    .unwrap();
            }
        }
    }
    assert!(c
        .service
        .core
        .app
        .service_tasks(None, 64)
        .unwrap()
        .next
        .is_some());
    c.service.reclaim_materials().unwrap();
    assert!(
        source.exists(),
        "the last page still retains the shared object for delivery retry"
    );
    let last = requests.last().unwrap();
    let pending = c
        .service
        .core
        .app
        .service_delivery(last, &consumer, 64)
        .unwrap();
    assert!(!pending.is_empty());
    for item in pending {
        c.service
            .core
            .app
            .service_confirm(last, &consumer, &item.receipt.event_id)
            .unwrap();
    }
    // Preview reuses a settled digest before awaiting a real signed Start. Network ticks
    // reconcile journal facts while that unregistered preview remains usable.
    c._server.time.set(c.service.core.host.clock.now().unwrap());
    let materials = c.service.client.prepare(&c.offer).await.unwrap();
    // Keep the command producer alive: a closed channel is a real service stop.
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    crate::service::network(
        async {
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            assert!(source.exists());
            c.service
                .client
                .start_user_initiated(&c.offer, &materials)
                .await
        },
        &mut c.service.core,
        &mut commands,
    )
    .await
    .unwrap();
    assert!(source.exists());
    // A corrupt device coordinate must not hide a row from the complete GC scan.
    let sql = rusqlite::Connection::open(c._root.path.join("execution.sqlite")).unwrap();
    let original: Vec<u8> = sql
        .query_row(
            "SELECT plan FROM executions WHERE request_id=?1",
            [last.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let mut corrupted: serde_json::Value = serde_json::from_slice(&original).unwrap();
    corrupted["request"]["target"]["device"] = "foreign-device".into();
    sql.execute(
        "UPDATE executions SET plan=?1 WHERE request_id=?2",
        rusqlite::params![serde_json::to_vec(&corrupted).unwrap(), last.as_str()],
    )
    .unwrap();
    assert!(c.service.reclaim_materials().is_err());
    assert!(
        source.exists(),
        "failed or incomplete journal proof preserves all candidates"
    );
    let prior_claims = c._server.data.lock().unwrap().claim_ops.len();
    c.service.next_network = std::time::Instant::now();
    c.service.drive(&mut commands).await.unwrap();
    assert!(
        c._server.data.lock().unwrap().claim_ops.len() > prior_claims,
        "a deferred GC must still receive remote cancellation/offer messages"
    );
    assert!(source.exists());
    sql.execute(
        "UPDATE executions SET plan=?1 WHERE request_id=?2",
        rusqlite::params![original, last.as_str()],
    )
    .unwrap();
    drop(sql);
    // Fixed local resources may reuse a digest previously published as settled content.
    let original_image = c.service.core.config.interpreters[0].image.clone();
    let pinned = installation_security::Artifact {
        path: source.clone(),
        sha256: digest.as_str().into(),
        cdhash: None,
    };
    c.service.core.config.interpreters[0].image = pinned.clone();
    c.service.reclaim_materials().unwrap();
    assert!(source.exists(), "the configured interpreter remains pinned");
    c.service.core.config.interpreters[0].image = original_image;
    c.service.core.config.managers.push(SoftwareManager {
        executor: SoftwareManagerKind::PackageInstaller,
        image: pinned,
    });
    c.service.reclaim_materials().unwrap();
    assert!(source.exists(), "the configured manager remains pinned");
    c.service.core.config.managers.clear();
    // An unresolved original journal input can retain an interpreter no longer in config.
    let other_bytes = b"separate pending content";
    let other_digest =
        execution_contract::Digest::new(format!("{:x}", Sha256::digest(other_bytes))).unwrap();
    let other_source = c
        .service
        .core
        .material_root
        .publish(
            None,
            other_bytes.as_slice(),
            &other_digest,
            other_bytes.len() as u64,
        )
        .unwrap();
    let mut pending_spec = c
        .service
        .core
        .app
        .service_frozen_input(last)
        .unwrap()
        .spec()
        .clone();
    pending_spec.request.request_id = RequestId::new("gc-interpreter-pending").unwrap();
    pending_spec.launch.artifact.sha256 = other_digest;
    pending_spec.launch.interpreter.artifact.sha256 = digest;
    let pending_input =
        FrozenExecution::freeze(pending_spec, &plan::storage_limits().input).unwrap();
    assert!(matches!(
        c.service
            .core
            .app
            .request_execution(&caller, &pending_input),
        Err(execution_app::Error::Unbound)
    ));
    c.service
        .core
        .app
        .cancel(&caller, &pending_input.spec().request.request_id)
        .unwrap();
    c.service.reclaim_materials().unwrap();
    assert!(
        source.exists(),
        "the unresolved interpreter digest remains protected"
    );
    for item in c
        .service
        .core
        .app
        .service_delivery(&pending_input.spec().request.request_id, &consumer, 64)
        .unwrap()
    {
        c.service
            .core
            .app
            .service_confirm(
                &pending_input.spec().request.request_id,
                &consumer,
                &item.receipt.event_id,
            )
            .unwrap();
    }
    c.service.reclaim_materials().unwrap();
    assert!(!source.exists());
    assert!(!other_source.exists());
    c.service.reclaim_materials().unwrap();
}

#[tokio::test]
async fn material_owner_rejects_a_replaced_journal_and_keeps_original_material() {
    let c = Case::new(None).await;
    let root = c.service.core.config.material_root.clone();
    let journal = c._root.path.join("execution.sqlite");
    use sha2::{Digest as _, Sha256};
    let owner = execution_contract::Digest::new(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                &c.service.core.host.binding.authority,
                &c.service.core.host.binding.device
            ))
            .unwrap()
        )
    ))
    .unwrap();
    let _server = c._server;
    let saved_root = c._root;
    drop(c.service);
    std::fs::rename(&journal, journal.with_extension("retained.sqlite")).unwrap();
    platform_private_storage::write_new(&journal, b"different journal identity").unwrap();
    assert!(matches!(
        execution_runner::staging::MaterialRoot::open_fixture(&root, &owner, &journal),
        Err(execution_app::Error::Configuration)
    ));
    assert!(root.join(".owner").exists());
    drop(saved_root);
}

#[cfg(unix)]
#[tokio::test]
async fn service_assembly_creates_and_reopens_journal_with_its_actual_budgets() {
    let server = protocol::Server::new().await;
    let root = protocol::Root::new();
    let clock = SystemClock::new().unwrap();
    server.time.set(clock.now().unwrap());
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    drop(client);
    let limits = plan::storage_limits();
    assert_eq!(limits.input.max_output_bytes, 16_777_216);
    let image = installation_security::Artifact {
        path: "/bin/sh".into(),
        sha256: format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())),
        cdhash: None,
    };
    let config = ExecutionConfig {
        work_root: root.path.clone(),
        material_root: root.path.join("materials"),
        interpreters: vec![Interpreter {
            profile: wire::ExecutorProfile::PosixSh,
            image: image.clone(),
        }],
        managers: vec![],
        processes: 1,
    };
    platform_private_storage::directory(&config.material_root).unwrap();
    let journal = root.path.join("execution.sqlite");
    // Exercise the actual journal, NativeRunner and Enterprise authority, selecting only
    // explicit unprivileged material storage. This does not prove root ownership or IPC.
    for startup in [
        crate::ProductionStartup::Create,
        crate::ProductionStartup::Open,
    ] {
        let client = agent_client::Client::open(
            &root.path,
            server.config(),
            OpenMode::Existing,
            server.secrets.clone(),
            clock.clone(),
        )
        .unwrap();
        drop(
            DeviceService::assemble(
                client,
                &journal,
                startup,
                config.clone(),
                clock.clone(),
                UserResources {
                    image: image.clone(),
                    work_roots: Default::default(),
                },
                execution_runner::staging::MaterialRoot::open_fixture,
            )
            .unwrap(),
        );
    }
    assert!(journal.is_file());
}

#[tokio::test]
async fn renewal_and_revoked_restart_keep_the_existing_journal_authority() {
    let mut case = Case::new(None).await;
    let selected = case.selection(false);
    case.service.core.start_task(selected).unwrap();
    let binding = case.service.client.registration().unwrap();
    let records = case
        .service
        .core
        .app
        .backend_requests(&case.service.core.caller())
        .unwrap();
    assert!(!records.is_empty());
    case._server.data.lock().unwrap().activation_failure = true;
    assert_eq!(
        case.service.client.renew_certificate().await.unwrap_err(),
        Error::ActivationUnknown
    );
    let config = case.service.core.config.clone();
    let helpers = case.service.core.helpers.clone();
    let journal = case._root.path.join("execution.sqlite");
    let cfg = case._server.config();
    let secrets = case._server.secrets.clone();
    let clock = SystemClock::new().unwrap();
    drop(case.service);
    let mut client = Client::open(
        &case._root.path,
        cfg.clone(),
        OpenMode::Existing,
        secrets.clone(),
        clock.clone(),
    )
    .unwrap();
    client.renew_certificate().await.unwrap();
    assert_eq!(client.registration().unwrap(), binding);
    let service = DeviceService::assemble(
        client,
        &journal,
        ProductionStartup::Open,
        config.clone(),
        clock.clone(),
        helpers.clone(),
        execution_runner::staging::MaterialRoot::open_fixture,
    )
    .unwrap();
    assert_eq!(
        service
            .core
            .app
            .backend_requests(&service.core.caller())
            .unwrap(),
        records
    );
    drop(service);
    let mut client = Client::open(
        &case._root.path,
        cfg.clone(),
        OpenMode::Existing,
        secrets.clone(),
        clock.clone(),
    )
    .unwrap();
    case._server.data.lock().unwrap().denied = true;
    client
        .queue_report(
            "inventory",
            wire::ReportBody::Failed {
                code: wire::FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    assert_eq!(
        client.flush_reports(1).await.unwrap_err(),
        Error::CredentialRevoked
    );
    drop(client);
    let client = Client::open(
        &case._root.path,
        cfg,
        OpenMode::Existing,
        secrets,
        clock.clone(),
    )
    .unwrap();
    assert_eq!(
        client.enrollment_status().unwrap().phase,
        agent_client::EnrollmentPhase::Rejected
    );
    let mut service = DeviceService::assemble(
        client,
        &journal,
        ProductionStartup::Open,
        config,
        clock,
        helpers,
        execution_runner::staging::MaterialRoot::open_fixture,
    )
    .unwrap();
    assert_eq!(
        service
            .core
            .app
            .backend_requests(&service.core.caller())
            .unwrap(),
        records
    );
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    select_pki_refresh(&case._root.path);
    assert_eq!(
        service.drive(&mut commands).await.unwrap_err(),
        Error::CredentialRevoked
    );
    service.drive_error(Error::CredentialRevoked);
    service.drive(&mut commands).await.unwrap(); // Local reconcile remains available.
    assert!(service.core.host.revoked.load(Ordering::Acquire));
    let recovered = service
        .core
        .app
        .backend_requests(&service.core.caller())
        .unwrap();
    assert_eq!(recovered.len(), records.len());
    assert_eq!(
        recovered[0].action.offer().unwrap().request,
        records[0].action.offer().unwrap().request
    );
    assert_eq!(
        recovered[0].action.offer().unwrap().attempt,
        records[0].action.offer().unwrap().attempt
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn running_owner_publishes_current_credential_and_safe_status_without_execution_authority() {
    use execution_ipc::host::{
        CredentialStatus, Handler, PeerPolicy, Readiness, RegistrationPhase,
    };
    let case = Case::new(None).await;
    let data = case._server.data.clone();
    let image = case.service.core.config.interpreters[0].image.clone();
    let policy = PeerPolicy {
        images: vec![image],
        subjects: vec!["0".into()],
        interactive: false,
    };
    let mut owner = case.service.spawn(policy, None).unwrap();
    assert!(
        matches!(owner.registration_status(), Reply::RegistrationStatus { value } if value.credential == CredentialStatus::Active)
    );
    data.lock().unwrap().denied = true;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if matches!(owner.registration_status(), Reply::RegistrationStatus { value }
                if value.registered && value.phase == RegistrationPhase::Rejected && value.credential == CredentialStatus::Revoked) { break; }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    let reply = tokio::task::spawn_blocking(move || {
        let status = owner.service_status();
        owner.stop().unwrap();
        status
    })
    .await
    .unwrap();
    assert!(
        matches!(reply, Reply::ServiceStatus { value } if value.readiness == Readiness::NotReady)
    );
}

impl Case {
    async fn policy_call(&mut self, request: execution_ipc::host::Request) -> Reply {
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.service.core.enqueue_policy(Command {
            fence_epoch: self
                .service
                .core
                .fences
                .current(&self.selection.subject)
                .unwrap(),
            request: LocalRequest::operation(request),
            subject: self.selection.subject.clone(),
            session: self.selection.session,
            binding: Some(self.selection.binding.clone()),
            reply,
        });
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        self.service.drive_policy(&mut commands).await.unwrap();
        receive.await.unwrap()
    }
    fn context(&self) -> execution_contract::NativeContextRef {
        let ClientOrigin::Ai { context, .. } = &self.selection.origin else {
            panic!("AI binding")
        };
        context.clone()
    }
    async fn employee(&mut self, generation: &str) {
        self.policy_call(execution_ipc::host::Request::RevokeContext {})
            .await;
        let Reply::ContextBound { value } = self
            .policy_call(execution_ipc::host::Request::BindContext {
                expected: None,
                value: execution_ipc::host::NativeContextSession {
                    credential: Some(zeroize::Zeroizing::new("c".repeat(64))),
                    credential_origin: Some(self.service.client.configuration().origin.to_string()),
                    user: Some(wire::SelfServiceUser {
                        tenant_id: self.service.client.configuration().tenant,
                        instance_id: uuid::Uuid::from_u128(2),
                        principal_id: uuid::Uuid::from_u128(3),
                    }),
                    generation: plan::id(generation).unwrap(),
                    expires_at_unix_ms: self.service.core.host.clock.millis().unwrap() + 120_000,
                },
            })
            .await
        else {
            panic!("context binding")
        };
        let ClientOrigin::Ai { context, .. } = &mut self.selection.origin else {
            panic!("AI binding")
        };
        *context = value;
    }
}
#[tokio::test]
async fn native_user_handoff_rejects_another_origin_before_any_http_or_refresh() {
    use execution_ipc::host::{NativeContextSession, Request};
    for refresh in [false, true] {
        let mut c = Case::new(None).await;
        let context = c.context();
        if !refresh {
            c.policy_call(Request::RevokeContext {}).await;
        }
        let reply = c
            .policy_call(Request::BindContext {
                expected: refresh.then(|| context.id.clone()),
                value: NativeContextSession {
                    credential: Some(zeroize::Zeroizing::new("c".repeat(64))),
                    credential_origin: Some("https://other-authority.invalid".into()),
                    user: Some(wire::SelfServiceUser {
                        tenant_id: c.service.client.configuration().tenant,
                        instance_id: uuid::Uuid::from_u128(2),
                        principal_id: uuid::Uuid::from_u128(3),
                    }),
                    generation: plan::id(if refresh { "initial" } else { "new-generation" })
                        .unwrap(),
                    expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 60_000,
                },
            })
            .await;
        assert!(matches!(reply, Reply::Rejected));
        assert!(c._server.data.lock().unwrap().employee_calls.is_empty());
        if !refresh {
            assert!(matches!(
                c.policy_call(Request::PolicyDirectory {
                    context,
                    after: None,
                    origin: c.selection.origin.clone(),
                })
                .await,
                Reply::Rejected
            ));
            assert!(c._server.data.lock().unwrap().employee_calls.is_empty());
        }
    }
}

#[tokio::test]
async fn policy_confirmation_is_single_exact_and_bound_to_employee_generation() {
    use execution_ipc::host::Request;
    let mut c = Case::new(Some(RiskLevel::Two)).await;
    let directory = protocol::policy_directory_fixture();
    c._server.data.lock().unwrap().self_service_directory =
        Some(wire::SelfServiceDirectoryResponse {
            items: vec![directory.clone()],
            next_cursor: None,
            as_of: c._server.time.now().unwrap(),
        });
    c.employee("generation-one").await;
    let mut choice = PolicyChoice {
        request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
        policy_revision: directory.policy_revision,
        policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
        parameters: std::collections::BTreeMap::from([("name".into(), serde_json::json!("Acme"))]),
    };
    let invoke = Request::InvokePolicy {
        context: c.context(),
        choice: choice.clone(),
        origin: c.selection.origin.clone(),
    };
    let Reply::PolicyRequest { value: pending } = c.policy_call(invoke.clone()).await else {
        panic!("policy preparation")
    };
    assert_eq!(pending.state, BackendRequestState::AwaitingConfirmation);
    assert!(c._server.data.lock().unwrap().invocations.is_empty());
    assert_eq!(pending.risk.as_ref().unwrap().level, RiskLevel::Two);
    assert_eq!(pending.action.request(), &choice.request);
    let Reply::PolicyRequest { value: replay } = c.policy_call(invoke.clone()).await else {
        panic!("original replay")
    };
    assert_eq!(pending, replay);
    // No caller may override an administrator Fixed field or change the original choice.
    let mut changed = choice.clone();
    changed
        .parameters
        .insert("path".into(), serde_json::json!("/Library"));
    assert!(matches!(
        c.policy_call(Request::InvokePolicy {
            context: c.context(),
            choice: changed,
            origin: c.selection.origin.clone()
        })
        .await,
        Reply::Rejected
    ));
    c.employee("generation-two").await;
    assert!(matches!(
        c.policy_call(Request::ConfirmPolicy {
            context: c.context(),
            request: choice.request.clone()
        })
        .await,
        Reply::Rejected
    ));
    assert!(!c
        .service
        .core
        .policy_readable(
            &choice.request,
            &c.selection.subject,
            Some(&c.selection.binding)
        )
        .unwrap());
    assert!(c._server.data.lock().unwrap().invocations.is_empty());
    assert!(matches!(
        c.policy_call(Request::PolicyStatus {
            context: c.context(),
            request: choice.request.clone()
        })
        .await,
        Reply::Rejected
    ));
    // A new native context may prepare a new request, but cannot revive the old preparation.
    choice.request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    assert!(matches!(
        c.policy_call(Request::InvokePolicy {
            context: c.context(),
            choice: choice.clone(),
            origin: c.selection.origin.clone()
        })
        .await,
        Reply::PolicyRequest { .. }
    ));
    c._server.data.lock().unwrap().invocation_reply_lost = true;
    let Reply::PolicyRequest { value: submitted } = c
        .policy_call(Request::ConfirmPolicy {
            context: c.context(),
            request: choice.request.clone(),
        })
        .await
    else {
        panic!("single confirmation")
    };
    assert_eq!(submitted.state, BackendRequestState::Submitting);
    assert!(submitted.confirmation.is_some());
    {
        let data = c._server.data.lock().unwrap();
        let input = &data.invocations.values().next().unwrap().0;
        assert_eq!(input["source"], "ai");
        assert_eq!(input["confirmed"], true);
        assert_eq!(input["parameters"], serde_json::json!({"name":"Acme"}));
    }
    let Reply::PolicyStatus { value: status } = c
        .policy_call(Request::PolicyStatus {
            context: c.context(),
            request: choice.request.clone(),
        })
        .await
    else {
        panic!("lost reply status")
    };
    assert_eq!(status.request_id.to_string(), choice.request.as_str());
    assert_eq!(c._server.data.lock().unwrap().invocations.len(), 1);
    c.policy_call(Request::RevokeContext {}).await;
    assert!(matches!(
        c.policy_call(Request::PolicyStatus {
            context: c.context(),
            request: choice.request
        })
        .await,
        Reply::Rejected
    ));
}
#[tokio::test]
async fn policy_low_risk_and_human_only_settings_have_separate_native_sources() {
    use execution_ipc::host::Request;
    for human in [false, true] {
        let mut c = Case::new(Some(RiskLevel::Two)).await;
        let mut directory = protocol::policy_directory_fixture();
        directory.self_service.risk_level = 1;
        directory.self_service.allow_ai = !human;
        c._server.data.lock().unwrap().self_service_directory =
            Some(wire::SelfServiceDirectoryResponse {
                items: vec![directory.clone()],
                next_cursor: None,
                as_of: c._server.time.now().unwrap(),
            });
        c.employee("generation-one").await;
        let choice = PolicyChoice {
            request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
            policy_revision: directory.policy_revision,
            policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
            parameters: std::collections::BTreeMap::from([(
                "name".into(),
                serde_json::json!("Acme"),
            )]),
        };
        let origin = if human {
            ClientOrigin::Desktop {}
        } else {
            c.selection.origin.clone()
        };
        let Reply::PolicyRequest { value } = c
            .policy_call(Request::InvokePolicy {
                context: c.context(),
                choice: choice.clone(),
                origin,
            })
            .await
        else {
            panic!("native source")
        };
        assert_eq!(value.state, BackendRequestState::Submitting);
        assert!(value.confirmation.is_none());
        assert_eq!(value.risk.is_none(), human);
        {
            let data = c._server.data.lock().unwrap();
            let input = &data.invocations.values().next().unwrap().0;
            assert_eq!(input["source"], if human { "human" } else { "ai" });
            assert_eq!(input["confirmed"], false);
        }
        let mut stale = choice.clone();
        stale.request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
        stale.policy_revision += 1;
        assert!(matches!(
            c.policy_call(Request::InvokePolicy {
                context: c.context(),
                choice: stale,
                origin: c.selection.origin.clone()
            })
            .await,
            Reply::Rejected
        ));
    }
}

#[tokio::test]
async fn policy_offer_binds_the_original_journal_and_signed_start_before_dispatch() {
    use execution_ipc::host::Request;
    for level in [1, 2] {
        for change in 0..5 {
            let mut c = Case::new(None).await;
            c.service.release_waiting();
            let mut directory = protocol::policy_directory_fixture();
            directory.self_service.risk_level = level;
            c._server.data.lock().unwrap().self_service_directory =
                Some(wire::SelfServiceDirectoryResponse {
                    items: vec![directory.clone()],
                    next_cursor: None,
                    as_of: c._server.time.now().unwrap(),
                });
            c.employee("generation-one").await;
            let choice = PolicyChoice {
                request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
                policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
                policy_revision: directory.policy_revision,
                policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
                parameters: std::collections::BTreeMap::from([(
                    "name".into(),
                    serde_json::json!("Acme"),
                )]),
            };
            assert!(matches!(
                c.policy_call(Request::InvokePolicy {
                    context: c.context(),
                    choice: choice.clone(),
                    origin: c.selection.origin.clone()
                })
                .await,
                Reply::PolicyRequest { .. }
            ));
            if level == 2 {
                assert!(matches!(
                    c.policy_call(Request::ConfirmPolicy {
                        context: c.context(),
                        request: choice.request.clone()
                    })
                    .await,
                    Reply::PolicyRequest { .. }
                ));
            }
            let record = c
                .service
                .core
                .app
                .backend_request(&c.service.core.caller(), &choice.request)
                .unwrap()
                .unwrap();
            let accepted = c
                ._server
                .data
                .lock()
                .unwrap()
                .invocations
                .values()
                .next()
                .unwrap()
                .1
                .clone();
            {
                let mut data = c._server.data.lock().unwrap();
                data.script();
                let wire::TaskPayload::TaskSpec(mut spec) =
                    data.offer.as_ref().unwrap().payload.clone()
                else {
                    unreachable!()
                };
                spec.task_id = accepted.run_id;
                data.task = accepted.run_id;
                spec.resource = match &directory.task {
                    wire::SelfServiceTask::Script { resource, .. } => resource.clone(),
                    _ => unreachable!(),
                };
                spec.parameters = std::collections::BTreeMap::from([
                    ("name".into(), serde_json::json!("Acme")),
                    ("path".into(), serde_json::json!("/Applications")),
                ]);
                spec.authorization = wire::TaskAuthorization::SelfService {
                    request_id: accepted.request_id,
                    subject: wire::SelfServiceSubject::User(wire::SelfServiceUser {
                        tenant_id: data.tenant,
                        instance_id: uuid::Uuid::from_u128(2),
                        principal_id: uuid::Uuid::from_u128(3),
                    }),
                    source: wire::SelfServiceCaller::Ai,
                    policy_id: directory.policy_id,
                    policy_revision: directory.policy_revision,
                    policy_version: directory.policy_version,
                    allow_ai: true,
                    risk_level: level,
                    confirmed: level == 2,
                };
                match change {
                    1 => {
                        spec.parameters
                            .insert("path".into(), serde_json::json!("/Library"));
                    }
                    2 => spec.resource.version = "v2".into(),
                    3 => {
                        if let wire::TaskAuthorization::SelfService {
                            subject: wire::SelfServiceSubject::User(actor),
                            ..
                        } = &mut spec.authorization
                        {
                            actor.principal_id = uuid::Uuid::new_v4();
                        }
                    }
                    4 => {
                        if let wire::TaskAuthorization::SelfService {
                            policy_revision, ..
                        } = &mut spec.authorization
                        {
                            *policy_revision += 1;
                        }
                    }
                    _ => (),
                }
                data.offer = Some(data.signed(wire::TaskPayload::TaskSpec(spec)));
                data.explicit_offers = true;
            }
            // Every alternative is independently signed and actually verified by the HTTP client.
            let offer = c.service.client.claim().await.unwrap().offer.unwrap();
            if change != 0 {
                assert!(c.service.bind_policy_offer(&offer).is_err());
                assert_eq!(
                    c.service
                        .core
                        .app
                        .backend_request(&c.service.core.caller(), &choice.request)
                        .unwrap()
                        .unwrap(),
                    record
                );
                assert!(!c._server.data.lock().unwrap().started);
                continue;
            }
            let selection = c.service.bind_policy_offer(&offer).unwrap();
            assert_eq!(selection.request, choice.request);
            let bound = c
                .service
                .core
                .app
                .backend_request(&c.service.core.caller(), &choice.request)
                .unwrap()
                .unwrap();
            assert_eq!(bound.trigger, record.trigger);
            assert_eq!(bound.confirmation, record.confirmation);
            let materials = c.service.client.prepare(&offer).await.unwrap();
            c.service.client.received(&offer).await.unwrap();
            let start = c
                .service
                .client
                .start_user_initiated(&offer, &materials)
                .await
                .unwrap();
            let mut input: ExecutionInput = serde_json::from_str(include_str!(
                "../../../crates/execution-contract/tests/fixtures/plan.json"
            ))
            .unwrap();
            input.request.request_id = choice.request.clone();
            input.request.initiator = Initiator::Backend {
                task: plan::id(offer.task_id().to_string()).unwrap(),
                attempt: plan::id(offer.attempt_id().to_string()).unwrap(),
                trigger: bound.trigger.clone(),
            };
            let frozen = FrozenExecution::freeze(input, &plan::storage_limits().input).unwrap();
            let risk = TrustedRisk::from_offer(&offer);
            assert!(ProductGateProof::verify_start(
                &offer,
                &start,
                &frozen,
                Some(&bound),
                risk.as_ref(),
                c.service.core.host.clock.millis().unwrap()
            )
            .is_ok());
            // A revoke which ACKs while HTTP is in flight invalidates this same generation.
            let admission = c.service.core.policy_fence(&bound).unwrap().unwrap();
            c.service.core.fences.revoke(&c.selection.subject).unwrap();
            assert!(admission.0.admit(&admission.1, admission.2).is_err());
            assert!(c.service.core.current_policy_login(&bound).is_err());
            c.employee("generation-two").await;
            assert!(c.service.bind_policy_offer(&offer).is_err());
            assert_eq!(
                c.service
                    .core
                    .app
                    .backend_request(&c.service.core.caller(), &choice.request)
                    .unwrap()
                    .unwrap(),
                bound
            );
        }
    }
}

#[tokio::test]
async fn revoked_generation_cannot_rebind_from_a_queued_native_handoff() {
    let mut c = Case::new(None).await;
    c.employee("generation-one").await;
    let epoch = c.service.core.fences.current(&c.selection.subject).unwrap();
    c.service.core.fences.revoke(&c.selection.subject).unwrap();
    let (reply, receive) = tokio::sync::oneshot::channel();
    c.service.core.enqueue_policy(Command {
        fence_epoch: epoch,
        subject: c.selection.subject.clone(),
        session: c.selection.session,
        binding: Some(c.selection.binding.clone()),
        request: LocalRequest::operation(execution_ipc::host::Request::BindContext {
            expected: Some(c.context().id),
            value: execution_ipc::host::NativeContextSession {
                credential: Some(zeroize::Zeroizing::new("c".repeat(64))),
                credential_origin: Some(c.service.client.configuration().origin.to_string()),
                user: Some(wire::SelfServiceUser {
                    tenant_id: c.service.client.configuration().tenant,
                    instance_id: uuid::Uuid::from_u128(2),
                    principal_id: uuid::Uuid::from_u128(3),
                }),
                generation: plan::id("generation-one").unwrap(),
                expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 60000,
            },
        }),
        reply,
    });
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    c.service.drive_policy(&mut commands).await.unwrap();
    assert!(matches!(receive.await.unwrap(), Reply::Rejected));
}

#[tokio::test]
async fn software_observation_rejects_unselected_definition_without_creating_an_attempt() {
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    for change in 0..5 {
        let mut selected = BackendSelection {
            request: c.selection.request.clone(),
            task: c.selection.task.clone(),
            attempt: c.selection.attempt.clone(),
            revision: c.selection.revision.clone(),
        };
        match change {
            0 => selected.request = RequestId::new("different").unwrap(),
            1 => selected.task = plan::id("different").unwrap(),
            2 => selected.attempt = plan::id("different").unwrap(),
            3 => selected.revision = Digest::new("f".repeat(64)).unwrap(),
            _ => (),
        }
        let choice = SoftwareReadChoice {
            request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            input: SoftwareReadSelection {
                selection: selected,
                step: if change == 4 { 100 } else { 0 },
            },
        };
        assert!(matches!(
            c.policy_call(Request::ObserveSoftware {
                choice: choice.clone(),
                origin: c.selection.origin.clone()
            })
            .await,
            Reply::Rejected
        ));
        assert!(c
            .service
            .core
            .app
            .backend_request(&c.service.core.caller(), &choice.request)
            .unwrap()
            .is_none());
        assert!(c
            .service
            .core
            .app
            .frozen_input(&c.service.core.caller(), &choice.request)
            .is_err());
    }
    assert!(!c._server.data.lock().unwrap().started);
}

#[tokio::test]
async fn settled_software_observation_replays_the_original_fact_and_never_creates_execution() {
    // The fixture supplies a trusted selection; the production native collector and SQLite
    // transition are real. This test does not claim proof of a registered OS helper session.
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    let definition = Digest::new("a".repeat(64)).unwrap();
    let mut originals = vec![];
    for unavailable in [false, true] {
        let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
        let choice = SoftwareReadChoice {
            request: request.clone(),
            input: SoftwareReadSelection {
                selection: BackendSelection {
                    request: c.selection.request.clone(),
                    task: c.selection.task.clone(),
                    attempt: c.selection.attempt.clone(),
                    revision: c.selection.revision.clone(),
                },
                step: 0,
            },
        };
        let now = c.service.core.host.clock.millis().unwrap();
        let record = BackendRequest {
            action: PreparedAction::SoftwareRead {
                choice: choice.clone(),
                definition: definition.clone(),
                expires_at_unix_ms: now + 60000,
                observation: None,
            },
            trigger: c
                .selection
                .trigger(&c.service.core.host.binding.device)
                .unwrap(),
            risk: Some(ExecutionRisk {
                level: RiskLevel::One,
                source: RiskSource::Software {
                    definition: definition.clone(),
                },
                expires_at_unix_ms: now + 60000,
            }),
            confirmation: None,
            revision: 1,
            state: BackendRequestState::Ready,
            failure: None,
        };
        let observed = c
            .service
            .core
            .record_software_observation(
                record,
                || {
                    execution_runner::software::observe_native(
                        &SoftwareDetector::PkgReceipt {
                            receipt: if unavailable {
                                "../../invalid".into()
                            } else {
                                format!("com.rss-mdm.uninstalled-test-{}", std::process::id())
                            },
                            version: PackageValue::new("1").unwrap(),
                        },
                        &RunAs::System {
                            platform: Platform::Macos,
                        },
                    )
                    .map_err(crate::error::app_error)
                },
                "fixture-package".into(),
                "1".into(),
            )
            .unwrap();
        assert_eq!(observed.state, BackendRequestState::Observed);
        assert!(observed.confirmation.is_none());
        let PreparedAction::SoftwareRead {
            observation: Some(fact),
            ..
        } = &observed.action
        else {
            panic!("collected fact")
        };
        assert_eq!(
            fact.quality,
            if unavailable {
                OutputQuality::Failed
            } else {
                OutputQuality::Complete
            }
        );
        assert!(fact.collected_at_unix_ms >= now);
        assert!(c
            .service
            .core
            .app
            .frozen_input(&c.service.core.caller(), &request)
            .is_err());
        let Reply::PolicyRequest { value } = c
            .policy_call(Request::ObserveSoftware {
                choice: choice.clone(),
                origin: c.selection.origin.clone(),
            })
            .await
        else {
            panic!("original journal fact")
        };
        assert_eq!(*value, observed);
        let mut changed = choice;
        changed.input.step += 1;
        assert!(matches!(
            c.policy_call(Request::ObserveSoftware {
                choice: changed,
                origin: c.selection.origin.clone()
            })
            .await,
            Reply::Rejected
        ));
        originals.push(observed.action.request().clone());
    }
    assert_ne!(originals[0], originals[1]);
    assert!(!c._server.data.lock().unwrap().started);
}

#[tokio::test]
async fn durable_ready_policy_resumes_original_uuid_from_each_native_entrypoint() {
    use execution_ipc::host::Request;
    for entry in 0..3 {
        let mut c = Case::new(Some(RiskLevel::Two)).await;
        let mut directory = protocol::policy_directory_fixture();
        directory.self_service.risk_level = if entry == 2 { 2 } else { 1 };
        c._server.data.lock().unwrap().self_service_directory =
            Some(wire::SelfServiceDirectoryResponse {
                items: vec![directory.clone()],
                next_cursor: None,
                as_of: c._server.time.now().unwrap(),
            });
        c.employee("generation-one").await;
        let choice = PolicyChoice {
            request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
            policy_revision: directory.policy_revision,
            policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
            parameters: std::collections::BTreeMap::from([(
                "name".into(),
                serde_json::json!("Acme"),
            )]),
        };
        let invoke = Request::InvokePolicy {
            context: c.context(),
            choice: choice.clone(),
            origin: c.selection.origin.clone(),
        };
        let Reply::PolicyRequest { value } = c.policy_call(invoke).await else {
            panic!("initial preparation")
        };
        let value = if entry == 2 {
            let Reply::PolicyRequest { value } = c
                .policy_call(Request::ConfirmPolicy {
                    context: c.context(),
                    request: choice.request.clone(),
                })
                .await
            else {
                panic!("confirmation")
            };
            value
        } else {
            value
        };
        let mut restored = *value;
        let original = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
        if let PreparedAction::Policy { choice, run_id, .. } = &mut restored.action {
            choice.request = original.clone();
            *run_id = None;
        }
        restored.state = BackendRequestState::Ready;
        restored.revision = 1;
        c.service
            .core
            .app
            .record_backend_request(&c.service.core.caller(), None, &restored)
            .unwrap();
        let request = match entry {
            0 => Request::InvokePolicy {
                context: c.context(),
                choice: match &restored.action {
                    PreparedAction::Policy { choice, .. } => choice.clone(),
                    _ => unreachable!(),
                },
                origin: c.selection.origin.clone(),
            },
            1 => Request::PolicyStatus {
                context: c.context(),
                request: original.clone(),
            },
            _ => Request::ConfirmPolicy {
                context: c.context(),
                request: original.clone(),
            },
        };
        let Reply::PolicyRequest { value } = c.policy_call(request).await else {
            panic!("Ready recovery")
        };
        assert_eq!(value.state, BackendRequestState::Submitting);
        assert_eq!(value.action.request(), &original);
        assert!(c
            ._server
            .data
            .lock()
            .unwrap()
            .invocations
            .contains_key(&uuid::Uuid::parse_str(original.as_str()).unwrap()));
    }
}
#[tokio::test]
async fn unaccepted_policy_expiry_is_durable_and_status_remains_read_only() {
    use execution_ipc::host::Request;
    let mut c = Case::new(Some(RiskLevel::Two)).await;
    let mut directory = protocol::policy_directory_fixture();
    directory.self_service.risk_level = 1;
    c._server.data.lock().unwrap().self_service_directory =
        Some(wire::SelfServiceDirectoryResponse {
            items: vec![directory.clone()],
            next_cursor: None,
            as_of: c._server.time.now().unwrap(),
        });
    c.employee("generation-one").await;
    c._server.data.lock().unwrap().invocation_unaccepted = true;
    let original = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let choice = PolicyChoice {
        request: original.clone(),
        policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
        policy_revision: directory.policy_revision,
        policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
        parameters: std::collections::BTreeMap::from([("name".into(), serde_json::json!("Acme"))]),
    };
    let Reply::PolicyRequest { value } = c
        .policy_call(Request::InvokePolicy {
            context: c.context(),
            choice,
            origin: c.selection.origin.clone(),
        })
        .await
    else {
        panic!("unaccepted invocation")
    };
    assert_eq!(value.state, BackendRequestState::Submitting);
    // Restore an already-expired durable record at the interrupted submission boundary.
    // SystemClock intentionally rejects artificial forward movement, so the expiry is fixed
    // before the initial journal insert, preserving the immutable preparation contract.
    let mut expired = *value;
    let original = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    if let PreparedAction::Policy {
        choice,
        expires_at_unix_ms,
        ..
    } = &mut expired.action
    {
        choice.request = original.clone();
        *expires_at_unix_ms = c.service.core.host.clock.millis().unwrap() - 1;
    }
    expired.risk.as_mut().unwrap().expires_at_unix_ms = expired.action.expires_at_unix_ms();
    expired.state = BackendRequestState::Ready;
    expired.revision = 1;
    c.service
        .core
        .app
        .record_backend_request(&c.service.core.caller(), None, &expired)
        .unwrap();
    c.service
        .core
        .transition(&original, BackendRequestState::Submitting, None)
        .unwrap();
    c._server.data.lock().unwrap().invocation_unaccepted = false;
    for _ in 0..2 {
        let Reply::PolicyRequest { value } = c
            .policy_call(Request::PolicyStatus {
                context: c.context(),
                request: original.clone(),
            })
            .await
        else {
            panic!("expiry status")
        };
        assert_eq!(value.state, BackendRequestState::Failed);
        assert_eq!(value.failure, Some(BackendRequestFailure::Expired));
    }
    assert!(c._server.data.lock().unwrap().invocations.is_empty());
}

fn select_pki_refresh(root: &std::path::Path) {
    let sql = rusqlite::Connection::open(root.join("communication.sqlite")).unwrap();
    let body: Vec<u8> = sql
        .query_row("SELECT body FROM state WHERE key='pki'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let mut pki: serde_json::Value = serde_json::from_slice(&body).unwrap();
    pki["refresh_at"] = 0.into();
    pki["retry_at"] = 0.into();
    sql.execute(
        "UPDATE state SET body=?1 WHERE key='pki'",
        [serde_json::to_vec(&pki).unwrap()],
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_revocation_and_crl_expiry_close_admission_while_pending_status_waits() {
    for revocation in [true, false] {
        let mut c = Case::new(Some(RiskLevel::Two)).await;
        c.service.core.start_task(c.selection(false)).unwrap();
        let original = c.record();
        assert_eq!(original.state, BackendRequestState::AwaitingConfirmation);
        let binding = c.service.client.registration().unwrap();
        c._server.data.lock().unwrap().registration_failure = true;
        assert_eq!(
            c.service.client.renew_certificate().await.unwrap_err(),
            Error::Unavailable
        );
        let pause = Arc::new(tokio::sync::Notify::new());
        let now = c.service.core.host.clock.now().unwrap();
        {
            let crl = protocol::pki::crl(
                2,
                0,
                if revocation {
                    4_102_444_800
                } else {
                    (now + 2) as u64
                },
                if revocation { &[1] } else { &[] },
            );
            let mut data = c._server.data.lock().unwrap();
            data.trust["revision"] = 2.into();
            data.trust["issuers"][0]["crl"] = crl.clone().into();
            data.trust["issuers"][0]["root_crl"] = crl.into();
            data.status_pause = Some(pause.clone());
        }
        // Select the refresh deterministically through the existing SQLite seam; the server still
        // supplies actual signed CRLs and the communication owner performs all production validation.
        select_pki_refresh(&c._root.path);
        let data = c._server.data.clone();
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        let mut observed = false;
        let core = &mut c.service.core;
        let result =
            crate::service::network_loop(c.service.client.drive_pki(), &mut commands, |_| {
                core.enforce_identity_gate()?;
                core.reconcile()?;
                if data.lock().unwrap().status_waiting && !core.host.identity_ready() {
                    observed = true;
                    assert!(core.host.revoked.load(Ordering::Acquire));
                    let retained = core
                        .app
                        .backend_request(&core.caller(), original.action.request())
                        .unwrap()
                        .unwrap();
                    assert_eq!(retained.state, BackendRequestState::Failed);
                    assert_eq!(retained.failure, Some(BackendRequestFailure::Revoked));
                    assert_eq!(retained.action, original.action);
                    pause.notify_one();
                }
                Ok(false)
            })
            .await;
        assert!(
            observed,
            "identity gate must close before the pending status returns"
        );
        assert!(result.is_err());
        c.service.drive_error(result.unwrap_err());
        assert!(c.service.waiting.is_none());
        assert_eq!(c.service.client.registration().unwrap(), binding);
        assert!(matches!(
            c.service.core.start_task(c.selection(false)),
            Err(Error::Denied)
        ));
        assert!(c._server.data.lock().unwrap().start_ops.is_empty());
        assert_eq!(c.record().action, original.action);
    }
}

#[tokio::test]
async fn explicit_upgrade_failures_and_exact_resume_preserve_registration_and_journal() {
    use crate::installation::{resume_matches, run_upgrade, UpgradeStep};
    for fault in [
        UpgradeStep::Stop,
        UpgradeStep::Lease,
        UpgradeStep::Publish,
        UpgradeStep::Register,
    ] {
        let mut c = Case::new(Some(RiskLevel::Two)).await;
        c.service.core.start_task(c.selection(false)).unwrap();
        let binding = c.service.client.registration().unwrap();
        let record = c.record();
        let owner = c.service.core.host.binding.clone();
        let config = c._root.path.join("upgrade-config.json");
        let old = serde_json::json!({"version":4,"installation_id":"retained","installation_artifact":"old"});
        let next = serde_json::json!({"version":5,"installation_id":"retained","installation_artifact":"new"});
        std::fs::write(&config, serde_json::to_vec(&old).unwrap()).unwrap();
        let mut registered = false;
        assert_eq!(
            run_upgrade(false, |step| {
                if step == fault {
                    return Err(Error::Unavailable);
                }
                if step == UpgradeStep::Publish {
                    let staged = config.with_extension("staged");
                    std::fs::write(&staged, serde_json::to_vec(&next).unwrap()).unwrap();
                    std::fs::rename(staged, &config).unwrap();
                }
                if step == UpgradeStep::Register {
                    registered = true;
                }
                Ok(())
            })
            .unwrap_err(),
            Error::Unavailable
        );
        assert!(!registered);
        let retained: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
        if fault == UpgradeStep::Register {
            assert_eq!(retained, next);
            assert_eq!(
                resume_matches("new", Path::new("/service"), "other", Path::new("/service"))
                    .unwrap_err(),
                Error::Conflict
            );
            resume_matches("new", Path::new("/service"), "new", Path::new("/service")).unwrap();
            run_upgrade(true, |step| {
                assert!(matches!(step, UpgradeStep::Register | UpgradeStep::Desktop));
                if step == UpgradeStep::Register {
                    registered = true;
                }
                Ok(())
            })
            .unwrap();
            assert!(registered);
        } else {
            assert_eq!(retained, old);
        }
        assert_eq!(c.service.client.registration().unwrap(), binding);
        assert_eq!(c.record(), record);
        assert_eq!(c.service.core.host.binding, owner);
    }
}

#[tokio::test]
async fn software_self_service_binds_signed_policy_and_native_context() {
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    c.service.release_waiting();
    c.employee("software-user").await;
    let mut directory = protocol::policy_directory_fixture();
    directory.self_service.risk_level = 2;
    let wire::TaskPayload::SoftwareTaskSpec(spec) = c.offer.payload() else {
        panic!("software")
    };
    let step = spec.steps.last().unwrap();
    directory.task = wire::SelfServiceTask::Software {
        resource: wire::TaskResource {
            id: "software-resource".into(),
            version: step.action.version.clone(),
            variant: "default".into(),
        },
        run_as: backend::commands::software_commands(&step.action)
            .unwrap()
            .install
            .run_as,
        target: step.target.clone(),
    };
    c._server.data.lock().unwrap().self_service_directory =
        Some(wire::SelfServiceDirectoryResponse {
            items: vec![directory.clone()],
            next_cursor: None,
            as_of: c._server.time.now().unwrap(),
        });
    let choice = PolicyChoice {
        request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
        policy_revision: directory.policy_revision,
        policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
        parameters: Default::default(),
    };
    let Reply::PolicyRequest { value: pending } = c
        .policy_call(Request::InvokePolicy {
            context: c.context(),
            choice: choice.clone(),
            origin: c.selection.origin.clone(),
        })
        .await
    else {
        panic!("software preparation")
    };
    assert_eq!(pending.state, BackendRequestState::AwaitingConfirmation);
    let Reply::PolicyRequest { value: submitted } = c
        .policy_call(Request::ConfirmPolicy {
            context: c.context(),
            request: choice.request.clone(),
        })
        .await
    else {
        panic!("software confirmation")
    };
    assert_eq!(submitted.state, BackendRequestState::Submitting);
    let accepted = c
        ._server
        .data
        .lock()
        .unwrap()
        .invocations
        .values()
        .next()
        .unwrap()
        .1
        .clone();
    {
        let mut data = c._server.data.lock().unwrap();
        let wire::TaskPayload::SoftwareTaskSpec(mut spec) =
            data.offer.as_ref().unwrap().payload.clone()
        else {
            panic!("software")
        };
        spec.task_id = accepted.run_id;
        data.task = accepted.run_id;
        spec.authorization = wire::TaskAuthorization::SelfService {
            request_id: accepted.request_id,
            subject: wire::SelfServiceSubject::User(wire::SelfServiceUser {
                tenant_id: data.tenant,
                instance_id: uuid::Uuid::from_u128(2),
                principal_id: uuid::Uuid::from_u128(3),
            }),
            source: wire::SelfServiceCaller::Ai,
            policy_id: directory.policy_id,
            policy_revision: directory.policy_revision,
            policy_version: directory.policy_version,
            allow_ai: true,
            risk_level: 2,
            confirmed: true,
        };
        data.offer = Some(data.signed(wire::TaskPayload::SoftwareTaskSpec(spec)));
        data.explicit_offers = true;
    }
    let offer = c.service.client.claim().await.unwrap().offer.unwrap();
    c.service.bind_policy_offer(&offer).unwrap();
    let bound = c
        .service
        .core
        .app
        .backend_request(&c.service.core.caller(), &choice.request)
        .unwrap()
        .unwrap();
    assert_eq!(bound.risk.unwrap().level, RiskLevel::Two);
    c.employee("other-context").await;
    assert!(c.service.bind_policy_offer(&offer).is_err());
    assert!(!c
        .service
        .core
        .policy_readable(
            &choice.request,
            &c.selection.subject,
            Some(&c.selection.binding)
        )
        .unwrap());
}

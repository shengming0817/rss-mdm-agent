//! Real signed offers/Start and SQLite, with risk available only in this test assembly.
use super::*;
use crate::{
    backend::gate::{ProductGateProof, TrustedRisk},
    protocol_test_support as protocol, SoftwareManager, SoftwareManagerKind,
};
use agent_client::{Clock, OpenMode};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply};
use rss_mdm_agent_wire::runtime as wire;
use sha2::{Digest as _, Sha256};

pub(crate) struct Case {
    pub(crate) service: DeviceService<protocol::Secrets>,
    pub(crate) _server: protocol::Server,
    _root: protocol::Root,
    offer: Offer,
    selection: Selection,
}

// Stage the verified HTTP offer at the local gate seam. Production self-service
// admission binds an existing Policy invocation; these tests exercise the gate
// and waiting-owner lifecycle independently, with explicit fixture risk.
async fn stage_gate_offer(service: &mut DeviceService<protocol::Secrets>) {
    let offer = service.client.claim().await.unwrap().offer.unwrap();
    service.client.received(&offer).await.unwrap();
    let materials = service.client.prepare(&offer).await.unwrap();
    service.core.available = Some(offered(&offer).unwrap());
    service.core.available_risk = TrustedRisk::from_offer(&offer);
    service.core.available_start = backend::gate::TrustedStart::from_offer(&offer);
    service.waiting = Some((offer, materials));
}

impl Case {
    pub(crate) fn take_telemetry(&mut self) -> crate::telemetry::Telemetry {
        self.service
            .core
            .telemetry
            .take()
            .expect("registered telemetry owner")
    }
    pub(crate) async fn new(level: Option<RiskLevel>) -> Self {
        Self::with_offer_lifetime(level, 60).await
    }
    async fn with_offer_lifetime(level: Option<RiskLevel>, seconds: i64) -> Self {
        Self::with_offer_mode(level, seconds, true).await
    }
    async fn with_offer_mode(level: Option<RiskLevel>, seconds: i64, user: bool) -> Self {
        Self::with_report_collection(level, seconds, user, false).await
    }
    async fn with_report_collection(
        level: Option<RiskLevel>,
        seconds: i64,
        user: bool,
        mdm: bool,
    ) -> Self {
        let server = protocol::Server::new().await;
        if mdm {
            let mut collections = protocol::collections();
            let mut definition = serde_json::to_value(&collections[0]).unwrap();
            definition["dataset"] = serde_json::json!("channel.mdm.enrollment");
            definition["fields"] = serde_json::json!([definition["fields"][0].clone()]);
            definition["fields"][0]["key"] = serde_json::json!("channel.mdm.enrollment");
            collections.push(serde_json::from_value(definition).unwrap());
            server.data.lock().unwrap().collections = Some(collections);
        }
        let root = protocol::Root::new();
        let clock = SystemClock::new().unwrap();
        server.time.set(clock.now().unwrap());
        {
            let mut data = server.data.lock().unwrap();
            data.software(1, user);
            let wire::TaskPayload::Software(mut spec) =
                data.offer.as_ref().unwrap().payload.clone()
            else {
                panic!("software")
            };
            spec.expires_at = clock.now().unwrap() + seconds;
            data.offer = Some(data.signed(wire::TaskPayload::Software(spec)));
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
                #[cfg(test)]
                worker_image_fixture: None,
                local_permissions: crate::local_permissions::Config {
                    policy_file: "/unavailable/local-permissions.json".into(),
                },
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
            uuid::Uuid::from_u128(2909),
        )
        .unwrap();
        stage_gate_offer(&mut service).await;
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
    let _budget = crate::scenario_budget::enter();
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
        stage_gate_offer(&mut c.service).await;
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
    let _budget = crate::scenario_budget::enter();
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
    let _budget = crate::scenario_budget::enter();
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
async fn helper_status_never_claims_readiness_without_a_verified_current_helper() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    for (stopping, binding) in [
        (false, None),
        (false, Some(c.selection.binding.clone())),
        (true, Some(c.selection.binding.clone())),
    ] {
        c.service.core.stopping = stopping;
        let (reply, receive) = tokio::sync::oneshot::channel();
        c.service.core.handle(Command {
            fence_epoch: 0,
            request: LocalRequest::operation(execution_ipc::host::Request::HelperStatus {}),
            subject: c.selection.subject.clone(),
            session: c.selection.session,
            binding,
            reply,
        });
        assert!(matches!(
            receive.await.unwrap(),
            Reply::Rejected | Reply::Unavailable
        ));
        assert!(c.service.core.policy_commands.is_empty());
        assert!(c._server.data.lock().unwrap().invocations.is_empty());
    }
}

#[tokio::test]
async fn one_desktop_confirmation_retains_ai_origin_and_cannot_be_replaced_by_start() {
    let _budget = crate::scenario_budget::enter();
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
    let _budget = crate::scenario_budget::enter();
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

fn complete_material_gc<S: agent_client::IdentityProvider>(
    service: &mut crate::DeviceService<S>,
) -> Result<(), agent_client::Error> {
    service.material_gc = Default::default();
    for _ in 0..20000 {
        service.reclaim_step()?;
        if service.material_gc.idle() {
            return Ok(());
        }
    }
    panic!("GC did not complete its fixed-step pass");
}

#[tokio::test]
async fn material_gc_finishes_old_content_under_continuing_publication_and_new_journal_rows() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    let caller = c.service.core.caller();
    let consumer = plan::id(RESULT_CONSUMER).unwrap();
    let bytes = vec![5; 8 * 1024 * 1024];
    let digest = execution_contract::Digest::new(format!("{:x}", Sha256::digest(&bytes))).unwrap();
    let old = c
        .service
        .core
        .material_root
        .publish(None, bytes.as_slice(), &digest, bytes.len() as u64)
        .unwrap();
    let mut spec: ExecutionInput = serde_json::from_str(include_str!(
        "../../../crates/execution-contract/tests/fixtures/plan.json"
    ))
    .unwrap();
    spec.request.authority = c.service.core.host.binding.authority.clone();
    spec.request.actor = caller.actor.clone();
    spec.request.target.device = c.service.core.host.binding.device.clone();
    spec.request.initiator = Initiator::Backend {
        task: plan::id("gc-continuing-task").unwrap(),
        attempt: plan::id("gc-continuing-attempt").unwrap(),
        trigger: BackendTrigger::Automatic {},
    };
    spec.request.delegation = None;
    spec.run_as = RunAs::System {
        platform: spec.request.target.platform,
    };
    spec.session_requirement = SessionRequirement::NotRequired {};
    let register = |service: &mut DeviceService<protocol::Secrets>,
                    name: String,
                    digest: execution_contract::Digest| {
        let mut next = spec.clone();
        next.request.request_id = RequestId::new(name).unwrap();
        next.launch.artifact.sha256 = digest;
        let input = FrozenExecution::freeze(next, &plan::storage_limits().input).unwrap();
        assert!(matches!(
            service.core.app.request_execution(&caller, &input),
            Err(execution_app::Error::Unbound)
        ));
        service
            .core
            .app
            .cancel(&caller, &input.spec().request.request_id)
            .unwrap();
        for item in service
            .core
            .app
            .service_delivery(&input.spec().request.request_id, &consumer, 64)
            .unwrap()
        {
            service
                .core
                .app
                .service_confirm(
                    &input.spec().request.request_id,
                    &consumer,
                    &item.receipt.event_id,
                )
                .unwrap();
        }
    };
    for index in 0..65 {
        register(&mut c.service, format!("gc-old-{index:03}"), digest.clone());
    }
    let mut publications = 0;
    for step in 0..512 {
        if step % 16 == 0 {
            let bytes = format!("new material {step}");
            let digest =
                execution_contract::Digest::new(format!("{:x}", Sha256::digest(bytes.as_bytes())))
                    .unwrap();
            c.service
                .core
                .material_root
                .publish(None, bytes.as_bytes(), &digest, bytes.len() as u64)
                .unwrap();
            register(&mut c.service, format!("gc-new-{step:03}"), digest);
            publications += 1;
        }
        c.service.reclaim_step().unwrap();
        if !old.exists() {
            assert!(
                publications >= 2,
                "fixture must interleave publication with the old pass"
            );
            return;
        }
    }
    panic!("continuing publication starved reclamation of settled old material");
}

#[tokio::test]
async fn material_gc_scans_every_journal_page_and_waits_for_delivery_confirmation() {
    let _budget = crate::scenario_budget::enter();
    material_gc_history(1).await;
}

#[tokio::test]
#[ignore = "large historical journal; run explicitly for scale verification"]
async fn material_gc_large_history_does_not_exhaust_live_candidate_budget() {
    let _budget = crate::scenario_budget::enter();
    material_gc_history(512).await;
}

async fn material_gc_history(materials_per_request: usize) {
    // Keep Offer/Start expiry out of GC verification, including the manual scale case.
    let fixture_started = std::time::Instant::now();
    let mut c = Case::with_offer_mode(None, 600, false).await;
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
    let consumer = plan::id(RESULT_CONSUMER).unwrap();
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
            // Absent historical paths must not consume the deletion-candidate budget.
            // The manual scale case supplies 32,768 of them before the final live row.
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
            let materials = (0..materials_per_request)
                .map(|member| {
                    let sha = execution_contract::Digest::new(format!(
                        "{:064x}",
                        index * materials_per_request + member
                    ))
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
    eprintln!(
        "material_gc_fixture_ms={}",
        fixture_started.elapsed().as_millis()
    );
    let gc_started = std::time::Instant::now();
    complete_material_gc(&mut c.service).unwrap();
    eprintln!("material_gc_pass_ms={}", gc_started.elapsed().as_millis());
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
            c.service.client.request_start(&c.offer, &materials).await
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
    assert!(complete_material_gc(&mut c.service).is_err());
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
    complete_material_gc(&mut c.service).unwrap();
    assert!(source.exists(), "the configured interpreter remains pinned");
    c.service.core.config.interpreters[0].image = original_image;
    c.service.core.config.managers.push(SoftwareManager {
        executor: SoftwareManagerKind::PackageInstaller,
        image: pinned,
    });
    complete_material_gc(&mut c.service).unwrap();
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
    complete_material_gc(&mut c.service).unwrap();
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
    assert!(source.exists() && other_source.exists());

    let reconcile_started = std::time::Instant::now();
    c.service.core.reconcile().unwrap();
    eprintln!(
        "material_gc_reconcile_ms={}",
        reconcile_started.elapsed().as_millis()
    );
    // Leave a fresh pass at its journal phase before starting the real owner thread.
    // Commands use the production 3-second ServiceHandle timeout while large rows
    // are scanned on successive owner ticks. Transport fixtures remain alive here.
    c.service.material_gc = Default::default();
    c.service.reclaim_step().unwrap();
    c.service.waiting = None;
    c.service.next_network = std::time::Instant::now() + std::time::Duration::from_secs(60);
    c.service.next_enrollment = c.service.next_network;
    let policy = execution_ipc::host::PeerPolicy {
        images: vec![c.service.core.config.interpreters[0].image.clone()],
        subjects: vec!["0".into()],
        interactive: false,
    };
    let mut owner = c.service.spawn(policy, None).unwrap();
    let maximum = tokio::task::spawn_blocking(move || {
        use execution_ipc::host::Handler;
        let mut maximum = std::time::Duration::ZERO;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while source.exists() || other_source.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "owner GC never completed"
            );
            let started = std::time::Instant::now();
            assert!(matches!(
                owner.service_status(),
                Reply::ServiceStatus { .. }
            ));
            assert!(started.elapsed() < std::time::Duration::from_secs(3));
            maximum = maximum.max(started.elapsed());
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        owner.stop().unwrap();
        maximum
    })
    .await
    .unwrap();
    assert!(maximum < std::time::Duration::from_secs(3));
    eprintln!("material_gc_owner_ipc_max_ms={}", maximum.as_millis());
}

#[tokio::test]
async fn material_owner_rejects_a_replaced_journal_and_keeps_original_material() {
    let _budget = crate::scenario_budget::enter();
    let c = Case::new(None).await;
    let root = c.service.core.config.material_root.clone();
    let journal = c._root.path.join("execution.sqlite");
    use sha2::{Digest as _, Sha256};
    let owner = execution_contract::Digest::new(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                &c.service.core.host.root.authority,
                &c.service.core.host.root.device
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
    let _budget = crate::scenario_budget::enter();
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
        #[cfg(test)]
        worker_image_fixture: None,
        local_permissions: crate::local_permissions::Config {
            policy_file: "/unavailable/local-permissions.json".into(),
        },
        work_root: root.path.clone(),
        material_root: root.path.join("materials"),
        // Native collection/telemetry do not require an installed script interpreter.
        interpreters: vec![],
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
                uuid::Uuid::from_u128(2909),
            )
            .unwrap(),
        );
    }
    assert!(journal.is_file());
}

#[tokio::test]
async fn renewal_and_revoked_restart_keep_the_existing_journal_authority() {
    let _budget = crate::scenario_budget::enter();
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
        uuid::Uuid::from_u128(2909),
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
        uuid::Uuid::from_u128(2909),
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
    let _budget = crate::scenario_budget::enter();
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
    assert!(matches!(reply, Reply::ServiceStatus { value } if value.readiness == Readiness::Ready));
}

#[tokio::test]
async fn installation_owner_replies_to_platform_sso_before_enterprise_registration() {
    let _budget = crate::scenario_budget::enter();
    let c = Case::new(None).await;
    let root = protocol::Root::new();
    let clock = SystemClock::new().unwrap();
    let client = Client::open(
        &root.path,
        c._server.config(),
        OpenMode::Create,
        c._server.secrets.clone(),
        clock.clone(),
    )
    .unwrap();
    let mut config = c.service.core.config.clone();
    config.work_root = root.path.clone();
    config.material_root = root.path.join("materials");
    platform_private_storage::directory(&config.material_root).unwrap();
    let mut service = DeviceService::assemble(
        client,
        &root.path.join("execution.sqlite"),
        ProductionStartup::Create,
        config,
        clock,
        c.service.core.helpers.clone(),
        execution_runner::staging::MaterialRoot::open_fixture,
        uuid::Uuid::new_v4(),
    )
    .unwrap();
    assert!(service.client.registration().is_err());
    let (reply, mut receive) = tokio::sync::oneshot::channel();
    let (sender, mut commands) = tokio::sync::mpsc::channel(1);
    sender
        .send(Command {
            fence_epoch: 0,
            subject: c.selection.subject.clone(),
            session: c.selection.session,
            binding: Some(c.selection.binding.clone()),
            reply,
            request: LocalRequest::operation(execution_ipc::host::Request::PlatformSso {
                input: execution_ipc::platform_sso::Operation::Bootstrap {
                    scope: rss_mdm_agent_wire::platform_sso::Kind::Device,
                },
            }),
        })
        .await
        .unwrap();
    service.drive(&mut commands).await.unwrap();
    assert!(matches!(receive.try_recv(), Ok(Reply::Rejected)));
    assert!(service.core.psso.commands.is_empty());
    assert!(service.client.registration().is_err());
}

impl Case {
    fn scope(&self) -> crate::service::policy::SessionKey {
        crate::service::policy::SessionKey::new(
            self.selection.subject.clone(),
            self.selection.binding.clone(),
        )
    }
    async fn recent_call(&mut self, before: Option<String>) -> Reply {
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.service.core.handle(Command {
            fence_epoch: self.service.core.fences.current(&self.scope()).unwrap(),
            subject: self.selection.subject.clone(),
            session: self.selection.session,
            binding: Some(self.selection.binding.clone()),
            reply,
            request: LocalRequest::operation(execution_ipc::host::Request::RecentTasks { before }),
        });
        receive.await.unwrap()
    }
    async fn policy_call(&mut self, request: execution_ipc::host::Request) -> Reply {
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.service.core.enqueue_policy(Command {
            fence_epoch: self.service.core.fences.current(&self.scope()).unwrap(),
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
                    access: execution_ipc::host::NativeAccess::User {
                        credential: zeroize::Zeroizing::new("c".repeat(64)),

                        user: wire::SelfServiceUser {
                            tenant_id: self.service.client.configuration().tenant,
                            instance_id: crate::protocol_test_support::SERVICE_ID,
                            principal_id: uuid::Uuid::from_u128(3),
                        },
                    },
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
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        // Advance the independent presence projection through the existing owner tick.
        self.service
            .synchronize_current_user(&mut commands)
            .await
            .unwrap();
        self.service
            .synchronize_current_user(&mut commands)
            .await
            .unwrap();
    }
}
#[tokio::test]
async fn native_user_handoff_rejects_another_service_before_any_http_or_refresh() {
    let _budget = crate::scenario_budget::enter();
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
                    access: execution_ipc::host::NativeAccess::User {
                        credential: zeroize::Zeroizing::new("c".repeat(64)),

                        user: wire::SelfServiceUser {
                            tenant_id: c.service.client.configuration().tenant,
                            instance_id: uuid::Uuid::new_v4(),
                            principal_id: uuid::Uuid::from_u128(3),
                        },
                    },
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
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    let mut c = Case::new(Some(RiskLevel::Two)).await;
    let directory = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
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
        locator: execution_contract::PolicyLocator { after: None },
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
            locator: execution_contract::PolicyLocator { after: None },
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
            locator: execution_contract::PolicyLocator { after: None },
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
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for human in [false, true] {
        let mut c = Case::new(Some(RiskLevel::Two)).await;
        let mut directory =
            protocol::policy_directory_fixture(c.service.client.configuration().tenant);
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
                locator: execution_contract::PolicyLocator { after: None },
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
                locator: execution_contract::PolicyLocator { after: None },
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
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for device in [false, true] {
        for level in [1, 2] {
            for change in 0..6 {
                let mut c = Case::new(None).await;
                c.service.release_waiting();
                let mut directory =
                    protocol::policy_directory_fixture(c.service.client.configuration().tenant);
                directory.self_service.risk_level = level;
                if device {
                    directory.subject = wire::SelfServiceSubject::Device {};
                }
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
                        locator: execution_contract::PolicyLocator { after: None },
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
                    let wire::TaskPayload::Script(mut spec) =
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
                        subject: directory.subject.clone(),
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
                        3 if device => {
                            if let wire::TaskAuthorization::SelfService { subject, .. } =
                                &mut spec.authorization
                            {
                                *subject = wire::SelfServiceSubject::User {
                                    user: wire::SelfServiceUser {
                                        tenant_id: data.tenant,
                                        instance_id: crate::protocol_test_support::SERVICE_ID,
                                        principal_id: uuid::Uuid::new_v4(),
                                    },
                                };
                            }
                        }
                        3 => {
                            if let wire::TaskAuthorization::SelfService {
                                subject: wire::SelfServiceSubject::User { user: actor },
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
                        5 => {
                            if let wire::TaskAuthorization::SelfService { subject, .. } =
                                &mut spec.authorization
                            {
                                *subject = if device {
                                    wire::SelfServiceSubject::User {
                                        user: wire::SelfServiceUser {
                                            tenant_id: data.tenant,
                                            instance_id: crate::protocol_test_support::SERVICE_ID,
                                            principal_id: uuid::Uuid::from_u128(3),
                                        },
                                    }
                                } else {
                                    wire::SelfServiceSubject::Device {}
                                };
                            }
                        }
                        _ => (),
                    }
                    data.offer = Some(data.signed(wire::TaskPayload::Script(spec)));
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
                let admission = c.service.core.policy_fence(&bound).unwrap();
                c.service.core.fences.revoke(&c.scope()).unwrap();
                if device {
                    assert!(admission.is_none());
                    assert!(c.service.core.current_policy_access(&bound).is_ok());
                    assert!(!c
                        .service
                        .core
                        .credential_leases
                        .contains_key(&choice.request));
                } else {
                    let admission = admission.unwrap();
                    assert!(admission.0.admit(&admission.1, admission.2).is_err());
                    assert!(c.service.core.current_policy_access(&bound).is_err());
                }
                c.employee("generation-two").await;
                if device {
                    assert!(c.service.core.current_policy_access(&bound).is_ok());
                } else {
                    assert!(c.service.bind_policy_offer(&offer).is_err());
                }
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
}

#[tokio::test]
async fn revoked_generation_cannot_rebind_from_a_queued_native_handoff() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    c.employee("generation-one").await;
    let epoch = c.service.core.fences.current(&c.scope()).unwrap();
    c.service.core.fences.revoke(&c.scope()).unwrap();
    let (reply, receive) = tokio::sync::oneshot::channel();
    c.service.core.enqueue_policy(Command {
        fence_epoch: epoch,
        subject: c.selection.subject.clone(),
        session: c.selection.session,
        binding: Some(c.selection.binding.clone()),
        request: LocalRequest::operation(execution_ipc::host::Request::BindContext {
            expected: Some(c.context().id),
            value: execution_ipc::host::NativeContextSession {
                access: execution_ipc::host::NativeAccess::User {
                    credential: zeroize::Zeroizing::new("c".repeat(64)),

                    user: wire::SelfServiceUser {
                        tenant_id: c.service.client.configuration().tenant,
                        instance_id: crate::protocol_test_support::SERVICE_ID,
                        principal_id: uuid::Uuid::from_u128(3),
                    },
                },
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
    let _budget = crate::scenario_budget::enter();
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
    let _budget = crate::scenario_budget::enter();
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
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for entry in 0..3 {
        let mut c = Case::new(Some(RiskLevel::Two)).await;
        let mut directory =
            protocol::policy_directory_fixture(c.service.client.configuration().tenant);
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
            locator: execution_contract::PolicyLocator { after: None },
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
                locator: execution_contract::PolicyLocator { after: None },
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
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    let mut c = Case::new(Some(RiskLevel::Two)).await;
    let mut directory = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
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
            locator: execution_contract::PolicyLocator { after: None },
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
async fn device_context_binding_does_not_wait_for_background_pki_http() {
    use execution_ipc::host::{Handler, NativeAccess, NativeContextSession, PeerPolicy, Request};
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    c.service.waiting = None;
    c._server.data.lock().unwrap().registration_failure = true;
    assert_eq!(
        c.service.client.renew_certificate().await.unwrap_err(),
        Error::Unavailable
    );
    let pause = Arc::new(tokio::sync::Notify::new());
    c._server.data.lock().unwrap().status_pause = Some(pause.clone());
    select_pki_refresh(&c._root.path);
    let subject = c.selection.subject.clone();
    let session = c.selection.session;
    let binding = c.selection.binding.clone();
    let policy = PeerPolicy {
        images: vec![c.service.core.config.interpreters[0].image.clone()],
        subjects: vec![subject.clone()],
        interactive: false,
    };
    let mut owner = c.service.spawn(policy, None).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !c._server.data.lock().unwrap().status_waiting {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (send, mut receive) = tokio::sync::oneshot::channel();
    let worker = tokio::task::spawn_blocking(move || {
        assert!(matches!(
            owner.call(
                LocalRequest::operation(Request::RevokeContext {}),
                subject.clone(),
                session,
                Some(binding.clone())
            ),
            Reply::ContextRevoked
        ));
        let reply = owner.call(
            LocalRequest::operation(Request::BindContext {
                value: NativeContextSession {
                    access: NativeAccess::Device {},
                    generation: plan::id("device-during-background-http").unwrap(),
                    expires_at_unix_ms: u64::MAX,
                },
                expected: None,
            }),
            subject,
            session,
            Some(binding),
        );
        let _ = send.send(reply);
        owner.stop().unwrap();
    });
    let immediate = tokio::time::timeout(std::time::Duration::from_secs(1), &mut receive).await;
    pause.notify_waiters();
    let (before_release, reply) = match immediate {
        Ok(reply) => (true, reply.unwrap()),
        Err(_) => (false, receive.await.unwrap()),
    };
    worker.await.unwrap();
    assert!(
        before_release,
        "device login waited for unrelated background HTTP"
    );
    assert!(matches!(reply, Reply::ContextBound { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_revocation_and_crl_expiry_close_admission_while_pending_status_waits() {
    let _budget = crate::scenario_budget::enter();
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
                // Observe readiness before this tick enforces it. A CRL may expire during
                // reconciliation; that transition belongs to the next enforcement tick.
                let identity_ready = core.host.identity_ready();
                core.enforce_identity_gate()?;
                core.reconcile()?;
                if data.lock().unwrap().status_waiting && !identity_ready {
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
async fn retired_installation_command_preserves_live_registration_and_journal() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(Some(RiskLevel::Two)).await;
    c.service.core.start_task(c.selection(false)).unwrap();
    let binding = c.service.client.registration().unwrap();
    let record = c.record();
    let owner = c.service.core.host.binding.clone();
    let secrets = c._server.secrets.0.lock().unwrap().clone();
    let config = c._root.path.join("execution.json");
    let candidate = c._root.path.join("candidate.json");
    let old =
        serde_json::to_vec(&serde_json::json!({"version":4,"installation_id":"retained"})).unwrap();
    let next = serde_json::to_vec(&serde_json::json!({"version":2,"product_version":"candidate"}))
        .unwrap();
    std::fs::write(&config, &old).unwrap();
    std::fs::write(&candidate, &next).unwrap();
    for _ in 0..2 {
        // The running communication owner retains its writer lease throughout rejection.
        let lease =
            platform_private_storage::open_existing(&c._root.path.join("client.lock")).unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&lease).is_err());
        let args = vec![
            "upgrade-device-pki".into(),
            "--scope".into(),
            "system".into(),
            "--config".into(),
            config.clone().into(),
            "--candidate".into(),
            candidate.clone().into(),
        ];
        assert_eq!(crate::installation::run(&args), Err(Error::Configuration));
        assert!(fs2::FileExt::try_lock_exclusive(&lease).is_err());
        assert_eq!(std::fs::read(&config).unwrap(), old);
        assert_eq!(std::fs::read(&candidate).unwrap(), next);
        assert!(!c._root.path.join("installation.lock").exists());
        assert!(!c._root.path.join("install-manifests").exists());
        assert_eq!(c.service.client.registration().unwrap(), binding);
        assert_eq!(c.record(), record);
        assert_eq!(c.service.core.host.binding, owner);
        assert_eq!(*c._server.secrets.0.lock().unwrap(), secrets);
    }
}

#[tokio::test]
async fn software_self_service_binds_signed_policy_and_native_context() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    c.service.release_waiting();
    c.employee("software-user").await;
    let mut directory = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
    directory.self_service.risk_level = 2;
    let wire::TaskPayload::Software(spec) = c.offer.payload() else {
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
    {
        let mut data = c._server.data.lock().unwrap();
        data.self_service_directory = Some(wire::SelfServiceDirectoryResponse {
            items: vec![directory.clone()],
            next_cursor: None,
            as_of: c._server.time.now().unwrap(),
        });
        data.self_service_software = Some(protocol::self_service_software::Template {
            spec: (**spec).clone(),
        });
        data.offer = None;
        data.explicit_offers = true;
        data.invocation_reply_lost = true;
    }
    let choice = PolicyChoice {
        request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
        policy_revision: directory.policy_revision,
        policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
        parameters: Default::default(),
    };
    let Reply::PolicyRequest { value: pending } = c
        .policy_call(Request::InvokePolicy {
            locator: execution_contract::PolicyLocator { after: None },
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
    assert_eq!(accepted.request_id.to_string(), choice.request.as_str());
    assert_eq!(c._server.data.lock().unwrap().self_service_offers.len(), 1);
    assert_eq!(c._server.data.lock().unwrap().invocations.len(), 1);
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

impl Case {
    async fn device(&mut self, generation: &str) {
        use execution_ipc::host::{NativeAccess, NativeContextSession, Request};
        self.policy_call(Request::RevokeContext {}).await;
        let Reply::ContextBound { value } = self
            .policy_call(Request::BindContext {
                expected: None,
                value: NativeContextSession {
                    access: NativeAccess::Device {},
                    generation: plan::id(generation).unwrap(),
                    expires_at_unix_ms: u64::MAX,
                },
            })
            .await
        else {
            panic!("device context")
        };
        let ClientOrigin::Ai { context, .. } = &mut self.selection.origin else {
            panic!("AI")
        };
        *context = value;
    }
    fn policy_choice(&self, directory: &wire::SelfServiceDirectoryItem) -> PolicyChoice {
        PolicyChoice {
            request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            policy_id: plan::id(directory.policy_id.to_string()).unwrap(),
            policy_revision: directory.policy_revision,
            policy_version: plan::id(directory.policy_version.to_string()).unwrap(),
            parameters: std::collections::BTreeMap::from([(
                "name".into(),
                serde_json::json!("Acme"),
            )]),
        }
    }
}

#[tokio::test]
async fn device_directory_and_invocation_are_credential_free_with_or_without_employee_login() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for logged_in in [false, true] {
        let mut c = Case::new(None).await;
        if !logged_in {
            c.device("device-only").await;
        }
        let mut item = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
        item.subject = wire::SelfServiceSubject::Device {};
        item.self_service.risk_level = 1;
        let mut user_item = item.clone();
        user_item.policy_id = uuid::Uuid::new_v4();
        user_item.subject = wire::SelfServiceSubject::User {
            user: wire::SelfServiceUser {
                tenant_id: c.service.client.configuration().tenant,
                instance_id: crate::protocol_test_support::SERVICE_ID,
                principal_id: uuid::Uuid::from_u128(3),
            },
        };
        c._server.data.lock().unwrap().self_service_directory =
            Some(wire::SelfServiceDirectoryResponse {
                items: vec![item.clone(), user_item],
                next_cursor: None,
                as_of: 1,
            });
        let Reply::PolicyDirectory { value } = c
            .policy_call(Request::PolicyDirectory {
                context: c.context(),
                after: None,
                origin: c.selection.origin.clone(),
            })
            .await
        else {
            panic!("directory")
        };
        assert_eq!(value.items.len(), if logged_in { 2 } else { 1 });
        let choice = c.policy_choice(&item);
        let Reply::PolicyRequest { value } = c
            .policy_call(Request::InvokePolicy {
                locator: execution_contract::PolicyLocator { after: None },
                context: c.context(),
                choice: choice.clone(),
                origin: c.selection.origin.clone(),
            })
            .await
        else {
            panic!("device invocation")
        };
        assert!(matches!(
            value.action,
            PreparedAction::Policy {
                access: PolicyAccess::Device {},
                ..
            }
        ));
        assert_eq!(value.state, BackendRequestState::Submitting);
        let data = c._server.data.lock().unwrap();
        assert!(data
            .employee_calls
            .last()
            .unwrap()
            .0
            .ends_with("/invocations"));
        assert!(!data.employee_credentials.last().unwrap());
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains(&"c".repeat(64)));
        assert!(!encoded.contains("principalId"));
    }
}

#[tokio::test]
async fn retired_ai_ingress_cannot_read_or_invoke_but_original_device_consent_continues() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    let mut item = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
    item.subject = wire::SelfServiceSubject::Device {};
    item.self_service.risk_level = 2;
    c._server.data.lock().unwrap().self_service_directory =
        Some(wire::SelfServiceDirectoryResponse {
            items: vec![item.clone()],
            next_cursor: None,
            as_of: 1,
        });
    let choice = c.policy_choice(&item);
    let old = c.context();
    let origin = c.selection.origin.clone();
    let Reply::PolicyRequest { value: pending } = c
        .policy_call(Request::InvokePolicy {
            locator: execution_contract::PolicyLocator { after: None },
            context: old.clone(),
            choice: choice.clone(),
            origin: origin.clone(),
        })
        .await
    else {
        panic!("pending")
    };
    assert_eq!(pending.state, BackendRequestState::AwaitingConfirmation);
    c.policy_call(Request::RevokeContext {}).await;
    // Device continuation does not require enterprise memory, even before a fresh ingress.
    assert!(c.service.core.current_policy_access(&pending).is_ok());
    c.device("logged-out").await;
    let calls = c._server.data.lock().unwrap().employee_calls.len();
    for request in [
        Request::PolicyDirectory {
            context: old.clone(),
            after: None,
            origin: origin.clone(),
        },
        Request::InvokePolicy {
            locator: execution_contract::PolicyLocator { after: None },
            context: c.context(),
            choice: choice.clone(),
            origin: origin.clone(),
        },
    ] {
        assert!(matches!(c.policy_call(request).await, Reply::Rejected));
    }
    assert_eq!(c._server.data.lock().unwrap().employee_calls.len(), calls);
    let Reply::PolicyRequest { value: confirmed } = c
        .policy_call(Request::ConfirmPolicy {
            context: c.context(),
            request: choice.request.clone(),
        })
        .await
    else {
        panic!("confirmation")
    };
    assert_eq!(confirmed.trigger, pending.trigger);
    assert_eq!(confirmed.action.request(), &choice.request);
    assert_eq!(confirmed.state, BackendRequestState::Submitting);
    let consent = confirmed.confirmation.clone();
    assert!(consent.is_some());
    let Reply::PolicyRequest { value: again } = c
        .policy_call(Request::ConfirmPolicy {
            context: c.context(),
            request: choice.request.clone(),
        })
        .await
    else {
        panic!("original")
    };
    assert_eq!(again.confirmation, consent);
    let data = c._server.data.lock().unwrap();
    assert_eq!(
        data.employee_calls
            .iter()
            .filter(|(url, _)| url.ends_with("/invocations"))
            .count(),
        1
    );
    assert_eq!(data.employee_calls.last().unwrap().1["source"], "ai");
    assert!(data.employee_calls.last().unwrap().1["confirmed"]
        .as_bool()
        .unwrap());
    assert!(!data.employee_credentials.last().unwrap());
}

#[tokio::test]
async fn unknown_device_submission_queries_original_before_any_repeat_post_after_logout() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for accepted in [false, true] {
        let mut c = Case::new(None).await;
        let mut item = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
        item.subject = wire::SelfServiceSubject::Device {};
        item.self_service.risk_level = 1;
        {
            let mut data = c._server.data.lock().unwrap();
            data.self_service_directory = Some(wire::SelfServiceDirectoryResponse {
                items: vec![item.clone()],
                next_cursor: None,
                as_of: 1,
            });
            data.invocation_reply_lost = accepted;
            data.invocation_unaccepted = !accepted;
        }
        let choice = c.policy_choice(&item);
        let Reply::PolicyRequest { value: unknown } = c
            .policy_call(Request::InvokePolicy {
                locator: execution_contract::PolicyLocator { after: None },
                context: c.context(),
                choice: choice.clone(),
                origin: c.selection.origin.clone(),
            })
            .await
        else {
            panic!("unknown")
        };
        assert!(matches!(
            unknown.action,
            PreparedAction::Policy { run_id: None, .. }
        ));
        c.device("logout").await;
        let offset = {
            let mut data = c._server.data.lock().unwrap();
            data.invocation_unaccepted = false;
            data.employee_calls.len()
        };
        let reply = c
            .policy_call(Request::PolicyStatus {
                context: c.context(),
                request: choice.request.clone(),
            })
            .await;
        assert!(matches!(
            reply,
            Reply::PolicyStatus { .. } | Reply::PolicyRequest { .. }
        ));
        let data = c._server.data.lock().unwrap();
        let calls = &data.employee_calls[offset..];
        assert!(calls[0].0.ends_with(choice.request.as_str()));
        assert_eq!(
            calls
                .iter()
                .filter(|(url, _)| url.ends_with("/invocations"))
                .count(),
            usize::from(!accepted)
        );
        assert!(data.employee_credentials[offset..]
            .iter()
            .all(|present| !present));
        assert_eq!(data.invocations.len(), 1);
        let posted: Vec<_> = data
            .employee_calls
            .iter()
            .filter(|(url, _)| url.ends_with("/invocations"))
            .map(|(_, body)| body)
            .collect();
        assert!(posted.iter().all(|body| *body == posted[0]));
    }
}

#[tokio::test]
async fn fences_isolate_os_sessions_and_user_requests_never_downgrade() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    let mut item = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
    item.self_service.risk_level = 1;
    c._server.data.lock().unwrap().self_service_directory =
        Some(wire::SelfServiceDirectoryResponse {
            items: vec![item.clone()],
            next_cursor: None,
            as_of: 1,
        });
    let choice = c.policy_choice(&item);
    let Reply::PolicyRequest { value } = c
        .policy_call(Request::InvokePolicy {
            locator: execution_contract::PolicyLocator { after: None },
            context: c.context(),
            choice: choice.clone(),
            origin: c.selection.origin.clone(),
        })
        .await
    else {
        panic!("user")
    };
    let other = crate::service::policy::SessionKey::new(
        c.selection.subject.clone(),
        plan::id("another-os-session").unwrap(),
    );
    c.service.core.fences.revoke(&other).unwrap();
    assert!(c.service.core.current_policy_access(&value).is_ok());
    c.device("logout").await;
    assert!(c.service.core.current_policy_access(&value).is_err());
    let calls = c._server.data.lock().unwrap().employee_calls.len();
    assert!(matches!(
        c.policy_call(Request::PolicyStatus {
            context: c.context(),
            request: choice.request.clone()
        })
        .await,
        Reply::Rejected
    ));
    assert_eq!(c._server.data.lock().unwrap().employee_calls.len(), calls);
    assert!(matches!(
        value.action,
        PreparedAction::Policy {
            access: PolicyAccess::User { .. },
            ..
        }
    ));
}

impl Case {
    async fn accepted_policy_offer(&mut self, device: bool) -> (BackendRequest, Offer) {
        self.accepted_policy_offer_lifetime(device, None).await
    }
    async fn accepted_policy_offer_lifetime(
        &mut self,
        device: bool,
        seconds: Option<i64>,
    ) -> (BackendRequest, Offer) {
        use execution_ipc::host::Request;
        let mut item =
            protocol::policy_directory_fixture(self.service.client.configuration().tenant);
        item.self_service.risk_level = 2;
        if device {
            item.subject = wire::SelfServiceSubject::Device {};
        }
        self._server.data.lock().unwrap().self_service_directory =
            Some(wire::SelfServiceDirectoryResponse {
                items: vec![item.clone()],
                next_cursor: None,
                as_of: 1,
            });
        let choice = self.policy_choice(&item);
        let reply = self
            .policy_call(Request::InvokePolicy {
                locator: execution_contract::PolicyLocator { after: None },
                context: self.context(),
                choice: choice.clone(),
                origin: self.selection.origin.clone(),
            })
            .await;
        assert!(matches!(reply, Reply::PolicyRequest { .. }));
        let Reply::PolicyRequest { value: record } = self
            .policy_call(Request::ConfirmPolicy {
                context: self.context(),
                request: choice.request.clone(),
            })
            .await
        else {
            panic!("accepted")
        };
        {
            let mut data = self._server.data.lock().unwrap();
            let accepted = data.invocations
                [&uuid::Uuid::parse_str(choice.request.as_str()).unwrap()]
                .1
                .clone();
            data.script();
            let wire::TaskPayload::Script(mut spec) = data.offer.as_ref().unwrap().payload.clone()
            else {
                panic!("script")
            };
            spec.task_id = accepted.run_id;
            if let Some(seconds) = seconds {
                spec.expires_at = self.service.core.host.clock.now().unwrap() + seconds;
            }
            data.task = accepted.run_id;
            let wire::SelfServiceTask::Script {
                resource, run_as, ..
            } = &item.task
            else {
                panic!("script")
            };
            spec.resource = resource.clone();
            spec.run_as = *run_as;
            spec.parameters = std::collections::BTreeMap::from([
                ("name".into(), serde_json::json!("Acme")),
                ("path".into(), serde_json::json!("/Applications")),
            ]);
            spec.authorization = wire::TaskAuthorization::SelfService {
                request_id: accepted.request_id,
                subject: item.subject,
                source: wire::SelfServiceCaller::Ai,
                policy_id: item.policy_id,
                policy_revision: item.policy_revision,
                policy_version: item.policy_version,
                allow_ai: true,
                risk_level: 2,
                confirmed: true,
            };
            data.offer = Some(data.signed(wire::TaskPayload::Script(spec)));
            data.explicit_offers = true;
        }
        let offer = self.service.client.claim().await.unwrap().offer.unwrap();
        self.service.client.received(&offer).await.unwrap();
        (*record, offer)
    }
    fn policy_record(&self, request: &RequestId) -> BackendRequest {
        self.service
            .core
            .app
            .backend_request(&self.service.core.caller(), request)
            .unwrap()
            .unwrap()
    }
    // Stage a historical elapsed deadline in the actual SQLite fixture. SystemClock and
    // production immutable-action/CAS rules remain unchanged; this is not a migration.
    fn elapsed_policy(&self, request: &RequestId) -> BackendRequest {
        let mut record = self.policy_record(request);
        let until = self.service.core.host.clock.millis().unwrap() - 1;
        let PreparedAction::Policy {
            expires_at_unix_ms, ..
        } = &mut record.action
        else {
            panic!("policy")
        };
        *expires_at_unix_ms = until;
        record.risk.as_mut().unwrap().expires_at_unix_ms = until;
        if let Some(confirmation) = &mut record.confirmation {
            confirmation.confirmed_at_unix_ms = until - 1;
            confirmation.expires_at_unix_ms = until;
        }
        let sql = rusqlite::Connection::open(self._root.path.join("execution.sqlite")).unwrap();
        assert_eq!(
            sql.execute(
                "UPDATE backend_requests SET body=?1 WHERE request_id=?2",
                rusqlite::params![serde_json::to_vec(&record).unwrap(), request.as_str()],
            )
            .unwrap(),
            1
        );
        assert_eq!(self.policy_record(request), record);
        record
    }
    fn reopen(self) -> Self {
        let Self {
            _server,
            _root,
            service,
            offer,
            selection,
        } = self;
        let config = service.core.config.clone();
        let helpers = service.core.helpers.clone();
        drop(service);
        let clock = SystemClock::new().unwrap();
        let client = Client::open(
            &_root.path,
            _server.config(),
            OpenMode::Existing,
            _server.secrets.clone(),
            clock.clone(),
        )
        .unwrap();
        let service = DeviceService::assemble(
            client,
            &_root.path.join("execution.sqlite"),
            crate::ProductionStartup::Open,
            config,
            clock,
            helpers,
            execution_runner::staging::MaterialRoot::open_fixture,
            uuid::Uuid::from_u128(2909),
        )
        .unwrap();
        Self {
            _server,
            _root,
            service,
            offer,
            selection,
        }
    }
}

#[tokio::test]
async fn device_recovery_reopens_both_journals_and_preserves_every_accepted_intent_at_capacity() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    c.service.release_waiting();
    c.service
        .abandon(c.offer.task_id(), &mut commands)
        .await
        .unwrap();
    let (first, first_offer) = c.accepted_policy_offer(true).await;
    let (second, second_offer) = c.accepted_policy_offer(true).await;
    let posts = c
        ._server
        .data
        .lock()
        .unwrap()
        .employee_calls
        .iter()
        .filter(|(url, _)| url.ends_with("/invocations"))
        .count();
    let mut c = c.reopen();
    assert!(c.service.core.contexts.is_empty());
    // A failed future claim cannot interfere with recovery or allocate another offer.
    c._server.data.lock().unwrap().claim_failure = true;
    let result = c.service.drive(&mut commands).await;
    assert!(matches!(result, Err(Error::Unavailable)), "{result:?}");
    let waiting = &c.service.waiting.as_ref().unwrap().0;
    assert_eq!(waiting.task_id(), first_offer.task_id());
    assert_eq!(waiting.attempt_id(), first_offer.attempt_id());
    let tasks = c
        .service
        .client
        .pending_tasks(c.service.client.configuration().limits.pending_tasks)
        .unwrap();
    assert!(tasks.contains(&first_offer.task_id()));
    assert!(tasks.contains(&second_offer.task_id()));
    for original in [&first, &second] {
        let restored = c
            .service
            .core
            .app
            .backend_request(&c.service.core.caller(), original.action.request())
            .unwrap()
            .unwrap();
        assert_eq!(&restored, original);
        assert!(restored.confirmation.is_some());
        assert!(c.service.core.current_policy_access(&restored).is_ok());
    }
    assert_eq!(
        c._server
            .data
            .lock()
            .unwrap()
            .employee_calls
            .iter()
            .filter(|(url, _)| url.ends_with("/invocations"))
            .count(),
        posts
    );
    assert!(c
        .service
        .core
        .app
        .service_tasks(None, 64)
        .unwrap()
        .items
        .is_empty());
    assert!(c._server.data.lock().unwrap().start_ops.is_empty());
}

#[tokio::test]
async fn recovery_never_reconstructs_device_consent_from_a_retired_journal_intent() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    c.service.release_waiting();
    c.service
        .abandon(c.offer.task_id(), &mut commands)
        .await
        .unwrap();
    let (_, offer) = c.accepted_policy_offer(true).await;
    let request = backend::request_id(&offer).unwrap();
    c.service
        .core
        .transition(
            &request,
            BackendRequestState::Cancelled,
            Some(BackendRequestFailure::Revoked),
        )
        .unwrap();
    let mut c = c.reopen();
    let prior_claims = c._server.data.lock().unwrap().claim_ops.len();
    // Acknowledged cancellation retires the original offer; the next claim has no work.
    c.service.drive(&mut commands).await.unwrap();
    assert!(c._server.data.lock().unwrap().claim_ops.len() > prior_claims);
    assert!(c.service.waiting.is_none());
    assert!(c
        .service
        .client
        .pending_tasks(c.service.client.configuration().limits.pending_tasks)
        .unwrap()
        .is_empty());
    assert!(c._server.data.lock().unwrap().start_ops.is_empty());
    let cancelled = c
        .service
        .core
        .app
        .backend_request(&c.service.core.caller(), &request)
        .unwrap()
        .unwrap();
    assert_eq!(cancelled.state, BackendRequestState::Cancelled);
    assert_eq!(cancelled.failure, Some(BackendRequestFailure::Revoked));
}

#[tokio::test]
async fn actual_post_start_dispatch_boundary_rejects_revoked_user_before_any_runner_attempt() {
    let _budget = crate::scenario_budget::enter();
    for revoke_original in [false, true] {
        let mut c = Case::new(None).await;
        c.service.release_waiting();
        let (original, offer) = c.accepted_policy_offer(false).await;
        let selection = c.service.bind_policy_offer(&offer).unwrap();
        let materials = c.service.client.prepare(&offer).await.unwrap();
        let start = c
            .service
            .client
            .start_user_initiated(&offer, &materials)
            .await
            .unwrap();
        assert!(!c._server.data.lock().unwrap().start_ops.is_empty());
        let scope = if revoke_original {
            c.scope()
        } else {
            crate::service::policy::SessionKey::new(
                c.selection.subject.clone(),
                plan::id("other-os-session").unwrap(),
            )
        };
        c.service.core.fences.revoke(&scope).unwrap();
        if revoke_original {
            // The production submit wrapper verifies the helper before this exact boundary.
            // This staged peer fixture proves the shared Start-to-runner path, not native IPC.
            assert!(matches!(
                crate::backend::dispatch_started(
                    &mut c.service,
                    offer,
                    materials,
                    Some(selection),
                    start,
                    None
                ),
                Err(Error::Identity)
            ));
            assert!(c
                .service
                .core
                .app
                .service_tasks(None, 64)
                .unwrap()
                .items
                .is_empty());
            assert_eq!(
                c.service
                    .core
                    .app
                    .backend_request(&c.service.core.caller(), original.action.request())
                    .unwrap()
                    .unwrap()
                    .trigger,
                original.trigger
            );
        } else {
            assert!(c.service.core.current_policy_access(&original).is_ok());
        }
    }
}

#[tokio::test]
async fn expired_device_confirmation_checks_original_os_owner_before_read_or_write() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    c.device("original-device").await;
    let mut item = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
    item.subject = wire::SelfServiceSubject::Device {};
    c._server.data.lock().unwrap().self_service_directory =
        Some(wire::SelfServiceDirectoryResponse {
            items: vec![item.clone()],
            next_cursor: None,
            as_of: 1,
        });
    let choice = c.policy_choice(&item);
    assert!(matches!(
        c.policy_call(Request::InvokePolicy {
            locator: execution_contract::PolicyLocator { after: None },
            context: c.context(),
            choice: choice.clone(),
            origin: c.selection.origin.clone(),
        })
        .await,
        Reply::PolicyRequest { .. }
    ));
    let original = c.elapsed_policy(&choice.request);
    assert_eq!(original.state, BackendRequestState::AwaitingConfirmation);
    let subject = c.selection.subject.clone();
    let binding = c.selection.binding.clone();
    let origin = c.selection.origin.clone();
    for other_subject in [false, true] {
        c.selection.binding = plan::id("another-os-login").unwrap();
        c.selection.subject = if other_subject {
            "another-os-account".into()
        } else {
            subject.clone()
        };
        c.device(if other_subject {
            "another-account"
        } else {
            "another-login"
        })
        .await;
        assert!(matches!(
            c.policy_call(Request::ConfirmPolicy {
                context: c.context(),
                request: choice.request.clone(),
            })
            .await,
            Reply::Rejected
        ));
        assert_eq!(c.policy_record(&choice.request), original);
        assert!(c._server.data.lock().unwrap().invocations.is_empty());
    }
    c.selection.subject = subject;
    c.selection.binding = binding;
    c.selection.origin = origin;
    let Reply::PolicyRequest { value } = c
        .policy_call(Request::ConfirmPolicy {
            context: c.context(),
            request: choice.request.clone(),
        })
        .await
    else {
        panic!("owner expiration")
    };
    assert_eq!(value.state, BackendRequestState::Failed);
    assert_eq!(value.failure, Some(BackendRequestFailure::Expired));
    assert_eq!(value.revision, original.revision + 1);
}

#[tokio::test]
async fn transient_start_replays_original_attempt_without_retiring_policy_or_consent() {
    let _budget = crate::scenario_budget::enter();
    for device in [false, true] {
        let mut c = Case::new(None).await;
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        c.service.release_waiting();
        c.service
            .abandon(c.offer.task_id(), &mut commands)
            .await
            .unwrap();
        let (original, offer) = c.accepted_policy_offer(device).await;
        c.service.bind_policy_offer(&offer).unwrap();
        let before = c.policy_record(original.action.request());
        let materials = c.service.client.prepare(&offer).await.unwrap();
        c._server.data.lock().unwrap().start_failure = true;
        let error = c
            .service
            .client
            .start_user_initiated(&offer, &materials)
            .await
            .err()
            .unwrap();
        assert_eq!(error, Error::Unavailable);
        assert_eq!(
            c.service
                .submission_result(original.action.request(), Err(error))
                .unwrap_err(),
            error
        );
        assert_eq!(c.policy_record(original.action.request()), before);
        assert!(c
            .service
            .core
            .app
            .service_tasks(None, 64)
            .unwrap()
            .items
            .is_empty());
        drop(materials);
        // Device resumes across both journals. User resumes only while its exact native
        // context remains live in this owner; a process restart cannot reconstruct it.
        let mut c = if device { c.reopen() } else { c };
        c.service.next_network = std::time::Instant::now();
        c.service.drive(&mut commands).await.unwrap();
        let (restored, materials) = c.service.waiting.take().unwrap();
        assert_eq!(restored.task_id(), offer.task_id());
        assert_eq!(restored.attempt_id(), offer.attempt_id());
        let selection = c.service.bind_policy_offer(&restored).unwrap();
        let start = c
            .service
            .client
            .start_user_initiated(&restored, &materials)
            .await
            .unwrap();
        let ops = c._server.data.lock().unwrap().start_ops.clone();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0], ops[1]);
        let retained = c.policy_record(original.action.request());
        assert_eq!(retained.confirmation, original.confirmation);
        assert_eq!(retained.trigger, original.trigger);
        assert_eq!(retained.action.request(), original.action.request());
        assert_eq!(
            selection
                .trigger(&c.service.core.host.binding.device)
                .unwrap(),
            original.trigger
        );
        let wire::TaskPayload::Script(spec) = start.payload() else {
            panic!("script")
        };
        let (compiled, _artifacts) = backend::plan::script(
            &restored,
            &materials,
            spec,
            (&c.service.core.host.binding, &c.service.core.host.actor),
            &c.service.core.config.interpreters,
            (
                &c.service.core.config.work_root,
                materials.files()[0].path(),
            ),
            None,
        )
        .unwrap();
        let mut input = compiled.spec().clone();
        let Initiator::Backend { trigger, .. } = &mut input.request.initiator else {
            panic!("backend")
        };
        *trigger = original.trigger.clone();
        let input = FrozenExecution::freeze(input, &plan::storage_limits().input).unwrap();
        // The actual product gate needs restored signed risk as well as original consent.
        // This fixture has no authenticated system/helper runner and grants no native launch.
        ProductGateProof::verify_start(
            &restored,
            &start,
            &input,
            Some(&retained),
            c.service.core.available_risk.as_ref(),
            c.service.core.host.clock.millis().unwrap(),
        )
        .unwrap();
        let caller = c.service.core.caller();
        assert!(matches!(
            c.service.core.app.request_execution(&caller, &input),
            Err(execution_app::Error::Unbound)
        ));
        assert_eq!(
            c.service
                .core
                .app
                .service_tasks(None, 64)
                .unwrap()
                .items
                .len(),
            1
        );
        let committed = c.policy_record(original.action.request());
        // Once local intent exists, its journal owns recovery even if a caller observes
        // a retryable transport error or a subsequent native rejection.
        for error in [Error::Unavailable, Error::Denied] {
            assert_eq!(
                c.service
                    .submission_result(original.action.request(), Err(error))
                    .unwrap_err(),
                error
            );
            assert_eq!(c.policy_record(original.action.request()), committed);
        }
        assert_eq!(
            c.service
                .core
                .app
                .service_tasks(None, 64)
                .unwrap()
                .items
                .len(),
            1
        );
        let status = c
            .service
            .core
            .app
            .status(&caller, original.action.request())
            .unwrap();
        assert_eq!(
            status.attempts, 0,
            "no native runner was authorized by this fixture"
        );
    }
}

#[tokio::test]
async fn restarted_policy_expiry_preserves_original_run_queries_without_start() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for offer_expired in [false, true] {
        let mut c = Case::new(None).await;
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        c.service.release_waiting();
        c.service
            .abandon(c.offer.task_id(), &mut commands)
            .await
            .unwrap();
        let (original, _) = c
            .accepted_policy_offer_lifetime(true, offer_expired.then_some(2))
            .await;
        let request = original.action.request().clone();
        let before = if offer_expired {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            original.clone()
        } else {
            c.elapsed_policy(&request)
        };
        let mut c = c.reopen();
        c.service.drive(&mut commands).await.unwrap();
        let expired = c.policy_record(&request);
        assert_eq!(expired.failure, Some(BackendRequestFailure::Expired));
        assert_eq!(expired.state, BackendRequestState::Failed);
        assert_eq!(expired.confirmation, before.confirmation);
        assert_eq!(expired.trigger, before.trigger);
        assert!(c.service.waiting.is_none());
        assert!(c
            .service
            .client
            .pending_tasks(c.service.client.configuration().limits.pending_tasks)
            .unwrap()
            .is_empty());
        c.device("new-device-context").await;
        let offset = c._server.data.lock().unwrap().employee_calls.len();
        let Reply::PolicyStatus { value } = c
            .policy_call(Request::PolicyStatus {
                context: c.context(),
                request: request.clone(),
            })
            .await
        else {
            panic!("original Run query")
        };
        assert_eq!(value.request_id.to_string(), request.as_str());
        let PreparedAction::Policy {
            run_id: Some(run_id),
            ..
        } = &before.action
        else {
            panic!("Run")
        };
        assert_eq!(value.run_id.to_string(), run_id.as_str());
        let data = c._server.data.lock().unwrap();
        assert_eq!(data.employee_calls.len(), offset + 1);
        assert!(data.employee_calls[offset].0.ends_with(request.as_str()));
        assert!(!data.employee_credentials[offset]);
        assert!(data.start_ops.is_empty());
        assert!(c
            .service
            .core
            .app
            .service_tasks(None, 64)
            .unwrap()
            .items
            .is_empty());
    }
}

#[tokio::test]
async fn operation_draft_freezes_current_parameters_once_and_queries_do_not_execute() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for level in [1, 2] {
        let mut c = Case::new(None).await;
        let mut directory =
            protocol::policy_directory_fixture(c.service.client.configuration().tenant);
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
            parameters: Default::default(), // An incomplete draft is not an executable choice.
        };
        let Reply::PolicyRequest { value: draft } = c
            .policy_call(Request::PreparePolicy {
                locator: execution_contract::PolicyLocator { after: None },
                context: c.context(),
                choice: choice.clone(),
                origin: c.selection.origin.clone(),
            })
            .await
        else {
            panic!("draft")
        };
        assert_eq!(draft.state, BackendRequestState::Draft);
        assert!(draft.confirmation.is_none());
        for _ in 0..2 {
            let Reply::PolicyRequest { value } = c
                .policy_call(Request::PolicyStatus {
                    context: c.context(),
                    request: choice.request.clone(),
                })
                .await
            else {
                panic!("draft query")
            };
            assert_eq!(*value, *draft);
        }
        // The original confirmation endpoint cannot accidentally authorize a draft.
        c.policy_call(Request::ConfirmPolicy {
            context: c.context(),
            request: choice.request.clone(),
        })
        .await;
        assert!(c._server.data.lock().unwrap().invocations.is_empty());
        let mut input = PolicySubmission {
            request: choice.request.clone(),
            revision: draft.revision,
            parameters: std::collections::BTreeMap::from([(
                "path".into(),
                serde_json::json!("/tmp/forged"),
            )]),
        };
        assert!(matches!(
            c.policy_call(Request::SubmitPolicy {
                context: c.context(),
                input: input.clone()
            })
            .await,
            Reply::OperationRejected {
                reason: OperationRejection::InvalidParameters
            }
        ));
        input.parameters =
            std::collections::BTreeMap::from([("name".into(), serde_json::json!("User edit"))]);
        input.revision += 1;
        assert!(matches!(
            c.policy_call(Request::SubmitPolicy {
                context: c.context(),
                input: input.clone()
            })
            .await,
            Reply::OperationRejected {
                reason: OperationRejection::Conflict
            }
        ));
        input.revision = draft.revision;
        let Reply::PolicyRequest { value: submitted } = c
            .policy_call(Request::SubmitPolicy {
                context: c.context(),
                input: input.clone(),
            })
            .await
        else {
            panic!("save")
        };
        assert_eq!(submitted.state, BackendRequestState::Submitting);
        assert_eq!(submitted.confirmation.is_some(), level == 2);
        // A lost reply is recovered by the same request and same frozen final values.
        let Reply::PolicyRequest { value: repeated } = c
            .policy_call(Request::SubmitPolicy {
                context: c.context(),
                input: input.clone(),
            })
            .await
        else {
            panic!("replay")
        };
        assert_eq!(*repeated, *submitted);
        input
            .parameters
            .insert("name".into(), serde_json::json!("different"));
        assert!(matches!(
            c.policy_call(Request::SubmitPolicy {
                context: c.context(),
                input
            })
            .await,
            Reply::OperationRejected {
                reason: OperationRejection::Conflict
            }
        ));
        let data = c._server.data.lock().unwrap();
        assert_eq!(data.invocations.len(), 1);
        let invocation = &data.invocations.values().next().unwrap().0;
        assert_eq!(invocation["parameters"]["name"], "User edit");
        assert_eq!(invocation["source"], "ai");
        assert_eq!(invocation["confirmed"], level == 2);
        drop(data);
        let mut c = c.reopen();
        assert_eq!(c.policy_record(&choice.request), *submitted);
        let mut forged = (*submitted).clone();
        forged.revision += 1;
        if let PreparedAction::Policy { choice, .. } = &mut forged.action {
            choice
                .parameters
                .insert("name".into(), serde_json::json!("changed after restart"));
        }
        assert!(c
            .service
            .core
            .app
            .record_backend_request(&c.service.core.caller(), Some(&submitted), &forged)
            .is_err());
        assert_eq!(c.policy_record(&choice.request), *submitted);
    }
}

#[tokio::test]
async fn operation_draft_rejects_changed_policy_and_retired_native_context() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for mode in 0..3 {
        let mut c = Case::new(None).await;
        let directory = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
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
            parameters: Default::default(),
        };
        let Reply::PolicyRequest { value } = c
            .policy_call(Request::PreparePolicy {
                locator: execution_contract::PolicyLocator { after: None },
                context: c.context(),
                choice: choice.clone(),
                origin: c.selection.origin.clone(),
            })
            .await
        else {
            panic!("draft")
        };
        if mode == 2 {
            c = c.reopen();
            assert_eq!(c.policy_record(&choice.request), *value);
            assert!(c._server.data.lock().unwrap().invocations.is_empty());
            c.employee("generation-restart").await;
        } else if mode == 1 {
            c.employee("generation-two").await;
        } else {
            c._server
                .data
                .lock()
                .unwrap()
                .self_service_directory
                .as_mut()
                .unwrap()
                .items[0]
                .policy_revision += 1;
        }
        assert!(matches!(
            c.policy_call(Request::SubmitPolicy {
                context: c.context(),
                input: PolicySubmission {
                    request: choice.request.clone(),
                    revision: value.revision,
                    parameters: std::collections::BTreeMap::from([(
                        "name".into(),
                        serde_json::json!("User edit")
                    )]),
                }
            })
            .await,
            Reply::OperationRejected { .. }
        ));
        assert!(c._server.data.lock().unwrap().invocations.is_empty());
        let retained = c
            .service
            .core
            .app
            .backend_request(&c.service.core.caller(), &choice.request)
            .unwrap()
            .unwrap();
        assert_eq!(retained.state, BackendRequestState::Draft);
        assert!(retained.confirmation.is_none());
    }
}

#[tokio::test]
async fn operation_draft_rejects_secret_and_unresolved_schemas_before_persistence() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for schema in [
        serde_json::json!({"type":"object","properties":{"name":{"type":"string","writeOnly":true}}}),
        serde_json::json!({"type":"object","properties":{"name":{"type":"object","properties":{"secret":{"format":"password"}}}}}),
        serde_json::json!({"$ref":"#/$defs/secret","$defs":{"secret":{"type":"string","writeOnly":true}}}),
    ] {
        let mut c = Case::new(None).await;
        let mut directory =
            protocol::policy_directory_fixture(c.service.client.configuration().tenant);
        if let wire::SelfServiceTask::Script {
            parameter_schema, ..
        } = &mut directory.task
        {
            *parameter_schema = schema;
        }
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
            parameters: std::collections::BTreeMap::from([(
                "name".into(),
                serde_json::json!("secret-must-not-be-returned"),
            )]),
        };
        let reply = c
            .policy_call(Request::PreparePolicy {
                locator: execution_contract::PolicyLocator { after: None },
                context: c.context(),
                choice: choice.clone(),
                origin: c.selection.origin.clone(),
            })
            .await;
        assert!(matches!(reply, Reply::Rejected));
        assert!(c
            .service
            .core
            .app
            .backend_request(&c.service.core.caller(), &choice.request)
            .unwrap()
            .is_none());
        assert!(c._server.data.lock().unwrap().invocations.is_empty());
    }
}

#[tokio::test]
async fn full_report_queue_after_restart_drains_before_retrying_native_observation() {
    let _budget = crate::scenario_budget::enter();
    let mut case = Case::with_report_collection(None, 600, true, true).await;
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    case.service.release_waiting();
    case.service
        .abandon(case.offer.task_id(), &mut commands)
        .await
        .unwrap();
    let limit = case.service.client.configuration().limits.pending_reports;
    for _ in 0..limit {
        case.service
            .client
            .queue_report(
                "inventory",
                wire::ReportBody::Failed {
                    code: wire::FailureCode::CollectionFailed,
                },
                1,
            )
            .unwrap();
    }
    case._server.data.lock().unwrap().report_failure = true;
    assert_eq!(
        case.service.client.flush_reports(1).await,
        Err(Error::Unavailable)
    );
    let config = case.service.core.config.clone();
    let helpers = case.service.core.helpers.clone();
    drop(case.service);
    case._server.data.lock().unwrap().offer = None;
    let clock = SystemClock::new().unwrap();
    let client = Client::open(
        &case._root.path,
        case._server.config(),
        OpenMode::Existing,
        case._server.secrets.clone(),
        clock.clone(),
    )
    .unwrap();
    let mut service = DeviceService::assemble(
        client,
        &case._root.path.join("execution.sqlite"),
        ProductionStartup::Open,
        config,
        clock,
        helpers,
        execution_runner::staging::MaterialRoot::open_fixture,
        uuid::Uuid::from_u128(2909),
    )
    .unwrap();
    assert!(service.core.mdm.observed_at.is_none());
    for _ in 0..limit.div_ceil(16) + 1 {
        service.next_network = std::time::Instant::now();
        service.drive(&mut commands).await.unwrap();
    }
    let next_task = {
        let mut data = case._server.data.lock().unwrap();
        data.task = uuid::Uuid::new_v4();
        data.attempt = uuid::Uuid::new_v4();
        data.software(1, true);
        data.task
    };
    service.next_network = std::time::Instant::now();
    service.drive(&mut commands).await.unwrap();
    assert_eq!(
        service.waiting.as_ref().unwrap().0.task_id(),
        next_task,
        "report capacity must not starve task claims"
    );
    assert_eq!(
        service.client.flush_reports(1).await.unwrap(),
        0,
        "the owner must already have ACKed the backlog"
    );
    let data = case._server.data.lock().unwrap();
    assert_eq!(data.reports.len(), limit + 1);
    let latest = data
        .reports
        .values()
        .find(|r| r["collection"]["dataset"] == "channel.mdm.enrollment")
        .unwrap();
    assert_eq!(
        latest["body"]["values"][0]["value"]["value"]["value"],
        service.core.mdm.state.as_str()
    );
    assert_eq!(service.core.mdm_reported, Some(service.core.mdm.state));
}

#[tokio::test]
async fn startup_authority_uses_verified_payload_not_presentation() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    let selection = c.selection(true);
    let view = c.service.core.available.as_mut().unwrap();
    view.presentation.title = "Ordinary display change".into();
    view.presentation.summary = BackendTaskSummary::Credential {};
    view.presentation
        .extra
        .insert("future".into(), serde_json::json!({"layout":"compact"}));
    c.service.core.start_task(selection).unwrap();
    assert_eq!(c.record().state, BackendRequestState::Ready);
}

#[tokio::test]
async fn startup_authority_requires_current_verified_revision() {
    let _budget = crate::scenario_budget::enter();
    for missing in [true, false] {
        let mut c = Case::new(None).await;
        let mut selection = c.selection(true);
        if missing {
            c.service.core.available_start = None;
        } else {
            let changed = Digest::new("a".repeat(64)).unwrap();
            c.service.core.available.as_mut().unwrap().revision = changed.clone();
            selection.revision = changed;
        }
        assert!(matches!(
            c.service.core.start_task(selection),
            Err(Error::Denied)
        ));
    }
}

#[tokio::test]
async fn current_user_lost_ack_does_not_gate_native_context_or_device_resources() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::{NativeAccess, NativeContextSession, Request};
    let mut c = Case::new(None).await;
    c.policy_call(Request::RevokeContext {}).await;
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    c.service
        .synchronize_current_user(&mut commands)
        .await
        .unwrap();
    let value = NativeContextSession {
        access: NativeAccess::User {
            credential: zeroize::Zeroizing::new("c".repeat(64)),
            user: protocol::current_user::user(c.service.client.configuration().tenant),
        },
        generation: plan::id("pending-enterprise").unwrap(),
        expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 60000,
    };
    c._server.data.lock().unwrap().current_user.lose_reply = true;
    let before = c._server.data.lock().unwrap().current_user.inputs.len();
    let Reply::ContextBound { value: context } = c
        .policy_call(Request::BindContext {
            value: value.clone(),
            expected: None,
        })
        .await
    else {
        panic!("local binding waited for MDM presence");
    };
    assert_eq!(
        c._server.data.lock().unwrap().current_user.inputs.len(),
        before,
        "native context must perform no presence HTTP"
    );
    c.service
        .synchronize_current_user(&mut commands)
        .await
        .unwrap();
    assert_eq!(
        c.service.client.current_user_status().unwrap().phase,
        execution_contract::CurrentUserSyncPhase::BindingUnknown
    );
    assert!(matches!(
        c.policy_call(Request::PolicyDirectory {
            context: context.clone(),
            after: None,
            origin: ClientOrigin::Desktop {}
        })
        .await,
        Reply::PolicyDirectory { .. }
    ));
    assert!(matches!(
        c.policy_call(Request::BindContext {
            value,
            expected: Some(context.id)
        })
        .await,
        Reply::ContextBound { .. }
    ));
    c.service
        .synchronize_current_user(&mut commands)
        .await
        .unwrap();
    let d = c._server.data.lock().unwrap();
    assert_eq!(d.current_user.inputs.len(), before + 2);
    assert_eq!(
        d.current_user.inputs[before],
        d.current_user.inputs[before + 1]
    );
}
#[tokio::test]
async fn raw_context_fence_does_not_delete_the_server_relation() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    let before = c._server.data.lock().unwrap().current_user.revision;
    c.policy_call(execution_ipc::host::Request::RevokeContext {})
        .await;
    assert_eq!(c._server.data.lock().unwrap().current_user.revision, before);
    let reply = c
        .policy_call(execution_ipc::host::Request::RetireEnterpriseContext {
            generation: Some(plan::id("initial").unwrap()),
            proof: Some(zeroize::Zeroizing::new("c".repeat(64))),
            recover: false,
        })
        .await;
    assert!(
        matches!(reply,Reply::CurrentUserSync{value} if value.phase==execution_contract::CurrentUserSyncPhase::Unbound)
    );
}

#[tokio::test]
async fn expired_enterprise_lease_settles_current_user_without_a_live_desktop() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::{NativeAccess, NativeContextSession, Request};
    let mut c = Case::new(None).await;
    let context = c.context();
    let user = protocol::current_user::user(c.service.client.configuration().tenant);
    let reply = c
        .policy_call(Request::BindContext {
            value: NativeContextSession {
                access: NativeAccess::User {
                    credential: zeroize::Zeroizing::new("c".repeat(64)),
                    user,
                },
                generation: context.generation.clone(),
                expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 100,
            },
            expected: Some(context.id),
        })
        .await;
    assert!(matches!(reply, Reply::ContextBound { .. }));
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    c._server.data.lock().unwrap().current_user.lose_reply = true;
    c.service.prune_credentials().unwrap();
    c.service
        .synchronize_current_user(&mut commands)
        .await
        .unwrap();
    assert_eq!(
        c.service.client.current_user_status().unwrap().phase,
        execution_contract::CurrentUserSyncPhase::CleanupPending
    );
    c.service
        .synchronize_current_user(&mut commands)
        .await
        .unwrap();
    assert_eq!(
        c.service.client.current_user_status().unwrap().phase,
        execution_contract::CurrentUserSyncPhase::Unbound
    );
    let data = c._server.data.lock().unwrap();
    let inputs = &data.current_user.inputs;
    assert_eq!(inputs[inputs.len() - 1], inputs[inputs.len() - 2]);
    assert!(data.current_user.user.is_none());
    assert!(data.current_user.owner.is_some());
}

#[tokio::test]
async fn expired_other_peer_with_the_same_generation_cannot_retire_latest_owner() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::{NativeAccess, NativeContextSession, Request};
    let mut c = Case::new(None).await;
    let context = c.context();
    let reply = c
        .policy_call(Request::BindContext {
            value: NativeContextSession {
                access: NativeAccess::User {
                    credential: zeroize::Zeroizing::new("c".repeat(64)),
                    user: protocol::current_user::user(c.service.client.configuration().tenant),
                },
                generation: context.generation.clone(),
                expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 100,
            },
            expected: Some(context.id),
        })
        .await;
    assert!(matches!(reply, Reply::ContextBound { .. }));
    let original_binding = c.selection.binding.clone();
    c.selection.binding = plan::id("another-authenticated-login").unwrap();
    c.employee("initial").await;
    // A lost-ACK retry from the older peer is not a new user selection.
    c.selection.binding = original_binding;
    let retry = c
        .policy_call(Request::BindContext {
            value: NativeContextSession {
                access: NativeAccess::User {
                    credential: zeroize::Zeroizing::new("c".repeat(64)),
                    user: protocol::current_user::user(c.service.client.configuration().tenant),
                },
                generation: context.generation,
                expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 100,
            },
            expected: None,
        })
        .await;
    assert!(matches!(retry, Reply::ContextBound { .. }));
    let revision = c._server.data.lock().unwrap().current_user.revision;
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
    c.service
        .synchronize_current_user(&mut commands)
        .await
        .unwrap();
    assert_eq!(
        c.service.client.current_user_status().unwrap().phase,
        execution_contract::CurrentUserSyncPhase::Bound
    );
    assert_eq!(
        c._server.data.lock().unwrap().current_user.revision,
        revision
    );
}

#[tokio::test]
async fn service_restart_retires_confirmed_relation_and_replays_cleanup_but_preserves_unknown_put()
{
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::{NativeAccess, NativeContextSession, Request};
    for unknown_put in [false, true] {
        let mut c = Case::new(None).await;
        if unknown_put {
            c.policy_call(Request::RevokeContext {}).await;
            let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
            c.service
                .synchronize_current_user(&mut commands)
                .await
                .unwrap();
            c._server.data.lock().unwrap().current_user.lose_reply = true;
            let reply = c
                .policy_call(Request::BindContext {
                    value: NativeContextSession {
                        access: NativeAccess::User {
                            credential: zeroize::Zeroizing::new("c".repeat(64)),
                            user: protocol::current_user::user(
                                c.service.client.configuration().tenant,
                            ),
                        },
                        generation: plan::id("unconfirmed-login").unwrap(),
                        expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 60_000,
                    },
                    expected: None,
                })
                .await;
            assert!(matches!(reply, Reply::ContextBound { .. }));
            c.service
                .synchronize_current_user(&mut commands)
                .await
                .unwrap();
            assert_eq!(
                c.service.client.current_user_status().unwrap().phase,
                execution_contract::CurrentUserSyncPhase::BindingUnknown
            );
        }
        let config = c.service.core.config.clone();
        let helpers = c.service.core.helpers.clone();
        let Case {
            _server: server,
            _root: root,
            service,
            ..
        } = c;
        drop(service);
        let assemble = || {
            let clock = SystemClock::new().unwrap();
            let client = Client::open(
                &root.path,
                server.config(),
                OpenMode::Existing,
                server.secrets.clone(),
                clock.clone(),
            )
            .unwrap();
            DeviceService::assemble(
                client,
                &root.path.join("execution.sqlite"),
                ProductionStartup::Open,
                config.clone(),
                clock,
                helpers.clone(),
                execution_runner::staging::MaterialRoot::open_fixture,
                uuid::Uuid::from_u128(2909),
            )
            .unwrap()
        };
        let mut service = assemble();
        assert!(service.core.contexts.is_empty());
        let before = server.data.lock().unwrap().current_user.inputs.len();
        let (_sender, mut commands) = tokio::sync::mpsc::channel(1);
        if unknown_put {
            service
                .synchronize_current_user(&mut commands)
                .await
                .unwrap();
            assert_eq!(
                service.client.current_user_status().unwrap().phase,
                execution_contract::CurrentUserSyncPhase::BindingUnknown
            );
            assert_eq!(
                server.data.lock().unwrap().current_user.inputs.len(),
                before
            );
            continue;
        }
        server.data.lock().unwrap().current_user.lose_reply = true;
        service
            .synchronize_current_user(&mut commands)
            .await
            .unwrap();
        assert_eq!(
            service.client.current_user_status().unwrap().phase,
            execution_contract::CurrentUserSyncPhase::CleanupPending
        );
        drop(service);
        let mut service = assemble();
        service
            .synchronize_current_user(&mut commands)
            .await
            .unwrap();
        assert_eq!(
            service.client.current_user_status().unwrap().phase,
            execution_contract::CurrentUserSyncPhase::Unbound
        );
        let data = server.data.lock().unwrap();
        assert_eq!(data.current_user.inputs.len(), before + 2);
        assert_eq!(
            data.current_user.inputs[before],
            data.current_user.inputs[before + 1]
        );
        assert!(data.current_user.user.is_none());
        assert!(data.current_user.owner.is_some());
    }
}

#[tokio::test]
async fn local_data_queries_keep_os_prerequisites_exact_authority_and_offline_read_fences() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::{NativeAccess, NativeContextSession, Request};
    let mut c = Case::new(None).await;
    c.service.client.drive_pki().await.unwrap();
    c.service.update_identity_gate();
    assert!(c.service.core.host.identity_ready());
    let context = c.context();
    let origin = ClientOrigin::Desktop {};
    let directory = c
        .policy_call(Request::Data {
            context: context.clone(),
            origin: origin.clone(),
            input: DataOperation::Directory(QueryDirectory {
                after: None,
                limit: 128,
                policy_page: None,
            }),
        })
        .await;
    let Reply::Data {
        value: DataResponse::Directory(page),
    } = directory
    else {
        panic!("local directory")
    };
    let query = page
        .items
        .into_iter()
        .find(|q| q.title.contains("Memory"))
        .unwrap()
        .reference;
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let submit = QuerySubmit {
        request: request.clone(),
        query: query.clone(),
        parameters: Default::default(),
    };
    let call = || Request::Data {
        context: context.clone(),
        origin: origin.clone(),
        input: DataOperation::Submit(submit.clone()),
    };
    let first = c.policy_call(call()).await;
    if execution_ipc::host::current_subject().unwrap() == "0"
        && installation_security::protected(&std::env::current_exe().unwrap()).is_ok()
    {
        assert!(matches!(
            first,
            Reply::Data {
                value: DataResponse::Status(_)
            }
        ));
        c.policy_call(call()).await;
        assert_eq!(
            c.service
                .core
                .app
                .status(&c.service.core.local_caller(), &request)
                .unwrap()
                .attempts,
            1
        );
    } else {
        // The developer's non-System process must never weaken production material/run-as checks.
        assert!(matches!(
            first,
            Reply::DataFailure {
                reason: DataError::Denied
            }
        ));
        assert!(!c
            .service
            .core
            .app
            .has_service_execution(&request, &c.service.core.host.root.device)
            .unwrap());
    }
    // Opaque, non-dispatchable fixture input tests the product grant and revoke fence;
    // it does not pretend this developer-owned executable satisfies System staging.
    let now = c.service.core.host.clock.millis().unwrap();
    let artifact = ExactArtifactRef {
        resource: plan::reference("proof-only", "1").unwrap(),
        sha256: Digest::new("a".repeat(64)).unwrap(),
    };
    let frozen = FrozenExecution::freeze(
        ExecutionInput {
            schema_version: V1,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: request.clone(),
                authority: c.service.core.host.root.authority.clone(),
                actor: c.service.core.host.local_actor.clone(),
                initiator: Initiator::Human {
                    os_session: OsSessionRef {
                        device: c.service.core.host.root.device.clone(),
                        account: OsAccountRef {
                            platform: plan::platform().unwrap(),
                            subject: plan::id(&c.selection.subject).unwrap(),
                        },
                        session: c.selection.binding.clone(),
                    },
                },
                delegation: None,
                target: Target {
                    device: c.service.core.host.root.device.clone(),
                    platform: plan::platform().unwrap(),
                    scope: TargetScope::Device {},
                },
                operation: Operation {
                    action: plan::id("local-query").unwrap(),
                    resource: plan::reference(query.id.as_str(), query.revision.as_str()).unwrap(),
                },
                parameters: Default::default(),
            },
            launch: LaunchSpec {
                artifact: artifact.clone(),
                interpreter: InterpreterRef {
                    artifact,
                    profile: plan::reference("native-collection-worker", "1").unwrap(),
                },
                argv: vec![LaunchArg::ArtifactPath {}],
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output: OutputSpec {
                    format: OutputFormat::Json { max_rows: 1 },
                    stdout: TextEncoding::Utf8,
                    stderr: TextEncoding::Utf8,
                },
                cwd: c._root.path.to_str().unwrap().into(),
                env: Default::default(),
            },
            run_as: RunAs::System {
                platform: plan::platform().unwrap(),
            },
            session_requirement: SessionRequirement::NotRequired {},
            constraints: IsolationPolicy::OsIdentity {},
            budget: ExecutionBudget {
                total_timeout_ms: 60000,
                total_output_bytes: 65536,
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: now,
                expires_at_unix_ms: now + 90000,
            },
            policy: plan::reference("local-native-query", "1").unwrap(),
        },
        &plan::storage_limits().input,
    )
    .unwrap();
    *c.service.core.host.local_read.lock().unwrap() =
        Some(crate::local_execution::ReadPermit::new(frozen.clone()).unwrap());
    let attempt = AttemptId::new("proof-only-no-dispatch").unwrap();
    let facts =
        execution_admission::AuthorityVerifier::verify(&c.service.core.host, &frozen, &attempt)
            .unwrap();
    assert!(matches!(facts.verified_origin, Initiator::Human { .. }));
    assert_eq!(facts.risk, Some(RiskLevel::One));
    *c.service.core.host.local_read.lock().unwrap() = None;
    assert!(execution_admission::AuthorityVerifier::verify(
        &c.service.core.host,
        &frozen,
        &attempt
    )
    .is_err());
    let old_epoch = c.service.core.fences.current(&c.scope()).unwrap();
    c.policy_call(Request::RevokeContext {}).await;
    let (reply_tx, _) = tokio::sync::oneshot::channel();
    let stale = Command {
        request: LocalRequest::operation(Request::ServiceStatus {}),
        fence_epoch: old_epoch,
        subject: c.selection.subject.clone(),
        session: c.selection.session,
        binding: Some(c.selection.binding.clone()),
        reply: reply_tx,
    };
    assert!(c.service.core.submit_local(&stale, &frozen).is_err());
    assert!(c.service.core.host.local_read.lock().unwrap().is_none());
    assert!(matches!(
        c.policy_call(Request::Data {
            context: context.clone(),
            origin: origin.clone(),
            input: DataOperation::Directory(QueryDirectory {
                after: None,
                limit: 16,
                policy_page: None
            })
        })
        .await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
    c.service
        .core
        .host
        .revoked
        .store(true, std::sync::atomic::Ordering::Release);
    let Reply::ContextBound { value: offline } = c
        .policy_call(Request::BindContext {
            expected: None,
            value: NativeContextSession {
                access: NativeAccess::Device {},
                generation: plan::id("offline-generation").unwrap(),
                expires_at_unix_ms: c.service.core.host.clock.millis().unwrap() + 60_000,
            },
        })
        .await
    else {
        panic!("offline OS binding")
    };
    // A queued backend operation must not delay a purely local read.
    let (queued_reply, _queued_receive) = tokio::sync::oneshot::channel();
    c.service.core.enqueue_policy(Command {
        fence_epoch: c.service.core.fences.current(&c.scope()).unwrap(),
        request: LocalRequest::operation(Request::PolicyDirectory {
            context: offline.clone(),
            origin: origin.clone(),
            after: None,
        }),
        subject: c.selection.subject.clone(),
        session: c.selection.session,
        binding: Some(c.selection.binding.clone()),
        reply: queued_reply,
    });
    let queued = c.service.core.policy_commands.len();
    let (reply, mut receive) = tokio::sync::oneshot::channel();
    c.service.core.handle(Command {
        fence_epoch: c.service.core.fences.current(&c.scope()).unwrap(),
        request: LocalRequest::operation(Request::Data {
            context: offline,
            origin,
            input: DataOperation::Read(DataRead {
                selection: DataSelection::Query { query },
                after: None,
                limit: 16,
                max_age_seconds: None,
            }),
        }),
        subject: c.selection.subject.clone(),
        session: c.selection.session,
        binding: Some(c.selection.binding.clone()),
        reply,
    });
    assert!(matches!(
        receive.try_recv().unwrap(),
        Reply::Data {
            value: DataResponse::Read(_)
        }
    ));
    assert_eq!(c.service.core.policy_commands.len(), queued);
}

#[tokio::test]
async fn late_policy_page_is_revalidated_for_invoke_draft_and_confirmation_without_scanning() {
    let _budget = crate::scenario_budget::enter();
    use execution_ipc::host::Request;
    for draft in [false, true] {
        let mut c = Case::new(None).await;
        let directory = protocol::policy_directory_fixture(c.service.client.configuration().tenant);
        let cursor = uuid::Uuid::from_u128(18);
        {
            let mut data = c._server.data.lock().unwrap();
            for page in 0..18 {
                data.self_service_pages.insert(
                    if page == 0 {
                        None
                    } else {
                        Some(uuid::Uuid::from_u128(page))
                    },
                    wire::SelfServiceDirectoryResponse {
                        items: vec![],
                        next_cursor: Some(uuid::Uuid::from_u128(page + 1)),
                        as_of: c._server.time.now().unwrap(),
                    },
                );
            }
            data.self_service_pages.insert(
                Some(cursor),
                wire::SelfServiceDirectoryResponse {
                    items: vec![directory.clone()],
                    next_cursor: None,
                    as_of: c._server.time.now().unwrap(),
                },
            );
        }
        c.employee("late-page").await;
        let choice = c.policy_choice(&directory);
        let locator = execution_contract::PolicyLocator {
            after: Some(plan::id(cursor.to_string()).unwrap()),
        };
        let request = if draft {
            Request::PreparePolicy {
                context: c.context(),
                choice: choice.clone(),
                locator: locator.clone(),
                origin: c.selection.origin.clone(),
            }
        } else {
            Request::InvokePolicy {
                context: c.context(),
                choice: choice.clone(),
                locator: locator.clone(),
                origin: c.selection.origin.clone(),
            }
        };
        let offset = c._server.data.lock().unwrap().employee_calls.len();
        let Reply::PolicyRequest { value } = c.policy_call(request).await else {
            panic!("late-page choice")
        };
        assert_eq!(
            value.state,
            if draft {
                BackendRequestState::Draft
            } else {
                BackendRequestState::AwaitingConfirmation
            }
        );
        assert!(c._server.data.lock().unwrap().invocations.is_empty());
        let calls = c._server.data.lock().unwrap().employee_calls[offset..].to_vec();
        assert_eq!(calls.len(), 2);
        assert!(calls
            .iter()
            .all(|(url, _)| url.contains(&format!("after={cursor}"))));
        if draft {
            c.policy_call(Request::SubmitPolicy {
                context: c.context(),
                input: execution_contract::PolicySubmission {
                    request: choice.request.clone(),
                    revision: value.revision,
                    parameters: choice.parameters.clone(),
                },
            })
            .await;
        } else {
            c.policy_call(Request::ConfirmPolicy {
                context: c.context(),
                request: choice.request.clone(),
            })
            .await;
        }
        assert!(c
            ._server
            .data
            .lock()
            .unwrap()
            .invocations
            .contains_key(&uuid::Uuid::parse_str(choice.request.as_str()).unwrap()));
    }
}

/// Explicit policy authority fixture with real service/SQLite; native detector is read-only.
/// This never establishes System privilege, a real install/uninstall or Windows acceptance.
#[cfg(target_os = "macos")]
async fn local_software_case() -> (Case, LocalSoftwareChoice) {
    use crate::local_permissions::Restrictions;
    use execution_ipc::host::Request;
    let mut c = Case::new(None).await;
    let context = c.context();
    let Reply::Data {
        value: DataResponse::Directory(directory),
    } = c
        .policy_call(Request::Data {
            context: context.clone(),
            origin: ClientOrigin::Desktop {},
            input: DataOperation::Directory(QueryDirectory {
                after: None,
                limit: 128,
                policy_page: None,
            }),
        })
        .await
    else {
        panic!("directory");
    };
    let query = directory
        .items
        .into_iter()
        .find(|d| d.domains.contains(&"software.receipts".into()))
        .unwrap()
        .reference;
    let receipt = format!("org.rss.fixture.never-installed.{}", uuid::Uuid::new_v4());
    c.service.core.data.snapshot(&CollectedData{
        query:query.clone(),scope:DataScope::Device,request:None,collected_at:Some(c.service.core.host.clock.millis().unwrap()/1000),quality:DataQuality::Complete,enumeration_complete:true,mapping_version:Some(1),fields:Default::default(),
        rows:vec![serde_json::json!({"id":"fixture-receipt","native_id":receipt,"name":"Explicit receipt fixture","version":"1.0.0","scope":"device","owner":"","origin":"pkg_receipt"})],stale:false,next:None,
    }).unwrap();
    let (reply, receive) = tokio::sync::oneshot::channel();
    c.service.core.handle(Command {
        fence_epoch: c.service.core.fences.current(&c.scope()).unwrap(),
        subject: c.selection.subject.clone(),
        session: c.selection.session,
        binding: Some(c.selection.binding.clone()),
        reply,
        request: LocalRequest::operation(Request::SoftwareRead {
            context: context.clone(),
            origin: ClientOrigin::Desktop {},
            input: SoftwareRead {
                read: DataRead {
                    selection: DataSelection::Query { query },
                    after: None,
                    limit: 16,
                    max_age_seconds: Some(300),
                },
            },
        }),
    });
    let reply = receive.await.unwrap();
    let page = match reply {
        Reply::SoftwarePage { value } => value,
        Reply::DataFailure { reason } => panic!("software facts unavailable: {reason:?}"),
        _ => panic!("unexpected software facts reply"),
    };
    let item = page.items.into_iter().next().unwrap();
    // This fixture supplies only a native identity, never executable content.
    c.service.core.local_target_fixture = Some(|item| crate::local_software::native::Target::App {
        path: std::path::PathBuf::from("/Applications/ExplicitFixture.app"),
        device: 1,
        inode: 1,
        bundle: item.native_id.clone(),
        version: item.version.clone().unwrap(),
        team: "FIXTURE".into(),
    });
    let rules = Restrictions::installation_default();
    let path = c._root.path.join("explicit-fixture-policy.json");
    std::fs::write(&path, serde_json::to_vec(&rules).unwrap()).unwrap();
    c.service.core.config.local_permissions.policy_file = path;
    c.service.core.local_policy_fixture = Some(crate::local_permissions::Policy::fixture);
    c.service.core.config.worker_image_fixture = Some(fixture_worker_image);
    let choice = LocalSoftwareChoice {
        request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        software: item.reference,
        action: LocalSoftwareAction::Detect,
    };
    (c, choice)
}
#[cfg(target_os = "macos")]
impl Case {
    async fn local_software_call(&mut self, input: LocalSoftwareOperation, ai: bool) -> Reply {
        self.policy_call(execution_ipc::host::Request::LocalSoftware {
            context: self.context(),
            origin: if ai {
                self.selection.origin.clone()
            } else {
                ClientOrigin::Desktop {}
            },
            input,
        })
        .await
    }
}
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires root and a protected temporary directory for the real runner/journal seam"]
async fn successful_local_software_assessment_confirms_receipts_and_releases_materials() {
    use std::os::unix::fs::PermissionsExt;
    let _budget = crate::scenario_budget::enter();
    fn image(config: &ExecutionConfig) -> Result<SoftwareMaterial, agent_client::Error> {
        plan::pin(&config.work_root.join("software-result-worker"))
    }
    for action in [LocalSoftwareAction::Detect, LocalSoftwareAction::Uninstall] {
        let (mut c, mut choice) = local_software_case().await;
        choice.action = action;
        // Explicit physical worker fixture: only reports a typed result, never changes software.
        let result = serde_json::to_string(&SoftwareWorkerResult {
            native_output_bytes: 0,
            detected: Some(SoftwareState::Absent {}),
            closed: true,
            diagnostics: String::new(),
        })
        .unwrap();
        let worker = c
            .service
            .core
            .config
            .work_root
            .join("software-result-worker");
        std::fs::write(&worker, format!("#!/bin/sh\nprintf '%s\\n' '{result}'\n")).unwrap();
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o700)).unwrap();
        c.service.core.config.worker_image_fixture = Some(image);
        let prepared = match c
            .local_software_call(LocalSoftwareOperation::Prepare(choice.clone()), false)
            .await
        {
            Reply::SoftwarePrepared { value } => value,
            Reply::DataFailure { reason } => panic!("prepare: {reason:?}"),
            _ => panic!("unexpected prepare reply"),
        };
        assert!(matches!(
            c.local_software_call(
                LocalSoftwareOperation::Submit(LocalSoftwareSubmission {
                    request: choice.request.clone(),
                    digest: prepared.digest,
                }),
                false,
            )
            .await,
            Reply::SoftwareStatus { .. }
        ));
        let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                c.service.core.reconcile().unwrap();
                let status = c
                    .service
                    .core
                    .app
                    .status(&c.service.core.local_caller(), &choice.request)
                    .unwrap();
                if status.phase == execution_app::TaskPhase::Verified {
                    break status;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("a successful native result must settle the original request");
        assert_eq!(status.attempts, 1);
        assert_eq!(
            status.assessment,
            Some(if action == LocalSoftwareAction::Uninstall {
                execution_lifecycle::EffectAssessment::Satisfied
            } else {
                execution_lifecycle::EffectAssessment::NoEffect
            })
        );
        let consumer = plan::id(RESULT_CONSUMER).unwrap();
        c.service.core.finish_local_result(&choice.request).unwrap();
        assert!(c
            .service
            .core
            .app
            .service_delivery(&choice.request, &consumer, 32)
            .unwrap()
            .is_empty());
        assert!(
            c.service
                .core
                .app
                .service_material_fact(&choice.request, &consumer)
                .unwrap()
                .reclaimable
        );
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires root for production material/OS identity admission; explicitly run the built test binary"]
async fn local_software_api_stages_no_attempt_and_replays_the_original_after_lost_ack() {
    let _budget = crate::scenario_budget::enter();
    let (mut c, choice) = local_software_case().await;
    let prepared = match c
        .local_software_call(LocalSoftwareOperation::Prepare(choice.clone()), true)
        .await
    {
        Reply::SoftwarePrepared { value } => value,
        Reply::DataFailure { reason } => panic!("prepare: {reason:?}"),
        _ => panic!("unexpected prepare reply"),
    };
    assert_eq!(
        c.service
            .core
            .app
            .status(&c.service.core.local_caller(), &choice.request)
            .unwrap()
            .attempts,
        0
    );
    let original = c
        .service
        .core
        .app
        .frozen_input(&c.service.core.local_caller(), &choice.request)
        .unwrap();
    assert!(!c.service.core.host.materials.registered(&original).unwrap());
    // Discarded acknowledgement, repeated prepare and explicit status reuse the retained UUID/digest.
    let Reply::SoftwarePrepared { value: replay } = c
        .local_software_call(LocalSoftwareOperation::Prepare(choice.clone()), true)
        .await
    else {
        panic!("replay");
    };
    assert_eq!(prepared.digest, replay.digest);
    let submit = LocalSoftwareSubmission {
        request: choice.request.clone(),
        digest: prepared.digest.clone(),
    };
    assert!(matches!(
        c.local_software_call(LocalSoftwareOperation::Submit(submit.clone()), true)
            .await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
    let mut wrong = submit.clone();
    wrong.digest = Digest::new("9".repeat(64)).unwrap();
    assert!(matches!(
        c.local_software_call(LocalSoftwareOperation::Submit(wrong), false)
            .await,
        Reply::DataFailure {
            reason: DataError::Conflict
        }
    ));
    let Reply::SoftwareStatus { value: started } = c
        .local_software_call(LocalSoftwareOperation::Submit(submit.clone()), false)
        .await
    else {
        panic!("submit");
    };
    assert_eq!(started.execution.attempts, 1);
    assert!(matches!(started.prepared.initiator, Initiator::Ai { .. }));
    let Reply::SoftwareStatus { value: again } = c
        .local_software_call(LocalSoftwareOperation::Submit(submit), false)
        .await
    else {
        panic!("submit replay");
    };
    assert_eq!(again.execution.attempts, 1);
    assert_eq!(again.prepared.digest, prepared.digest);
    let Reply::SoftwareHistory { value: history } = c
        .local_software_call(
            LocalSoftwareOperation::History(SoftwareHistoryQuery {
                after: None,
                limit: 64,
            }),
            false,
        )
        .await
    else {
        panic!("history");
    };
    assert!(history
        .items
        .iter()
        .any(|i| i.prepared.request == choice.request));
    let Reply::SoftwareStatus { value: cancel } = c
        .local_software_call(
            LocalSoftwareOperation::Cancel(QueryRequest {
                request: choice.request.clone(),
            }),
            false,
        )
        .await
    else {
        panic!("cancel");
    };
    assert!(cancel.execution.cancel_requested);
    assert_eq!(cancel.execution.attempts, 1);
    // A retired context cannot use a retained intent to read or cancel a request.
    let context = c.context();
    c.policy_call(execution_ipc::host::Request::RevokeContext {})
        .await;
    assert!(matches!(
        c.policy_call(execution_ipc::host::Request::LocalSoftware {
            context,
            origin: ClientOrigin::Desktop {},
            input: LocalSoftwareOperation::Status(QueryRequest {
                request: choice.request
            })
        })
        .await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
    c.employee("fresh-software-history-context").await;
    assert!(
        matches!(c.local_software_call(LocalSoftwareOperation::History(SoftwareHistoryQuery{after:None,limit:64}),false).await,
        Reply::SoftwareHistory{value} if value.items.is_empty())
    );
}
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires root for production material/OS identity admission; explicitly run the built test binary"]
async fn local_software_api_rechecks_rules_and_snapshot_before_any_attempt() {
    let _budget = crate::scenario_budget::enter();
    for changed_rules in [false, true] {
        let (mut c, choice) = local_software_case().await;
        let prepared = match c
            .local_software_call(LocalSoftwareOperation::Prepare(choice.clone()), false)
            .await
        {
            Reply::SoftwarePrepared { value } => value,
            Reply::DataFailure { reason } => panic!("prepare: {reason:?}"),
            _ => panic!("unexpected prepare reply"),
        };
        if changed_rules {
            let path = &c.service.core.config.local_permissions.policy_file;
            let mut rules: crate::local_permissions::Restrictions =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            rules.allow_launch = false;
            std::fs::write(path, serde_json::to_vec(&rules).unwrap()).unwrap();
        } else {
            // Changing the immutable observation selected by the original cursor is not a new grant.
            let read = DataRead {
                selection: DataSelection::Query {
                    query: choice.software.query.clone(),
                },
                after: None,
                limit: 16,
                max_age_seconds: None,
            };
            let mut value = c
                .service
                .core
                .data
                .read(
                    &read,
                    &c.selection.subject,
                    &c.selection.binding,
                    c.service.core.host.clock.millis().unwrap() / 1000,
                )
                .unwrap();
            value.rows[0]["version"] = serde_json::json!("2.0.0");
            c.service.core.data.snapshot(&value).unwrap();
        }
        let reply = c
            .local_software_call(
                LocalSoftwareOperation::Submit(LocalSoftwareSubmission {
                    request: choice.request.clone(),
                    digest: prepared.digest,
                }),
                false,
            )
            .await;
        assert!(matches!(
            reply,
            Reply::DataFailure {
                reason: DataError::Denied | DataError::Conflict
            }
        ));
        assert_eq!(
            c.service
                .core
                .app
                .status(&c.service.core.local_caller(), &choice.request)
                .unwrap()
                .attempts,
            0
        );
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires root for production material/OS identity admission; explicitly run the built test binary"]
async fn installation_owner_exists_before_registration_and_preserves_both_scopes_after_reopen() {
    use execution_admission::AuthorityVerifier;
    use execution_app::AppHost;
    let _budget = crate::scenario_budget::enter();
    let server = protocol::Server::new().await;
    server.data.lock().unwrap().explicit_offers = true;
    let root = protocol::Root::new();
    let clock = SystemClock::new().unwrap();
    server.time.set(clock.now().unwrap());
    let installation = uuid::Uuid::new_v4();
    let cfg = server.config();
    let image = installation_security::Artifact {
        path: "/bin/sh".into(),
        sha256: format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())),
        cdhash: None,
    };
    let config = ExecutionConfig {
        #[cfg(test)]
        worker_image_fixture: Some(fixture_worker_image),
        local_permissions: crate::local_permissions::Config {
            policy_file: root.path.join("local-permissions.json"),
        },
        work_root: root.path.clone(),
        material_root: root.path.join("materials"),
        interpreters: vec![],
        managers: vec![],
        processes: 2,
    };
    platform_private_storage::directory(&config.material_root).unwrap();
    let helpers = UserResources {
        image,
        work_roots: Default::default(),
    };
    let journal = root.path.join("execution.sqlite");
    let client = Client::open(
        &root.path,
        cfg.clone(),
        OpenMode::Create,
        server.secrets.clone(),
        clock.clone(),
    )
    .unwrap();
    let mut service = DeviceService::assemble(
        client,
        &journal,
        ProductionStartup::Create,
        config.clone(),
        clock.clone(),
        helpers.clone(),
        execution_runner::staging::MaterialRoot::open_fixture,
        installation,
    )
    .unwrap();
    assert!(service.client.registration().is_err());
    let root_binding = service.core.host.service_binding().unwrap();
    assert!(matches!(root_binding.authority, Authority::Local { .. }));
    let definition = service
        .core
        .data
        .directory("0")
        .unwrap()
        .into_iter()
        .find(|d| d.domains.contains(&"hardware.memory".into()))
        .unwrap();
    let mut read = service
        .core
        .data
        .local_definition(&definition.reference, "0")
        .unwrap()
        .read;
    let now = clock.millis().unwrap();
    read.deadline = (now / 1000 + 90) as i64;
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let initiator = Initiator::Human {
        os_session: OsSessionRef {
            device: root_binding.device.clone(),
            account: OsAccountRef {
                platform: Platform::Macos,
                subject: plan::id("501").unwrap(),
            },
            session: plan::id("fixture-login").unwrap(),
        },
    };
    let (local, materials) = backend::collection::compile_read(
        &read,
        request.clone(),
        initiator,
        Operation {
            action: plan::id("local-query").unwrap(),
            resource: plan::reference("native-read", "1").unwrap(),
        },
        plan::reference("local-native-query", "1").unwrap(),
        (now / 1000 + 30) as i64,
        &root_binding,
        &service.core.host.local_actor,
        (&service.core.config, &mut service.core.material_root),
        None,
    )
    .unwrap();
    service
        .core
        .host
        .materials
        .register(&local, materials)
        .unwrap();
    service
        .core
        .app
        .stage_execution(&service.core.local_caller(), &local)
        .unwrap();
    *service.core.host.local_read.lock().unwrap() =
        Some(crate::local_execution::ReadPermit::new(local.clone()).unwrap());
    let attempt = AttemptId::new("scope-proof-attempt").unwrap();
    assert_eq!(
        service.core.host.verify(&local, &attempt).unwrap().risk,
        Some(RiskLevel::One)
    );
    *service.core.host.local_read.lock().unwrap() = None;
    assert!(service.core.host.verify(&local, &attempt).is_err());
    let capabilities = vec![
        wire::Capability::InventoryCollectionV1,
        wire::Capability::TaskExecuteV1,
    ];
    service
        .client
        .prepare_authorization(capabilities.clone())
        .unwrap();
    let (_sender, mut commands) = tokio::sync::mpsc::channel(8);
    server.data.lock().unwrap().registration_failure = true;
    assert!(service.drive(&mut commands).await.is_err());
    assert!(service.client.has_pending_enrollment().unwrap());
    assert_eq!(service.core.host.service_binding().unwrap(), root_binding);
    server.data.lock().unwrap().registration_failure = false;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while service.client.registration().is_err() {
            service.next_enrollment = std::time::Instant::now();
            service.drive(&mut commands).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the production enrollment retry must eventually register");
    assert_eq!(
        service.client.registration().unwrap().capabilities,
        capabilities
    );
    service.client.drive_task_trust().await.unwrap();
    service.activate_enterprise().unwrap();
    assert_eq!(service.core.host.service_binding().unwrap(), root_binding);
    let mut remote = local.spec().clone();
    remote.request.request_id = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    remote.request.authority = service.core.host.binding.authority.clone();
    remote.request.actor = service.core.host.actor.clone();
    remote.request.target.device = service.core.host.binding.device.clone();
    let remote = FrozenExecution::freeze(remote, &plan::storage_limits().input).unwrap();
    let materials = crate::recovery::materials(&remote, &config, &helpers).unwrap();
    service
        .core
        .host
        .materials
        .register(&remote, materials)
        .unwrap();
    service
        .core
        .app
        .stage_execution(&service.core.caller(), &remote)
        .unwrap();
    assert!(
        service.core.host.verify(&remote, &attempt).is_err(),
        "a namespace alone cannot replace a signed enterprise Start"
    );
    assert!(service
        .core
        .app
        .frozen_input(&service.core.caller(), &request)
        .is_err());
    assert!(service
        .core
        .app
        .frozen_input(
            &service.core.local_caller(),
            &remote.spec().request.request_id
        )
        .is_err());
    assert_eq!(
        service
            .core
            .app
            .service_tasks(None, 128)
            .unwrap()
            .items
            .len(),
        2
    );
    let mut collision = remote.spec().clone();
    collision.request.request_id = request.clone();
    let collision = FrozenExecution::freeze(collision, &plan::storage_limits().input).unwrap();
    let materials = crate::recovery::materials(&collision, &config, &helpers).unwrap();
    service
        .core
        .host
        .materials
        .register(&collision, materials)
        .unwrap();
    assert!(matches!(
        service
            .core
            .app
            .stage_execution(&service.core.caller(), &collision),
        Err(execution_app::Error::Conflict)
    ));
    // Preparation also reserves the global UUID, before an execution exists.
    let prepared_id = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let record = BackendRequest {
        action: PreparedAction::Offered {
            offer: BackendTask {
                request: prepared_id.clone(),
                task: plan::id("remote-task").unwrap(),
                attempt: plan::id("remote-attempt").unwrap(),
                revision: Digest::new("a".repeat(64)).unwrap(),
                presentation: BackendPresentation {
                    title: "Remote task".into(),
                    summary: BackendTaskSummary::Software {
                        intent: SoftwareOperation::Uninstall,
                        steps: vec![BackendStepSummary {
                            package: "fixed".into(),
                            version: "1".into(),
                            identity: BackendIdentity::User,
                        }],
                    },
                    extra: Default::default(),
                },
                expires_at: (now / 1000 + 60) as i64,
                user_initiated: true,
            },
        },
        trigger: BackendTrigger::Human {
            os_session: match &local.spec().request.initiator {
                Initiator::Human { os_session } => os_session.clone(),
                _ => unreachable!(),
            },
        },
        risk: None,
        confirmation: None,
        revision: 1,
        state: BackendRequestState::Ready,
        failure: None,
    };
    service
        .core
        .app
        .record_backend_request(&service.core.caller(), None, &record)
        .unwrap();
    let mut collision = local.spec().clone();
    collision.request.request_id = prepared_id;
    let collision = FrozenExecution::freeze(collision, &plan::storage_limits().input).unwrap();
    let materials = crate::recovery::materials(&collision, &config, &helpers).unwrap();
    service
        .core
        .host
        .materials
        .register(&collision, materials)
        .unwrap();
    assert!(matches!(
        service
            .core
            .app
            .stage_execution(&service.core.local_caller(), &collision),
        Err(execution_app::Error::Conflict)
    ));
    let mut reverse = record.clone();
    let PreparedAction::Offered { offer } = &mut reverse.action else {
        unreachable!()
    };
    offer.request = request.clone();
    assert!(matches!(
        service
            .core
            .app
            .record_backend_request(&service.core.caller(), None, &reverse),
        Err(execution_app::Error::Conflict)
    ));
    service
        .core
        .app
        .cancel(&service.core.local_caller(), &request)
        .unwrap();
    service.core.finish_local_result(&request).unwrap();
    let consumer = plan::id(crate::service::RESULT_CONSUMER).unwrap();
    assert!(
        service
            .core
            .app
            .service_material_fact(&request, &consumer)
            .unwrap()
            .reclaimable
    );
    assert!(
        !service
            .core
            .app
            .service_material_fact(&remote.spec().request.request_id, &consumer)
            .unwrap()
            .reclaimable
    );
    drop(service);
    let client = Client::open(
        &root.path,
        cfg,
        OpenMode::Existing,
        server.secrets.clone(),
        clock.clone(),
    )
    .unwrap();
    let service = DeviceService::assemble(
        client,
        &journal,
        ProductionStartup::Open,
        config,
        clock,
        helpers,
        execution_runner::staging::MaterialRoot::open_fixture,
        installation,
    )
    .unwrap();
    assert_eq!(service.core.host.service_binding().unwrap(), root_binding);
    assert_eq!(
        service
            .core
            .app
            .service_tasks(None, 128)
            .unwrap()
            .items
            .len(),
        2
    );
    assert_eq!(
        service
            .core
            .app
            .frozen_input(&service.core.local_caller(), &request)
            .unwrap()
            .digest(),
        local.digest()
    );
}

#[tokio::test]
#[ignore = "requires root for production material/OS identity admission; explicitly run the built test binary"]
async fn settings_desktop_cache_and_retained_task_are_bound_to_exact_context() {
    use execution_ipc::host::Request;
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    c.employee("settings-owner").await;
    c.service.core.config.worker_image_fixture = Some(fixture_worker_image);
    let permissions = c._root.path.join("settings-permissions.json");
    std::fs::write(
        &permissions,
        serde_json::to_vec(&crate::local_permissions::Restrictions::installation_default())
            .unwrap(),
    )
    .unwrap();
    c.service.core.config.local_permissions.policy_file = permissions;
    c.service.core.local_policy_fixture = Some(crate::local_permissions::Policy::fixture);
    let snapshot = SettingsSnapshot {
        section: SettingsSection::Network,
        revision: Digest::new("a".repeat(64)).unwrap(),
        collected_at_unix_ms: c.service.core.host.clock.millis().unwrap(),
        source: "native fixture".into(),
        targets: vec![],
    };
    c.service
        .core
        .data
        .settings_write(&c.selection.subject, &c.selection.binding, &snapshot)
        .unwrap();
    let call = |context, origin, input| Request::Settings {
        context,
        origin,
        input,
    };
    let read = SettingsOperation::Snapshot {
        section: SettingsSection::Network,
    };
    assert!(
        matches!(c.policy_call(call(c.context(),ClientOrigin::Desktop{},read.clone())).await,Reply::Settings {value} if value.snapshot==Some(snapshot.clone()) && !value.write_allowed && value.reason.is_some())
    );
    assert!(matches!(
        c.policy_call(call(c.context(), c.selection.origin.clone(), read.clone()))
            .await,
        Reply::DataFailure { .. }
    ));
    let mut wrong = c.context();
    wrong.generation = plan::id("other-generation").unwrap();
    assert!(matches!(
        c.policy_call(call(wrong, ClientOrigin::Desktop {}, read))
            .await,
        Reply::DataFailure { .. }
    ));
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let policy =
        crate::local_permissions::Policy::fixture(&c.service.core.config.local_permissions)
            .unwrap();
    let (frozen, artifacts) = crate::local_settings::compile::prepare(
        crate::local_settings::Action::Read {
            section: SettingsSection::Network,
        },
        request.clone(),
        &c.service.core.host.root,
        &c.service.core.host.local_actor,
        Initiator::Human {
            os_session: OsSessionRef {
                device: c.service.core.host.root.device.clone(),
                account: OsAccountRef {
                    platform: Platform::Macos,
                    subject: plan::id(&c.selection.subject).unwrap(),
                },
                session: c.selection.binding.clone(),
            },
        },
        &c.context(),
        &policy,
        c.service.core.host.clock.millis().unwrap(),
        &c.service.core.config,
        &mut c.service.core.material_root,
        None,
    )
    .unwrap();
    c.service
        .core
        .host
        .materials
        .register(&frozen, artifacts)
        .unwrap();
    c.service
        .core
        .app
        .stage_execution(&c.service.core.local_caller(), &frozen)
        .unwrap();
    for _ in 0..2 {
        assert!(
            matches!(c.policy_call(call(c.context(),ClientOrigin::Desktop{},SettingsOperation::Status{request:request.clone()})).await,Reply::Settings{value} if value.digest.as_ref()==Some(frozen.digest()) && value.effect==SettingsEffect::Pending)
        );
    }
    assert!(matches!(
        c.policy_call(call(
            c.context(),
            ClientOrigin::Desktop {},
            SettingsOperation::Refresh {
                request: request.clone(),
                section: SettingsSection::Power
            }
        ))
        .await,
        Reply::DataFailure { .. }
    ));
    assert!(matches!(
        c.policy_call(call(
            c.context(),
            ClientOrigin::Desktop {},
            SettingsOperation::KeepDisplay {
                request: request.clone(),
                digest: frozen.digest().clone()
            }
        ))
        .await,
        Reply::DataFailure { .. }
    ));
    assert!(
        matches!(c.policy_call(call(c.context(),ClientOrigin::Desktop{},SettingsOperation::Cancel{request:request.clone()})).await,Reply::Settings{value} if value.finished && value.effect==SettingsEffect::Cancelled)
    );
    assert!(
        matches!(c.policy_call(call(c.context(),ClientOrigin::Desktop{},SettingsOperation::Refresh{request:request.clone(),section:SettingsSection::Network})).await,Reply::Settings{value} if value.finished && value.digest.as_ref()==Some(frozen.digest()))
    );
    assert_eq!(
        c.service
            .core
            .app
            .status(&c.service.core.local_caller(), &request)
            .unwrap()
            .attempts,
        0
    );
    c.employee("new-login-context").await;
    assert!(matches!(
        c.policy_call(call(
            c.context(),
            ClientOrigin::Desktop {},
            SettingsOperation::Status { request }
        ))
        .await,
        Reply::DataFailure { .. }
    ));
}

fn fixture_worker_image(_: &ExecutionConfig) -> Result<SoftwareMaterial, agent_client::Error> {
    plan::pin(std::path::Path::new("/bin/sh"))
}

#[tokio::test]
async fn settings_cache_is_read_only_and_rejects_ai_and_retired_context_without_os_dispatch() {
    use execution_ipc::host::Request;
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    let call = |context, origin| Request::Settings {
        context,
        origin,
        input: SettingsOperation::Snapshot {
            section: SettingsSection::Network,
        },
    };
    assert!(
        matches!(c.policy_call(call(c.context(),ClientOrigin::Desktop{})).await,Reply::Settings {value} if value.snapshot.is_none() && !value.write_allowed)
    );
    assert!(matches!(
        c.policy_call(call(c.context(), c.selection.origin.clone()))
            .await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
    let old = c.context();
    c.employee("retired-settings-context").await;
    assert!(matches!(
        c.policy_call(call(old, ClientOrigin::Desktop {})).await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
    assert!(c
        .service
        .core
        .app
        .service_tasks(None, 64)
        .unwrap()
        .items
        .is_empty());
}

#[tokio::test]
async fn network_check_defaults_and_invalid_targets_use_the_authenticated_service_boundary() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    c.employee("network-test").await;
    use execution_ipc::host::Request;
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let call = |context, origin, input| Request::NetworkCheck {
        context,
        origin,
        input,
    };
    let expected_port = c._server.url.port_or_known_default().unwrap();
    let defaults = c
        .policy_call(call(
            c.context(),
            ClientOrigin::Desktop {},
            NetworkCheckOperation::Defaults,
        ))
        .await;
    assert!(
        matches!(defaults,Reply::NetworkDefaults {target:Some(target)} if target.host=="127.0.0.1" && target.port==expected_port)
    );
    assert!(matches!(
        c.policy_call(call(
            c.context(),
            c.selection.origin.clone(),
            NetworkCheckOperation::Defaults
        ))
        .await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
    let invalid = NetworkCheckOperation::Run {
        request: request.clone(),
        target: NetworkTarget {
            host: "https://example.com/command".into(),
            port: 443,
        },
    };
    assert!(matches!(
        c.policy_call(call(c.context(), ClientOrigin::Desktop {}, invalid))
            .await,
        Reply::DataFailure {
            reason: DataError::InvalidInput
        }
    ));
    assert!(!c
        .service
        .core
        .app
        .has_service_execution(&request, &c.service.core.host.root.device)
        .unwrap());
    let retired = c.context();
    c.employee("network-next-user").await;
    assert!(matches!(
        c.policy_call(call(
            retired,
            ClientOrigin::Desktop {},
            NetworkCheckOperation::Defaults
        ))
        .await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
}

#[tokio::test]
#[ignore = "requires root for production material/OS identity admission; explicitly run the built test binary"]
async fn network_check_replay_and_cancel_keep_the_original_sqlite_plan_and_login() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    c.employee("network-request-owner").await;
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let target = NetworkTarget {
        host: "example.com".into(),
        port: 443,
    };
    c.service.core.config.worker_image_fixture = Some(fixture_worker_image);
    let os_session = OsSessionRef {
        device: c.service.core.host.root.device.clone(),
        account: OsAccountRef {
            platform: Platform::Macos,
            subject: plan::id(&c.selection.subject).unwrap(),
        },
        session: c.selection.binding.clone(),
    };
    // Stage the actual compiler output without dispatch; System material inspection needs root.
    let (frozen, artifacts) = crate::network_check::compile::prepare(
        target.clone(),
        request.clone(),
        &c.service.core.host.root,
        &c.service.core.host.local_actor,
        Initiator::Human { os_session },
        &c.context(),
        c.service.core.host.clock.millis().unwrap(),
        &c.service.core.config,
        &mut c.service.core.material_root,
        None,
    )
    .unwrap();
    c.service
        .core
        .host
        .materials
        .register(&frozen, artifacts)
        .unwrap();
    c.service
        .core
        .app
        .stage_execution(&c.service.core.local_caller(), &frozen)
        .unwrap();
    let Reply::RecentTasks { value } = c.recent_call(None).await else {
        panic!("recent tasks");
    };
    assert!(
        value
            .items
            .iter()
            .any(|item| item.details.action.request_id == request
                && item.details.status.attempts == 0)
    );
    use execution_ipc::host::Request;
    let call = |context, input| Request::NetworkCheck {
        context,
        origin: ClientOrigin::Desktop {},
        input,
    };
    for _ in 0..2 {
        assert!(
            matches!(c.policy_call(call(c.context(),NetworkCheckOperation::Run {request:request.clone(),target:target.clone()})).await,Reply::NetworkCheck {value} if value.status.content_digest==*frozen.digest() && value.status.attempts==0 && value.result.is_none())
        );
    }
    assert!(matches!(
        c.policy_call(call(
            c.context(),
            NetworkCheckOperation::Run {
                request: request.clone(),
                target: NetworkTarget {
                    host: "other.example".into(),
                    port: 80
                }
            }
        ))
        .await,
        Reply::DataFailure {
            reason: DataError::Conflict
        }
    ));
    assert!(
        matches!(c.policy_call(call(c.context(),NetworkCheckOperation::Cancel {request:request.clone()})).await,Reply::NetworkCheck {value} if value.status.phase==execution_app::TaskPhase::Cancelled && value.status.attempts==0)
    );
    c.employee("network-retired-owner").await;
    let Reply::RecentTasks { value } = c.recent_call(None).await else {
        panic!("recent tasks");
    };
    assert!(value
        .items
        .iter()
        .all(|item| item.details.action.request_id != request));

    assert!(matches!(
        c.policy_call(call(c.context(), NetworkCheckOperation::Status { request }))
            .await,
        Reply::DataFailure {
            reason: DataError::Denied
        }
    ));
}

#[tokio::test]
async fn recent_activity_rejects_invalid_cursors_without_creating_work() {
    let _budget = crate::scenario_budget::enter();
    let mut c = Case::new(None).await;
    assert!(
        matches!(c.recent_call(None).await, Reply::RecentTasks{value} if value.items.is_empty() && value.next.is_none())
    );
    for cursor in ["0", "-1", "01", "not-a-cursor"] {
        assert!(matches!(
            c.recent_call(Some(cursor.into())).await,
            Reply::Rejected
        ));
    }
    assert!(c
        .service
        .core
        .app
        .service_tasks(None, 64)
        .unwrap()
        .items
        .is_empty());
}

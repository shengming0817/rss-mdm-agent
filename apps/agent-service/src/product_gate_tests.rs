//! Real signed offers/Start and SQLite, with risk available only in this test assembly.
use super::*;
use crate::{
    backend::gate::{ProductGateProof, TrustedRisk},
    protocol_test_support as protocol,
};
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
        let server = protocol::Server::new().await;
        let root = protocol::Root::new();
        let clock = SystemClock::new().unwrap();
        server.time.set(clock.now().unwrap());
        server.data.lock().unwrap().software(1, true);
        let mut client = server.client(&root, OpenMode::Create);
        server.register(&mut client).await;
        let offer = client.claim().await.unwrap().offer.unwrap();
        let view = offered(&offer).unwrap();
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
        let mut service = DeviceService::open(
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
        )
        .unwrap();
        service.core.available = Some(view.clone());
        service.core.available_risk = level.map(|level| {
            TrustedRisk::fixture(
                &offer,
                BackendRiskDecision {
                    level,
                    policy: plan::reference("backend-risk-policy", "1").unwrap(),
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
                config: plan::reference("ai", "1").unwrap(),
                conversation: plan::id("conversation").unwrap(),
                tool_call: plan::id("tool-call").unwrap(),
            },
        };
        Self {
            _server: server,
            _root: root,
            service,
            offer,
            selection,
        }
    }
    fn selection(&self, human: bool) -> Selection {
        let mut selection = Selection::from_record(&BackendRequest {
            offer: offered(&self.offer).unwrap(),
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
            Some(RiskLevel::Zero | RiskLevel::One) => BackendRequestState::Ready,
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
        .request_start(&c.offer, &materials)
        .await
        .unwrap();
    // Test the proof's binding independently of native package staging/physical execution.
    let mut spec: ExecutionInput = serde_json::from_str(include_str!(
        "../../../crates/execution-contract/tests/fixtures/plan.json"
    ))
    .unwrap();
    spec.request.request_id = record.offer.request.clone();
    spec.request.initiator = Initiator::Backend {
        task: record.offer.task.clone(),
        attempt: record.offer.attempt.clone(),
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

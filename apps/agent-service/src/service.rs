use crate::error::app_error as map_app_error;
use crate::{
    backend::{self, host::EnterpriseHost, offered, plan, ExecutionBridge},
    DeviceSecrets, Interpreter, SystemClock,
};
use agent_client::{Client, CredentialRedactor, Error, IdentityProvider, Materials, Offer};
use execution_app::{AppConfig, ExecutionApp, RequestContext};
use execution_contract::{
    BackendRequest, BackendRequestFailure, BackendRequestState, BackendTrigger, PreparedAction,
    RequestId,
};
use execution_runner::{MaterialRegistry, NativeRunner};
use rss_mdm_agent_wire::runtime as wire;
mod policy;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// Explicit local execution resources from the protected deployment configuration.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    pub work_root: PathBuf,
    pub material_root: PathBuf,
    pub interpreters: Vec<Interpreter>,
    pub managers: Vec<crate::backend::plan::SoftwareManager>,
    pub processes: usize,
}
// Helper availability is not device registration identity. A closed login/endpoint
// rejects helper work without entering the backend credential revocation path.
pub(crate) fn helper_error(error: execution_app::Error) -> Error {
    match error {
        execution_app::Error::Unbound => Error::Unavailable,
        other => map_app_error(other),
    }
}
#[cfg(test)]
#[test]
fn unavailable_helper_does_not_revoke_device_identity() {
    assert_eq!(
        helper_error(execution_app::Error::Unbound),
        Error::Unavailable
    );
    assert_eq!(helper_error(execution_app::Error::Denied), Error::Denied);
    assert_eq!(
        map_app_error(execution_app::Error::Unbound),
        Error::Identity
    );
}

/// Administrator-pinned helpers; these select OS mechanisms, not enterprise authorization.
#[derive(Clone)]
pub struct UserResources {
    pub image: installation_security::Artifact,
    pub work_roots: std::collections::BTreeMap<String, PathBuf>,
}
impl UserResources {
    pub(crate) fn connect(
        &self,
        selection: Option<&Selection>,
    ) -> Result<Arc<execution_ipc::helper::Connection>, Error> {
        let (subject, session) = match selection {
            Some(selection) => (selection.subject.clone(), selection.session),
            None => execution_ipc::host::active_user_session().map_err(helper_error)?,
        };
        let root = self.work_roots.get(&subject).ok_or(Error::Denied)?;
        let connection = execution_ipc::helper::Connection::connect(
            execution_ipc::host::PeerPolicy {
                images: vec![self.image.clone()],
                subjects: vec![subject.clone()],
                interactive: true,
            },
            subject,
            session,
        )
        .map_err(helper_error)?;
        if selection.is_some_and(|s| s.binding != connection.context().binding) {
            return Err(Error::Untrusted);
        }
        if root.canonicalize()? != connection.context().work_root {
            return Err(Error::Denied);
        }
        Ok(Arc::new(connection))
    }
}
/// One device communication owner and one authoritative execution journal.
/// The OS service supplies this assembly; no fixture runner or authority is selectable.
pub struct DeviceService<S: IdentityProvider = DeviceSecrets> {
    pub(crate) client: Client<S, SystemClock>,
    pub(crate) core: Core,
    pub(crate) bridge: ExecutionBridge<CredentialRedactor>,
    waiting: Option<(Offer, Materials)>,
    next_network: std::time::Instant,
    next_enrollment: std::time::Instant,
    recovering: bool,
    pub(crate) material_gc: crate::material_gc::MaterialGc,
    pub(crate) installation: Option<crate::deployment::Deployment>,
}
impl<S: IdentityProvider> DeviceService<S> {
    /// Open an already registered device. Create/open selection never follows an error fallback.
    pub fn open(
        client: Client<S, SystemClock>,
        journal: &Path,
        startup: ProductionStartup,
        config: ExecutionConfig,
        clock: SystemClock,
        helpers: UserResources,
    ) -> Result<Self, Error> {
        Self::assemble(
            client,
            journal,
            startup,
            config,
            clock,
            helpers,
            execution_runner::staging::MaterialRoot::open,
        )
    }
    // The only production caller supplies MaterialRoot::open. Unit preparation fixtures
    // select an explicit fixture constructor without changing dispatch or journal ownership.
    fn assemble(
        mut client: Client<S, SystemClock>,
        journal: &Path,
        startup: ProductionStartup,
        config: ExecutionConfig,
        clock: SystemClock,
        helpers: UserResources,
        open_materials: fn(
            &Path,
            &execution_contract::Digest,
            &Path,
        )
            -> Result<execution_runner::staging::MaterialRoot, execution_app::Error>,
    ) -> Result<Self, Error> {
        if config.interpreters.is_empty()
            || config.interpreters.len() > 8
            || config.processes == 0
            || config.processes > 128
        {
            return Err(Error::Configuration);
        }
        let mut profiles = Vec::new();
        for interpreter in &config.interpreters {
            interpreter
                .image
                .verify(&interpreter.image.path)
                .map_err(|_| Error::Untrusted)?;
            profiles.push(interpreter.profile);
        }
        client.set_profiles(profiles)?;
        platform_private_storage::validate(&config.work_root)?;
        let (binding, actor) = plan::context(
            client.configuration().origin.as_str(),
            client.configuration().tenant,
            &client.registration()?,
        )?;
        let materials = MaterialRegistry::new(client.configuration().limits.pending_tasks)
            .map_err(crate::error::app_error)?;
        let runner = NativeRunner::with_materials(
            plan::id("native-device-runner")?,
            materials.clone(),
            config.processes,
        )
        .map_err(crate::error::app_error)?;
        let host = EnterpriseHost {
            binding,
            actor,
            clock,
            materials,
            subject: execution_ipc::host::current_subject().map_err(crate::error::app_error)?,
            current: Arc::new(Mutex::new(None)),
            identity_until: client.runtime_gate(),
            revoked: Arc::new(AtomicBool::new(client.runtime_readiness().is_err())),
        };
        let app = assemble_journal(
            journal,
            startup,
            host.clone(),
            runner,
            app_config(),
            plan::storage_limits(),
        )
        .map_err(crate::error::app_error)?;
        use sha2::{Digest as _, Sha256};
        let owner = execution_contract::Digest::new(format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(&host.binding.authority, &host.binding.device))
                    .map_err(|_| Error::Configuration)?
            )
        ))
        .map_err(|_| Error::Configuration)?;
        let material_root = open_materials(&config.material_root, &owner, journal)
            .map_err(crate::error::app_error)?;
        let output = client.output_policy()?;
        let telemetry = if client
            .registration()?
            .capabilities
            .contains(&wire::Capability::TelemetryCollectV1)
        {
            Some(crate::telemetry::Telemetry::open(&client)?)
        } else {
            None
        };
        Ok(Self {
            client,
            core: Core {
                telemetry,
                app,
                host,
                config,
                material_root,
                helpers,
                available: None,
                available_risk: None,
                selected: None,
                stopping: false,
                recovery_cursor: None,
                mdm: execution_ipc::host::MdmStatus {
                    state: wire::MdmEnrollmentState::Unknown,
                    observed_at: None,
                },
                mdm_reported: None,
                contexts: Default::default(),
                fences: Default::default(),
                credential_leases: Default::default(),
                policy_commands: Default::default(),
            },
            bridge: ExecutionBridge::new(plan::id("backend-results")?, output),
            waiting: None,
            next_network: std::time::Instant::now(),
            next_enrollment: std::time::Instant::now(),
            recovering: true,
            material_gc: Default::default(),
            installation: None,
        })
    }
    /// Reconcile persisted intents and deliver existing evidence before claiming new work.
    async fn drive(
        &mut self,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        if let Some(deployment) = self.installation.clone() {
            if let Some(permit) = self.client.accepted_unmanage(deployment.installation_id)? {
                self.core.host.revoked.store(true, Ordering::Release);
                self.core.revoke_preparations()?;
                self.release_waiting();
                self.stop()?;
                if self
                    .core
                    .app
                    .all_attempts_quiescent(128)
                    .map_err(map_app_error)?
                {
                    let permit = self.client.unmanage_drained(permit)?;
                    crate::installation::launch_unmanage(&deployment, &permit)?;
                }
                return Ok(());
            }
        }
        while let Ok(command) = commands.try_recv() {
            self.core.handle(command);
        }
        self.update_identity_gate();
        if self.core.stopping {
            return Ok(());
        }
        self.core.step_telemetry();
        self.reclaim_owner_step();
        if std::time::Instant::now() >= self.next_enrollment {
            self.core.reconcile()?;
            self.next_enrollment = std::time::Instant::now() + std::time::Duration::from_secs(2);
            let result = network(self.client.drive_pki(), &mut self.core, commands).await;
            self.update_identity_gate();
            result?;
        }
        // Task trust belongs to the communication owner. A failed refresh retains usable installed keys.
        let _ = network(self.client.drive_task_trust(), &mut self.core, commands).await;
        self.update_identity_gate();
        if !self.core.host.identity_ready() {
            self.core.revoke_preparations()?;
            self.core.reconcile()?;
            return Ok(());
        }
        if let Some(action) = self
            .core
            .telemetry
            .as_mut()
            .map(|t| t.action())
            .transpose()?
            .flatten()
        {
            let result = network(
                async { Ok(crate::telemetry::request(&self.client, &action).await) },
                &mut self.core,
                commands,
            )
            .await;
            if let Some(telemetry) = &mut self.core.telemetry {
                telemetry.apply(&action, result.unwrap_or_else(|e| Err(e.into())))?;
            }
        }
        self.prune_credentials()?;
        self.drive_policy(commands).await?;
        if let Some((offer, _)) = &self.waiting {
            let request = backend::request_id(offer)?;
            if self
                .core
                .app
                .backend_request(&self.core.caller(), &request)
                .map_err(crate::error::app_error)?
                .is_some_and(|p| {
                    matches!(
                        p.state,
                        BackendRequestState::Cancelled | BackendRequestState::Failed
                    )
                })
            {
                let task = offer.task_id();
                self.release_waiting();
                self.abandon(task, commands).await?;
            }
        }
        // Self-service consent already lives in the original journal. A retired ingress
        // must not discard accepted Device work. Dispatch verifies its original OS session.
        if let Some((offer, _)) = &self.waiting {
            if is_policy_offer(offer) {
                let offer = self.client.retained_offer(offer.task_id())?;
                match self.bind_policy_offer(&offer) {
                    Ok(selection) => self.core.selected = Some(selection),
                    Err(error) => {
                        if retryable_submission(error) {
                            return Err(error);
                        }
                        let task = offer.task_id();
                        self.core.transition(
                            &backend::request_id(&offer)?,
                            BackendRequestState::Failed,
                            Some(if matches!(error, Error::Identity | Error::Denied) {
                                BackendRequestFailure::Revoked
                            } else if matches!(error, Error::Expired) {
                                BackendRequestFailure::Expired
                            } else {
                                BackendRequestFailure::PreparationFailed
                            }),
                        )?;
                        self.release_waiting();
                        self.abandon(task, commands).await?;
                        return Err(error);
                    }
                }
            }
        }
        if self.core.selected.is_some() {
            if let Some((offer, materials)) = self.waiting.take() {
                let request = backend::request_id(&offer)?;
                let selection = self.core.selected.take();
                self.core
                    .transition(&request, BackendRequestState::Submitting, None)?;
                let result = backend::submit(self, offer, materials, selection, commands).await;
                self.core.available = None;
                self.core.available_risk = None;
                self.submission_result(&request, result)?;
            }
        }
        self.core.reconcile()?;
        if std::time::Instant::now() < self.next_network {
            return Ok(());
        }
        self.next_network = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let caller = self.core.caller();
        if self.waiting.as_ref().is_some_and(|(offer, _)| {
            self.core
                .host
                .clock
                .millis()
                .is_ok_and(|now| now / 1000 >= offer.payload().expires_at() as u64)
        }) {
            if let Some((offer, _)) = &self.waiting {
                self.core.transition(
                    &backend::request_id(offer)?,
                    BackendRequestState::Failed,
                    Some(BackendRequestFailure::Expired),
                )?;
            }
            self.release_waiting();
        }
        let pending = self
            .client
            .pending_tasks(self.client.configuration().limits.pending_tasks)?;
        for task in pending {
            while let Ok(command) = commands.try_recv() {
                self.core.handle(command);
            }
            if self.core.stopping {
                return Ok(());
            }
            let retained = self.client.retained_offer(task)?;
            if matches!(retained.payload(), wire::TaskPayload::Unmanage(_)) {
                self.accept_retirement(&retained, commands).await?;
                return Ok(());
            }
            if self.client.bound_request(task)?.is_some() {
                if !self
                    .bridge
                    .recover_binding(&mut self.client, task, &self.core.app)?
                {
                    self.abandon(task, commands).await?;
                    continue;
                }
                let retained = self.client.retained_offer(task)?;
                if let wire::TaskPayload::Credential(spec) = retained.payload() {
                    let root = crate::credential_worker::root(&self.core.config);
                    if let Some(escrow) = crate::credential_worker::pending(&root, spec)? {
                        network(
                            self.client.escrow_credentials(task, escrow.clone()),
                            &mut self.core,
                            commands,
                        )
                        .await?;
                        crate::credential_worker::acknowledge(&root, spec, &escrow)?;
                    }
                }
                if let Some(mut pending) =
                    self.bridge
                        .prepare_delivery(&mut self.client, task, &mut self.core.app, 64)?
                {
                    network(
                        self.bridge.send_delivery(&mut self.client, &mut pending),
                        &mut self.core,
                        commands,
                    )
                    .await?;
                    self.bridge
                        .finish_delivery(&mut self.client, pending, &mut self.core.app)?;
                }
                let request = self.client.bound_request(task)?.ok_or(Error::Conflict)?;
                let input = self
                    .core
                    .app
                    .frozen_input(&caller, &request)
                    .map_err(crate::error::app_error)?;
                match self
                    .bridge
                    .finish(&mut self.client, task, &self.core.app, &caller)
                {
                    Ok(()) => match self.core.host.materials.retire(input.digest()) {
                        Ok(()) | Err(execution_app::Error::Conflict) => (),
                        Err(error) => return Err(map_app_error(error)),
                    },
                    Err(Error::Conflict) => {
                        // Only unresolved original work needs recovery material. Settled tasks
                        // must release first, without rebuilding every native phase after ACK.
                        if !self
                            .core
                            .host
                            .materials
                            .registered(&input)
                            .map_err(crate::error::app_error)?
                        {
                            if let Ok(materials) = crate::recovery::materials(
                                &input,
                                &self.core.config,
                                &self.core.helpers,
                            ) {
                                self.core
                                    .host
                                    .materials
                                    .register(&input, materials)
                                    .map_err(crate::error::app_error)?;
                            }
                        }
                    }
                    Err(error) => return Err(error),
                }
            } else if self.recovering
                || self
                    .waiting
                    .as_ref()
                    .is_none_or(|(offer, _)| offer.task_id() != task)
            {
                let offer = self.client.retained_offer(task)?;
                if let Some(record) = self.policy_intent(&offer)? {
                    let now = self.core.host.clock.millis()?;
                    if now >= record.action.expires_at_unix_ms()
                        || now / 1000 >= offer.payload().expires_at() as u64
                    {
                        self.core.transition(
                            record.action.request(),
                            BackendRequestState::Failed,
                            Some(BackendRequestFailure::Expired),
                        )?;
                        self.abandon(task, commands).await?;
                        continue;
                    }
                    if let Err(error) = self.core.current_policy_access(&record) {
                        if retryable_submission(error) {
                            return Err(error);
                        }
                        self.core.transition(
                            record.action.request(),
                            BackendRequestState::Failed,
                            Some(if error == Error::Expired {
                                BackendRequestFailure::Expired
                            } else {
                                BackendRequestFailure::Revoked
                            }),
                        )?;
                        self.abandon(task, commands).await?;
                        continue;
                    }
                    if self.waiting.is_some() {
                        // Capacity delays this original intent; it never revokes consent.
                        continue;
                    }
                    // Recover only the already recorded consent and verified original offer.
                    // Binding and Start still recheck the original OS session and current device.
                    let materials =
                        network(self.client.prepare(&offer), &mut self.core, commands).await?;
                    self.core.available_risk = backend::gate::TrustedRisk::from_offer(&offer);
                    self.core.available = Some(offered(&offer)?);
                    self.waiting = Some((offer, materials));
                } else {
                    // No original resumable local intent means no execution may have started.
                    self.abandon(task, commands).await?;
                }
            }
        }
        self.recovering = false;
        let now = i64::try_from(crate::wall()? / 1000).map_err(|_| Error::Clock)?;
        if self
            .core
            .mdm
            .observed_at
            .is_none_or(|at| now.saturating_sub(at) >= 30)
        {
            let expected = self.client.native_enrollment_origin()?;
            let observed = network(
                async { Ok(crate::mdm_state::observe(expected.as_deref()).await) },
                &mut self.core,
                commands,
            )
            .await?;
            self.core.mdm = execution_ipc::host::MdmStatus {
                state: observed,
                observed_at: Some(now),
            };
        }
        if let Some(observed_at) = self.core.mdm.observed_at {
            if self.core.mdm_reported != Some(self.core.mdm.state)
                && self
                    .client
                    .registration()?
                    .collections
                    .iter()
                    .any(|c| c.dataset() == "channel.mdm.enrollment")
            {
                let body = serde_json::from_value(serde_json::json!({"kind":"snapshot","values":[{"field":"channel.mdm.enrollment","value":{"kind":"value","value":{"kind":"string","value":self.core.mdm.state.as_str()}}}]})).map_err(|_| Error::Protocol)?;
                match self
                    .client
                    .queue_report("channel.mdm.enrollment", body, observed_at)
                {
                    Ok(_) => self.core.mdm_reported = Some(self.core.mdm.state),
                    // Retain the latest observation, but never make production capacity block ACK consumption.
                    Err(Error::Capacity) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        let reports = 16.min(self.client.configuration().limits.pending_reports);
        network(self.client.flush_reports(reports), &mut self.core, commands).await?;
        let mut context = self.client.execution_context()?;
        context.os_version = crate::os_version::current()?;
        context.interactive_user = self.core.helpers.connect(None).ok().and_then(|helper| {
            let generated_uid =
                crate::account_identity::generated_uid(&helper.context().subject).ok()?;
            Some(wire::SoftwareInteractiveUser {
                identity: helper.context().subject.clone(),
                session_id: login_id(helper.context()),
                administrator: false,
                generated_uid,
            })
        });
        self.client.set_execution_context(context)?;
        let interpreters = self.core.config.interpreters.clone();
        let environment = network(
            crate::collection_environment::observe(&interpreters),
            &mut self.core,
            commands,
        )
        .await?;
        self.client.set_collection_environment(environment)?;
        let claim = match network(self.client.claim(), &mut self.core, commands).await {
            Ok(v) => v,
            Err(e) => {
                if matches!(
                    e,
                    Error::CredentialRevoked | Error::CredentialRetired | Error::CertificateExpired
                ) {
                    self.core.host.revoked.store(true, Ordering::Release);
                    self.core
                        .app
                        .stop_active(128)
                        .map_err(crate::error::app_error)?;
                }
                return Err(e);
            }
        };
        for cancel in claim.cancellations {
            if self.client.bound_request(cancel.task_id())?.is_some() {
                self.bridge
                    .cancel(&self.client, &cancel, &mut self.core.app, &caller)?;
            } else if self.waiting.as_ref().is_some_and(|(offer, _)| {
                offer.task_id() == cancel.task_id() && offer.attempt_id() == cancel.attempt_id()
            }) {
                self.release_waiting();
                // Withdrawal has no execution effect; the original request remains queryable.
                let pending = self.bridge.prepare_abandonment(
                    &mut self.client,
                    cancel.task_id(),
                    &self.core.app,
                )?;
                self.core.transition(
                    pending.request_id(),
                    BackendRequestState::Cancelled,
                    Some(BackendRequestFailure::Revoked),
                )?;
                self.abandon(cancel.task_id(), commands).await?;
            }
        }
        if let Some(offer) = claim.offer {
            if matches!(offer.payload(), wire::TaskPayload::Unmanage(_)) {
                self.accept_retirement(&offer, commands).await?;
                return Ok(());
            }

            if self.waiting.as_ref().is_some_and(|(old, _)| {
                old.task_id() == offer.task_id() && old.attempt_id() == offer.attempt_id()
            }) {
                return Ok(());
            }
            if self.waiting.is_some() {
                return Err(Error::Capacity);
            }
            if self.client.bound_request(offer.task_id())?.is_some() {
                return Ok(());
            }
            self.core.available_risk = backend::gate::TrustedRisk::from_offer(&offer);
            self.core.available = Some(offered(&offer)?);
            network(self.client.received(&offer), &mut self.core, commands).await?;
            let materials = network(self.client.prepare(&offer), &mut self.core, commands).await?;
            if is_policy_offer(&offer) {
                self.waiting = Some((offer, materials));
                // The next owner iteration resumes automatically from the original consent.
                return Ok(());
            }
            if matches!(offer.payload(),wire::TaskPayload::Software(p) if p.start_mode==wire::SoftwareStartMode::UserInitiated)
            {
                self.waiting = Some((offer, materials));
                return Ok(());
            }
            backend::submit(self, offer, materials, None, commands).await?;
            self.core.available = None;
            self.core.available_risk = None;
        }
        Ok(())
    }
    async fn accept_retirement(
        &mut self,
        offer: &Offer,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        let installation = self
            .installation
            .as_ref()
            .ok_or(Error::Unsupported)?
            .installation_id;
        network(
            self.client.authorize_unmanage(offer, installation),
            &mut self.core,
            commands,
        )
        .await?;
        self.core.host.revoked.store(true, Ordering::Release);
        Ok(())
    }
    fn submission_result(
        &mut self,
        request: &RequestId,
        result: Result<execution_app::ExecutionStatus, Error>,
    ) -> Result<execution_app::ExecutionStatus, Error> {
        if let Err(error) = result {
            // Delivery may already have committed. Its journal owns recovery; never
            // convert an ambiguous response into permission to abandon that execution.
            let delivered = self
                .core
                .app
                .has_service_execution(request, &self.core.host.binding.device)
                .map_err(crate::error::app_error)?;
            let retryable = retryable_submission(error)
                && self
                    .core
                    .app
                    .backend_request(&self.core.caller(), request)
                    .map_err(crate::error::app_error)?
                    .is_some_and(|record| {
                        record.state == BackendRequestState::Submitting
                            && matches!(
                                record.action,
                                execution_contract::PreparedAction::Policy { .. }
                            )
                    });
            if !delivered && !retryable {
                self.core.transition(
                    request,
                    BackendRequestState::Failed,
                    Some(match error {
                        Error::Expired => BackendRequestFailure::Expired,
                        Error::Identity | Error::Denied => BackendRequestFailure::Revoked,
                        _ => BackendRequestFailure::PreparationFailed,
                    }),
                )?;
            }
            return Err(error);
        }
        result
    }
    fn policy_intent(&self, offer: &Offer) -> Result<Option<BackendRequest>, Error> {
        let request = backend::request_id(offer)?;
        Ok(self
            .core
            .app
            .backend_request(&self.core.caller(), &request)
            .map_err(crate::error::app_error)?
            .filter(|record| {
                record.state == BackendRequestState::Submitting
                    && matches!(
                        record.action,
                        execution_contract::PreparedAction::Policy { .. }
                    )
            }))
    }
    fn release_waiting(&mut self) {
        self.waiting = None;
        self.core.available = None;
        self.core.available_risk = None;
        self.core.selected = None;
    }
    async fn abandon(
        &mut self,
        task: uuid::Uuid,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        let mut pending =
            self.bridge
                .prepare_abandonment(&mut self.client, task, &self.core.app)?;
        if !self
            .core
            .app
            .backend_request(&self.core.caller(), pending.request_id())
            .map_err(crate::error::app_error)?
            .is_some_and(|record| {
                matches!(
                    record.state,
                    BackendRequestState::Failed | BackendRequestState::Cancelled
                )
            })
        {
            self.core.transition(
                pending.request_id(),
                BackendRequestState::Failed,
                Some(BackendRequestFailure::Interrupted),
            )?;
        }
        network(
            self.bridge.send_abandonment(&mut self.client, &mut pending),
            &mut self.core,
            commands,
        )
        .await?;
        self.bridge
            .finish_abandonment(&mut self.client, pending, &self.core.app)
    }
    /// Start the single service owner; native callbacks never access SQLite or network directly.
    pub fn spawn(
        mut self,
        policy: execution_ipc::host::PeerPolicy,
        helper_policy: Option<execution_ipc::host::PeerPolicy>,
    ) -> Result<ServiceHandle, Error>
    where
        S: Send + 'static,
    {
        policy.validate().map_err(crate::error::app_error)?;
        if let Some(policy) = &helper_policy {
            policy.validate().map_err(crate::error::app_error)?;
        }
        let fences = self.core.fences.clone();
        let (sender, mut commands) = tokio::sync::mpsc::channel(32);
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let registration = Arc::new(Mutex::new(crate::registration::project(
            self.client.enrollment_status()?,
        )));
        let published = registration.clone();
        let (completed, completion) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new().name("rss-device-owner".into()).spawn(move||{
            struct Finished(Arc<AtomicBool>);impl Drop for Finished{fn drop(&mut self){self.0.store(true,Ordering::Release);}}
            let _finished=Finished(done);
            let result=(||->Result<(),Error>{
                let runtime=owner_runtime()?;
                runtime.block_on(async{
                    let mut timer=tokio::time::interval(std::time::Duration::from_millis(50));
                    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    while !self.core.stopping {
                        tokio::select! {
                            command=commands.recv()=>match command{Some(command)=>self.core.handle(command),None=>break},
                            _=timer.tick()=>{if let Err(error)=self.drive(&mut commands).await{self.drive_error(error);}}
                        }
                        if self.core.selected.is_some(){if let Err(error)=self.drive(&mut commands).await{self.drive_error(error);}}
                        *published.lock().map_err(|_| Error::Unavailable)? = crate::registration::project(self.client.enrollment_status()?);
                    }
                    self.stop()
                })
            })();
            if let Err(error)=&result{eprintln!("agent_owner: {error}");}
            let _ = completed.send(result);
        })?;
        Ok(ServiceHandle {
            sender,
            fences,
            policy,
            helper_policy,
            finished,
            registration,
            completion,
        })
    }
    fn update_identity_gate(&mut self) {
        if let Some(deployment) = &self.installation {
            if !matches!(
                self.client.accepted_unmanage(deployment.installation_id),
                Ok(None)
            ) {
                self.core.host.revoked.store(true, Ordering::Release);
                return;
            }
        }
        match self.client.runtime_deadline().and_then(|until| {
            if self.core.host.identity_until.expires_at() == Some(until) {
                Ok(until)
            } else {
                Err(Error::Identity)
            }
        }) {
            Ok(_) => {
                self.core.host.revoked.store(false, Ordering::Release);
            }
            Err(_) => {
                self.core.host.revoked.store(true, Ordering::Release);
                if let Err(error) = self.core.revoke_preparations() {
                    eprintln!("agent_revoke_preparation: {error}");
                }
                self.release_waiting();
                if let Err(error) = self.core.app.stop_active(128) {
                    eprintln!("agent_revoke_stop: {error}");
                }
            }
        }
    }
    fn drive_error(&mut self, error: Error) {
        eprintln!("agent_drive: {error}");
        self.update_identity_gate();
    }
    /// Stop accepted work. Stopping never claims process termination or effect rollback.
    pub fn stop(&mut self) -> Result<(), Error> {
        let telemetry = self
            .core
            .telemetry
            .as_mut()
            .map(|t| t.shutdown())
            .transpose();
        let commands = self
            .core
            .app
            .stop_active(128)
            .map_err(crate::error::app_error);
        commands?;
        telemetry.map(|_| ())
    }
}

fn retryable_submission(error: Error) -> bool {
    matches!(
        error,
        Error::Unavailable | Error::Capacity | Error::Storage | Error::Clock
    )
}

pub(crate) struct Selection {
    pub(crate) request: RequestId,
    pub(crate) task: execution_contract::Id,
    pub(crate) attempt: execution_contract::Id,
    pub(crate) revision: execution_contract::Digest,
    pub(crate) subject: String,
    pub(crate) session: u32,
    pub(crate) binding: execution_contract::Id,
    pub(crate) origin: execution_ipc::host::ClientOrigin,
}
pub(crate) struct Core {
    telemetry: Option<crate::telemetry::Telemetry>,
    pub(crate) app: ExecutionApp<EnterpriseHost, NativeRunner, execution_sqlite::Store>,
    pub(crate) host: EnterpriseHost,
    pub(crate) config: ExecutionConfig,
    pub(crate) material_root: execution_runner::staging::MaterialRoot,
    pub(crate) helpers: UserResources,
    pub(crate) available: Option<execution_contract::BackendTask>,
    pub(crate) available_risk: Option<backend::gate::TrustedRisk>,
    pub(crate) selected: Option<Selection>,
    pub(crate) stopping: bool,
    pub(crate) recovery_cursor: Option<RequestId>,
    pub(crate) mdm: execution_ipc::host::MdmStatus,
    mdm_reported: Option<wire::MdmEnrollmentState>,
    contexts: policy::Contexts,
    fences: Arc<policy::Fences>,
    credential_leases: std::collections::BTreeMap<RequestId, Arc<zeroize::Zeroizing<String>>>,
    policy_commands: policy::Commands,
}
impl Core {
    fn step_telemetry(&mut self) {
        if let Some(mut telemetry) = self.telemetry.take() {
            if let Err(error) = telemetry.step(self) {
                eprintln!("agent_telemetry: {error}");
            }
            self.telemetry = Some(telemetry);
        }
    }
    fn enforce_identity_gate(&mut self) -> Result<(), Error> {
        if !self.host.identity_ready() {
            self.host.revoked.store(true, Ordering::Release);
            self.revoke_preparations()?;
            self.app.stop_active(128).map_err(map_app_error)?;
        }
        Ok(())
    }

    fn reconcile(&mut self) -> Result<(), Error> {
        self.step_telemetry();
        // Each network tick advances one request. Historical pages must not monopolize
        // the single owner while an HTTP deadline or revocation command is pending.
        let page = self
            .app
            .service_tasks(self.recovery_cursor.as_ref(), 1)
            .map_err(crate::error::app_error)?;
        for item in &page.items {
            let request = &item.status.operation_request_id;
            self.app
                .reconcile(request)
                .map_err(crate::error::app_error)?;
        }
        self.recovery_cursor = page.next;
        Ok(())
    }
    fn revoke_preparations(&mut self) -> Result<(), Error> {
        let active = self
            .available
            .as_ref()
            .map(|v| v.request.clone())
            .or_else(|| self.selected.as_ref().map(|v| v.request.clone()));
        if let Some(request) = active {
            self.transition(
                &request,
                BackendRequestState::Failed,
                Some(BackendRequestFailure::Revoked),
            )?;
        }
        for record in self
            .app
            .backend_requests(&self.caller())
            .map_err(crate::error::app_error)?
        {
            if !self
                .app
                .has_service_execution(&record.action.request().clone(), &self.host.binding.device)
                .map_err(crate::error::app_error)?
            {
                self.transition(
                    &record.action.request().clone(),
                    BackendRequestState::Failed,
                    Some(BackendRequestFailure::Revoked),
                )?;
            }
        }
        Ok(())
    }
    fn pending(&self, request: &RequestId, subject: &str) -> Result<Option<BackendRequest>, Error> {
        if self
            .app
            .has_service_execution(request, &self.host.binding.device)
            .map_err(crate::error::app_error)?
        {
            return Ok(None);
        }
        let value = self
            .app
            .backend_request(&self.caller(), request)
            .map_err(crate::error::app_error)?;
        if value
            .as_ref()
            .is_some_and(|p| trigger_subject(&p.trigger) != Some(subject))
        {
            return Err(Error::Denied);
        }
        Ok(value)
    }
    fn transition(
        &mut self,
        request: &RequestId,
        state: BackendRequestState,
        failure: Option<BackendRequestFailure>,
    ) -> Result<Option<BackendRequest>, Error> {
        let Some(previous) = self
            .app
            .backend_request(&self.caller(), request)
            .map_err(crate::error::app_error)?
        else {
            return Ok(None);
        };
        if matches!(
            previous.state,
            BackendRequestState::Observed
                | BackendRequestState::Cancelled
                | BackendRequestState::Failed
        ) {
            return Ok(Some(previous));
        }
        let mut next = previous.clone();
        next.revision += 1;
        next.state = state;
        next.failure = failure;
        self.app
            .record_backend_request(&self.caller(), Some(&previous), &next)
            .map_err(crate::error::app_error)?;
        Ok(Some(next))
    }
    fn start_task(&mut self, selection: Selection) -> Result<execution_ipc::host::Reply, Error> {
        use execution_ipc::host::Reply;
        self.require_origin_context(&selection.subject, &selection.binding, &selection.origin)?;
        if !self.host.identity_ready() {
            return Err(Error::Denied);
        }
        if let Some(previous) = self
            .app
            .backend_request(&self.caller(), &selection.request)
            .map_err(map_app_error)?
        {
            if !previous
                .action
                .offer()
                .is_some_and(|offer| selection.matches(offer))
                || previous.trigger != selection.trigger(&self.host.binding.device)?
            {
                return Err(Error::Conflict);
            }
            // Replays never confirm, change provenance, or create a new attempt.
            if previous.state == BackendRequestState::Ready
                && self.available.as_ref() == previous.action.offer()
            {
                self.selected = Some(Selection::from_record(&previous)?);
            }
            return Ok(Reply::Pending {
                value: Box::new(previous),
            });
        }
        let offer = self.available.as_ref().ok_or(Error::Unavailable)?.clone();
        let now = self.host.clock.millis()?;
        if !selection.matches(&offer)
            || !offer.user_initiated
            || !matches!(
                offer.summary,
                execution_contract::BackendTaskSummary::Software {
                    intent: execution_contract::SoftwareOperation::Install
                        | execution_contract::SoftwareOperation::Uninstall,
                    ..
                }
            )
            || now / 1000 >= u64::try_from(offer.expires_at).map_err(|_| Error::Clock)?
        {
            return Err(Error::Denied);
        }
        let trigger = selection.trigger(&self.host.binding.device)?;
        let risk = if matches!(trigger, BackendTrigger::Ai { .. }) {
            self.available_risk
                .as_ref()
                .and_then(|r| r.decision(&offer.revision))
                .cloned()
        } else {
            None
        };
        let (state, failure) = if matches!(trigger, BackendTrigger::Ai { .. }) {
            if risk.as_ref().is_some_and(|r| now >= r.expires_at_unix_ms) {
                (
                    BackendRequestState::Failed,
                    Some(BackendRequestFailure::Expired),
                )
            } else {
                match risk.as_ref().map(|r| r.level) {
                    Some(execution_contract::RiskLevel::One) => (BackendRequestState::Ready, None),
                    Some(execution_contract::RiskLevel::Two) => {
                        (BackendRequestState::AwaitingConfirmation, None)
                    }
                    _ => (
                        BackendRequestState::Failed,
                        Some(if risk.is_none() {
                            BackendRequestFailure::RiskUnknown
                        } else {
                            BackendRequestFailure::RiskBlocked
                        }),
                    ),
                }
            }
        } else {
            (BackendRequestState::Ready, None)
        };
        let next = BackendRequest {
            action: PreparedAction::Offered {
                offer: offer.clone(),
            },
            trigger,
            risk,
            confirmation: None,
            revision: 1,
            state,
            failure,
        };
        self.app
            .record_backend_request(&self.caller(), None, &next)
            .map_err(map_app_error)?;
        if state == BackendRequestState::Failed {
            return Ok(Reply::Pending {
                value: Box::new(next),
            });
        }
        if state == BackendRequestState::Ready {
            self.selected = Some(Selection::from_record(&next)?);
        }
        Ok(Reply::Queued {
            request: offer.request,
            task: offer.task,
            attempt: offer.attempt,
            confirmation_required: state == BackendRequestState::AwaitingConfirmation,
        })
    }
    fn confirm_task(&mut self, selection: Selection) -> Result<execution_ipc::host::Reply, Error> {
        use execution_ipc::host::Reply;
        if !self.host.identity_ready() {
            return Err(Error::Denied);
        }
        let previous = self
            .app
            .backend_request(&self.caller(), &selection.request)
            .map_err(map_app_error)?
            .ok_or(Error::Denied)?;
        let BackendTrigger::Ai { os_session, .. } = &previous.trigger else {
            return Err(Error::Denied);
        };
        if !previous
            .action
            .offer()
            .is_some_and(|offer| selection.matches(offer))
            || !same_login(&previous.trigger, &selection)
            || previous
                .risk
                .as_ref()
                .is_none_or(|r| r.level != execution_contract::RiskLevel::Two)
        {
            return Err(Error::Denied);
        }
        if previous.state != BackendRequestState::AwaitingConfirmation {
            return Ok(Reply::Pending {
                value: Box::new(previous),
            });
        }
        if self.available.as_ref() != previous.action.offer() {
            return Err(Error::Denied);
        }
        let now = self.host.clock.millis()?;
        let risk = self
            .available_risk
            .as_ref()
            .and_then(|r| {
                previous
                    .action
                    .offer()
                    .and_then(|offer| r.decision(&offer.revision))
            })
            .ok_or(Error::Denied)?;
        if previous.risk.as_ref() != Some(risk) {
            return Err(Error::Denied);
        }
        let until = now.saturating_add(60_000).min(risk.expires_at_unix_ms).min(
            u64::try_from(previous.action.offer().ok_or(Error::Denied)?.expires_at)
                .map_err(|_| Error::Clock)?
                .checked_mul(1000)
                .ok_or(Error::Clock)?,
        );
        if now >= until {
            return Err(Error::Expired);
        }
        let mut next = previous.clone();
        next.revision += 1;
        next.state = BackendRequestState::Ready;
        next.confirmation = Some(execution_contract::BackendConfirmation {
            os_session: os_session.clone(),
            confirmed_at_unix_ms: now,
            expires_at_unix_ms: until,
        });
        self.app
            .record_backend_request(&self.caller(), Some(&previous), &next)
            .map_err(map_app_error)?;
        self.selected = Some(Selection::from_record(&next)?);
        Ok(Reply::Queued {
            request: next.action.request().clone(),
            task: next.action.offer().ok_or(Error::Denied)?.task.clone(),
            attempt: next.action.offer().ok_or(Error::Denied)?.attempt.clone(),
            confirmation_required: false,
        })
    }
    pub(crate) fn caller(&self) -> RequestContext {
        RequestContext {
            actor: self.host.actor.clone(),
        }
    }
    fn can_read(&self, request: &RequestId, subject: &str) -> Result<bool, Error> {
        use execution_contract::{BackendTrigger, Initiator, RunAs};
        let input = self
            .app
            .frozen_input(&self.caller(), request)
            .map_err(crate::error::app_error)?;
        Ok(match &input.spec().request.initiator {
            Initiator::Backend {
                trigger:
                    BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. },
                ..
            } => os_session.account.subject.as_str() == subject,
            Initiator::Backend {
                trigger: BackendTrigger::Automatic {},
                ..
            } => match &input.spec().run_as {
                RunAs::User { account } => account.subject.as_str() == subject,
                _ => true,
            },
            _ => false,
        })
    }
    fn handle(&mut self, command: Command) {
        if let Err(error) = self.enforce_identity_gate() {
            eprintln!("agent_identity_gate: {error}");
        }
        use execution_ipc::host::{Reply, Request};
        if matches!(
            command.request.as_operation(),
            Some(
                execution_ipc::host::Request::BindContext { .. }
                    | Request::RevokeContext {}
                    | execution_ipc::host::Request::ObserveSoftware { .. }
                    | execution_ipc::host::Request::PolicyDirectory { .. }
                    | execution_ipc::host::Request::InvokePolicy { .. }
                    | execution_ipc::host::Request::PreparePolicy { .. }
                    | execution_ipc::host::Request::SubmitPolicy { .. }
                    | execution_ipc::host::Request::ConfirmPolicy { .. }
                    | execution_ipc::host::Request::PolicyStatus { .. }
            )
        ) {
            self.enqueue_policy(command);
            return;
        }
        if command.reply.is_closed() {
            return;
        }
        let reply = (|| -> Result<Reply, Error> {
            match command.request.into_operation() {
                Some(
                    Request::BindContext { .. }
                    | Request::RevokeContext {}
                    | Request::ObserveSoftware { .. }
                    | Request::PolicyDirectory { .. }
                    | Request::InvokePolicy { .. }
                    | Request::PreparePolicy { .. }
                    | Request::SubmitPolicy { .. }
                    | Request::ConfirmPolicy { .. }
                    | Request::PolicyStatus { .. },
                ) => Err(Error::Denied),
                Some(Request::MdmStatus {}) => Ok(Reply::MdmStatus {
                    value: self.mdm.clone(),
                }),
                Some(Request::DeviceOverview {}) => Ok(crate::device_overview::read(
                    &command.subject,
                    command.session,
                )),
                Some(Request::ServiceStatus {}) => {
                    let readiness = if !self.host.identity_ready() {
                        execution_ipc::host::Readiness::NotReady
                    } else {
                        execution_ipc::host::Readiness::Ready
                    };
                    Ok(Reply::ServiceStatus {
                        value: {
                            let mut value = execution_ipc::host::ServiceStatus::new(readiness);
                            value.telemetry = self.telemetry.as_ref().map(|t| t.summary.clone());
                            value
                        },
                    })
                }
                None => {
                    self.stopping = true;
                    Ok(Reply::Unavailable)
                }
                Some(
                    Request::RegistrationStatus {}
                    | Request::RequestRegistration {}
                    | Request::RegistrationContext {}
                    | Request::ImportRegistration { .. },
                ) => Ok(Reply::Rejected),
                Some(Request::Tasks { after }) => {
                    let mut value = self
                        .app
                        .tasks(&self.caller(), after.as_ref(), 64)
                        .map_err(crate::error::app_error)?;
                    value.items.retain(|item| {
                        self.can_read(&item.action.request_id, &command.subject)
                            .unwrap_or(false)
                            && self
                                .policy_readable(
                                    &item.action.request_id,
                                    &command.subject,
                                    command.binding.as_ref(),
                                )
                                .unwrap_or(false)
                    });
                    Ok(Reply::Tasks {
                        value,
                        available: self.available.iter().cloned().collect(),
                        preparations: self
                            .app
                            .backend_requests(&self.caller())
                            .map_err(crate::error::app_error)?
                            .into_iter()
                            .filter(|p| {
                                trigger_subject(&p.trigger) == Some(command.subject.as_str())
                                    && self
                                        .policy_readable(
                                            p.action.request(),
                                            &command.subject,
                                            command.binding.as_ref(),
                                        )
                                        .unwrap_or(false)
                                    && (matches!(
                                        p.action,
                                        execution_contract::PreparedAction::Policy { .. }
                                    ) || !self
                                        .app
                                        .has_service_execution(
                                            &p.action.request().clone(),
                                            &self.host.binding.device,
                                        )
                                        .unwrap_or(true))
                            })
                            .collect(),
                    })
                }
                Some(Request::StartTask {
                    request,
                    task,
                    attempt,
                    revision,
                    origin,
                }) => self.start_task(Selection {
                    request,
                    task,
                    attempt,
                    revision,
                    subject: command.subject,
                    session: command.session,
                    binding: command.binding.ok_or(Error::Denied)?,
                    origin,
                }),
                Some(Request::ConfirmTask { selection }) => self.confirm_task(Selection {
                    request: selection.request,
                    task: selection.task,
                    attempt: selection.attempt,
                    revision: selection.revision,
                    subject: command.subject,
                    session: command.session,
                    binding: command.binding.ok_or(Error::Denied)?,
                    origin: execution_ipc::host::ClientOrigin::Desktop {},
                }),
                Some(Request::Status { request }) => {
                    if !self.policy_readable(
                        &request,
                        &command.subject,
                        command.binding.as_ref(),
                    )? {
                        return Err(Error::Denied);
                    }
                    if let Some(value) = self.pending(&request, &command.subject)? {
                        return Ok(Reply::Pending {
                            value: Box::new(value),
                        });
                    }
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Status {
                        value: self
                            .app
                            .status(&self.caller(), &request)
                            .map_err(crate::error::app_error)?,
                    })
                }
                Some(Request::Details { request }) => {
                    if !self.policy_readable(
                        &request,
                        &command.subject,
                        command.binding.as_ref(),
                    )? {
                        return Err(Error::Denied);
                    }
                    if let Some(value) = self.pending(&request, &command.subject)? {
                        return Ok(Reply::Pending {
                            value: Box::new(value),
                        });
                    }
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Details {
                        value: Box::new(
                            self.app
                                .task_details(&self.caller(), &request)
                                .map_err(crate::error::app_error)?,
                        ),
                    })
                }
                Some(Request::Cancel { request }) => {
                    if !self.policy_readable(
                        &request,
                        &command.subject,
                        command.binding.as_ref(),
                    )? {
                        return Err(Error::Denied);
                    }
                    if let Some(value) = self.pending(&request, &command.subject)? {
                        let value = self
                            .transition(
                                &value.action.request().clone(),
                                BackendRequestState::Cancelled,
                                None,
                            )?
                            .ok_or(Error::Conflict)?;
                        if self.selected.as_ref().is_some_and(|s| s.request == request) {
                            self.selected = None;
                        }
                        return Ok(Reply::Pending {
                            value: Box::new(value),
                        });
                    }
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    let input = self
                        .app
                        .frozen_input(&self.caller(), &request)
                        .map_err(crate::error::app_error)?;
                    if !matches!(
                        &input.spec().request.initiator,
                        execution_contract::Initiator::Backend {
                            trigger: execution_contract::BackendTrigger::Human { .. }
                                | execution_contract::BackendTrigger::Ai { .. },
                            ..
                        }
                    ) {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Status {
                        value: self
                            .app
                            .cancel(&self.caller(), &request)
                            .map_err(crate::error::app_error)?,
                    })
                }
            }
        })();
        let _ = command.reply.send(match reply {
            Ok(reply) => reply,
            Err(
                Error::Denied
                | Error::Conflict
                | Error::Untrusted
                | Error::Identity
                | Error::Protocol,
            ) => Reply::Rejected,
            Err(_) => Reply::Unavailable,
        });
    }
}
enum LocalRequest {
    Operation(Box<execution_ipc::host::Request>),
    Stop,
}
impl LocalRequest {
    fn operation(request: execution_ipc::host::Request) -> Self {
        Self::Operation(Box::new(request))
    }
    fn as_operation(&self) -> Option<&execution_ipc::host::Request> {
        match self {
            Self::Operation(request) => Some(request),
            Self::Stop => None,
        }
    }
    fn into_operation(self) -> Option<execution_ipc::host::Request> {
        match self {
            Self::Operation(request) => Some(*request),
            Self::Stop => None,
        }
    }
}
pub(crate) struct Command {
    request: LocalRequest,
    fence_epoch: u64,
    subject: String,
    session: u32,
    binding: Option<execution_contract::Id>,
    reply: tokio::sync::oneshot::Sender<execution_ipc::host::Reply>,
}
/// Synchronous owner work selected while transport remains pending.
pub(crate) enum NetworkEvent {
    Reconcile,
    Command(Box<Command>),
    Closed,
}
pub(crate) async fn network<T>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    core: &mut Core,
    commands: &mut tokio::sync::mpsc::Receiver<Command>,
) -> Result<T, Error> {
    network_loop(future, commands, |event| {
        core.enforce_identity_gate()?;
        match event {
            NetworkEvent::Reconcile => core.reconcile()?,
            NetworkEvent::Command(command) => core.handle(*command),
            NetworkEvent::Closed => core.stopping = true,
        }
        Ok(core.stopping)
    })
    .await
}
// One synchronous callback borrows the owner exclusively; neither a task nor a lock owns journal IO.
// ref: tokio tokio-1.47.1 tokio/src/sync/mpsc/bounded.rs (single receiver, cancel-safe recv).
pub(crate) async fn network_loop<T>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    commands: &mut tokio::sync::mpsc::Receiver<Command>,
    mut owner: impl FnMut(NetworkEvent) -> Result<bool, Error>,
) -> Result<T, Error> {
    tokio::pin!(future);
    let period = std::time::Duration::from_millis(50);
    let mut timer = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let event = tokio::select! {
            result = &mut future => return result,
            command = commands.recv() => match command {
                Some(command) => NetworkEvent::Command(Box::new(command)),
                None => NetworkEvent::Closed,
            },
            _ = timer.tick() => NetworkEvent::Reconcile,
        };
        if owner(event)? {
            return Err(Error::Unavailable);
        }
    }
}
/// Native callbacks forward bounded commands to the only journal/network owner.
pub struct ServiceHandle {
    sender: tokio::sync::mpsc::Sender<Command>,
    fences: Arc<policy::Fences>,
    policy: execution_ipc::host::PeerPolicy,
    helper_policy: Option<execution_ipc::host::PeerPolicy>,
    finished: Arc<AtomicBool>,
    completion: std::sync::mpsc::Receiver<Result<(), Error>>,
    registration: Arc<Mutex<execution_ipc::host::RegistrationStatus>>,
}
impl ServiceHandle {
    pub(crate) fn service_status(&self) -> execution_ipc::host::Reply {
        self.call(
            LocalRequest::operation(execution_ipc::host::Request::ServiceStatus {}),
            String::new(),
            0,
            None,
        )
    }
    pub(crate) fn registration_status(&self) -> execution_ipc::host::Reply {
        match self.registration.lock() {
            Ok(value) => execution_ipc::host::Reply::RegistrationStatus {
                value: value.clone(),
            },
            Err(_) => execution_ipc::host::Reply::Unavailable,
        }
    }
    fn call(
        &self,
        request: LocalRequest,
        subject: String,
        session: u32,
        binding: Option<execution_contract::Id>,
    ) -> execution_ipc::host::Reply {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        if self
            .sender
            .try_send(Command {
                fence_epoch: binding
                    .as_ref()
                    .and_then(|binding| {
                        self.fences
                            .current(&policy::SessionKey::new(subject.clone(), binding.clone()))
                            .ok()
                    })
                    .unwrap_or(u64::MAX),
                request,
                subject,
                session,
                binding,
                reply,
            })
            .is_err()
        {
            return execution_ipc::host::Reply::Unavailable;
        }
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        {
            Ok(v) => v,
            Err(_) => return execution_ipc::host::Reply::Unavailable,
        };
        let started = std::time::Instant::now();
        let reply = runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(3), receiver)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(execution_ipc::host::Reply::Unavailable)
        });
        let elapsed = started.elapsed().as_millis();
        if elapsed > 1500 {
            eprintln!("RSS_IPC_OWNER_TIMING elapsedMs={elapsed}");
        }
        reply
    }
}
impl execution_ipc::host::Handler for ServiceHandle {
    fn session_fence(&self) -> Option<Arc<dyn execution_ipc::host::SessionFence>> {
        Some(Arc::new(policy::NativeFence {
            policy: self.policy.clone(),
            fences: self.fences.clone(),
            finished: self.finished.clone(),
        }))
    }
    fn peer_policy(&self) -> Option<execution_ipc::host::PeerPolicy> {
        let mut policy = self.policy.clone();
        if let Some(helper) = &self.helper_policy {
            for image in &helper.images {
                if !policy.images.iter().any(|i| {
                    i.path == image.path && i.sha256 == image.sha256 && i.cdhash == image.cdhash
                }) {
                    policy.images.push(image.clone());
                }
            }
            for subject in &helper.subjects {
                if !policy.subjects.contains(subject) {
                    policy.subjects.push(subject.clone());
                }
            }
        }
        Some(policy)
    }
    fn register_helper(
        &mut self,
        peer: &execution_ipc::host::Peer,
    ) -> Result<(), execution_app::Error> {
        peer.authenticate(
            self.helper_policy
                .as_ref()
                .ok_or(execution_app::Error::Denied)?,
        )?;
        Ok(())
    }
    fn handle(
        &mut self,
        peer: &execution_ipc::host::Peer,
        request: execution_ipc::host::Request,
    ) -> execution_ipc::host::Reply {
        match peer.authenticate(&self.policy) {
            Ok(subject) => match peer.session_binding() {
                Ok(binding) => self.call(
                    LocalRequest::operation(request),
                    subject,
                    peer.session(),
                    Some(binding),
                ),
                Err(_) => execution_ipc::host::Reply::Rejected,
            },
            Err(_) => execution_ipc::host::Reply::Rejected,
        }
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        if self.finished.load(Ordering::Acquire) {
            Err(execution_app::Error::Unavailable)
        } else {
            Ok(())
        }
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        let _ = self.call(LocalRequest::Stop, String::new(), 0, None);
        match self
            .completion
            .recv_timeout(std::time::Duration::from_secs(5))
        {
            Ok(Ok(())) => Ok(()),
            _ => Err(execution_app::Error::OutcomeUnknown),
        }
    }
}

fn trigger_subject(trigger: &BackendTrigger) -> Option<&str> {
    match trigger {
        BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. } => {
            Some(os_session.account.subject.as_str())
        }
        _ => None,
    }
}
fn same_login(trigger: &BackendTrigger, selection: &Selection) -> bool {
    match trigger {
        BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. } => {
            os_session.account.subject.as_str() == selection.subject
                && os_session.session == selection.binding
        }
        _ => false,
    }
}
impl Selection {
    fn matches(&self, offer: &execution_contract::BackendTask) -> bool {
        self.request == offer.request
            && self.task == offer.task
            && self.attempt == offer.attempt
            && self.revision == offer.revision
    }
    pub(crate) fn trigger(
        &self,
        device: &execution_contract::DeviceId,
    ) -> Result<BackendTrigger, Error> {
        let os_session = execution_contract::OsSessionRef {
            device: device.clone(),
            account: execution_contract::OsAccountRef {
                platform: plan::platform()?,
                subject: plan::id(&self.subject)?,
            },
            session: self.binding.clone(),
        };
        Ok(match &self.origin {
            execution_ipc::host::ClientOrigin::Desktop {} => BackendTrigger::Human { os_session },
            execution_ipc::host::ClientOrigin::Ai {
                config,
                conversation,
                tool_call,
                context,
            } => BackendTrigger::Ai {
                os_session,
                config: config.clone(),
                conversation: conversation.clone(),
                tool_call: tool_call.clone(),
                context: context.clone(),
            },
        })
    }
    fn from_record(record: &BackendRequest) -> Result<Self, Error> {
        let (os_session, origin) = match &record.trigger {
            BackendTrigger::Human { os_session } => {
                (os_session, execution_ipc::host::ClientOrigin::Desktop {})
            }
            BackendTrigger::Ai {
                os_session,
                config,
                conversation,
                tool_call,
                context,
            } => (
                os_session,
                execution_ipc::host::ClientOrigin::Ai {
                    config: config.clone(),
                    conversation: conversation.clone(),
                    tool_call: tool_call.clone(),
                    context: context.clone(),
                },
            ),
            _ => return Err(Error::Denied),
        };
        Ok(Self {
            request: record.action.request().clone(),
            task: record.action.offer().ok_or(Error::Denied)?.task.clone(),
            attempt: record.action.offer().ok_or(Error::Denied)?.attempt.clone(),
            revision: record.action.offer().ok_or(Error::Denied)?.revision.clone(),
            subject: os_session.account.subject.as_str().into(),
            session: os_session
                .session
                .as_str()
                .split('/')
                .nth(1)
                .ok_or(Error::Identity)?
                .parse()
                .map_err(|_| Error::Identity)?,
            binding: os_session.session.clone(),
            origin,
        })
    }
}

pub(crate) fn login_id(context: &execution_ipc::helper::UserContext) -> uuid::Uuid {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(context.binding.as_str().as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

pub(crate) fn app_config() -> AppConfig {
    let bounds = plan::storage_limits().input;
    AppConfig {
        revision: 1,
        max_rules: 1,
        max_profiles: 1,
        max_capability_entries: 32,
        max_timeout_ms: bounds.max_timeout_ms,
        max_output_bytes: bounds.max_output_bytes,
    }
}

pub(crate) fn owner_runtime() -> Result<tokio::runtime::Runtime, Error> {
    // The block_on caller remains the only SQLite/journal owner. Two fixed reactor workers
    // keep bounded transport alive while that caller performs synchronous OS fact checks.
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| Error::Unavailable)
}

/// Explicit production journal lifecycle, owned by the service composition root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionStartup {
    /// Create a new journal without replacing an existing file.
    Create,
    /// Open the existing authority and exact supported schema.
    Open,
}

pub(crate) fn assemble_journal<H: execution_app::AppHost, R: execution_app::RunnerPort>(
    path: &Path,
    startup: ProductionStartup,
    host: H,
    runner: R,
    config: AppConfig,
    limits: execution_sqlite::Limits,
) -> Result<ExecutionApp<H, R, execution_sqlite::Store>, execution_app::Error> {
    // Validate production inputs before any database access, including read-only inspection.
    execution_app::Configuration::new(config, limits.input)?;
    let binding = host.service_binding()?;
    if matches!(
        binding.authority,
        execution_contract::Authority::Test { .. }
    ) || runner.mode() != execution_lifecycle::ExecutionMode::Real
    {
        return Err(execution_app::Error::Unbound);
    }
    host.reliable_now()?;
    let journal = match startup {
        ProductionStartup::Create => {
            execution_sqlite::Store::initialize_production(path, binding.authority, limits)?
        }
        ProductionStartup::Open => {
            match execution_sqlite::Store::open(path, &binding.authority, limits)? {
                execution_sqlite::OpenOutcome::Ready(journal) => *journal,
                execution_sqlite::OpenOutcome::UnsupportedSchema { found, supported } => {
                    return Err(execution_app::Error::UnsupportedSchema { found, supported });
                }
            }
        }
    };
    ExecutionApp::new(journal, host, runner, config)
}

#[cfg(all(test, target_os = "macos"))]
#[path = "product_gate_tests.rs"]
pub(crate) mod product_gate_tests;

fn is_policy_offer(offer: &Offer) -> bool {
    matches!(offer.payload(), wire::TaskPayload::Enrollment(p)
        if matches!(p.authorization, wire::TaskAuthorization::SelfService { .. }))
        || matches!(offer.payload(), wire::TaskPayload::Script(p)
        if matches!(p.authorization, wire::TaskAuthorization::SelfService { .. }))
        || matches!(offer.payload(), wire::TaskPayload::Software(p)
            if matches!(p.authorization, wire::TaskAuthorization::SelfService { .. }))
}

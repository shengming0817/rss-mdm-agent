//! Desktop settings intents share the production execution owner and journal.
use super::*;
use crate::local_settings::{self, Action};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply};
fn metadata<T: serde::de::DeserializeOwned>(plan: &FrozenExecution, key: &str) -> Result<T, Error> {
    let Some(InputValue::Literal { value }) = plan.spec().request.parameters.get(key) else {
        return Err(Error::Untrusted);
    };
    serde_json::from_value(value.clone()).map_err(|_| Error::Untrusted)
}
impl Core {
    pub(super) fn capture_settings(&mut self, request: &RequestId) -> Result<(), Error> {
        let frozen = self
            .app
            .service_frozen_input(request)
            .map_err(map_app_error)?;
        if frozen.spec().launch.interpreter.profile.id.as_str() != local_settings::PROFILE {
            return Ok(());
        }
        let response = self.settings_status(&frozen)?;
        if response.finished {
            if let Initiator::Human { os_session } = &frozen.spec().request.initiator {
                if let Some(snapshot) = &response.snapshot {
                    self.data.settings_write(
                        os_session.account.subject.as_str(),
                        &os_session.session,
                        snapshot,
                    )?;
                }
            }
            let path = local_settings::control_path(&self.config, request)?;
            if path.exists() {
                installation_security::protected(&path).map_err(|_| Error::Denied)?;
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
    fn settings_permissions(&self, mut response: SettingsResponse) -> SettingsResponse {
        if let Some(snapshot) = &response.snapshot {
            response.write_allowed =
                self.software_policy()
                    .is_ok_and(|policy| match snapshot.section {
                        SettingsSection::Display => policy.rules.allow_display,
                        SettingsSection::Network => policy.rules.allow_proxy,
                        SettingsSection::Power | SettingsSection::Lock => policy.rules.allow_power,
                        _ => false,
                    });
            if !response.write_allowed
                && matches!(
                    snapshot.section,
                    SettingsSection::Display
                        | SettingsSection::Network
                        | SettingsSection::Power
                        | SettingsSection::Lock
                )
                && response.reason.is_none()
            {
                response.reason = Some("管理员尚未委派此项修改权限".into());
            }
        }
        response
    }
    pub(super) fn settings_operation(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        input: &SettingsOperation,
    ) -> Result<Reply, Error> {
        self.data_context(command, context)?;
        if !matches!(origin, ClientOrigin::Desktop {}) {
            return Err(Error::Denied);
        }
        self.require_origin_context(
            &command.subject,
            command.binding.as_ref().ok_or(Error::Denied)?,
            origin,
        )?;
        let (request, action) = match input {
            SettingsOperation::Snapshot { section } => {
                return Ok(Reply::Settings {
                    value: self.settings_permissions(SettingsResponse {
                        write_allowed: false,
                        request: None,
                        digest: None,
                        finished: true,
                        effect: SettingsEffect::Observed,
                        snapshot: self.data.settings_read(
                            &command.subject,
                            command.binding.as_ref().ok_or(Error::Denied)?,
                            *section,
                        )?,
                        keep_before_unix_ms: None,
                        reason: None,
                    }),
                });
            }
            SettingsOperation::Status { request }
            | SettingsOperation::Cancel { request }
            | SettingsOperation::KeepDisplay { request, .. } => {
                let frozen = self.settings_input(command, context, request)?;
                if matches!(input, SettingsOperation::Cancel { .. }) {
                    if matches!(
                        metadata::<Action>(&frozen, "local-settings-action")?,
                        Action::Change {
                            patch: SettingsPatch::Display { .. },
                            ..
                        }
                    ) {
                        match local_settings::decide(&self.config, request, false) {
                            Ok(()) | Err(Error::Conflict) => {}
                            Err(error) => return Err(error),
                        }
                    }
                    self.app
                        .cancel(&self.local_caller(), request)
                        .map_err(map_app_error)?;
                }
                if let SettingsOperation::KeepDisplay { digest, .. } = input {
                    let policy = self.software_policy()?;
                    let action: Action = metadata(&frozen, "local-settings-action")?;
                    local_settings::Permit::new(frozen.clone(), policy, &action)?
                        .verify(&frozen, self.host.clock.millis()?)?;
                    if digest != frozen.digest() {
                        return Err(Error::Conflict);
                    }
                    let response = self.settings_status(&frozen)?;
                    if response.effect != SettingsEffect::Preview
                        || response.keep_before_unix_ms.is_none_or(|deadline| {
                            self.host.clock.millis().map_or(true, |now| now >= deadline)
                        })
                    {
                        return Err(Error::Expired);
                    }
                    let fences = self.fences.clone();
                    let key = policy::SessionKey::new(
                        command.subject.clone(),
                        command.binding.clone().ok_or(Error::Denied)?,
                    );
                    let _guard = fences.admit(&key, command.fence_epoch)?;
                    if self
                        .app
                        .status(&self.local_caller(), request)
                        .map_err(map_app_error)?
                        .cancel_requested
                    {
                        return Err(Error::Denied);
                    }
                    local_settings::decide(&self.config, request, true)?;
                }
                self.app.reconcile(request).map_err(map_app_error)?;
                let response = self.settings_status(&frozen)?;
                if response.finished {
                    if let Some(snapshot) = &response.snapshot {
                        self.data.settings_write(
                            &command.subject,
                            command.binding.as_ref().ok_or(Error::Denied)?,
                            snapshot,
                        )?;
                    }
                }
                return Ok(Reply::Settings {
                    value: self.settings_permissions(response),
                });
            }
            SettingsOperation::Refresh { request, section } => {
                (request, Action::Read { section: *section })
            }
            SettingsOperation::OpenNative { request, section } => {
                (request, Action::Open { section: *section })
            }
            SettingsOperation::Change {
                request,
                expected,
                patch,
            } => {
                let before = if self
                    .app
                    .has_service_execution(request, &self.host.root.device)
                    .map_err(map_app_error)?
                {
                    let original = self.settings_input(command, context, request)?;
                    match metadata::<Action>(&original, "local-settings-action")? {
                        Action::Change { before, .. } => before,
                        _ => return Err(Error::Conflict),
                    }
                } else {
                    self.data
                        .settings_read(
                            &command.subject,
                            command.binding.as_ref().ok_or(Error::Denied)?,
                            patch.section(),
                        )?
                        .ok_or(Error::Unavailable)?
                };
                (
                    request,
                    Action::Change {
                        expected: expected.clone(),
                        before,
                        patch: patch.clone(),
                    },
                )
            }
        };
        uuid::Uuid::parse_str(request.as_str()).map_err(|_| Error::Protocol)?;
        // Global request IDs remain unique across local and enterprise namespaces.
        if self
            .app
            .has_service_execution(request, &self.host.root.device)
            .map_err(map_app_error)?
        {
            let frozen = self.settings_input(command, context, request)?;
            let original: Action = metadata(&frozen, "local-settings-action")?;
            let same = match (&action, &original) {
                (
                    Action::Change {
                        expected: a,
                        patch: b,
                        ..
                    },
                    Action::Change {
                        expected: c,
                        patch: d,
                        ..
                    },
                ) => a == c && b == d,
                _ => action == original,
            };
            if !same {
                return Err(Error::Conflict);
            }
            let response = self.settings_admit(command, &frozen, &original)?;
            return Ok(Reply::Settings {
                value: self.settings_permissions(response),
            });
        }
        if self.stopping || self.host.local_stopped.load(Ordering::Acquire) {
            return Err(Error::Denied);
        }
        let now = self.host.clock.millis()?;
        if let Action::Change {
            expected,
            before,
            patch,
        } = &action
        {
            if !patch.valid()
                || before.revision != *expected
                || now.saturating_sub(before.collected_at_unix_ms) > 30_000
            {
                return Err(Error::Conflict);
            }
        }
        let policy = self.software_policy()?;
        // User visual state and Windows WinINet always execute in the exact attested login.
        let needs_user = matches!(action, Action::Open { .. })
            || matches!(
                action.section(),
                SettingsSection::Display | SettingsSection::Sound | SettingsSection::Lock
            )
            || cfg!(windows) && action.section() == SettingsSection::Network;
        let helper = if needs_user {
            Some(self.helpers.connect_peer(
                &command.subject,
                command.session,
                command.binding.as_ref().ok_or(Error::Denied)?,
            )?)
        } else {
            None
        };
        let initiator = self.software_initiator(command, origin)?;
        let (frozen, artifacts) = local_settings::compile::prepare(
            action.clone(),
            request.clone(),
            &self.host.root,
            &self.host.local_actor,
            initiator,
            context,
            &policy,
            now,
            &self.config,
            &mut self.material_root,
            helper,
        )?;
        self.host
            .materials
            .register(&frozen, artifacts)
            .map_err(map_app_error)?;
        let response = self.settings_admit(command, &frozen, &action)?;
        Ok(Reply::Settings {
            value: self.settings_permissions(response),
        })
    }
    fn settings_admit(
        &mut self,
        command: &Command,
        frozen: &FrozenExecution,
        action: &Action,
    ) -> Result<SettingsResponse, Error> {
        let request = &frozen.spec().request.request_id;
        let status = self.app.status(&self.local_caller(), request);
        if status
            .as_ref()
            .is_ok_and(|s| s.attempts > 0 || s.cancel_requested || s.admission.is_some())
        {
            return self.settings_status(frozen);
        }
        let policy = self.software_policy()?;
        let permit = local_settings::Permit::new(frozen.clone(), policy, action)?;
        permit.verify(frozen, self.host.clock.millis()?)?;
        if !self
            .host
            .materials
            .registered(frozen)
            .map_err(map_app_error)?
        {
            let artifacts = crate::recovery::materials(frozen, &self.config, &self.helpers)?;
            self.host
                .materials
                .register(frozen, artifacts)
                .map_err(map_app_error)?;
        }
        let fences = self.fences.clone();
        let key = policy::SessionKey::new(
            command.subject.clone(),
            command.binding.clone().ok_or(Error::Denied)?,
        );
        let _guard = fences.admit(&key, command.fence_epoch)?;
        *self
            .host
            .local_settings
            .lock()
            .map_err(|_| Error::Unavailable)? = Some(permit);
        let result = (|| {
            let status = self
                .app
                .request_execution(&self.local_caller(), frozen)
                .map_err(map_app_error)?;
            if status.phase == execution_app::TaskPhase::ConfirmationRequired {
                self.app
                    .confirm_execution(&self.local_caller(), request, frozen.digest(), true)
                    .map_err(map_app_error)?;
            }
            self.settings_status(frozen)
        })();
        *self
            .host
            .local_settings
            .lock()
            .map_err(|_| Error::Unavailable)? = None;
        result
    }
    fn settings_input(
        &self,
        command: &Command,
        context: &NativeContextRef,
        request: &RequestId,
    ) -> Result<FrozenExecution, Error> {
        let plan = self
            .app
            .frozen_input(&self.local_caller(), request)
            .map_err(map_app_error)?;
        if plan.spec().launch.interpreter.profile.id.as_str() != local_settings::PROFILE
            || metadata::<NativeContextRef>(&plan, "local-settings-context")? != *context
        {
            return Err(Error::Denied);
        }
        let Initiator::Human { os_session } = &plan.spec().request.initiator else {
            return Err(Error::Denied);
        };
        if os_session.account.subject.as_str() != command.subject
            || command.binding.as_ref() != Some(&os_session.session)
            || os_session.device != self.host.root.device
        {
            return Err(Error::Denied);
        }
        Ok(plan)
    }
    fn settings_status(&mut self, plan: &FrozenExecution) -> Result<SettingsResponse, Error> {
        let request = &plan.spec().request.request_id;
        let status = self
            .app
            .status(&self.local_caller(), request)
            .map_err(map_app_error)?;
        let mut response = SettingsResponse {
            write_allowed: false,
            request: Some(request.clone()),
            digest: Some(plan.digest().clone()),
            finished: status.cancel_requested && status.attempts == 0,
            effect: if status.cancel_requested && status.attempts == 0 {
                SettingsEffect::Cancelled
            } else if status.attempts == 0 {
                SettingsEffect::Pending
            } else {
                SettingsEffect::Unknown
            },
            snapshot: None,
            keep_before_unix_ms: None,
            reason: None,
        };
        if let Some(facts) = self
            .app
            .service_process_evidence(request)
            .map_err(map_app_error)?
        {
            if facts.content_digest != *plan.digest() {
                return Err(Error::Untrusted);
            }
            let rows = facts
                .stdout
                .split(|b| *b == b'\n')
                .filter(|row| !row.is_empty())
                .collect::<Vec<_>>();
            if rows.len() > 2 {
                return Err(Error::Protocol);
            }
            for row in rows {
                let Ok(value) = serde_json::from_slice::<SettingsResponse>(row) else {
                    continue;
                };
                if value.request.as_ref() != Some(request) {
                    return Err(Error::Untrusted);
                }
                if !value.finished && value.effect == SettingsEffect::Preview && !facts.finished {
                    response = value;
                } else if value.finished
                    && facts.finished
                    && facts.quiescent
                    && facts.quality != OutputQuality::Truncated
                    && facts.total_output_bytes <= plan.spec().budget.total_output_bytes
                    && serde_json::from_slice::<CollectionCompletion>(&facts.stderr).is_ok_and(
                        |proof| {
                            proof.closed
                                && proof.input_sha256 == plan.spec().launch.artifact.sha256.as_str()
                                && proof.output_sha256
                                    == crate::local_execution::hash(&facts.stdout)
                                        .map(|h| h.as_str().to_owned())
                                        .unwrap_or_default()
                        },
                    )
                {
                    response = value;
                }
            }
            if facts.finished && !response.finished {
                response.effect = SettingsEffect::Unknown;
                response.finished = facts.quiescent;
                response.reason = Some("执行已中断或缺少完整回读，当前效果无法确认".into());
            }
            if response.finished {
                let assessment = match response.effect {
                    SettingsEffect::Applied => execution_lifecycle::EffectAssessment::Satisfied,
                    SettingsEffect::Observed
                    | SettingsEffect::OpenRequested
                    | SettingsEffect::Restored => execution_lifecycle::EffectAssessment::NoEffect,
                    SettingsEffect::Conflict => execution_lifecycle::EffectAssessment::NotSatisfied,
                    _ => execution_lifecycle::EffectAssessment::Unknown,
                };
                self.assess_local(plan, facts, assessment)?;
            }
        }
        response.request = Some(request.clone());
        response.digest = Some(plan.digest().clone());
        Ok(response)
    }
}

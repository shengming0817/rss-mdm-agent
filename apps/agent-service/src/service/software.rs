//! Software facts and local intents are owned by the same authenticated system service.
use super::*;
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply};
use serde_json::Value;
use uuid::Uuid;

fn text<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
}
impl Core {
    fn select_local_software(
        &self,
        item: &InstalledSoftware,
        row: &Value,
        action: LocalSoftwareAction,
    ) -> Result<crate::local_software::native::Target, Error> {
        #[cfg(test)]
        if let Some(select) = self.local_target_fixture {
            return Ok(select(item));
        }
        crate::local_software::native::select(item, row, action)
    }

    pub(super) fn software_policy(&self) -> Result<crate::local_permissions::Policy, Error> {
        #[cfg(test)]
        if let Some(open) = self.local_policy_fixture {
            return open(&self.config.local_permissions);
        }
        crate::local_permissions::Policy::open(&self.config.local_permissions)
    }
    pub(super) fn read_software(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        input: &SoftwareRead,
    ) -> Result<Reply, Error> {
        self.data_context(command, context)?;
        if matches!(origin, ClientOrigin::Ai {context: ai, ..} if ai != context) {
            return Err(Error::Identity);
        }
        let binding = command.binding.as_ref().ok_or(Error::Denied)?;
        self.data
            .authorize_read(&input.read, &command.subject, binding)?;
        let (data, snapshot) = self.data.read_page(
            &input.read,
            &command.subject,
            binding,
            self.host.clock.millis()? / 1000,
        )?;
        let descriptor = self
            .local_directory(command)?
            .into_iter()
            .find(|d| d.reference == data.query)
            .ok_or(Error::Unsupported)?;
        if !descriptor.domains.iter().any(|d| {
            matches!(
                d.as_str(),
                "software.installations" | "software.receipts" | "software.packages"
            )
        }) {
            return Err(Error::Unsupported);
        }
        let offset = input.read.after.as_ref().map_or(0, |c| c.offset);
        let policy = self.software_policy().ok();
        let usable =
            !data.stale && data.quality == DataQuality::Complete && data.enumeration_complete;
        let mut items = Vec::new();
        for (index, row) in data.rows.iter().enumerate() {
            if let Some(mut item) = software_item(
                row,
                &data.query,
                &snapshot,
                offset + index as u32,
                &command.subject,
            )? {
                item.actions =
                    software_actions(&item, row, policy.as_ref().map(|p| &p.rules), usable);
                items.push(item);
            }
        }
        Ok(Reply::SoftwarePage {
            value: SoftwarePage {
                query: data.query,
                collected_at: data.collected_at,
                quality: data.quality,
                enumeration_complete: data.enumeration_complete,
                stale: data.stale,
                items,
                next: data.next,
            },
        })
    }
}

pub(super) fn software_item(
    row: &Value,
    query: &QueryRef,
    snapshot: &Digest,
    offset: u32,
    subject: &str,
) -> Result<Option<InstalledSoftware>, Error> {
    let scope = match (text(row, "scope"), text(row, "owner")) {
        (Some("device"), None) => DataScope::Device,
        (Some("user"), Some(owner)) if owner == subject => DataScope::User {
            subject: plan::id(subject)?,
        },
        (Some("user"), _) => return Ok(None),
        _ => return Err(Error::Untrusted),
    };
    let origin = text(row, "origin").ok_or(Error::Untrusted)?;
    let kind = match origin {
        "application" => InstalledSoftwareKind::Application,
        "pkg_receipt" => InstalledSoftwareKind::Pkg,
        "homebrew" => InstalledSoftwareKind::Homebrew,
        value if value.starts_with("win32.uninstall.") => InstalledSoftwareKind::WindowsProgram,
        value if value.starts_with("msix") => InstalledSoftwareKind::Msix,
        _ => return Err(Error::Unsupported),
    };
    let native_id = text(row, "native_id").ok_or(Error::Untrusted)?.to_owned();
    let reference = SoftwareRef {
        query: query.clone(),
        cursor: DataCursor {
            snapshot: snapshot.clone(),
            offset,
        },
        installation: plan::id(text(row, "id").ok_or(Error::Untrusted)?)?,
    };
    Ok(Some(InstalledSoftware {
        reference,
        kind,
        name: text(row, "name")
            .unwrap_or(&native_id)
            .chars()
            .take(256)
            .collect(),
        version: text(row, "version").map(str::to_owned),
        native_id,
        origin: origin.into(),
        scope,
        publisher: text(row, "publisher")
            .or_else(|| text(row, "team_id"))
            .map(str::to_owned),
        actions: Vec::new(),
    }))
}

impl Core {
    pub(super) fn software_initiator(
        &self,
        command: &Command,
        origin: &ClientOrigin,
    ) -> Result<Initiator, Error> {
        let binding = command.binding.as_ref().ok_or(Error::Denied)?;
        self.require_origin_context(&command.subject, binding, origin)?;
        let os_session = OsSessionRef {
            device: self.host.root.device.clone(),
            account: OsAccountRef {
                platform: plan::platform()?,
                subject: plan::id(&command.subject)?,
            },
            session: binding.clone(),
        };
        Ok(match origin {
            ClientOrigin::Desktop {} => Initiator::Human { os_session },
            ClientOrigin::Ai {
                config,
                conversation,
                tool_call,
                ..
            } => Initiator::Ai {
                provider: plan::id("local-ai-host")?,
                os_session,
                config: config.clone(),
                conversation: conversation.clone(),
                tool_call: tool_call.clone(),
            },
        })
    }
    fn resolve_software(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        reference: &SoftwareRef,
    ) -> Result<(InstalledSoftware, Value), Error> {
        let input = SoftwareRead {
            read: DataRead {
                selection: DataSelection::Query {
                    query: reference.query.clone(),
                },
                after: Some(reference.cursor.clone()),
                limit: 1,
                max_age_seconds: Some(300),
            },
        };
        let Reply::SoftwarePage { value } = self.read_software(command, context, origin, &input)?
        else {
            return Err(Error::Protocol);
        };
        if value.stale || value.quality != DataQuality::Complete || !value.enumeration_complete {
            return Err(Error::Conflict);
        }
        let item = value
            .items
            .into_iter()
            .next()
            .filter(|v| v.reference == *reference)
            .ok_or(Error::Conflict)?;
        let data = self.data.read(
            &input.read,
            &command.subject,
            command.binding.as_ref().ok_or(Error::Denied)?,
            self.host.clock.millis()? / 1000,
        )?;
        Ok((item, data.rows.into_iter().next().ok_or(Error::NotFound)?))
    }
    fn local_software_input(
        &self,
        command: &Command,
        context: &NativeContextRef,
        request: &RequestId,
        mutating: bool,
    ) -> Result<FrozenExecution, Error> {
        self.data_context(command, context)?;
        let frozen = self
            .app
            .frozen_input(&self.local_caller(), request)
            .map_err(map_app_error)?;
        let session = match &frozen.spec().request.initiator {
            Initiator::Human { os_session } | Initiator::Ai { os_session, .. } => os_session,
            _ => return Err(Error::Denied),
        };
        if !frozen
            .spec()
            .policy
            .id
            .as_str()
            .starts_with("local-software:")
            || session.account.subject.as_str() != command.subject
            || session.device != self.host.root.device
        {
            return Err(Error::Denied);
        }
        let original: NativeContextRef = parameter(&frozen, "local-software-context")?;
        if original != *context {
            return Err(Error::Identity);
        }
        if mutating {
            if self.stopping
                || self.host.local_stopped.load(Ordering::Acquire)
                || command.binding.as_ref() != Some(&session.session)
            {
                return Err(Error::Denied);
            }
        }
        Ok(frozen)
    }
    pub(super) fn software_operation(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        input: &LocalSoftwareOperation,
    ) -> Result<Reply, Error> {
        self.data_context(command, context)?;
        match input {
            LocalSoftwareOperation::History(query) => {
                self.software_initiator(command, origin)?;
                if query.limit == 0 || query.limit > 128 {
                    return Err(Error::Protocol);
                }
                let page = self
                    .app
                    .service_tasks(query.after.as_ref(), usize::from(query.limit))
                    .map_err(map_app_error)?;
                let mut items = Vec::new();
                for details in page.items {
                    let matches = details
                        .action
                        .policy
                        .id
                        .as_str()
                        .starts_with("local-software:")
                        && match &details.action.initiator {
                            Initiator::Human { os_session } | Initiator::Ai { os_session, .. } => {
                                os_session.account.subject.as_str() == command.subject
                            }
                            _ => false,
                        };
                    if !matches {
                        continue;
                    }
                    let frozen = self.local_software_input(
                        command,
                        context,
                        &details.status.operation_request_id,
                        false,
                    )?;
                    if let Reply::SoftwareStatus { value } = self.software_status(&frozen)? {
                        items.push(value);
                    }
                }
                Ok(Reply::SoftwareHistory {
                    value: execution_app::LocalSoftwareHistory {
                        items,
                        next: page.next,
                    },
                })
            }
            LocalSoftwareOperation::Prepare(choice) => {
                if self.stopping || self.host.local_stopped.load(Ordering::Acquire) {
                    return Err(Error::Denied);
                }
                let id = Uuid::parse_str(choice.request.as_str()).map_err(|_| Error::Protocol)?;
                if id.is_nil() || id.to_string() != choice.request.as_str() {
                    return Err(Error::Protocol);
                }
                let initiator = self.software_initiator(command, origin)?;
                if self
                    .app
                    .has_service_execution(&choice.request, &self.host.root.device)
                    .map_err(map_app_error)?
                {
                    let frozen =
                        self.local_software_input(command, context, &choice.request, true)?;
                    let original: LocalSoftwareChoice =
                        parameter(&frozen, "local-software-choice")?;
                    if original != *choice || frozen.spec().request.initiator != initiator {
                        return Err(Error::Conflict);
                    }
                    return Ok(Reply::SoftwarePrepared {
                        value: software_preview(&frozen)?,
                    });
                }
                let (item, row) =
                    self.resolve_software(command, context, origin, &choice.software)?;
                let policy = self.software_policy()?;
                if policy.rules.protects(&item)
                    && matches!(
                        choice.action,
                        LocalSoftwareAction::Update | LocalSoftwareAction::Uninstall
                    )
                {
                    return Err(Error::Denied);
                }
                if choice.action == LocalSoftwareAction::Update {
                    return Err(Error::Unsupported);
                }
                let helper = if matches!(item.scope, DataScope::User { .. })
                    || choice.action == LocalSoftwareAction::Launch
                {
                    Some(self.helpers.connect_peer(
                        &command.subject,
                        command.session,
                        command.binding.as_ref().ok_or(Error::Denied)?,
                    )?)
                } else {
                    None
                };
                let target = self.select_local_software(&item, &row, choice.action)?;
                let (frozen, artifacts) = crate::local_software::compile::native(
                    target,
                    choice,
                    &item,
                    &self.host.root,
                    &self.host.local_actor,
                    initiator,
                    context,
                    &policy,
                    self.host.clock.millis()?,
                    &self.config,
                    &mut self.material_root,
                    helper,
                )?;
                policy.current()?;
                self.data_context(command, context)?;
                self.host
                    .materials
                    .register(&frozen, artifacts)
                    .map_err(map_app_error)?;
                let scope = policy::SessionKey::new(
                    command.subject.clone(),
                    command.binding.clone().ok_or(Error::Denied)?,
                );
                let fences = self.fences.clone();
                let _fence = fences.admit(&scope, command.fence_epoch)?;
                let staged = self.app.stage_execution(&self.local_caller(), &frozen);
                // Preparation has no running attempt. Keep the canonical input in the journal;
                // submission reconstructs and rechecks materials instead of pinning registry capacity.
                self.host
                    .materials
                    .retire(frozen.digest())
                    .map_err(map_app_error)?;
                staged.map_err(map_app_error)?;
                Ok(Reply::SoftwarePrepared {
                    value: software_preview(&frozen)?,
                })
            }
            LocalSoftwareOperation::Submit(submit) => {
                if !matches!(origin, ClientOrigin::Desktop {}) {
                    return Err(Error::Denied);
                }
                let frozen = self.local_software_input(command, context, &submit.request, true)?;
                if frozen.digest() != &submit.digest {
                    return Err(Error::Conflict);
                }
                let before = self
                    .app
                    .status(&self.local_caller(), &submit.request)
                    .map_err(map_app_error)?;
                // Once admitted, reconciliation owns recovery. No source refresh can start it again.
                if before.attempts > 0 || before.cancel_requested || before.admission.is_some() {
                    return self.software_status(&frozen);
                }
                let preview = software_preview(&frozen)?;
                if self.host.clock.millis()? >= preview.start_before_unix_ms {
                    return Err(Error::Expired);
                }
                let (item, row) =
                    self.resolve_software(command, context, origin, &preview.choice.software)?;
                let policy = self.software_policy()?;
                if (policy.rules.protects(&item)
                    && matches!(
                        preview.choice.action,
                        LocalSoftwareAction::Update | LocalSoftwareAction::Uninstall
                    ))
                    || frozen.spec().policy.revision.as_str() != policy.revision.as_str()
                {
                    return Err(Error::Denied);
                }
                if preview.choice.action == LocalSoftwareAction::Update {
                    return Err(Error::Unsupported);
                }
                let helper = if matches!(frozen.spec().run_as, RunAs::User { .. }) {
                    Some(self.helpers.connect_peer(
                        &command.subject,
                        command.session,
                        command.binding.as_ref().ok_or(Error::Denied)?,
                    )?)
                } else {
                    None
                };
                if frozen.spec().launch.interpreter.profile.id.as_str()
                    != crate::local_software::native::PROFILE
                    || !crate::local_software::native::allowed(&policy.rules, preview.choice.action)
                {
                    return Err(Error::Denied);
                }
                let original: crate::local_software::native::Target =
                    parameter(&frozen, "local-software-native")?;
                if self.select_local_software(&item, &row, preview.choice.action)? != original {
                    return Err(Error::Conflict);
                }
                let artifacts = crate::local_software::compile::native_materials(
                    &frozen,
                    &self.config,
                    helper,
                )?;
                if self
                    .host
                    .materials
                    .registered(&frozen)
                    .map_err(map_app_error)?
                {
                    self.host
                        .materials
                        .inspect(&frozen)
                        .map_err(map_app_error)?;
                } else {
                    self.host
                        .materials
                        .register(&frozen, artifacts)
                        .map_err(map_app_error)?;
                }
                let permit = Arc::new(crate::local_software::Permit::new(frozen.clone(), policy)?);
                let scope = policy::SessionKey::new(
                    command.subject.clone(),
                    command.binding.clone().ok_or(Error::Denied)?,
                );
                let fences = self.fences.clone();
                let _fence = fences.admit(&scope, command.fence_epoch)?;
                *self
                    .host
                    .local_software
                    .lock()
                    .map_err(|_| Error::Unavailable)? = Some(permit);
                let result = (|| {
                    let status = self
                        .app
                        .request_execution(&self.local_caller(), &frozen)
                        .map_err(map_app_error)?;
                    if status.phase == execution_app::TaskPhase::ConfirmationRequired {
                        self.app
                            .confirm_execution(
                                &self.local_caller(),
                                &submit.request,
                                &submit.digest,
                                true,
                            )
                            .map_err(map_app_error)?;
                    }
                    self.software_status(&frozen)
                })();
                *self
                    .host
                    .local_software
                    .lock()
                    .map_err(|_| Error::Unavailable)? = None;
                result
            }
            LocalSoftwareOperation::Status(query) | LocalSoftwareOperation::Cancel(query) => {
                let frozen = self.local_software_input(command, context, &query.request, false)?;
                if matches!(input, LocalSoftwareOperation::Cancel(_)) {
                    self.app
                        .cancel(&self.local_caller(), &query.request)
                        .map_err(map_app_error)?;
                }
                self.app.reconcile(&query.request).map_err(map_app_error)?;
                self.software_status(&frozen)
            }
        }
    }
    fn software_status(&self, frozen: &FrozenExecution) -> Result<Reply, Error> {
        let detected = if frozen.spec().launch.interpreter.profile.id.as_str()
            == crate::local_software::native::PROFILE
        {
            self.app
                .service_process_evidence(&frozen.spec().request.request_id)
                .map_err(map_app_error)?
                .filter(|p| {
                    p.finished
                        && p.quiescent
                        && p.exit_code == Some(0)
                        && p.quality == OutputQuality::Complete
                })
                .and_then(|p| serde_json::from_slice::<SoftwareWorkerResult>(&p.stdout).ok())
                .filter(|r| r.closed)
                .and_then(|r| r.detected)
        } else {
            None
        };
        Ok(Reply::SoftwareStatus {
            value: execution_app::LocalSoftwareStatus {
                detected,
                prepared: software_preview(frozen)?,
                execution: self
                    .app
                    .status(&self.local_caller(), &frozen.spec().request.request_id)
                    .map_err(map_app_error)?,
            },
        })
    }
}
fn parameter<T: serde::de::DeserializeOwned>(
    frozen: &FrozenExecution,
    key: &str,
) -> Result<T, Error> {
    let Some(InputValue::Literal { value }) = frozen.spec().request.parameters.get(key) else {
        return Err(Error::Untrusted);
    };
    serde_json::from_value(value.clone()).map_err(|_| Error::Untrusted)
}
fn software_preview(frozen: &FrozenExecution) -> Result<LocalSoftwarePrepared, Error> {
    Ok(LocalSoftwarePrepared {
        request: frozen.spec().request.request_id.clone(),
        digest: frozen.digest().clone(),
        choice: parameter(frozen, "local-software-choice")?,
        name: parameter(frozen, "local-software-name")?,
        initiator: frozen.spec().request.initiator.clone(),
        identity: match frozen.spec().run_as {
            RunAs::System { .. } => BackendIdentity::System,
            RunAs::User { .. } => BackendIdentity::User,
        },
        target_version: parameter(frozen, "local-software-target-version")?,
        start_before_unix_ms: frozen
            .spec()
            .validity
            .expires_at_unix_ms
            .saturating_sub(frozen.spec().budget.total_timeout_ms),
    })
}

fn software_actions(
    item: &InstalledSoftware,
    row: &Value,
    rules: Option<&crate::local_permissions::Restrictions>,
    usable: bool,
) -> Vec<SoftwareActionView> {
    let native = match item.kind {
        InstalledSoftwareKind::Application => {
            item.publisher.is_some()
                && item.version.is_some()
                && text(row, "signature_status") == Some("signed")
                && text(row, "location")
                    .is_some_and(|p| std::path::Path::new(p).is_absolute() && p.ends_with(".app"))
        }
        InstalledSoftwareKind::WindowsProgram => {
            item.native_id.starts_with('{')
                && item.native_id.ends_with('}')
                && Uuid::parse_str(&item.native_id).is_ok()
                && item
                    .publisher
                    .as_ref()
                    .is_some_and(|p| !p.starts_with("Microsoft"))
        }
        InstalledSoftwareKind::Msix => {
            matches!(item.scope, DataScope::User { .. }) && item.origin == "msix.registered"
        }
        _ => false,
    };
    [
        LocalSoftwareAction::Launch,
        LocalSoftwareAction::Update,
        LocalSoftwareAction::Uninstall,
        LocalSoftwareAction::Detect,
    ]
    .into_iter()
    .map(|action| {
        let reason = if !usable {
            Some(SoftwareBlock::ObservationUnavailable)
        } else if let Some(rules) = rules {
            if action == LocalSoftwareAction::Uninstall
                && item.kind == InstalledSoftwareKind::Application
                && matches!(item.scope, DataScope::Device)
                && text(row, "location").is_none_or(|p| {
                    std::path::Path::new(p).parent() != Some(std::path::Path::new("/Applications"))
                })
            {
                Some(SoftwareBlock::Unsupported)
            } else if rules.protects(item)
                && matches!(
                    action,
                    LocalSoftwareAction::Update | LocalSoftwareAction::Uninstall
                )
            {
                Some(SoftwareBlock::Protected)
            } else if (action == LocalSoftwareAction::Launch && !rules.allow_launch)
                || (action == LocalSoftwareAction::Uninstall && !rules.allow_native_uninstall)
            {
                Some(SoftwareBlock::RuleDisabled)
            } else if action == LocalSoftwareAction::Update {
                Some(SoftwareBlock::UpdateUnknown)
            } else if action == LocalSoftwareAction::Launch
                && !matches!(
                    item.kind,
                    InstalledSoftwareKind::Application | InstalledSoftwareKind::Msix
                )
            {
                Some(SoftwareBlock::NoGui)
            } else if !native {
                Some(SoftwareBlock::Unsupported)
            } else {
                None
            }
        } else {
            Some(SoftwareBlock::RuleDisabled)
        };
        SoftwareActionView {
            action,
            available: reason.is_none(),
            reason,
            target_version: None,
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn query() -> QueryRef {
        QueryRef {
            id: plan::id("native:software").unwrap(),
            revision: Digest::new("11".repeat(32)).unwrap(),
        }
    }
    #[test]
    fn device_inventory_does_not_expose_other_users_installations() {
        let row = json!({"id":"installation","native_id":"package","name":"App","version":"1","scope":"user","owner":"other-user","origin":"msix.registered"});
        assert!(software_item(
            &row,
            &query(),
            &Digest::new("22".repeat(32)).unwrap(),
            0,
            "current-user"
        )
        .unwrap()
        .is_none());
    }
    #[test]
    fn application_reference_freezes_the_snapshot_position_without_accepting_paths_as_input() {
        let row = json!({"id":"installation","native_id":"com.example.app","name":"App","version":"1","scope":"device","owner":"","origin":"application","location":"/Applications/App.app"});
        let snapshot = Digest::new("22".repeat(32)).unwrap();
        let item = software_item(&row, &query(), &snapshot, 129, "current-user")
            .unwrap()
            .unwrap();
        assert_eq!(item.reference.cursor.snapshot, snapshot);
        assert_eq!(item.reference.cursor.offset, 129);
        assert_eq!(item.kind, InstalledSoftwareKind::Application);
        assert_eq!(item.scope, DataScope::Device);
        assert!(serde_json::to_string(&item)
            .unwrap()
            .find("/Applications/")
            .is_none());
    }
    #[test]
    fn a_receipt_is_not_a_gui_application() {
        let row = json!({"id":"receipt","native_id":"com.example.pkg","name":"Package","version":"1","scope":"device","owner":"","origin":"pkg_receipt"});
        let item = software_item(
            &row,
            &query(),
            &Digest::new("22".repeat(32)).unwrap(),
            0,
            "user",
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.kind, InstalledSoftwareKind::Pkg);
    }
}

//! Read projections from the original owners.
use super::*;
impl Core {
    pub(in crate::service) fn task_detail(
        &self,
        command: &Command,
        reference: &TaskRef,
    ) -> Result<TaskDetail, Error> {
        let scope = self.task_scope(command)?;
        let (facts, revision, recorded_at, source, task_kind, stage, active, controllable) =
            match reference {
                TaskRef::Sampling { query } => {
                    let definition = self
                        .data
                        .directory(&command.subject)?
                        .into_iter()
                        .find(|v| {
                            v.reference == *query
                                && matches!(v.kind, execution_contract::QueryKind::Telemetry)
                        })
                        .ok_or(Error::Denied)?;
                    let read = execution_contract::DataRead {
                        selection: execution_contract::DataSelection::Query {
                            query: query.clone(),
                        },
                        after: None,
                        limit: 1,
                        max_age_seconds: None,
                    };
                    let binding = command.binding.as_ref().ok_or(Error::Denied)?;
                    self.data.authorize_read(&read, &command.subject, binding)?;
                    let latest = self.data.read(
                        &read,
                        &command.subject,
                        binding,
                        self.host.clock.millis()? / 1000,
                    )?;
                    let at = latest.collected_at.and_then(|v| v.checked_mul(1000));
                    let stage = label(&latest.quality);
                    (
                        TaskFacts::Sampling {
                            definition: Box::new(definition),
                            latest: Box::new(latest),
                        },
                        0,
                        at,
                        TaskSource::Policy,
                        TaskKind::Sampling,
                        stage,
                        false,
                        false,
                    )
                }
                TaskRef::Activity { id } => {
                    let cache = self
                        .management
                        .as_ref()
                        .filter(|c| c.scope == scope)
                        .ok_or(Error::NotFound)?;
                    let v = cache.items.get(id).ok_or(Error::NotFound)?.clone();
                    let source = match v.origin {
                        rss_mdm_agent_wire::activities::ActivityOrigin::Human => TaskSource::Human,
                        rss_mdm_agent_wire::activities::ActivityOrigin::Ai => TaskSource::Ai,
                        _ => TaskSource::Policy,
                    };
                    let stage = label(&v.phase);
                    let at = u64::try_from(v.recorded_at)
                        .ok()
                        .and_then(|v| v.checked_mul(1000));
                    (
                        TaskFacts::Management(Box::new(v)),
                        0,
                        at,
                        source,
                        TaskKind::Management,
                        stage,
                        false,
                        false,
                    )
                }
                TaskRef::Request { request } => {
                    if !self.policy_readable(request, &command.subject, command.binding.as_ref())? {
                        return Err(Error::Denied);
                    }
                    if let Some(v) = self.pending(request, &command.subject)? {
                        if v.trigger_os_session() != command.binding.as_ref() {
                            return Err(Error::Denied);
                        }
                        let source = trigger_source(&v.trigger);
                        let kind = match &v.action {
                            PreparedAction::SoftwareRead { .. } => TaskKind::Query,
                            PreparedAction::Offered { offer } => match offer.presentation.summary {
                                execution_contract::BackendTaskSummary::Software { .. } => {
                                    TaskKind::Software
                                }
                                execution_contract::BackendTaskSummary::Collection { .. } => {
                                    TaskKind::Query
                                }
                                _ => TaskKind::Policy,
                            },
                            _ => TaskKind::Policy,
                        };
                        let active = !matches!(
                            v.state,
                            BackendRequestState::Failed
                                | BackendRequestState::Cancelled
                                | BackendRequestState::Observed
                        );
                        (
                            TaskFacts::Preparation(Box::new(v.clone())),
                            v.revision,
                            Some(v.recorded_at_unix_ms),
                            source,
                            kind,
                            label(&v.state),
                            active,
                            active,
                        )
                    } else {
                        if !self.can_read(request, &command.subject)? {
                            return Err(Error::Denied);
                        }
                        let v = self
                            .app
                            .task_details(&self.request_caller(request)?, request)
                            .map_err(map_app_error)?;
                        let source = match &v.action.initiator {
                            Initiator::Human { .. } => TaskSource::Human,
                            Initiator::Ai { .. } => TaskSource::Ai,
                            Initiator::Backend { trigger, .. } => trigger_source(trigger),
                            _ => TaskSource::Policy,
                        };
                        let kind = if matches!(
                            v.action.execution,
                            execution_app::ExecutionSummary::SoftwareProgram { .. }
                        ) || v
                            .action
                            .operation
                            .action
                            .as_str()
                            .starts_with("local-software.")
                        {
                            TaskKind::Software
                        } else if v.action.interpreter.profile.id.as_str()
                            == crate::collection_worker::PROFILE
                            || self
                                .app
                                .service_frozen_input(request)
                                .map_err(map_app_error)?
                                .spec()
                                .request
                                .parameters
                                .contains_key(crate::backend::plan::SCRIPT_PROJECTION)
                        {
                            TaskKind::Query
                        } else {
                            TaskKind::Policy
                        };
                        let active = matches!(
                            v.status.phase,
                            execution_app::TaskPhase::Waiting
                                | execution_app::TaskPhase::ConfirmationRequired
                                | execution_app::TaskPhase::Accepted
                                | execution_app::TaskPhase::Running
                                | execution_app::TaskPhase::ExecutionEnded
                                | execution_app::TaskPhase::OutcomeUnknown
                        );
                        // The execution owner accepts a cancellation request independently
                        // of process/effect completion; it never promises rollback or stop.
                        let controllable =
                            source != TaskSource::Policy && !v.status.cancel_requested;
                        (
                            TaskFacts::Execution(Box::new(v.clone())),
                            v.revision,
                            Some(v.recorded_at_unix_ms),
                            source,
                            kind,
                            label(&v.status.phase),
                            active,
                            controllable,
                        )
                    }
                }
            };
        let mut detail = TaskDetail {
            reference: reference.clone(),
            scope,
            revision: TaskRevision {
                owner: revision,
                transfers: Vec::new(),
            },
            recorded_at,
            source,
            task_kind,
            stage,
            active,
            facts,
            nodes: Vec::new(),
            allowed_actions: vec![],
            control_reason: None,
            upload_acknowledged: None,
            collection_quality: None,
            collection_complete: None,
            software_state: None,
            policy: None,
            policy_revision: None,
            retry_of: None,
            related: Vec::new(),
            related_cursor: None,
            selection: None,
            missing: Vec::new(),
            parameter_summary: Default::default(),
            human_confirmation: false,
        };
        if let TaskFacts::Execution(value) = &detail.facts {
            if value
                .action
                .operation
                .action
                .as_str()
                .starts_with("local-software.")
            {
                let frozen = self
                    .app
                    .service_frozen_input(&value.action.request_id)
                    .map_err(map_app_error)?;
                if let execution_ipc::host::Reply::SoftwareStatus { value } =
                    self.software_status(&frozen)?
                {
                    detail.software_state = value.detected;
                }
            }
        }
        if let TaskRef::Activity { id } = &detail.reference {
            if let Some(remote) = self
                .management
                .as_ref()
                .filter(|v| v.scope == detail.scope)
                .and_then(|v| v.details.get(id))
            {
                detail.related = remote
                    .related
                    .iter()
                    .map(|v| TaskRef::Activity { id: v.id.clone() })
                    .collect();
                detail.missing = remote.missing.clone();
                detail.related_cursor = remote.related_cursor.clone();
            }
        }
        if let TaskFacts::Management(value) = &detail.facts {
            let candidate =
                if let (Some(task), Some(attempt)) = (value.links.task, value.links.attempt) {
                    match self.app.service_remote_request(
                        &plan::id(task.to_string())?,
                        &plan::id(attempt.to_string())?,
                    ) {
                        Ok(value) => value,
                        Err(execution_app::Error::Denied | execution_app::Error::NotFound) => None,
                        Err(error) => return Err(map_app_error(error)),
                    }
                } else {
                    value
                        .links
                        .request
                        .and_then(|id| RequestId::new(id.to_string()).ok())
                };
            if let Some(request) = candidate {
                if self
                    .policy_readable(&request, &command.subject, command.binding.as_ref())
                    .unwrap_or(false)
                    && self.can_read(&request, &command.subject).unwrap_or(false)
                {
                    detail.related.push(TaskRef::Request { request });
                }
            }
        }
        if let TaskRef::Request { request } = &detail.reference {
            if let Some(record) = self
                .app
                .backend_request(&self.caller(), request)
                .map_err(map_app_error)?
            {
                if let Some(offer) = record.action.offer() {
                    detail.selection = Some(execution_contract::BackendSelection {
                        request: offer.request.clone(),
                        task: offer.task.clone(),
                        attempt: offer.attempt.clone(),
                        revision: offer.revision.clone(),
                    });
                }
                detail.retry_of = record.retry_of;
                if let PreparedAction::Policy {
                    definition,
                    policy_revision,
                    ..
                } = record.action
                {
                    detail.policy = Some(definition);
                    detail.policy_revision = Some(policy_revision);
                    if detail.source != TaskSource::Policy
                        && self.host.identity_ready()
                        && matches!(&detail.facts,TaskFacts::Execution(v) if matches!(v.status.phase,execution_app::TaskPhase::AdmissionDenied|execution_app::TaskPhase::FailedBeforeDispatch)||(v.status.phase==execution_app::TaskPhase::Verified&&(v.status.assessment==Some(execution_lifecycle::EffectAssessment::NotSatisfied)||(v.status.assessment==Some(execution_lifecycle::EffectAssessment::NoEffect)&&v.status.process.as_ref().is_some_and(|p|p.exit_code.is_some_and(|code|code!=0))))))
                    {
                        detail.allowed_actions.push(TaskAction::Retry);
                    }
                }
            }
        }
        if controllable {
            detail.allowed_actions.push(TaskAction::Cancel);
            if !active {
                detail.control_reason = Some("execution-ended-cancel-request-only".into());
            }
        } else {
            detail.control_reason = Some(
                if source == TaskSource::Policy {
                    "managed-background-task"
                } else {
                    "owner-stage-is-not-interruptible"
                }
                .into(),
            );
        }
        if let Some((task, attempt)) = remote_pair(&detail.facts) {
            if let Some(cache) = self.management.as_ref().filter(|c| c.scope == detail.scope) {
                for activity in cache
                    .items
                    .values()
                    .filter(|v| v.links.task == Some(task) && v.links.attempt == Some(attempt))
                    .take(32)
                {
                    detail.related.push(TaskRef::Activity {
                        id: activity.id.clone(),
                    });
                }
            }
            detail.upload_acknowledged = self
                .deliveries
                .get(&format!("delivery-terminal/{task}/{attempt}"))
                .copied();
            let observations = self.transfers.observations(task, attempt)?;
            detail.revision.transfers = observations
                .iter()
                .map(|v| (v.key.clone(), v.revision))
                .collect();
            let until = match &detail.facts {
                TaskFacts::Preparation(v) => v.action.expires_at_unix_ms(),
                TaskFacts::Execution(v) => v.action.validity.expires_at_unix_ms,
                _ => 0,
            };
            let content_authorized =
                self.host.identity_ready() && self.host.clock.millis()? < until;
            if active && !content_authorized && controllable && !observations.is_empty() {
                detail.control_reason = Some("content-authorization-expired".into());
            }
            let resumable = active
                && controllable
                && content_authorized
                && !observations.is_empty()
                && observations.iter().all(|v| {
                    v.resumable
                        && (v.paused
                            || matches!(v.stage.as_str(), "downloading" | "paused" | "unobserved"))
                });
            if resumable {
                detail
                    .allowed_actions
                    .push(if observations.iter().all(|v| v.paused) {
                        TaskAction::Resume
                    } else {
                        TaskAction::Pause
                    });
            }
            for v in observations {
                detail.nodes.push(TaskNode {
                    exit_code: None,
                    detected: None,
                    quiescent: None,
                    id: format!("download/{attempt}/{}", v.key),
                    parent: None,
                    stage: if v.paused {
                        "paused".into()
                    } else {
                        v.stage.clone()
                    },
                    bytes: if v.stage == "unobserved" {
                        None
                    } else {
                        Some(v.bytes)
                    },
                    total: v.total,
                    attempt: Some(attempt.to_string()),
                    reason: if !v.resumable {
                        Some("native-source-has-no-range-resume".into())
                    } else if v.shared_active && v.paused {
                        Some("consumer-paused-shared-producer-active".into())
                    } else {
                        None
                    },
                });
            }
        }
        match &detail.facts {
            TaskFacts::Sampling { latest, .. } => {
                detail.collection_quality = Some(latest.quality.clone());
                detail.collection_complete = Some(latest.enumeration_complete);
            }
            TaskFacts::Execution(_) if detail.task_kind == TaskKind::Query => {
                if let TaskRef::Request { request } = &detail.reference {
                    let read = execution_contract::DataRead {
                        selection: execution_contract::DataSelection::Request {
                            request: request.clone(),
                        },
                        after: None,
                        limit: 1,
                        max_age_seconds: None,
                    };
                    let binding = command.binding.as_ref().ok_or(Error::Denied)?;
                    if self
                        .data
                        .authorize_read(&read, &command.subject, binding)
                        .is_ok()
                    {
                        if let Ok(data) = self.data.read(
                            &read,
                            &command.subject,
                            binding,
                            self.host.clock.millis()? / 1000,
                        ) {
                            detail.collection_quality = Some(data.quality);
                            detail.collection_complete = Some(data.enumeration_complete);
                        }
                    }
                }
            }
            _ => {}
        }
        if let TaskFacts::Execution(v) = &detail.facts {
            if let TaskRef::Request { request } = &detail.reference {
                if let Some(progress) = self
                    .app
                    .task_software_progress(&self.request_caller(request)?, request)
                    .map_err(map_app_error)?
                {
                    let mut parents = std::collections::BTreeSet::new();
                    for checkpoint in &progress.checkpoints {
                        use execution_contract::SoftwareCheckpoint as C;
                        let (step, phase, edge, sequence, exit_code, detected, quiescent) =
                            match checkpoint {
                                C::Begin { step, phase } => {
                                    (*step, label(phase), "begin", 0, None, None, None)
                                }
                                C::End {
                                    step,
                                    phase,
                                    process,
                                    detected,
                                    quiescent,
                                    ..
                                } => (
                                    *step,
                                    label(phase),
                                    "end",
                                    0,
                                    process.as_ref().and_then(|v| v.exit_code),
                                    detected.clone(),
                                    Some(*quiescent),
                                ),
                                C::CleanupBegin { step, sequence, .. } => (
                                    *step,
                                    "cleanup".into(),
                                    "begin",
                                    *sequence,
                                    None,
                                    None,
                                    None,
                                ),
                                C::CleanupEnd {
                                    step,
                                    sequence,
                                    process,
                                    resources_closed,
                                    ..
                                } => (
                                    *step,
                                    "cleanup".into(),
                                    "end",
                                    *sequence,
                                    process.exit_code,
                                    None,
                                    Some(*resources_closed),
                                ),
                                C::Complete { step } => {
                                    (*step, "step".into(), "complete", 0, None, None, Some(true))
                                }
                            };
                        let parent = format!("step/{}/{step}", progress.attempt_id.as_str());
                        if parents.insert(step) {
                            detail.nodes.push(TaskNode {
                                exit_code: None,
                                detected: None,
                                quiescent: None,
                                id: parent.clone(),
                                parent: None,
                                stage: "software-step".into(),
                                bytes: None,
                                total: None,
                                attempt: Some(progress.attempt_id.as_str().into()),
                                reason: None,
                            });
                        }
                        detail.nodes.push(TaskNode {
                            id: format!("{parent}/{phase}/{sequence}/{edge}"),
                            parent: Some(parent),
                            stage: format!("{edge}:{phase}"),
                            bytes: None,
                            total: None,
                            attempt: Some(progress.attempt_id.as_str().into()),
                            reason: Some("runner-phase-cannot-be-paused".into()),
                            exit_code,
                            detected,
                            quiescent,
                        });
                    }
                }
            }
            detail.nodes.push(TaskNode {
                exit_code: None,
                detected: None,
                quiescent: None,
                id: format!(
                    "execution/{}",
                    v.status
                        .attempt_id
                        .as_ref()
                        .map_or("pending", |v| v.as_str())
                ),
                parent: None,
                stage: label(&v.status.phase),
                bytes: None,
                total: None,
                attempt: v.status.attempt_id.as_ref().map(|v| v.as_str().into()),
                reason: None,
            });
        }
        if let TaskFacts::Preparation(record) = &mut detail.facts {
            detail.human_confirmation = record.state == BackendRequestState::AwaitingConfirmation
                && matches!(record.trigger, BackendTrigger::Ai { .. })
                && record.trigger_os_session() == command.binding.as_ref();
            if let PreparedAction::Policy { choice, task, .. } = &mut record.action {
                for (key, value) in &choice.parameters {
                    detail.parameter_summary.insert(
                        key.clone(),
                        match value {
                            serde_json::Value::Null => "null",
                            serde_json::Value::Bool(_) => "boolean",
                            serde_json::Value::Number(_) => "number",
                            serde_json::Value::String(_) => "string",
                            serde_json::Value::Array(_) => "array",
                            serde_json::Value::Object(_) => "object",
                        }
                        .into(),
                    );
                }
                choice.parameters.clear();
                if let wire::SelfServiceTask::Script {
                    parameter_sources, ..
                } = task.as_mut()
                {
                    for source in parameter_sources.values_mut() {
                        if let wire::ScriptParameterSource::Fixed { value } = source {
                            *value = serde_json::Value::String("[redacted]".into());
                        }
                    }
                }
            }
        }
        Ok(detail)
    }
    pub(super) fn visible_task_detail(
        &self,
        command: &Command,
        reference: &TaskRef,
    ) -> Result<Option<TaskDetail>, Error> {
        match self.task_detail(command, reference) {
            Ok(value) => Ok(Some(value)),
            Err(Error::Denied | Error::NotFound | Error::Identity) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

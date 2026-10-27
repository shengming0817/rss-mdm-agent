//! Authorized presentation and routing only. All writes remain with the original owner.
mod paging;
mod projection;
use super::*;
use execution_app::tasks::*;
use execution_contract::Initiator;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    scope: String,
    filter: String,
    snapshot: u64,
    preparations: u64,
    receipts: u64,
    definitions: String,
    management: String,
    lane: u8,
    after: Option<String>,
    position: Option<execution_app::TaskHistoryPosition>,
}
fn text<T: Serialize>(v: &T) -> Result<String, Error> {
    serde_json::to_string(v).map_err(|_| Error::Protocol)
}
fn hash<T: Serialize>(v: &T) -> Result<String, Error> {
    Ok(format!("{:x}", Sha256::digest(text(v)?.as_bytes())))
}
fn label<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}
fn trigger_source(v: &BackendTrigger) -> TaskSource {
    match v {
        BackendTrigger::Human { .. } => TaskSource::Human,
        BackendTrigger::Ai { .. } => TaskSource::Ai,
        _ => TaskSource::Policy,
    }
}
fn remote_pair(value: &TaskFacts) -> Option<(uuid::Uuid, uuid::Uuid)> {
    let (task, attempt) = match value {
        TaskFacts::Preparation(v) => {
            let v = v.action.offer()?;
            (&v.task, &v.attempt)
        }
        TaskFacts::Execution(v) => {
            let Initiator::Backend { task, attempt, .. } = &v.action.initiator else {
                return None;
            };
            (task, attempt)
        }
        _ => return None,
    };
    Some((
        uuid::Uuid::parse_str(task.as_str()).ok()?,
        uuid::Uuid::parse_str(attempt.as_str()).ok()?,
    ))
}
impl Core {
    pub(super) fn task_scope(&self, command: &Command) -> Result<String, Error> {
        let contexts: Vec<_> = self
            .data_contexts()
            .into_iter()
            .filter(|(subject, binding, _)| {
                subject == &command.subject && Some(binding) == command.binding.as_ref()
            })
            .collect();
        hash(&(
            &self.host.root.authority,
            &self.host.binding.authority,
            &self.host.binding.device,
            &command.subject,
            &command.binding,
            contexts,
        ))
    }
    pub(super) fn task_operation(
        &mut self,
        command: &Command,
        input: &TaskOperation,
        origin: &execution_ipc::host::ClientOrigin,
    ) -> Result<execution_ipc::host::Reply, Error> {
        if command.binding.is_none() {
            return Err(Error::Denied);
        }
        if self
            .fences
            .current(&policy::SessionKey::new(
                command.subject.clone(),
                command.binding.clone().ok_or(Error::Denied)?,
            ))
            .ok()
            != Some(command.fence_epoch)
        {
            return Err(Error::Denied);
        }
        if let execution_ipc::host::ClientOrigin::Ai { context, .. } = origin {
            self.data_context(command, context)?;
        }
        let value = match input {
            TaskOperation::List(q) => TaskResponse::Page(self.task_list(command, q)?),
            TaskOperation::Detail(reference) => {
                TaskResponse::Detail(Box::new(self.task_detail(command, reference)?))
            }
            TaskOperation::Control(control) => {
                TaskResponse::Control(self.task_control(command, control, origin)?)
            }
            TaskOperation::ControlStatus(query) => {
                TaskResponse::Control(self.task_control_status(command, query)?)
            }
        };
        Ok(execution_ipc::host::Reply::Task { value })
    }
    fn task_control(
        &mut self,
        command: &Command,
        c: &TaskControl,
        origin: &execution_ipc::host::ClientOrigin,
    ) -> Result<TaskControlReceipt, Error> {
        let detail = self.task_detail(command, &c.reference)?;
        if c.scope != detail.scope {
            return Err(Error::Denied);
        }
        let query = TaskControlQuery {
            reference: c.reference.clone(),
            id: c.id.clone(),
            scope: c.scope.clone(),
        };
        let transfer_receipt = remote_pair(&detail.facts)
            .map(|(task, attempt)| self.transfers.receipt(task, attempt, c.id.as_str()))
            .transpose()?
            .flatten();
        let execution_receipt = if let TaskRef::Request { request } = &c.reference {
            if matches!(&detail.facts, TaskFacts::Execution(_)) {
                self.app
                    .control_receipt(&self.request_caller(request)?, request, &c.id)
                    .map_err(map_app_error)?
            } else {
                None
            }
        } else {
            None
        };
        let preparation_receipt = matches!(&detail.facts, TaskFacts::Preparation(v) if v.cancellation_control.as_ref() == Some(&c.id));
        if transfer_receipt.is_some() && c.action == TaskAction::Cancel
            || (execution_receipt.is_some() || preparation_receipt)
                && matches!(c.action, TaskAction::Pause | TaskAction::Resume)
        {
            return Err(Error::Conflict);
        }
        if transfer_receipt.is_some() && matches!(c.action, TaskAction::Pause | TaskAction::Resume)
        {
            let (task, attempt) = remote_pair(&detail.facts).ok_or(Error::Protocol)?;
            let confirmed = self.transfers.control(
                task,
                attempt,
                c.id.as_str(),
                c.expected_revision.owner,
                &c.expected_revision.transfers,
                c.action == TaskAction::Pause,
            )?;
            return Ok(TaskControlReceipt {
                id: c.id.clone(),
                disposition: if confirmed {
                    ControlDisposition::Confirmed
                } else {
                    ControlDisposition::Unknown
                },
                created: None,
            });
        }
        let prior = self.task_control_status(command, &query)?;
        if prior.created.is_some() {
            return Err(Error::Conflict);
        }
        if matches!(prior.disposition, ControlDisposition::Confirmed)
            && c.action == TaskAction::Cancel
        {
            let matches = match &detail.facts {
                TaskFacts::Preparation(v) => {
                    v.cancellation_control.as_ref() == Some(&c.id)
                        && v.cancellation_expected_revision.as_ref() == Some(&c.expected_revision)
                }
                TaskFacts::Execution(_) => {
                    let TaskRef::Request { request } = &c.reference else {
                        return Err(Error::Protocol);
                    };
                    let receipt = self
                        .app
                        .cancel_control(
                            &self.request_caller(request)?,
                            request,
                            &c.id,
                            &c.expected_revision,
                        )
                        .map_err(map_app_error)?;
                    if matches!(
                        receipt_disposition(receipt.outcome),
                        ControlDisposition::Confirmed
                    ) {
                        self.task_cancel_settings(request)?;
                    }
                    return Ok(TaskControlReceipt {
                        id: c.id.clone(),
                        disposition: receipt_disposition(receipt.outcome),
                        created: None,
                    });
                }
                _ => false,
            };
            if matches {
                return Ok(prior);
            } else {
                return Err(Error::Conflict);
            }
        }
        if detail.revision != c.expected_revision {
            return Ok(TaskControlReceipt {
                id: c.id.clone(),
                disposition: ControlDisposition::Stale,
                created: None,
            });
        }
        if !detail.allowed_actions.contains(&c.action) {
            return Ok(TaskControlReceipt {
                id: c.id.clone(),
                disposition: ControlDisposition::Denied,
                created: None,
            });
        }
        let TaskRef::Request { request } = &c.reference else {
            return Err(Error::Denied);
        };
        if let execution_ipc::host::ClientOrigin::Ai { context, .. } = origin {
            self.data_context(command, context)?;
        }
        let disposition = if matches!(c.action, TaskAction::Pause | TaskAction::Resume) {
            let (task, attempt) = remote_pair(&detail.facts).ok_or(Error::Unsupported)?;
            let revisions = c.expected_revision.transfers.clone();
            if self.transfers.control(
                task,
                attempt,
                c.id.as_str(),
                c.expected_revision.owner,
                &revisions,
                c.action == TaskAction::Pause,
            )? {
                ControlDisposition::Confirmed
            } else {
                ControlDisposition::Unknown
            }
        } else if let TaskFacts::Preparation(_) = &detail.facts {
            let previous = self
                .app
                .backend_request(&self.caller(), request)
                .map_err(map_app_error)?
                .ok_or(Error::Denied)?;
            let mut next = previous.clone();
            next.revision += 1;
            next.state = BackendRequestState::Cancelled;
            next.cancellation_control = Some(c.id.clone());
            next.cancellation_expected_revision = Some(c.expected_revision.clone());
            self.app
                .record_backend_request(&self.caller(), Some(&previous), &next)
                .map_err(map_app_error)?;
            if self
                .selected
                .as_ref()
                .is_some_and(|s| s.request == *request)
            {
                self.selected = None;
            }
            ControlDisposition::Confirmed
        } else if let TaskFacts::Execution(_) = &detail.facts {
            let result = self
                .app
                .cancel_control(
                    &self.request_caller(request)?,
                    request,
                    &c.id,
                    &c.expected_revision,
                )
                .map_err(map_app_error)?;
            let disposition = receipt_disposition(result.outcome);
            if matches!(disposition, ControlDisposition::Confirmed) {
                self.task_cancel_settings(request)?;
            }
            disposition
        } else {
            ControlDisposition::Denied
        };
        if c.action == TaskAction::Cancel && matches!(disposition, ControlDisposition::Confirmed) {
            if let Some((task, attempt)) = remote_pair(&detail.facts) {
                self.transfers.detach(task, attempt)?;
            }
        }
        Ok(TaskControlReceipt {
            id: c.id.clone(),
            disposition,
            created: None,
        })
    }
    pub(in crate::service) fn task_control_id_used(
        &self,
        command: &Command,
        reference: &TaskRef,
        id: &execution_contract::Id,
    ) -> Result<bool, Error> {
        let detail = self.task_detail(command, reference)?;
        if let Some((task, attempt)) = remote_pair(&detail.facts) {
            if self
                .transfers
                .receipt(task, attempt, id.as_str())?
                .is_some()
            {
                return Ok(true);
            }
        }
        match &detail.facts {
            TaskFacts::Preparation(v) => Ok(v.cancellation_control.as_ref() == Some(id)),
            TaskFacts::Execution(_) if detail.source != TaskSource::Policy => {
                let TaskRef::Request { request } = reference else {
                    return Err(Error::Protocol);
                };
                Ok(self
                    .app
                    .control_receipt(&self.request_caller(request)?, request, id)
                    .map_err(map_app_error)?
                    .is_some())
            }
            _ => Ok(false),
        }
    }
    fn task_control_status(
        &self,
        command: &Command,
        q: &TaskControlQuery,
    ) -> Result<TaskControlReceipt, Error> {
        let detail = self.task_detail(command, &q.reference)?;
        if q.scope != detail.scope {
            return Err(Error::Denied);
        }
        if let TaskRef::Request { request } = &q.reference {
            let new_request = RequestId::new(q.id.as_str()).map_err(|_| Error::Protocol)?;
            if let Some(record) = self
                .app
                .backend_request(&self.caller(), &new_request)
                .map_err(map_app_error)?
            {
                if record.retry_of.as_ref() == Some(request)
                    && trigger_subject(&record.trigger) == Some(command.subject.as_str())
                    && record.trigger_os_session() == command.binding.as_ref()
                {
                    return Ok(TaskControlReceipt {
                        id: q.id.clone(),
                        disposition: ControlDisposition::Confirmed,
                        created: Some(TaskRef::Request {
                            request: new_request,
                        }),
                    });
                }
            }
        }
        let mut disposition = ControlDisposition::Unknown;
        if let Some((task, attempt)) = remote_pair(&detail.facts) {
            if self.transfers.receipt(task, attempt, q.id.as_str())? == Some(true) {
                disposition = ControlDisposition::Confirmed;
            }
        }
        match &detail.facts {
            TaskFacts::Preparation(v) if v.cancellation_control.as_ref() == Some(&q.id) => {
                disposition = ControlDisposition::Confirmed
            }
            TaskFacts::Execution(_) => {
                let TaskRef::Request { request } = &q.reference else {
                    return Err(Error::Protocol);
                };
                if detail.source != TaskSource::Policy {
                    if let Some(receipt) = self
                        .app
                        .control_receipt(&self.request_caller(request)?, request, &q.id)
                        .map_err(map_app_error)?
                    {
                        disposition = receipt_disposition(receipt.outcome);
                    }
                }
            }
            _ => {}
        }
        Ok(TaskControlReceipt {
            id: q.id.clone(),
            disposition,
            created: None,
        })
    }
}
fn receipt_disposition(outcome: execution_app::Outcome) -> ControlDisposition {
    match outcome {
        execution_app::Outcome::Stale => ControlDisposition::Stale,
        execution_app::Outcome::Rejected => ControlDisposition::Denied,
        _ => ControlDisposition::Confirmed,
    }
}

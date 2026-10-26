//! Stable bounded task discovery.
use super::*;
impl Core {
    pub(super) fn task_list(&self, command: &Command, q: &TaskQuery) -> Result<TaskPage, Error> {
        if !(1..=64).contains(&q.limit) || q.from.zip(q.until).is_some_and(|(a, b)| a >= b) {
            return Err(Error::Protocol);
        }
        let scope = self.task_scope(command)?;
        let mut filter = q.clone();
        filter.cursor = None;
        let fingerprint = hash(&filter)?;
        let mut definitions = self.data.directory(&command.subject)?;
        definitions.sort_by(|a, b| a.reference.id.cmp(&b.reference.id));
        let definitions_hash = hash(&definitions)?;
        let management_hash = hash(
            &self
                .management
                .as_ref()
                .filter(|c| c.scope == scope)
                .map(|c| (&c.as_of, &c.sync, &c.next)),
        )?;
        let mut cursor = if let Some(raw) = &q.cursor {
            if raw.len() > 4096 {
                return Err(Error::Protocol);
            }
            let c: Cursor = serde_json::from_str(raw).map_err(|_| Error::Protocol)?;
            if c.scope != scope || c.filter != fingerprint {
                return Err(Error::Conflict);
            }
            c
        } else {
            let (preparations, receipts) =
                self.app.service_task_watermarks().map_err(map_app_error)?;
            Cursor {
                preparations,
                receipts,
                definitions: definitions_hash.clone(),
                management: management_hash.clone(),
                scope: scope.clone(),
                filter: fingerprint,
                snapshot: self.host.clock.millis()?,
                lane: if q.view == TaskView::Management { 3 } else { 0 },
                after: None,
            }
        };
        if cursor.definitions != definitions_hash
            || q.view == TaskView::Management && cursor.management != management_hash
        {
            return Err(Error::Conflict);
        }
        let now = self.host.clock.millis()?;
        if now < cursor.snapshot || now - cursor.snapshot > 15 * 60 * 1000 {
            return Err(Error::Conflict);
        }
        let mut items = Vec::new();
        let mut next = None;
        if cursor.lane == 0 {
            let after = cursor
                .after
                .as_ref()
                .map(|v| RequestId::new(v).map_err(|_| Error::Protocol))
                .transpose()?;
            let candidates = self
                .app
                .backend_request_page(
                    &self.caller(),
                    after.as_ref(),
                    cursor.preparations,
                    usize::from(q.limit),
                )
                .map_err(map_app_error)?;
            let count = candidates.len();
            for record in candidates {
                cursor.after = Some(record.action.request().as_str().into());
                if record.recorded_at_unix_ms > cursor.snapshot {
                    continue;
                }
                let request = record.action.request();
                if let Some(item) = self.visible_task_detail(
                    command,
                    &TaskRef::Request {
                        request: request.clone(),
                    },
                )? {
                    if matches_query(&item, q) {
                        items.push(item);
                    }
                }
            }
            if count < usize::from(q.limit) {
                cursor.lane = 1;
                cursor.after = None;
            }
            next = Some(text(&cursor)?);
        }
        if cursor.lane == 1 && items.len() < usize::from(q.limit) {
            next = None;
            let before = cursor
                .after
                .as_ref()
                .map(|v| v.parse::<u64>().map_err(|_| Error::Protocol))
                .transpose()?
                .or(Some(cursor.receipts.checked_add(1).ok_or(Error::Capacity)?));
            let page = self
                .app
                .service_task_page(before, usize::from(q.limit) - items.len())
                .map_err(map_app_error)?;
            for record in page.items {
                // A preparation retains its discovery lane after it starts execution.
                if self
                    .app
                    .backend_request(&self.caller(), &record.details.action.request_id)
                    .map_err(map_app_error)?
                    .is_some()
                {
                    continue;
                }
                if record.recorded_at_unix_ms > cursor.snapshot {
                    continue;
                }
                if let Some(mut item) = self.visible_task_detail(
                    command,
                    &TaskRef::Request {
                        request: record.details.action.request_id.clone(),
                    },
                )? {
                    item.recorded_at = Some(record.recorded_at_unix_ms);
                    if matches_query(&item, q) {
                        items.push(item);
                    }
                }
            }
            if let Some(before) = page.next {
                cursor.after = Some(before);
                next = Some(text(&cursor)?);
            } else {
                cursor.lane = 2;
                cursor.after = None;
            }
        }
        if cursor.lane == 2 && items.len() < usize::from(q.limit) {
            let candidates: Vec<_> = definitions
                .into_iter()
                .filter(|v| {
                    matches!(v.kind, execution_contract::QueryKind::Telemetry)
                        && cursor
                            .after
                            .as_ref()
                            .is_none_or(|a| v.reference.id.as_str() > a.as_str())
                })
                .take(usize::from(q.limit) - items.len())
                .collect();
            let budget = usize::from(q.limit) - items.len();
            let count = candidates.len();
            for definition in candidates {
                cursor.after = Some(definition.reference.id.as_str().into());
                if let Some(item) = self.visible_task_detail(
                    command,
                    &TaskRef::Sampling {
                        query: definition.reference,
                    },
                )? {
                    if matches_query(&item, q) {
                        items.push(item);
                    }
                }
            }
            next = if count == budget {
                Some(text(&cursor)?)
            } else {
                None
            };
        } else if cursor.lane == 3 {
            let cache = self.management.as_ref().filter(|c| c.scope == scope);
            if let Some(cache) = cache {
                let candidates: Vec<_> = cache
                    .items
                    .values()
                    .filter(|v| {
                        cursor.after.as_ref().is_none_or(|a| v.id > *a)
                            && v.recorded_at >= 0
                            && (v.recorded_at as u64).saturating_mul(1000) <= cursor.snapshot
                    })
                    .take(usize::from(q.limit))
                    .collect();
                let count = candidates.len();
                for v in candidates {
                    cursor.after = Some(v.id.clone());
                    if let Some(item) =
                        self.visible_task_detail(command, &TaskRef::Activity { id: v.id.clone() })?
                    {
                        if matches_query(&item, q) {
                            items.push(item);
                        }
                    }
                }
                if count == usize::from(q.limit) {
                    next = Some(text(&cursor)?);
                }
            }
        }
        if cursor.lane == 2 && items.len() == usize::from(q.limit) {
            next = Some(text(&cursor)?);
        }
        let cache = self.management.as_ref().filter(|c| c.scope == scope);
        Ok(TaskPage {
            items,
            next,
            scope,
            as_of: cursor.snapshot,
            last_sync: cache.and_then(|c| c.as_of),
            remote_available: self.management_online,
            coverage: cache.map_or_else(Vec::new, |c| c.coverage.clone()),
            retained_from: cache.and_then(|c| c.retained_from),
            complete: if q.view == TaskView::Management {
                cache.is_some_and(|c| c.complete)
            } else {
                true
            },
        })
    }
}
fn matches_query(item: &TaskDetail, q: &TaskQuery) -> bool {
    if q.view == TaskView::Active && !item.active
        || q.source.is_some_and(|v| v != item.source)
        || q.task_kind.is_some_and(|v| v != item.task_kind)
        || q.status.as_ref().is_some_and(|v| *v != item.stage)
        || q.from
            .is_some_and(|v| item.recorded_at.is_none_or(|at| at < v))
        || q.until
            .is_some_and(|v| item.recorded_at.is_none_or(|at| at >= v))
    {
        return false;
    }
    if let Some(subject) = &q.subject {
        let original = match &item.facts {
            TaskFacts::Execution(v) => match &v.action.initiator {
                Initiator::Human { os_session } | Initiator::Ai { os_session, .. } => {
                    Some(os_session.account.subject.as_str())
                }
                Initiator::Backend {
                    trigger:
                        BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. },
                    ..
                } => Some(os_session.account.subject.as_str()),
                _ => None,
            },
            TaskFacts::Preparation(v) => trigger_subject(&v.trigger),
            TaskFacts::Sampling { definition, .. } => match &definition.scope {
                execution_contract::DataScope::User { subject } => Some(subject.as_str()),
                _ => None,
            },
            _ => None,
        };
        if original != Some(subject.as_str()) {
            return false;
        }
    }
    if let Some(policy) = &q.policy {
        return match &item.facts {
            TaskFacts::Execution(_) => item
                .policy
                .as_ref()
                .is_some_and(|v| v.id.as_str() == policy),
            TaskFacts::Management(v) => v.links.policy.is_some_and(|v| v.to_string() == *policy),
            TaskFacts::Sampling { .. } => false,
            TaskFacts::Preparation(v) => match &v.action {
                PreparedAction::Policy { choice, .. } => choice.policy_id.as_str() == policy,
                _ => false,
            },
        };
    }
    true
}

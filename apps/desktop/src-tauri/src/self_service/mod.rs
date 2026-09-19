//! Explicit in-memory fixtures, not an execution service or trusted approval authority.
mod fixtures;
mod interaction;
pub mod ipc;
mod model;
mod selection;
use execution_contract::{FrozenPlan, Id, RequestId};
use execution_interaction::{Interaction, Status};
pub use model::*;
use service_catalog::{DisplayDecision as Decision, DisplayStatus, FrozenCatalog, SelectionRef};
use std::collections::{BTreeMap, BTreeSet};

struct Prepared {
    selection: SelectionRef,
    plan: FrozenPlan,
    supplied: BTreeSet<String>,
}
struct RequestRecord {
    revision: u32,
    input_digest: String,
    prepared: Option<Prepared>,
    accepted: bool,
    interactions: Vec<Interaction>,
    result: Option<RequestStatus>,
}
/// One service instance owns all fixture requests. No task is executed by this type.
pub struct FixtureService {
    instance_id: String,
    catalog: FrozenCatalog,
    records: BTreeMap<RequestId, RequestRecord>,
    next_plan: u64,
    last_now: u64,
}
impl FixtureService {
    pub fn new(instance_id: String, now: u64) -> Result<Self> {
        Id::new(&instance_id).map_err(|_| error("instance", "无效测试服务实例"))?;
        Ok(Self {
            instance_id,
            catalog: fixtures::catalog(now)?,
            records: BTreeMap::new(),
            next_plan: 0,
            last_now: now,
        })
    }
    fn observe(&mut self, now: u64) -> Result<()> {
        if now < self.last_now {
            return Err(error("clock", "服务时钟回退，暂不接纳操作"));
        }
        self.last_now = now;
        Ok(())
    }
    fn instance(&self, id: &str) -> Result<()> {
        if id != self.instance_id {
            return Err(error("instance", "测试服务已重启；旧请求不可重放，请刷新"));
        }
        Ok(())
    }
    pub fn snapshot(&mut self, now: u64) -> Result<Snapshot> {
        self.observe(now)?;
        for record in self.records.values_mut().filter(|r| r.accepted) {
            interaction::expire(record, now)?;
        }
        let catalog = self.catalog_views(now)?;
        let requests = self
            .records
            .values()
            .filter(|r| r.accepted)
            .map(|r| self.request_view(r))
            .collect::<Result<_>>()?;
        Ok(Snapshot {
            mode: ServiceMode::Fixture,
            instance_id: self.instance_id.clone(),
            target_label: fixtures::TARGET,
            catalog,
            requests,
        })
    }
    pub fn preview(&mut self, draft: Draft, now: u64) -> Result<PlanView> {
        self.observe(now)?;
        self.instance(&draft.instance_id)?;
        if draft.revision == 0 {
            return Err(error("revision", "草稿版本必须为正整数"));
        }
        let bytes = serde_json::to_vec(&draft).map_err(|_| error("input", "草稿编码失败"))?;
        if bytes.len() > 16384 {
            return Err(error("limit", "草稿超过输入预算"));
        }
        let input_digest = fixtures::digest(&bytes);
        let previous = self.records.get(&draft.request_id);
        if let Some(record) = previous {
            if record.accepted {
                return Err(error("accepted", "请求已接纳；更改参数请新建请求"));
            }
            if draft.revision < record.revision {
                return Err(error("stale", "草稿已过期"));
            }
            if draft.revision == record.revision && input_digest != record.input_digest {
                return Err(error("conflict", "同一草稿版本的内容不同"));
            }
        } else if self.records.len() >= 128 {
            return Err(error("limit", "本次测试会话的请求数量已达上限"));
        }
        // Existing revisions lose their old plan even if validation fails. New invalid
        // requests never enter the bounded record table.
        let old = if let Some(record) = self.records.get_mut(&draft.request_id) {
            record.revision = draft.revision;
            record.input_digest = input_digest.clone();
            record.prepared.take()
        } else {
            None
        };
        let selected = selection::select(&self.catalog, &draft)?;
        self.requestable(&draft.item_id, now)?;
        let prepared = match old {
            Some(mut old)
                if old.selection == *selected.reference()
                    && now < old.plan.spec().validity.expires_at_unix_ms =>
            {
                old.supplied = draft.fields.keys().cloned().collect();
                old
            }
            _ => {
                self.next_plan += 1;
                let plan = fixtures::freeze(
                    &selected,
                    &draft.request_id,
                    format!("plan-{}-{}", self.instance_id, self.next_plan),
                    now,
                )?;
                let display = self.display(&draft.item_id, now);
                selected.display_status(
                    &plan.spec().request.target,
                    now,
                    Some(&service_catalog::ExternalAssessment {
                        selection: selected.reference().clone(),
                        target: plan.spec().request.target.clone(),
                        checked_at_unix_ms: now,
                        expires_at_unix_ms: plan.spec().validity.expires_at_unix_ms,
                        display,
                    }),
                )?;
                Prepared {
                    selection: selected.reference().clone(),
                    plan,
                    supplied: draft.fields.keys().cloned().collect(),
                }
            }
        };
        self.records.insert(
            draft.request_id.clone(),
            RequestRecord {
                revision: draft.revision,
                input_digest,
                prepared: Some(prepared),
                accepted: false,
                interactions: Vec::new(),
                result: None,
            },
        );
        self.plan_view(self.records.get(&draft.request_id).expect("inserted"))
    }
    pub fn submit(&mut self, input: Submission, now: u64) -> Result<RequestView> {
        self.observe(now)?;
        self.instance(&input.instance_id)?;
        let record = self
            .records
            .get(&input.request_id)
            .ok_or_else(|| error("notFound", "请求不存在"))?;
        let prepared = record
            .prepared
            .as_ref()
            .ok_or_else(|| error("stale", "请重新预览计划"))?;
        if prepared.plan.spec().plan_id != input.plan_id || prepared.plan.digest() != &input.digest
        {
            return Err(error("conflict", "请求绑定的计划不匹配"));
        }
        // Accepted retries return the original request even after catalog or plan expiry.
        if !record.accepted {
            if now >= prepared.plan.spec().validity.expires_at_unix_ms {
                return Err(error("expired", "计划已过期，请重新预览"));
            }
            self.requestable(&prepared.selection.item_id, now)?;
            let record = self.records.get_mut(&input.request_id).expect("found");
            interaction::start(record, now)?;
            record.accepted = true;
        }
        let record = self.records.get_mut(&input.request_id).expect("found");
        interaction::expire(record, now)?;
        self.request_view(self.records.get(&input.request_id).expect("found"))
    }
    pub fn respond(&mut self, input: Reply, now: u64) -> Result<RequestView> {
        self.observe(now)?;
        self.instance(&input.instance_id)?;
        let record = self
            .records
            .get_mut(&input.request_id)
            .ok_or_else(|| error("notFound", "请求不存在"))?;
        if !record.accepted {
            return Err(error("notFound", "请求尚未提交"));
        }
        interaction::respond(&self.catalog, record, &input, now)?;
        self.request_view(self.records.get(&input.request_id).expect("found"))
    }
    fn display(&self, id: &Id, now: u64) -> DisplayStatus {
        let mut display = DisplayStatus {
            visibility: Decision::Allowed,
            requestability: Decision::Allowed,
            executability: Decision::Allowed,
        };
        match id.as_str() {
            "office" => display.executability = Decision::Blocked,
            "blocked" | "withdrawn" => {
                display.requestability = Decision::Blocked;
                display.executability = Decision::Blocked;
            }
            "unsupported" => {
                display.requestability = Decision::UnsupportedTarget;
                display.executability = Decision::UnsupportedTarget;
            }
            _ => {}
        }
        if now >= self.catalog.snapshot().expires_at_unix_ms {
            display.requestability = Decision::Unknown;
            display.executability = Decision::Unknown;
        }
        display
    }
    fn requestable(&self, id: &Id, now: u64) -> Result<()> {
        if self.display(id, now).requestability != Decision::Allowed {
            return Err(error(
                "unavailable",
                "目录已失效、权限不足或目标不适用；无法提交",
            ));
        }
        Ok(())
    }
    fn catalog_views(&self, now: u64) -> Result<Vec<CatalogView>> {
        self.catalog
            .snapshot()
            .items
            .iter()
            .map(|item| {
                let operation = &item.operations[0];
                let projection =
                    self.catalog
                        .projection(&item.id, &operation.id, &fixtures::PARAMETERS)?;
                let availability = if item.state == service_catalog::PublicationState::Withdrawn {
                    Availability::Withdrawn
                } else if now >= self.catalog.snapshot().expires_at_unix_ms {
                    Availability::Expired
                } else {
                    Availability::Listed
                };
                Ok(CatalogView {
                    catalog: self.catalog.reference(),
                    item_id: item.id.clone(),
                    variant_id: operation.id.clone(),
                    kind: item.kind,
                    name: item.name.clone(),
                    description: item.description.clone(),
                    category: item.category.clone(),
                    resource: operation.resource.clone(),
                    fields: projection.fields().clone(),
                    input_schema: projection.input_schema().clone(),
                    display: self.display(&item.id, now),
                    reason: if availability == Availability::Expired {
                        "目录已过期，只能浏览".into()
                    } else {
                        item.description.clone()
                    },
                    availability,
                })
            })
            .collect()
    }
    fn plan_view(&self, record: &RequestRecord) -> Result<PlanView> {
        let prepared = record
            .prepared
            .as_ref()
            .ok_or_else(|| error("stale", "无可用计划"))?;
        let item = self
            .catalog
            .snapshot()
            .items
            .iter()
            .find(|i| i.id == prepared.selection.item_id)
            .expect("validated catalog item");
        let operation = &item.operations[0];
        let spec = prepared.plan.spec();
        let parameters = operation
            .parameters
            .iter()
            .map(|(key, field)| ParameterSummary {
                label: field.title.clone(),
                state: if matches!(
                    field.rule,
                    service_catalog::ParameterRule::SecretReference {}
                ) {
                    if spec.request.parameters.contains_key(key.as_str()) {
                        "秘密引用已提供（不回显）"
                    } else {
                        "未提供"
                    }
                } else if prepared.supplied.contains(key.as_str()) {
                    "已提供"
                } else if spec.request.parameters.contains_key(key.as_str()) {
                    "使用目录默认值"
                } else {
                    "未提供"
                },
            })
            .collect();
        Ok(PlanView {
            request_id: spec.request.request_id.clone(),
            revision: record.revision,
            plan_id: spec.plan_id.clone(),
            digest: prepared.plan.digest().clone(),
            item_id: item.id.clone(),
            title: item.name.clone(),
            action: spec.request.operation.action.clone(),
            resource: operation.resource.clone(),
            target: fixtures::TARGET.into(),
            run_as: "测试用户 fixture-user；不使用宿主登录身份".into(),
            network: "禁止网络；不会发起连接".into(),
            data_scope: "无文件读写、无子进程；仅内存测试数据".into(),
            permission: if item.id.as_str() == "office" {
                "需管理员批准；本服务不签发批准".into()
            } else {
                "固定测试场景允许；不代表真实执行授权".into()
            },
            parameters,
            expires_at_unix_ms: spec.validity.expires_at_unix_ms,
        })
    }
    fn request_view(&self, record: &RequestRecord) -> Result<RequestView> {
        let status = record.result.unwrap_or(
            if record.interactions.iter().any(|i| {
                matches!(i.snapshot().status, Status::Pending)
                    && matches!(
                        i.snapshot().spec.kind,
                        execution_interaction::Kind::AdministratorAuthorization { .. }
                    )
            }) {
                RequestStatus::Approval
            } else {
                RequestStatus::Waiting
            },
        );
        let message = match status {
            RequestStatus::Complete => "测试流程完成；没有安装软件或修改系统",
            RequestStatus::Stopped => "测试流程停止；不代表后台任务已取消",
            RequestStatus::RestartRequired => "待重启提示已记录；没有执行或安排重启",
            RequestStatus::UnknownEffect => "固定未知效果样本：等待核实，不能宣称成功或自动重跑",
            RequestStatus::Approval => "等待管理员批准；普通用户不能在此批准",
            RequestStatus::Waiting => "等待用户处理交互",
        };
        Ok(RequestView {
            plan: self.plan_view(record)?,
            status,
            message,
            interactions: record.interactions.iter().map(interaction::view).collect(),
        })
    }
}
#[cfg(test)]
mod tests;

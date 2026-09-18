use super::{model::*, selection, RequestRecord};
use execution_interaction::{
    Command, ConfirmationPurpose, Interaction, Kind, Limits, Outcome, Reference, Response, Spec,
    Status,
};
use service_catalog::FrozenCatalog;

fn reference(value: impl Into<String>) -> Result<Reference> {
    Ok(Reference::new(value)?)
}
fn open(record: &mut RequestRecord, kind: Kind, now: u64) -> Result<()> {
    let plan = &record.prepared.as_ref().expect("prepared").plan;
    let id = format!(
        "interaction-{}-{}",
        plan.spec().plan_id.as_str(),
        record.interactions.len()
    );
    let interaction = Interaction::open(
        Spec {
            id: reference(id)?,
            subject: reference(plan.spec().plan_id.as_str())?,
            kind,
            expires_at_unix_ms: plan.spec().validity.expires_at_unix_ms,
        },
        now,
        Limits {
            max_snapshot_bytes: 8192,
            max_lifetime_ms: 300_000,
        },
    )?;
    record.interactions.push(interaction);
    Ok(())
}
pub(super) fn start(record: &mut RequestRecord, now: u64) -> Result<()> {
    let item = record
        .prepared
        .as_ref()
        .expect("prepared")
        .selection
        .item_id
        .as_str();
    let kind = match item {
        "office" => Kind::AdministratorAuthorization {
            request: reference("fixture-admin-request")?,
        },
        "restart" => Kind::RestartPrompt {
            options: reference("fixture-restart-r1")?,
        },
        "maintenance" => Kind::MaintenanceWindow {
            options: reference("fixture-maintenance-r1")?,
        },
        "parameter-check" => Kind::ParameterInput {
            schema: reference("fixture-catalog-r1")?,
        },
        "unknown" => {
            record.result = Some("unknownEffect");
            return Ok(());
        }
        _ => Kind::UserConfirmation {
            purpose: ConfirmationPurpose::Continue,
        },
    };
    open(record, kind, now)
}
pub(super) fn expire(record: &mut RequestRecord, now: u64) -> Result<()> {
    for current in &mut record.interactions {
        if matches!(current.snapshot().status, Status::Pending) {
            if let Some(transition) = current.evaluate(Command::CheckExpiry {}, now)?.transition {
                *current = transition.next;
                record.result = Some("stopped");
            }
        }
    }
    Ok(())
}
pub(super) fn respond(
    catalog: &FrozenCatalog,
    record: &mut RequestRecord,
    input: &Reply,
    now: u64,
) -> Result<()> {
    let index = record
        .interactions
        .iter()
        .position(|i| i.snapshot().spec.id.as_str() == input.interaction_id)
        .ok_or_else(|| error("notFound", "交互不属于此请求"))?;
    let kind = record.interactions[index].snapshot().spec.kind.clone();
    let response = match (&kind, &input.answer) {
        (_, Answer::Cancel {}) => None,
        (Kind::UserConfirmation { .. }, Answer::Confirmation { accepted }) => {
            Some(Response::Confirmation {
                accepted: *accepted,
            })
        }
        (Kind::PrivacyConsent { .. }, Answer::PrivacyConsent { accepted }) => {
            Some(Response::PrivacyConsent {
                accepted: *accepted,
            })
        }
        (Kind::MaintenanceWindow { .. }, Answer::Choice { selection })
            if matches!(selection.as_str(), "morning" | "evening") =>
        {
            Some(Response::MaintenanceSelection {
                selection: reference(selection)?,
            })
        }
        (Kind::RestartPrompt { .. }, Answer::Choice { selection })
            if matches!(selection.as_str(), "later" | "acknowledged") =>
        {
            Some(Response::RestartSelection {
                selection: reference(selection)?,
            })
        }
        (Kind::ParameterInput { .. }, Answer::Parameters { fields }) => {
            let prepared = record.prepared.as_ref().expect("prepared");
            let draft = Draft {
                instance_id: input.instance_id.clone(),
                request_id: input.request_id.clone(),
                revision: record.revision,
                catalog: prepared.selection.catalog.clone(),
                item_id: prepared.selection.item_id.clone(),
                variant_id: prepared.selection.variant_id.clone(),
                fields: fields.clone(),
            };
            let bytes = serde_json::to_vec(&draft).map_err(|_| error("input", "参数编码失败"))?;
            if bytes.len() > 16384 {
                return Err(error("limit", "参数输入超过预算"));
            }
            let selected = selection::select(catalog, &draft)?;
            if selected.reference() != &prepared.selection {
                return Err(error("conflict", "参数改变了已接纳计划；请新建请求"));
            }
            Some(Response::ParameterSubmission {
                submission: reference(prepared.selection.arguments_digest.as_str())?,
            })
        }
        _ => {
            return Err(error(
                "interaction",
                "回答类型或选项不匹配；管理员批准不可由此入口提交",
            ))
        }
    };
    let command = match response {
        Some(response) => Command::Answer {
            id: input.command_id.clone(),
            response,
        },
        None => Command::Cancel {
            id: input.command_id.clone(),
        },
    };
    let evaluation = record.interactions[index].evaluate(command, now)?;
    let outcome = evaluation.outcome;
    if let Some(transition) = evaluation.transition {
        record.interactions[index] = transition.next;
    }
    match outcome {
        Outcome::Cancelled | Outcome::Expired => record.result = Some("stopped"),
        Outcome::Answered => match input.answer {
            Answer::Confirmation { accepted: false }
            | Answer::PrivacyConsent { accepted: false } => record.result = Some("stopped"),
            Answer::Confirmation { accepted: true } => open(
                record,
                Kind::PrivacyConsent {
                    scope: reference("fixture-privacy-r1")?,
                },
                now,
            )?,
            _ => {
                record.result = Some(if matches!(kind, Kind::RestartPrompt { .. }) {
                    "restartRequired"
                } else {
                    "complete"
                })
            }
        },
        // A repeated/late answer only observes the current record; never advances a second time.
        Outcome::Duplicate | Outcome::Late | Outcome::NotDue => {}
    }
    Ok(())
}
pub(super) fn view(interaction: &Interaction) -> InteractionView {
    let snapshot = interaction.snapshot();
    let (message, options) = match snapshot.spec.kind {
        Kind::UserConfirmation { .. } => {
            ("确认已阅读精确测试计划；此确认不构成管理员授权。", vec![])
        }
        Kind::PrivacyConsent { .. } => (
            "同意在当前进程内存中处理测试参数；不发送网络、不写入文件。",
            vec![],
        ),
        Kind::AdministratorAuthorization { .. } => (
            "等待外部管理员批准。本测试服务不签发批准，也不提供自我批准入口。",
            vec![],
        ),
        Kind::ParameterInput { .. } => (
            "请重新填写原计划参数进行复核；修改有效参数需要新建请求。",
            vec![],
        ),
        Kind::MaintenanceWindow { .. } => (
            "选择测试维护窗口；不会安排系统任务。",
            vec![
                Choice {
                    id: "morning",
                    label: "上午测试窗口",
                },
                Choice {
                    id: "evening",
                    label: "晚间测试窗口",
                },
            ],
        ),
        Kind::RestartPrompt { .. } => (
            "待重启仅为固定提示；选择不会触发重启。",
            vec![
                Choice {
                    id: "later",
                    label: "稍后提醒",
                },
                Choice {
                    id: "acknowledged",
                    label: "已阅读提示",
                },
            ],
        ),
    };
    InteractionView {
        id: snapshot.spec.id.as_str().into(),
        kind: snapshot.spec.kind.clone(),
        status: match snapshot.status {
            Status::Pending => "pending",
            Status::Answered { .. } => "answered",
            Status::Cancelled { .. } => "cancelled",
            Status::Expired { .. } => "expired",
        },
        message,
        expires_at_unix_ms: snapshot.spec.expires_at_unix_ms,
        options,
    }
}

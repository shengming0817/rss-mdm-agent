use super::plan;
use agent_client::{wire, Error, Offer};
pub(crate) fn offered(offer: &Offer) -> Result<execution_contract::BackendTask, Error> {
    use sha2::{Digest as _, Sha256};
    let bytes = serde_json::to_vec(offer.payload()).map_err(|_| Error::Protocol)?;
    Ok(execution_contract::BackendTask {
        summary: match offer.payload() {
            wire::TaskPayload::Script(p) => execution_contract::BackendTaskSummary::Script {
                identity: display_identity(p.run_as),
            },
            wire::TaskPayload::Software(p) => execution_contract::BackendTaskSummary::Software {
                intent: match p.intent {
                    wire::SoftwareTaskIntent::Install => {
                        execution_contract::SoftwareOperation::Install
                    }
                    wire::SoftwareTaskIntent::Uninstall => {
                        execution_contract::SoftwareOperation::Uninstall
                    }
                    wire::SoftwareTaskIntent::Detect => {
                        execution_contract::SoftwareOperation::Detect
                    }
                },
                steps: p
                    .steps
                    .iter()
                    .map(|step| execution_contract::BackendStepSummary {
                        package: step.action.package.clone(),
                        version: step.action.version.clone(),
                        identity: display_identity(match step.target {
                            wire::SoftwareExecutionTarget::Device => {
                                wire::ExecutionIdentity::System
                            }
                            wire::SoftwareExecutionTarget::User { .. } => {
                                wire::ExecutionIdentity::LoggedInUser
                            }
                        }),
                    })
                    .collect(),
            },
            _ => return Err(Error::Unsupported),
        },
        request: super::request_id(offer)?,
        task: plan::id(offer.task_id().to_string())?,
        attempt: plan::id(offer.attempt_id().to_string())?,
        revision: execution_contract::Digest::new(format!("{:x}", Sha256::digest(bytes)))
            .map_err(|_| Error::Protocol)?,
        title: match offer.payload() {
            wire::TaskPayload::Software(p) => p
                .steps
                .last()
                .map(|s| format!("{} {}", s.action.package, s.action.version))
                .ok_or(Error::Protocol)?,
            _ => format!("Task {}", offer.task_id()),
        },
        expires_at: offer.payload().expires_at(),
        user_initiated: matches!(offer.payload(),wire::TaskPayload::Software(p) if p.start_mode==wire::SoftwareStartMode::UserInitiated),
    })
}

fn display_identity(identity: wire::ExecutionIdentity) -> execution_contract::BackendIdentity {
    match identity {
        wire::ExecutionIdentity::System => execution_contract::BackendIdentity::System,
        wire::ExecutionIdentity::LoggedInUser => execution_contract::BackendIdentity::User,
    }
}

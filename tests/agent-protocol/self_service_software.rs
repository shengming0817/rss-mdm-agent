//! Explicit native-lab participant for the current directory/invocation contract.
use super::*;

#[derive(Clone)]
pub struct Template {
    pub spec: SoftwareTaskSpec,
}

pub fn idle(data: &Data) -> bool {
    data.offer.is_none()
        || data.results.iter().any(|(id, value)| {
            data.acknowledged.contains(id) && value["attemptId"] == data.attempt.to_string()
        })
}

pub fn publish(
    data: &mut Data,
    path: &str,
    receipt: &str,
) -> Result<SelfServiceDirectoryItem, &'static str> {
    if !idle(data) {
        return Err("current task has not settled");
    }
    let bytes = std::fs::read(path).map_err(|_| "package unavailable")?;
    data.task = Uuid::new_v4();
    data.attempt = Uuid::new_v4();
    data.bytes = bytes;
    data.software(1, false);
    let TaskPayload::Software(mut spec) = data.offer.take().unwrap().payload else {
        unreachable!()
    };
    spec.expires_at = data.time.now().unwrap() + 3600;
    let SoftwareTaskBehavior::Pkg(native) = &mut spec.steps[0].action.behavior else {
        unreachable!()
    };
    let SoftwareTaskDetection::PkgReceipt {
        receipt: target, ..
    } = &mut native.detect
    else {
        unreachable!()
    };
    *target = receipt.into();
    spec.definition_digest = Sha256::digest(serde_json::to_vec(&spec.steps).unwrap()).into();
    let mut item = policy_directory_fixture(data.tenant);
    item.subject = SelfServiceSubject::Device {};
    item.self_service.presentation = serde_json::from_value(json!({
        "displayName":"受控软件", "description":"安装受控验收软件包", "category":"验收",
        "prerequisites":"当前本机登录", "sideEffects":"安装固定软件包", "keywords":["software"]
    }))
    .unwrap();
    item.task = SelfServiceTask::Software {
        resource: TaskResource {
            id: "native-software".into(),
            version: spec.steps[0].action.version.clone(),
            variant: "default".into(),
        },
        run_as: ExecutionIdentity::System,
        target: SoftwareExecutionTarget::Device,
    };
    data.self_service_software = Some(Template { spec: *spec });
    data.self_service_directory = Some(SelfServiceDirectoryResponse {
        items: vec![item.clone()],
        next_cursor: None,
        as_of: data.time.now().unwrap(),
    });
    data.explicit_offers = true;
    Ok(item)
}

pub fn offer(
    data: &Data,
    input: &SelfServiceInvoke,
    accepted: &SelfServiceAccepted,
) -> Result<Option<SignedTask>, StatusCode> {
    let Some(template) = &data.self_service_software else {
        return Ok(None);
    };
    let item = data
        .self_service_directory
        .as_ref()
        .and_then(|page| {
            page.items
                .iter()
                .find(|item| item.policy_id == input.policy_id)
        })
        .ok_or(StatusCode::FORBIDDEN)?;
    if input.operation_id.is_nil()
        || input.policy_revision != item.policy_revision
        || input.policy_version != item.policy_version
        || !item.self_service.published
        || !input.parameters.is_empty()
        || item.self_service.risk_level != 2
        || template.spec.expires_at <= data.time.now().unwrap()
        || (matches!(input.source, SelfServiceCaller::Ai)
            && (!item.self_service.allow_ai || !input.confirmed))
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if !idle(data) {
        return Err(StatusCode::CONFLICT);
    }
    let SelfServiceTask::Software {
        resource,
        run_as,
        target,
    } = &item.task
    else {
        return Err(StatusCode::FORBIDDEN);
    };
    let step = template.spec.steps.last().ok_or(StatusCode::BAD_REQUEST)?;
    let SoftwareTaskBehavior::Pkg(native) = &step.action.behavior else {
        return Err(StatusCode::FORBIDDEN);
    };
    if template.spec.intent != SoftwareTaskIntent::Install
        || resource.version != step.action.version
        || run_as != &native.install.run_as
        || target != &step.target
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut spec = template.spec.clone();
    spec.task_id = accepted.run_id;
    spec.attempt_id = Uuid::new_v4();
    spec.start_mode = SoftwareStartMode::UserInitiated;
    spec.authorization = TaskAuthorization::SelfService {
        request_id: input.operation_id,
        subject: item.subject.clone(),
        source: input.source,
        policy_id: item.policy_id,
        policy_revision: item.policy_revision,
        policy_version: item.policy_version,
        allow_ai: item.self_service.allow_ai,
        risk_level: item.self_service.risk_level,
        confirmed: input.confirmed,
    };
    let payload = TaskPayload::try_from(spec).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Some(data.signed(payload)))
}

pub fn status(
    data: &Data,
    request: Uuid,
    accepted: &SelfServiceAccepted,
) -> Option<SelfServiceStatus> {
    let (offer, received_at) = data.self_service_offers.get(&request)?;
    let TaskPayload::Software(spec) = &offer.payload else {
        return None;
    };
    let result = data.results.iter().find_map(|(id, result)| {
        (data.acknowledged.contains(id) && result["attemptId"] == spec.attempt_id.to_string())
            .then(|| serde_json::from_value::<TaskEvent>(result["event"].clone()).ok())
            .flatten()
    });
    let effect = match &result {
        Some(TaskEvent::SoftwareResult(result))
            if matches!(
                agent_client::protocol::assess_software_result(result, spec),
                Ok(agent_client::protocol::SoftwareEvidenceAssessment::Satisfied)
            ) =>
        {
            SelfServiceSoftwareEffect::Verified
        }
        _ => SelfServiceSoftwareEffect::Unknown,
    };
    let execution = if matches!(effect, SelfServiceSoftwareEffect::Verified) {
        TaskExecution::Succeeded
    } else if result.is_some() {
        TaskExecution::Unknown
    } else if data.task == spec.task_id && data.started {
        TaskExecution::Running
    } else {
        TaskExecution::NotStarted
    };
    Some(SelfServiceStatus {
        request_id: request,
        run_id: accepted.run_id,
        attempt_id: Some(spec.attempt_id),
        state: TaskRunState {
            delivery: TaskDelivery::Received {
                attempt: spec.attempt_id,
                lease_until: spec.expires_at,
            },
            execution,
            cancellation: if matches!(result, Some(TaskEvent::Cancelled)) {
                RunCancellation::Confirmed
            } else {
                RunCancellation::None
            },
            deadline: spec.expires_at,
            started_at: None,
        },
        available_at: *received_at,
        deadline: spec.expires_at,
        result: result.map(|_| SelfServiceResult::Software { effect }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn software_publication_waits_for_an_exact_invocation_and_replays_its_original_offer() {
        let server = Server::new().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        server.register(&mut client).await;
        let package = root.path.join("fixture.pkg");
        std::fs::write(
            &package,
            b"explicit protocol fixture, not an installed package",
        )
        .unwrap();
        let mut data = server.data.lock().unwrap();
        let item = publish(&mut data, package.to_str().unwrap(), "org.rss.fixture").unwrap();
        assert!(data.offer.is_none());
        assert!(data.invocations.is_empty());
        let input = SelfServiceInvoke {
            operation_id: Uuid::new_v4(),
            policy_id: item.policy_id,
            policy_revision: item.policy_revision,
            policy_version: item.policy_version,
            source: SelfServiceCaller::Human,
            confirmed: false,
            parameters: BTreeMap::new(),
        };
        let uri = Uri::from_static("/self-service/invocations");
        let headers = HeaderMap::new();
        for field in 0..6 {
            let mut invalid = input.clone();
            match field {
                0 => invalid.operation_id = Uuid::nil(),
                1 => invalid.policy_id = Uuid::new_v4(),
                2 => invalid.policy_revision += 1,
                3 => invalid.policy_version = Uuid::new_v4(),
                4 => {
                    invalid
                        .parameters
                        .insert("command".into(), json!("unpublished"));
                }
                _ => invalid.source = SelfServiceCaller::Ai,
            }
            let response = super::super::self_service_response(
                &mut data,
                &uri,
                &headers,
                serde_json::to_value(invalid).unwrap(),
            );
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert!(data.invocations.is_empty());
            assert!(data.offer.is_none());
        }
        data.invocation_reply_lost = true;
        let body = serde_json::to_value(&input).unwrap();
        assert_eq!(
            super::super::self_service_response(&mut data, &uri, &headers, body.clone()).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let accepted = data.invocations[&input.operation_id].1.clone();
        let original = serde_json::to_vec(&data.offer).unwrap();
        let TaskPayload::Software(spec) = &data.offer.as_ref().unwrap().payload else {
            panic!("software")
        };
        assert_eq!(spec.task_id, accepted.run_id);
        assert!(
            matches!(spec.authorization, TaskAuthorization::SelfService { request_id, source: SelfServiceCaller::Human, confirmed: false, .. } if request_id == input.operation_id)
        );
        assert_eq!(
            super::super::self_service_response(&mut data, &uri, &headers, body.clone()).status(),
            StatusCode::ACCEPTED
        );
        assert_eq!(serde_json::to_vec(&data.offer).unwrap(), original);
        assert_eq!(data.self_service_offers.len(), 1);
        let mut changed = input.clone();
        changed.confirmed = true;
        assert_eq!(
            super::super::self_service_response(
                &mut data,
                &uri,
                &headers,
                serde_json::to_value(changed).unwrap()
            )
            .status(),
            StatusCode::CONFLICT
        );
        let mut other = input.clone();
        other.operation_id = Uuid::new_v4();
        assert_eq!(
            super::super::self_service_response(
                &mut data,
                &uri,
                &headers,
                serde_json::to_value(other).unwrap()
            )
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(data.invocations.len(), 1);
        let TaskPayload::Software(spec) = data.self_service_offers[&input.operation_id]
            .0
            .payload
            .clone()
        else {
            panic!("software");
        };
        let step = &spec.steps[0];
        let SoftwareTaskBehavior::Pkg(native) = &step.action.behavior else {
            panic!("pkg");
        };
        let SoftwareTaskDetection::PkgReceipt { receipt, .. } = &native.detect else {
            panic!("receipt");
        };
        let now = data.time.now().unwrap();
        let valid = SoftwareTaskResult {
            intent: spec.intent,
            definition_digest: spec.definition_digest,
            steps: vec![SoftwareStepResult {
                index: 0,
                step_digest: step.digest().unwrap(),
                target: step.target.clone(),
                package: step.action.package.clone(),
                identity: SoftwareObservedIdentity::PkgReceipt {
                    receipt: receipt.clone(),
                },
                before: SoftwareDetectionObservation::Absent {
                    evidence_sha256: [1; 32],
                    observed_at: now,
                },
                after: SoftwareDetectionObservation::Present {
                    version: step.action.version.clone(),
                    evidence_sha256: [2; 32],
                    observed_at: now,
                },
                process: SoftwareProcessObservation::Exited { code: 0 },
                reboot_required: false,
                diagnostics: TaskDiagnostics::new(String::new(), String::new(), 1, now, None)
                    .unwrap(),
            }],
        };
        let mut wrong_identity = valid.clone();
        wrong_identity.steps[0].identity = SoftwareObservedIdentity::PkgReceipt {
            receipt: "org.rss.wrong".into(),
        };
        let mut wrong_digest = valid.clone();
        wrong_digest.steps[0].step_digest[0] ^= 1;
        for (result, verified) in [
            (valid, true),
            (wrong_identity, false),
            (wrong_digest, false),
        ] {
            result.validate().unwrap();
            let event_id = Uuid::new_v4();
            let envelope = TaskEventRequest::new(
                event_id,
                spec.attempt_id,
                TaskEvent::SoftwareResult(result),
                spec.execution_context.clone(),
            )
            .unwrap();
            data.results.clear();
            data.acknowledged.clear();
            data.results.insert(
                event_id.to_string(),
                serde_json::to_value(envelope).unwrap(),
            );
            data.acknowledged.insert(event_id.to_string());
            let projected = status(&data, input.operation_id, &accepted).unwrap();
            assert_eq!(
                matches!(
                    projected.result,
                    Some(SelfServiceResult::Software {
                        effect: SelfServiceSoftwareEffect::Verified
                    })
                ),
                verified
            );
            assert_eq!(
                matches!(projected.state.execution, TaskExecution::Succeeded),
                verified
            );
        }
        data.offer = None;
        assert_eq!(
            super::super::self_service_response(&mut data, &uri, &headers, body).status(),
            StatusCode::ACCEPTED
        );
        assert!(
            data.offer.is_none(),
            "replay must not republish a consumed offer"
        );
    }
}

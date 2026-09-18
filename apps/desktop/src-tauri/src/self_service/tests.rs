use super::*;

#[test]
fn fixture_snapshot_never_claims_real_execution() {
    let mut service = FixtureService::new("fixture-test".into(), 1000).unwrap();
    let snapshot = service.snapshot(1000).unwrap();
    assert_eq!(snapshot.mode, "fixture");
    assert!(snapshot
        .catalog
        .iter()
        .any(|item| item.item_id.as_str() == "office"));
    assert!(snapshot.requests.is_empty());
}

fn setup(item: &str) -> (FixtureService, Draft) {
    let mut service = FixtureService::new("fixture-test".into(), 1000).unwrap();
    let view = service
        .snapshot(1000)
        .unwrap()
        .catalog
        .into_iter()
        .find(|v| v.item_id.as_str() == item)
        .unwrap();
    let mut fields = std::collections::BTreeMap::new();
    if matches!(item, "diagnostics" | "parameter-check") {
        fields.insert(
            "host".into(),
            FieldInput::Text {
                value: "example.invalid".into(),
            },
        );
    }
    let draft = Draft {
        instance_id: "fixture-test".into(),
        request_id: execution_contract::RequestId::new("request-1").unwrap(),
        revision: 1,
        catalog: view.catalog,
        item_id: view.item_id,
        variant_id: view.variant_id,
        fields,
    };
    (service, draft)
}
fn submission(plan: &PlanView) -> Submission {
    Submission {
        instance_id: "fixture-test".into(),
        request_id: plan.request_id.clone(),
        plan_id: plan.plan_id.clone(),
        digest: plan.digest.clone(),
    }
}
fn reply(task: &RequestView, answer: Answer) -> Reply {
    Reply {
        instance_id: "fixture-test".into(),
        request_id: task.plan.request_id.clone(),
        interaction_id: task.interactions.last().unwrap().id.clone(),
        command_id: execution_interaction::Reference::new("answer-1").unwrap(),
        answer,
    }
}
#[test]
fn defaults_precision_and_secrets_cross_the_same_catalog_runtime() {
    let (mut service, mut draft) = setup("diagnostics");
    let plan = service.preview(draft.clone(), 1000).unwrap();
    assert!(plan
        .parameters
        .iter()
        .any(|p| p.label == "采样次数" && p.state == "使用目录默认值"));
    draft.revision = 2;
    draft.fields.insert(
        "count".into(),
        FieldInput::Integer {
            value: "3.0".into(),
        },
    );
    let same = service.preview(draft.clone(), 1001).unwrap();
    assert_eq!(same.plan_id, plan.plan_id);
    draft.revision = 3;
    draft.fields.insert(
        "count".into(),
        FieldInput::Integer {
            value: "3.0000000000000000001".into(),
        },
    );
    assert!(service
        .preview(draft.clone(), 1002)
        .err()
        .unwrap()
        .message
        .contains("RoundedNumber"));
    assert_eq!(
        service.submit(submission(&plan), 1003).err().unwrap().code,
        "stale"
    );
    draft.revision = 4;
    draft.fields.remove("count");
    draft.fields.insert(
        "credential".into(),
        FieldInput::SecretReference {
            id: "secret-reference-canary".into(),
            revision: "r1".into(),
        },
    );
    let plan = service.preview(draft, 1004).unwrap();
    let task = service.submit(submission(&plan), 1005).unwrap();
    let output = serde_json::to_string(&task).unwrap();
    assert!(!output.contains("secret-reference-canary"));
    assert!(!output.contains("example.invalid"));
}
#[test]
fn request_identity_survives_retries_but_not_plan_substitution() {
    let (mut service, mut draft) = setup("office");
    let first = service.preview(draft.clone(), 1000).unwrap();
    draft.revision = 2;
    draft.fields.insert(
        "edition".into(),
        FieldInput::Text {
            value: "professional".into(),
        },
    );
    let changed = service.preview(draft.clone(), 1001).unwrap();
    assert_eq!(first.request_id, changed.request_id);
    assert_ne!(first.plan_id, changed.plan_id);
    assert_eq!(
        service.submit(submission(&first), 1002).err().unwrap().code,
        "conflict"
    );
    let task = service.submit(submission(&changed), 1003).unwrap();
    assert_eq!(task.status, "approval");
    assert_eq!(
        service
            .submit(submission(&changed), 1004)
            .unwrap()
            .plan
            .plan_id,
        changed.plan_id
    );
    assert_eq!(service.snapshot(1004).unwrap().requests.len(), 1);
    assert_eq!(service.preview(draft, 1004).err().unwrap().code, "accepted");
    assert!(service
        .respond(reply(&task, Answer::Confirmation { accepted: true }), 1004)
        .is_err());
    assert_eq!(
        service.snapshot(1004).unwrap().requests[0].status,
        "approval"
    );
}
#[test]
fn confirmations_consent_and_late_answers_do_not_grant_authority() {
    let (mut service, draft) = setup("diagnostics");
    let plan = service.preview(draft, 1000).unwrap();
    let first = service.submit(submission(&plan), 1000).unwrap();
    assert!(service
        .respond(
            reply(&first, Answer::PrivacyConsent { accepted: true }),
            1001
        )
        .is_err());
    let answer = reply(&first, Answer::Confirmation { accepted: true });
    let consent = service.respond(answer.clone(), 1001).unwrap();
    assert_eq!(consent.interactions.len(), 2);
    assert_eq!(
        service
            .respond(answer.clone(), 1002)
            .unwrap()
            .interactions
            .len(),
        2
    );
    let mut conflict = answer;
    conflict.answer = Answer::Confirmation { accepted: false };
    assert!(service.respond(conflict, 1002).is_err());
    let done = service
        .respond(
            reply(&consent, Answer::PrivacyConsent { accepted: true }),
            1003,
        )
        .unwrap();
    assert_eq!(done.status, "complete");
    assert!(done.message.contains("没有安装"));
}
#[test]
fn refusal_expiry_cancel_and_clock_regression_never_complete() {
    for answer in [Answer::Cancel {}, Answer::Confirmation { accepted: false }] {
        let (mut service, draft) = setup("diagnostics");
        let plan = service.preview(draft, 1000).unwrap();
        let task = service.submit(submission(&plan), 1000).unwrap();
        assert_eq!(
            service.respond(reply(&task, answer), 1001).unwrap().status,
            "stopped"
        );
    }
    let (mut service, draft) = setup("diagnostics");
    let plan = service.preview(draft, 1000).unwrap();
    let task = service.submit(submission(&plan), 1000).unwrap();
    assert_eq!(
        service
            .respond(
                reply(&task, Answer::Confirmation { accepted: true }),
                301000
            )
            .unwrap()
            .status,
        "stopped"
    );
    assert_eq!(
        service
            .submit(submission(&plan), 301001)
            .unwrap()
            .plan
            .plan_id,
        plan.plan_id
    );
    assert_eq!(service.snapshot(1000).err().unwrap().code, "clock");
}
#[test]
fn directory_and_instance_fail_closed_and_plan_is_immutable() {
    for item in ["blocked", "unsupported", "withdrawn"] {
        let (mut service, draft) = setup(item);
        assert!(service.preview(draft, 1000).is_err());
    }
    let (mut service, mut draft) = setup("office");
    draft.instance_id = "other-instance".into();
    assert_eq!(
        service.preview(draft.clone(), 1000).err().unwrap().code,
        "instance"
    );
    draft.instance_id = "fixture-test".into();
    assert_eq!(
        service.preview(draft, 3601000).err().unwrap().code,
        "unavailable"
    );
    assert!(service
        .snapshot(3601000)
        .unwrap()
        .catalog
        .iter()
        .all(|v| v.display.requestability != service_catalog::DisplayDecision::Allowed));
}
#[test]
fn prompt_options_and_parameter_reentry_are_service_validated() {
    for (item, answer, status) in [
        (
            "restart",
            Answer::Choice {
                selection: "later".into(),
            },
            "restartRequired",
        ),
        (
            "maintenance",
            Answer::Choice {
                selection: "morning".into(),
            },
            "complete",
        ),
    ] {
        let (mut service, draft) = setup(item);
        let plan = service.preview(draft, 1000).unwrap();
        let task = service.submit(submission(&plan), 1000).unwrap();
        assert!(service
            .respond(
                reply(
                    &task,
                    Answer::Choice {
                        selection: "arbitrary".into()
                    }
                ),
                1001
            )
            .is_err());
        assert_eq!(
            service.respond(reply(&task, answer), 1001).unwrap().status,
            status
        );
    }
    let (mut service, draft) = setup("parameter-check");
    let fields = draft.fields.clone();
    let plan = service.preview(draft, 1000).unwrap();
    let task = service.submit(submission(&plan), 1000).unwrap();
    assert!(service
        .respond(
            reply(
                &task,
                Answer::Parameters {
                    fields: Default::default()
                }
            ),
            1001
        )
        .is_err());
    assert_eq!(
        service
            .respond(reply(&task, Answer::Parameters { fields }), 1001)
            .unwrap()
            .status,
        "complete"
    );
}
#[test]
fn unknown_effect_is_preserved_and_snapshot_does_not_cancel() {
    let (mut service, draft) = setup("unknown");
    let plan = service.preview(draft, 1000).unwrap();
    let task = service.submit(submission(&plan), 1000).unwrap();
    assert_eq!(task.status, "unknownEffect");
    assert_eq!(
        service.snapshot(1001).unwrap().requests[0].status,
        "unknownEffect"
    );
    let (mut service, draft) = setup("office");
    let plan = service.preview(draft, 1000).unwrap();
    service.submit(submission(&plan), 1000).unwrap();
    assert_eq!(
        service.snapshot(1001).unwrap().requests[0].status,
        "approval"
    );
}

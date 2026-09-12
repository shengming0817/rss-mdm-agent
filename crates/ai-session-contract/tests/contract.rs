use ai_session_contract::{decode_event, Event, SessionLimits};

fn limits() -> SessionLimits {
    SessionLimits {
        max_input_bytes: 65536,
        max_text_bytes: 4096,
        max_content_parts: 32,
        max_argument_bytes: 8192,
    }
}
const CANCEL: &[u8] = br#"{"schemaVersion":1,"conversationId":"conversation-1","sequence":1,"event":{"kind":"cancelDispatched","turnId":"turn-1","outcome":"accepted"}}"#;
#[test]
fn accepted_cancel_is_not_a_terminal_event() {
    let event = decode_event(CANCEL, &limits()).unwrap();
    assert!(matches!(event.event, Event::CancelDispatched { .. }));
    assert!(event.terminal_outcome().is_none());
}
#[test]
fn unknown_events_versions_and_trusted_fields_are_rejected() {
    let source = std::str::from_utf8(CANCEL).unwrap();
    for input in [
        source.replace("cancelDispatched", "newEvent"),
        source.replace("\"schemaVersion\":1", "\"schemaVersion\":2"),
        source.replacen('{', "{\"approved\":true,", 1),
    ] {
        assert!(decode_event(input.as_bytes(), &limits()).is_err());
    }
}

fn fixtures() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("fixtures/events.json")).unwrap()
}
fn conversation() -> ai_session_contract::Conversation {
    serde_json::from_value(fixtures()[0]["event"]["conversation"].clone()).unwrap()
}
#[test]
fn schemas_and_encoding_roundtrip_all_protocol_cases() {
    let schemas = serde_json::json!({"event":ai_session_contract::event_schema(),"command":ai_session_contract::command_schema()});
    assert_eq!(
        schemas,
        serde_json::from_str::<serde_json::Value>(include_str!("fixtures/schemas.json")).unwrap()
    );
    let validator = jsonschema::validator_for(&schemas["event"]).unwrap();
    for value in fixtures() {
        assert!(validator.is_valid(&value));
        let decoded = decode_event(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
        let mut invalid = value;
        invalid["authorized"] = true.into();
        assert!(!validator.is_valid(&invalid));
    }
    let commands: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/commands.json")).unwrap();
    let validator = jsonschema::validator_for(&schemas["command"]).unwrap();
    for value in commands {
        assert!(validator.is_valid(&value));
        let decoded =
            ai_session_contract::decode_command(&serde_json::to_vec(&value).unwrap(), &limits())
                .unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    }
}
#[test]
fn cancellation_tail_and_transport_loss_do_not_confirm_termination() {
    let events = fixtures();
    for i in [5, 6, events.len() - 1] {
        let event = decode_event(&serde_json::to_vec(&events[i]).unwrap(), &limits()).unwrap();
        assert_eq!(event.terminal_outcome(), None);
    }
    let interrupted = decode_event(&serde_json::to_vec(&events[8]).unwrap(), &limits()).unwrap();
    assert_eq!(
        interrupted.terminal_outcome(),
        Some(ai_session_contract::TurnOutcome::Interrupted)
    );
}
#[test]
fn model_identity_claims_are_only_tool_arguments() {
    let value = fixtures()[3].clone();
    let event = decode_event(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap();
    if let Event::ToolCallProposed { proposal } = event.event {
        assert_eq!(proposal.arguments["approved"], true);
        assert!(!format!("{proposal:?}").contains("model-claim"));
    } else {
        panic!("expected proposal");
    }
    for key in [
        "actor",
        "approver",
        "authorized",
        "executionStatus",
        "trustLevel",
    ] {
        let mut bad = value.clone();
        bad["event"]["proposal"][key] = "model-claim".into();
        assert!(decode_event(&serde_json::to_vec(&bad).unwrap(), &limits()).is_err());
    }
}
#[test]
fn unknown_capability_is_a_closed_negative_state_not_a_fallback() {
    use ai_session_contract::*;
    let required = CapabilityRequirements {
        continuation: Some(ContinuationScope::SameProcess),
        interruption: Some(InterruptionConfirmation::RequestOnly),
        host_tool_control: true,
    };
    let supported = conversation().capabilities;
    assert!(supported.satisfies(required));
    for capability in [Continuation::Unknown {}, Continuation::Unsupported {}] {
        assert!(!EngineCapabilities {
            continuation: capability,
            ..supported
        }
        .satisfies(required));
    }
    for capability in [Interruption::Unknown {}, Interruption::Unsupported {}] {
        assert!(!EngineCapabilities {
            interruption: capability,
            ..supported
        }
        .satisfies(required));
    }
    for capability in [
        ToolControl::Unknown {},
        ToolControl::Unsupported {},
        ToolControl::Supported {
            mode: ToolControlMode::ProviderManaged,
        },
        ToolControl::Supported {
            mode: ToolControlMode::Disabled,
        },
    ] {
        assert!(!EngineCapabilities {
            tool_control: capability,
            ..supported
        }
        .satisfies(required));
    }
    let mut event = fixtures()[0].clone();
    event["event"]["conversation"]["capabilities"]["continuation"] =
        serde_json::json!({"status":"unknown"});
    assert!(decode_event(&serde_json::to_vec(&event).unwrap(), &limits()).is_ok());
    for bad in [
        serde_json::json!({"status":"newSupport"}),
        serde_json::json!({"status":"unknown","trusted":true}),
        serde_json::json!({"status":"supported","scope":"futureScope"}),
    ] {
        event["event"]["conversation"]["capabilities"]["continuation"] = bad;
        assert!(decode_event(&serde_json::to_vec(&event).unwrap(), &limits()).is_err());
    }
}
#[test]
fn resume_checks_exact_binding_and_generation_without_claiming_provider_success() {
    use ai_session_contract::*;
    let mut c = conversation();
    let mut binding = ResumeBinding {
        engine: c.engine.clone(),
        config: c.config.clone(),
    };
    assert_eq!(check_resume(&c, &binding), Ok(()));
    binding.engine.process_generation = Name::new("generation-2").unwrap();
    assert_eq!(
        check_resume(&c, &binding),
        Err(ResumeUnavailable::StaleProcessGeneration)
    );
    c.capabilities.continuation = Continuation::Supported {
        scope: ContinuationScope::AcrossProcesses,
    };
    assert_eq!(check_resume(&c, &binding), Ok(()));
    binding.engine.version = Name::new("2.0").unwrap();
    assert_eq!(
        check_resume(&c, &binding),
        Err(ResumeUnavailable::EngineChanged)
    );
    binding.engine = c.engine.clone();
    binding.config.revision = Name::new("2").unwrap();
    assert_eq!(
        check_resume(&c, &binding),
        Err(ResumeUnavailable::ConfigChanged)
    );
    binding.config = c.config.clone();
    c.capabilities.continuation = Continuation::Unknown {};
    assert_eq!(
        check_resume(&c, &binding),
        Err(ResumeUnavailable::UnknownCapability)
    );
    c.capabilities.continuation = Continuation::Unsupported {};
    assert_eq!(
        check_resume(&c, &binding),
        Err(ResumeUnavailable::Unsupported)
    );
}
#[test]
fn invalid_nested_variants_ids_configuration_and_sequences_are_rejected() {
    let mut cases = Vec::new();
    let mut event = fixtures()[0].clone();
    event["event"]["conversation"]["config"]["token"] = "secret".into();
    cases.push(event);
    let mut event = fixtures()[0].clone();
    event["event"]["conversation"]["id"] = "other".into();
    cases.push(event);
    let mut event = fixtures()[1].clone();
    event["event"]["message"]["content"][0]["kind"] = "html".into();
    cases.push(event);
    let mut event = fixtures()[1].clone();
    event["event"]["message"]["id"] = "".into();
    cases.push(event);
    let mut event = fixtures()[7].clone();
    event["event"]["outcome"] = "unknownFutureSuccess".into();
    cases.push(event);
    let mut event = fixtures()[7].clone();
    event["sequence"] = serde_json::json!(9007199254740992u64);
    cases.push(event);
    for bad in cases {
        assert!(decode_event(&serde_json::to_vec(&bad).unwrap(), &limits()).is_err());
    }
    let duplicate = std::str::from_utf8(CANCEL)
        .unwrap()
        .replace("\"sequence\":1", "\"sequence\":1,\"sequence\":2");
    assert!(decode_event(duplicate.as_bytes(), &limits()).is_err());
}
#[test]
fn bounded_inputs_and_user_command_roles_are_enforced() {
    let l = limits();
    assert!(decode_event(
        CANCEL,
        &SessionLimits {
            max_input_bytes: CANCEL.len() - 1,
            ..l
        }
    )
    .is_err());
    let events = fixtures();
    assert!(decode_event(
        &serde_json::to_vec(&events[1]).unwrap(),
        &SessionLimits {
            max_text_bytes: 2,
            ..l
        }
    )
    .is_err());
    assert!(decode_event(
        &serde_json::to_vec(&events[3]).unwrap(),
        &SessionLimits {
            max_argument_bytes: 2,
            ..l
        }
    )
    .is_err());
    assert!(decode_event(
        CANCEL,
        &SessionLimits {
            max_content_parts: 0,
            ..l
        }
    )
    .is_err());
    let commands: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/commands.json")).unwrap();
    let mut bad = commands[0].clone();
    bad["command"]["message"]["role"] = "assistant".into();
    assert!(ai_session_contract::decode_command(&serde_json::to_vec(&bad).unwrap(), &l).is_err());
}

#[test]
fn repeated_tool_argument_keys_are_rejected_at_every_depth() {
    let source = serde_json::to_string(&fixtures()[3]).unwrap();
    for arguments in [
        r#"{"approved":false,"approved":true}"#,
        r#"{"nested":{"target":"a","target":"b"}}"#,
        r#"{"array":[{"x":1,"x":2}]}"#,
    ] {
        let original =
            serde_json::to_string(&fixtures()[3]["event"]["proposal"]["arguments"]).unwrap();
        let bad = source.replace(&original, arguments);
        assert_eq!(
            decode_event(bad.as_bytes(), &limits()).unwrap_err(),
            ai_session_contract::ContractError::Encoding
        );
        assert!(serde_json::from_str::<ai_session_contract::EventEnvelope>(&bad).is_err());
    }
}

#[test]
fn constructed_envelopes_obey_the_same_encoded_size_budget_as_decoders() {
    let event = decode_event(CANCEL, &limits()).unwrap();
    let size = serde_json::to_vec(&event).unwrap().len();
    assert!(event
        .validate(&SessionLimits {
            max_input_bytes: size,
            ..limits()
        })
        .is_ok());
    assert_eq!(
        event
            .validate(&SessionLimits {
                max_input_bytes: size - 1,
                ..limits()
            })
            .unwrap_err(),
        ai_session_contract::ContractError::Limit
    );
    let values: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/commands.json")).unwrap();
    for value in values {
        let command =
            ai_session_contract::decode_command(&serde_json::to_vec(&value).unwrap(), &limits())
                .unwrap();
        let size = serde_json::to_vec(&command).unwrap().len();
        assert!(command
            .validate(&SessionLimits {
                max_input_bytes: size,
                ..limits()
            })
            .is_ok());
        assert_eq!(
            command
                .validate(&SessionLimits {
                    max_input_bytes: size - 1,
                    ..limits()
                })
                .unwrap_err(),
            ai_session_contract::ContractError::Limit
        );
    }
    let mut value = fixtures()[2].clone();
    value["event"]["text"] = serde_json::json!("\n".repeat(4096));
    let event: ai_session_contract::EventEnvelope = serde_json::from_value(value).unwrap();
    assert_eq!(
        event
            .validate(&SessionLimits {
                max_input_bytes: 5000,
                ..limits()
            })
            .unwrap_err(),
        ai_session_contract::ContractError::Limit
    );
}

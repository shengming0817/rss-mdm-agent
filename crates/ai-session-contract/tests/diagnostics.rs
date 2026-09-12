use ai_session_contract::{
    decode_command, decode_event, CommandEnvelope, ContractError, ErrorKind as K, Event,
    EventEnvelope, Field as F, Rule as R, SessionLimits,
};
use serde_json::{json, Value};
fn limits() -> SessionLimits {
    SessionLimits {
        max_input_bytes: 65536,
        max_text_bytes: 4096,
        max_content_parts: 32,
        max_argument_bytes: 8192,
    }
}
fn event(index: usize) -> Value {
    serde_json::from_str::<Vec<Value>>(include_str!("fixtures/events.json"))
        .unwrap()
        .remove(index)
}
fn command() -> Value {
    serde_json::from_str::<Vec<Value>>(include_str!("fixtures/commands.json"))
        .unwrap()
        .remove(0)
}
fn check(error: ContractError, kind: K, field: F, rule: R) {
    assert_eq!(
        (error.kind(), error.field(), error.rule()),
        (kind, field, rule)
    );
    let json = serde_json::to_string(&error).unwrap();
    assert_eq!(serde_json::from_str::<ContractError>(&json).unwrap(), error);
    for text in [json, error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("private"));
    }
    assert!(std::error::Error::source(&error).is_none());
}
#[test]
fn invalid_configuration_is_distinct_at_all_four_entry_points() {
    let l = limits();
    let e: EventEnvelope = serde_json::from_value(event(1)).unwrap();
    let c: CommandEnvelope = serde_json::from_value(command()).unwrap();
    for (bad, field) in [
        (
            SessionLimits {
                max_input_bytes: 0,
                ..l
            },
            F::InputBytes,
        ),
        (
            SessionLimits {
                max_text_bytes: 0,
                ..l
            },
            F::TextBytes,
        ),
        (
            SessionLimits {
                max_content_parts: 0,
                ..l
            },
            F::ContentParts,
        ),
        (
            SessionLimits {
                max_argument_bytes: 0,
                ..l
            },
            F::ArgumentBytes,
        ),
    ] {
        for result in [
            decode_event(b"private", &bad).map(|_| ()),
            decode_command(b"private", &bad).map(|_| ()),
            e.validate(&bad),
            c.validate(&bad),
        ] {
            check(
                result.unwrap_err(),
                K::InvalidConfiguration,
                field,
                R::NonZero,
            );
        }
    }
}
#[test]
fn empty_content_count_text_and_envelope_limits_have_separate_diagnostics() {
    let l = limits();
    for (parts, bounds, kind, field, rule) in [
        (json!([]), l, K::InvalidValue, F::ContentParts, R::Empty),
        (
            json!([{"kind":"text","text":"a"},{"kind":"text","text":"b"}]),
            SessionLimits {
                max_content_parts: 1,
                ..l
            },
            K::LimitExceeded,
            F::ContentParts,
            R::CollectionLimit,
        ),
        (
            json!([{"kind":"text","text":"private text"}]),
            SessionLimits {
                max_text_bytes: 2,
                ..l
            },
            K::LimitExceeded,
            F::TextBytes,
            R::ByteLimit,
        ),
        (
            json!([{"kind":"text","text":"private text"}]),
            SessionLimits {
                max_input_bytes: 1,
                ..l
            },
            K::LimitExceeded,
            F::InputBytes,
            R::ByteLimit,
        ),
    ] {
        let mut e = event(1);
        e["event"]["message"]["content"] = parts.clone();
        let mut c = command();
        c["command"]["message"]["content"] = parts;
        let e: EventEnvelope = serde_json::from_value(e).unwrap();
        let c: CommandEnvelope = serde_json::from_value(c).unwrap();
        for result in [
            e.validate(&bounds),
            c.validate(&bounds),
            decode_event(&serde_json::to_vec(&e).unwrap(), &bounds).map(|_| ()),
            decode_command(&serde_json::to_vec(&c).unwrap(), &bounds).map(|_| ()),
        ] {
            check(result.unwrap_err(), kind, field, rule);
        }
    }
}
#[test]
fn argument_byte_depth_and_node_errors_are_actionable_without_argument_names() {
    let mut e: EventEnvelope = serde_json::from_value(event(3)).unwrap();
    check(
        e.validate(&SessionLimits {
            max_argument_bytes: 1,
            ..limits()
        })
        .unwrap_err(),
        K::LimitExceeded,
        F::ArgumentNodes,
        R::NodeLimit,
    );
    if let Event::ToolCallProposed { proposal } = &mut e.event {
        proposal.arguments.clear();
        proposal
            .arguments
            .insert("private-key".into(), json!("private payload"));
    }
    check(
        e.validate(&SessionLimits {
            max_argument_bytes: 3,
            ..limits()
        })
        .unwrap_err(),
        K::LimitExceeded,
        F::ArgumentBytes,
        R::ByteLimit,
    );
    let mut deep = json!(null);
    for _ in 0..65 {
        deep = json!([deep]);
    }
    if let Event::ToolCallProposed { proposal } = &mut e.event {
        proposal.arguments.insert("private-key".into(), deep);
    }
    check(
        e.validate(&limits()).unwrap_err(),
        K::LimitExceeded,
        F::ArgumentDepth,
        R::DepthLimit,
    );
    let raw = serde_json::to_string(&event(3))
        .unwrap()
        .replace("\"approved\":true", "\"private-key\":1,\"private-key\":2");
    check(
        decode_event(raw.as_bytes(), &limits()).unwrap_err(),
        K::Encoding,
        F::Arguments,
        R::DuplicateKey,
    );
}
#[test]
fn version_identifiers_context_and_direction_survive_decode_with_no_payload() {
    for (index, path, value, kind, field, rule) in [
        (
            1,
            "/schemaVersion",
            json!(999),
            K::UnsupportedVersion,
            F::Version,
            R::Version,
        ),
        (
            1,
            "/event/message/id",
            json!("private invalid id"),
            K::InvalidValue,
            F::Message,
            R::Identifier,
        ),
        (
            0,
            "/event/conversation/id",
            json!("private-other"),
            K::InconsistentContext,
            F::Conversation,
            R::Mismatch,
        ),
        (
            1,
            "/sequence",
            json!(9007199254740992_u64),
            K::InvalidValue,
            F::Sequence,
            R::NumericRange,
        ),
    ] {
        let mut e = event(index);
        *e.pointer_mut(path).unwrap() = value;
        check(
            decode_event(&serde_json::to_vec(&e).unwrap(), &limits()).unwrap_err(),
            kind,
            field,
            rule,
        );
    }
    let mut c = command();
    c["command"]["message"]["role"] = json!("assistant");
    check(
        decode_command(&serde_json::to_vec(&c).unwrap(), &limits()).unwrap_err(),
        K::InvalidValue,
        F::MessageRole,
        R::Direction,
    );
    c = command();
    c["schemaVersion"] = json!(999);
    check(
        decode_command(&serde_json::to_vec(&c).unwrap(), &limits()).unwrap_err(),
        K::UnsupportedVersion,
        F::Version,
        R::Version,
    );
    for key in [
        "private-unknown",
        "ai-session-contract-diagnostic:{\"kind\":\"invalidValue\"}",
    ] {
        let mut e = event(1);
        e[key] = json!("private payload");
        check(
            decode_event(&serde_json::to_vec(&e).unwrap(), &limits()).unwrap_err(),
            K::Encoding,
            F::Document,
            R::Syntax,
        );
    }
    check(
        decode_event(b"{private", &limits()).unwrap_err(),
        K::Encoding,
        F::Document,
        R::Syntax,
    );
}

use ai_session_contract::{
    decode_event, ErrorKind, Event, Field, Rule, SessionLimits, TurnOutcome,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(
        std::env::args()
            .nth(1)
            .ok_or("event fixture path required")?,
    )?;
    let cases: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    let limits = SessionLimits {
        max_input_bytes: 65536,
        max_text_bytes: 4096,
        max_content_parts: 32,
        max_argument_bytes: 8192,
    };
    let mut cancelled = false;
    let mut tail = false;
    let mut interrupted = false;
    // Fixtures include alternative terminal examples. This selects one cancellation trace.
    for value in cases {
        let event = decode_event(&serde_json::to_vec(&value)?, &limits)?;
        match &event.event {
            Event::CancelDispatched { .. } => {
                cancelled = true;
                assert!(event.terminal_outcome().is_none());
            }
            Event::MessageDelta { .. } if cancelled => {
                tail = true;
                assert!(event.terminal_outcome().is_none());
            }
            Event::TurnFinished {
                outcome: TurnOutcome::Interrupted,
                ..
            } => {
                interrupted = true;
            }
            _ => {}
        }
    }
    assert!(cancelled && tail && interrupted);
    let invalid = decode_event(
        b"{}",
        &SessionLimits {
            max_text_bytes: 0,
            ..limits
        },
    )
    .unwrap_err();
    assert_eq!(
        (invalid.kind(), invalid.field(), invalid.rule()),
        (
            ErrorKind::InvalidConfiguration,
            Field::TextBytes,
            Rule::NonZero
        )
    );
    println!("ai-session-contract: independent consumer distinguished cancellation, tail data and confirmed interruption; no engine invoked");
    Ok(())
}

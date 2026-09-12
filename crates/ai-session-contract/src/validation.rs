use crate::{
    Command, CommandEnvelope, ContentPart, ContractError, ErrorKind, Event, EventEnvelope, Field,
    Message, Role, Rule, ToolCallResponse,
};
use serde::{de::DeserializeOwned, Serialize};
use std::io::{self, Write};

/// Mandatory host bounds, not values chosen by the model.
#[derive(Debug, Clone, Copy)]
pub struct SessionLimits {
    /// Positive maximum raw input and compact encoded envelope bytes.
    pub max_input_bytes: usize,
    /// Positive maximum accumulated UTF-8 text bytes within one content list or delta.
    pub max_text_bytes: usize,
    /// Positive maximum number of text content parts.
    pub max_content_parts: usize,
    /// Positive maximum encoded tool-argument bytes; also bounds the conservative node count.
    pub max_argument_bytes: usize,
}
impl SessionLimits {
    fn validate(&self) -> Result<(), ContractError> {
        for (value, field) in [
            (self.max_input_bytes, Field::InputBytes),
            (self.max_text_bytes, Field::TextBytes),
            (self.max_content_parts, Field::ContentParts),
            (self.max_argument_bytes, Field::ArgumentBytes),
        ] {
            if value == 0 {
                return Err(ContractError::new(
                    ErrorKind::InvalidConfiguration,
                    field,
                    Rule::NonZero,
                ));
            }
        }
        Ok(())
    }
}
fn decode<T: DeserializeOwned>(bytes: &[u8], limits: &SessionLimits) -> Result<T, ContractError> {
    limits.validate()?;
    if bytes.len() > limits.max_input_bytes {
        return Err(ContractError::new(
            ErrorKind::LimitExceeded,
            Field::InputBytes,
            Rule::ByteLimit,
        ));
    }
    serde_json::from_slice(bytes).map_err(ContractError::from_serde)
}
/// Decode and validate bounded V1 event bytes; returns only value-free contract diagnostics.
pub fn decode_event(bytes: &[u8], limits: &SessionLimits) -> Result<EventEnvelope, ContractError> {
    let event: EventEnvelope = decode(bytes, limits)?;
    event.validate(limits)?;
    Ok(event)
}
/// Decode and validate bounded V1 command bytes, including host message direction.
pub fn decode_command(
    bytes: &[u8],
    limits: &SessionLimits,
) -> Result<CommandEnvelope, ContractError> {
    let command: CommandEnvelope = decode(bytes, limits)?;
    command.validate(limits)?;
    Ok(command)
}
fn content(parts: &[ContentPart], l: &SessionLimits) -> Result<(), ContractError> {
    if parts.is_empty() {
        return Err(ContractError::new(
            ErrorKind::InvalidValue,
            Field::ContentParts,
            Rule::Empty,
        ));
    }
    if parts.len() > l.max_content_parts {
        return Err(ContractError::new(
            ErrorKind::LimitExceeded,
            Field::ContentParts,
            Rule::CollectionLimit,
        ));
    }
    let mut total: usize = 0;
    for ContentPart::Text { text } in parts {
        total = total.checked_add(text.len()).ok_or(ContractError::new(
            ErrorKind::LimitExceeded,
            Field::TextBytes,
            Rule::ByteLimit,
        ))?;
        if total > l.max_text_bytes {
            return Err(ContractError::new(
                ErrorKind::LimitExceeded,
                Field::TextBytes,
                Rule::ByteLimit,
            ));
        }
    }
    Ok(())
}
fn message(m: &Message, l: &SessionLimits) -> Result<(), ContractError> {
    content(&m.content, l)
}
fn response(r: &ToolCallResponse, l: &SessionLimits) -> Result<(), ContractError> {
    content(&r.content, l)
}
// Count compact JSON bytes without allocating a second full payload. This uses
// serde_json's actual serializer, including escaping and envelope metadata.
fn encoded_size<T: Serialize>(value: &T, limit: usize, field: Field) -> Result<(), ContractError> {
    struct Counter {
        remaining: usize,
        exceeded: bool,
    }
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.remaining {
                self.exceeded = true;
                return Err(io::Error::other("contract byte limit exceeded"));
            }
            self.remaining -= bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            // reason: a counting writer has no buffered data or external sink.
            Ok(())
        }
    }
    let mut counter = Counter {
        remaining: limit,
        exceeded: false,
    };
    serde_json::to_writer(&mut counter, value).map_err(|_| {
        if counter.exceeded {
            ContractError::new(ErrorKind::LimitExceeded, field, Rule::ByteLimit)
        } else {
            ContractError::new(ErrorKind::Encoding, field, Rule::Syntax)
        }
    })
}
impl EventEnvelope {
    /// Validate this constructed value against explicit host bounds. No authentication or execution occurs.
    pub fn validate(&self, limits: &SessionLimits) -> Result<(), ContractError> {
        limits.validate()?;
        if self.sequence > 9_007_199_254_740_991 {
            return Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::Sequence,
                Rule::NumericRange,
            ));
        }
        match &self.event {
            Event::ConversationStarted { conversation }
                if conversation.id != self.conversation_id =>
            {
                Err(ContractError::new(
                    ErrorKind::InconsistentContext,
                    Field::Conversation,
                    Rule::Mismatch,
                ))
            }
            Event::Message { message: m } => message(m, limits),
            Event::MessageDelta { text, .. } if text.len() > limits.max_text_bytes => Err(
                ContractError::new(ErrorKind::LimitExceeded, Field::TextBytes, Rule::ByteLimit),
            ),
            Event::ToolCallProposed { proposal } => {
                // An iterative depth check also protects programmatically constructed values before serialization.
                let mut pending: Vec<_> = proposal.arguments.values().map(|v| (v, 1)).collect();
                let mut nodes = 0usize;
                while let Some((value, depth)) = pending.pop() {
                    nodes += 1;
                    if depth > 64 {
                        return Err(ContractError::new(
                            ErrorKind::LimitExceeded,
                            Field::ArgumentDepth,
                            Rule::DepthLimit,
                        ));
                    }
                    if nodes > limits.max_argument_bytes {
                        return Err(ContractError::new(
                            ErrorKind::LimitExceeded,
                            Field::ArgumentNodes,
                            Rule::NodeLimit,
                        ));
                    }
                    match value {
                        serde_json::Value::Array(a) => {
                            pending.extend(a.iter().map(|v| (v, depth + 1)))
                        }
                        serde_json::Value::Object(o) => {
                            pending.extend(o.values().map(|v| (v, depth + 1)))
                        }
                        _ => {}
                    }
                }
                encoded_size(
                    &proposal.arguments,
                    limits.max_argument_bytes,
                    Field::ArgumentBytes,
                )
            }
            Event::ToolCallResponded { response: r } => response(r, limits),
            _ => Ok(()),
        }?;
        encoded_size(self, limits.max_input_bytes, Field::InputBytes)
    }
}
impl CommandEnvelope {
    /// Validate this constructed value against explicit host bounds. No authentication or execution occurs.
    pub fn validate(&self, limits: &SessionLimits) -> Result<(), ContractError> {
        limits.validate()?;
        match &self.command {
            Command::SendMessage { message: m } if m.role != Role::User => Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::MessageRole,
                Rule::Direction,
            )),
            Command::SendMessage { message: m } => message(m, limits),
            Command::RespondToTool { response: r } => response(r, limits),
            _ => Ok(()),
        }?;
        encoded_size(self, limits.max_input_bytes, Field::InputBytes)
    }
}

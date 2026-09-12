use crate::{
    Command, CommandEnvelope, ContentPart, ContractError, Event, EventEnvelope, Message, Role,
    ToolCallResponse,
};
use serde::de::DeserializeOwned;

/// Mandatory host bounds, not values chosen by the model.
#[derive(Debug, Clone, Copy)]
pub struct SessionLimits {
    pub max_input_bytes: usize,
    pub max_text_bytes: usize,
    pub max_content_parts: usize,
    pub max_argument_bytes: usize,
}
impl SessionLimits {
    fn validate(&self) -> Result<(), ContractError> {
        if [
            self.max_input_bytes,
            self.max_text_bytes,
            self.max_content_parts,
            self.max_argument_bytes,
        ]
        .contains(&0)
        {
            return Err(ContractError::Limit);
        }
        Ok(())
    }
}
fn decode<T: DeserializeOwned>(bytes: &[u8], limits: &SessionLimits) -> Result<T, ContractError> {
    limits.validate()?;
    if bytes.len() > limits.max_input_bytes {
        return Err(ContractError::Limit);
    }
    serde_json::from_slice(bytes).map_err(|_| ContractError::Encoding)
}
pub fn decode_event(bytes: &[u8], limits: &SessionLimits) -> Result<EventEnvelope, ContractError> {
    let event: EventEnvelope = decode(bytes, limits)?;
    event.validate(limits)?;
    Ok(event)
}
pub fn decode_command(
    bytes: &[u8],
    limits: &SessionLimits,
) -> Result<CommandEnvelope, ContractError> {
    let command: CommandEnvelope = decode(bytes, limits)?;
    command.validate(limits)?;
    Ok(command)
}
fn content(parts: &[ContentPart], l: &SessionLimits) -> Result<(), ContractError> {
    if parts.is_empty() || parts.len() > l.max_content_parts {
        return Err(ContractError::Limit);
    }
    let mut total: usize = 0;
    for ContentPart::Text { text } in parts {
        total = total.checked_add(text.len()).ok_or(ContractError::Limit)?;
        if total > l.max_text_bytes {
            return Err(ContractError::Limit);
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
impl EventEnvelope {
    pub fn validate(&self, limits: &SessionLimits) -> Result<(), ContractError> {
        limits.validate()?;
        if self.sequence > 9_007_199_254_740_991 {
            return Err(ContractError::Value);
        }
        match &self.event {
            Event::ConversationStarted { conversation }
                if conversation.id != self.conversation_id =>
            {
                Err(ContractError::Value)
            }
            Event::Message { message: m } => message(m, limits),
            Event::MessageDelta { text, .. } if text.len() > limits.max_text_bytes => {
                Err(ContractError::Limit)
            }
            Event::ToolCallProposed { proposal } => {
                // An iterative depth check also protects programmatically constructed values before serialization.
                let mut pending: Vec<_> = proposal.arguments.values().map(|v| (v, 1)).collect();
                let mut nodes = 0usize;
                while let Some((value, depth)) = pending.pop() {
                    nodes += 1;
                    if depth > 64 || nodes > limits.max_argument_bytes {
                        return Err(ContractError::Limit);
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
                let bytes =
                    serde_json::to_vec(&proposal.arguments).map_err(|_| ContractError::Encoding)?;
                if bytes.len() > limits.max_argument_bytes {
                    return Err(ContractError::Limit);
                }
                Ok(())
            }
            Event::ToolCallResponded { response: r } => response(r, limits),
            _ => Ok(()),
        }
    }
}
impl CommandEnvelope {
    pub fn validate(&self, limits: &SessionLimits) -> Result<(), ContractError> {
        limits.validate()?;
        match &self.command {
            Command::SendMessage { message: m } if m.role != Role::User => {
                Err(ContractError::Value)
            }
            Command::SendMessage { message: m } => message(m, limits),
            Command::RespondToTool { response: r } => response(r, limits),
            _ => Ok(()),
        }
    }
}

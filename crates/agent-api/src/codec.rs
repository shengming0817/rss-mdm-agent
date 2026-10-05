//! Validate API values against their imported, authoritative schema.
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
/// A closed schema/encoding failure, without input values or response bodies.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("invalid Agent API value")]
pub struct DecodeError;
const SCHEMAS: &[(&str, &str, &str)] = &[
    (
        "runtime",
        "BindingProfile",
        include_str!("../schema/runtime/binding-profile-v1.schema.json"),
    ),
    (
        "runtime",
        "ReportRequest",
        include_str!("../schema/runtime/report-request-v1.schema.json"),
    ),
    (
        "runtime",
        "ReportAck",
        include_str!("../schema/runtime/report-ack-v1.schema.json"),
    ),
    (
        "runtime",
        "ReportStatus",
        include_str!("../schema/runtime/report-status-v1.schema.json"),
    ),
    (
        "runtime",
        "ErrorBody",
        include_str!("../schema/runtime/error-body-v1.schema.json"),
    ),
    (
        "runtime",
        "TaskClaimRequest",
        include_str!("../schema/runtime/task-claim-request-v1.schema.json"),
    ),
    (
        "runtime",
        "TaskEventRequest",
        include_str!("../schema/runtime/task-event-request-v1.schema.json"),
    ),
    (
        "runtime",
        "TaskPayload",
        include_str!("../schema/runtime/task-payload-v1.schema.json"),
    ),
    (
        "runtime",
        "SignedTask",
        include_str!("../schema/runtime/signed-task-v1.schema.json"),
    ),
    (
        "runtime",
        "TaskClaimResponse",
        include_str!("../schema/runtime/task-claim-response-v1.schema.json"),
    ),
    (
        "runtime",
        "TaskEventAck",
        include_str!("../schema/runtime/task-event-ack-v1.schema.json"),
    ),
    (
        "runtime",
        "TaskAuthorization",
        include_str!("../schema/runtime/task-authorization-v1.schema.json"),
    ),
    (
        "runtime",
        "TaskResource",
        include_str!("../schema/runtime/task-resource-v1.schema.json"),
    ),
    (
        "runtime",
        "SelfServiceInvoke",
        include_str!("../schema/runtime/self-service-invoke-v1.schema.json"),
    ),
    (
        "runtime",
        "SelfServiceDirectory",
        include_str!("../schema/runtime/self-service-directory-v1.schema.json"),
    ),
    (
        "runtime",
        "SelfServiceDirectoryResponse",
        include_str!("../schema/runtime/self-service-directory-response-v1.schema.json"),
    ),
    (
        "runtime",
        "SelfServiceAccepted",
        include_str!("../schema/runtime/self-service-accepted-v1.schema.json"),
    ),
    (
        "runtime",
        "SelfServiceStatus",
        include_str!("../schema/runtime/self-service-status-v1.schema.json"),
    ),
    (
        "enroll",
        "GrantRequest",
        include_str!("../schema/enroll/grant-request-v1.schema.json"),
    ),
    (
        "enroll",
        "CertificateRequest",
        include_str!("../schema/enroll/certificate-request-v1.schema.json"),
    ),
    (
        "enroll",
        "ActivationRequest",
        include_str!("../schema/enroll/activation-request-v1.schema.json"),
    ),
    (
        "enroll",
        "StatusRequest",
        include_str!("../schema/enroll/status-request-v1.schema.json"),
    ),
    (
        "enroll",
        "GrantReceipt",
        include_str!("../schema/enroll/grant-receipt-v1.schema.json"),
    ),
    (
        "enroll",
        "Candidate",
        include_str!("../schema/enroll/candidate-v1.schema.json"),
    ),
    (
        "enroll",
        "ActivationReceipt",
        include_str!("../schema/enroll/activation-receipt-v1.schema.json"),
    ),
    (
        "enroll",
        "Progress",
        include_str!("../schema/enroll/progress-v1.schema.json"),
    ),
    (
        "enroll",
        "RecoverIssuance",
        include_str!("../schema/enroll/recover-issuance-v1.schema.json"),
    ),
    (
        "enroll",
        "NativeGrantRequest",
        include_str!("../schema/enroll/native-grant-request-v1.schema.json"),
    ),
    (
        "enroll",
        "ErrorBody",
        include_str!("../schema/enroll/error-body-v1.schema.json"),
    ),
    (
        "enroll",
        "AuthorizationRequest",
        include_str!("../schema/enroll/authorization-request-v1.schema.json"),
    ),
];
fn schema(name: &str, family: &str) -> Result<Value, DecodeError> {
    if let Some((_, _, text)) = SCHEMAS.iter().find(|(f, n, _)| *f == family && *n == name) {
        return serde_json::from_str(text).map_err(|_| DecodeError);
    }
    let parent = match name {
        "TaskSpec" | "SoftwareTaskSpec" | "EnrollmentTaskSpec" => Some("TaskPayload"),
        "ReportBody" => Some("ReportRequest"),
        "OutputChunk" | "ChunkedTaskResult" | "TaskResult" | "TaskEvent" => {
            Some("TaskEventRequest")
        }
        _ => None,
    };
    if let Some(parent) = parent {
        let (_, _, text) = SCHEMAS
            .iter()
            .find(|(f, n, _)| *f == "runtime" && *n == parent)
            .ok_or(DecodeError)?;
        let owner: Value = serde_json::from_str(text).map_err(|_| DecodeError)?;
        let mut selected = match name {
            "TaskSpec" => owner["oneOf"][0].clone(),
            "SoftwareTaskSpec" => owner["oneOf"][1].clone(),
            "EnrollmentTaskSpec" => owner["oneOf"][2].clone(),
            "ReportBody" => owner["properties"]["body"].clone(),
            "TaskEvent" => owner["properties"]["event"].clone(),
            _ => {
                let kind = match name {
                    "TaskResult" => "result",
                    "OutputChunk" => "output_chunk",
                    _ => "chunked_result",
                };
                let mut value = owner["properties"]["event"]["oneOf"]
                    .as_array()
                    .ok_or(DecodeError)?
                    .iter()
                    .find(|v| v["properties"]["kind"]["const"].as_str() == Some(kind))
                    .ok_or(DecodeError)?
                    .clone();
                value["properties"]
                    .as_object_mut()
                    .ok_or(DecodeError)?
                    .remove("kind");
                value["required"]
                    .as_array_mut()
                    .ok_or(DecodeError)?
                    .retain(|v| v.as_str() != Some("kind"));
                value
            }
        };
        selected["$defs"] = owner["$defs"].clone();
        selected["$schema"] = owner["$schema"].clone();
        return Ok(selected);
    }
    let lower = format!("{}{}", name[..1].to_ascii_lowercase(), &name[1..]);
    for (_, _, text) in SCHEMAS.iter().filter(|(f, _, _)| *f == family) {
        let owner: Value = serde_json::from_str(text).map_err(|_| DecodeError)?;
        if let Some(shape) = owner["$defs"]
            .get(name)
            .or_else(|| owner["$defs"].get(&lower))
        {
            let mut selected = shape.clone();
            selected["$defs"] = owner["$defs"].clone();
            selected["$schema"] = owner["$schema"].clone();
            return Ok(selected);
        }
    }
    Err(DecodeError)
}
fn validator(name: &'static str) -> Result<std::sync::Arc<jsonschema::Validator>, DecodeError> {
    use std::{
        collections::BTreeMap,
        sync::{Arc, OnceLock, RwLock},
    };
    static CACHE: OnceLock<RwLock<BTreeMap<&'static str, Arc<jsonschema::Validator>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(validator) = cache.read().map_err(|_| DecodeError)?.get(name) {
        return Ok(Arc::clone(validator));
    }
    let family = if name.contains("::enroll::") {
        "enroll"
    } else {
        "runtime"
    };
    let schema = schema(name.rsplit("::").next().ok_or(DecodeError)?, family)?;
    let validator = Arc::new(jsonschema::validator_for(&schema).map_err(|_| DecodeError)?);
    cache
        .write()
        .map_err(|_| DecodeError)?
        .insert(name, Arc::clone(&validator));
    Ok(validator)
}
/// Check a generated value before sending or trusting it.
pub fn validate<T: Serialize>(value: &T) -> Result<(), DecodeError> {
    let name = std::any::type_name::<T>();
    if !name.starts_with("agent_api::") {
        return Ok(());
    }
    let validator = validator(name)?;
    let value = serde_json::to_value(value).map_err(|_| DecodeError)?;
    if validator.is_valid(&value) {
        Ok(())
    } else {
        Err(DecodeError)
    }
}
/// Validate raw network JSON before conversion can normalize or discard invalid values.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    let name = std::any::type_name::<T>();
    if name.starts_with("agent_api::") {
        let validator = validator(name)?;
        let value: Value = serde_json::from_slice(bytes).map_err(|_| DecodeError)?;
        if !validator.is_valid(&value) {
            return Err(DecodeError);
        }
    }
    serde_json::from_slice(bytes).map_err(|_| DecodeError)
}

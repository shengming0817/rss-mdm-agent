//! Actual TLS listener stimuli for the backend-owned current-user contract.
use super::*;
use axum::http::Method;
#[derive(Default)]
pub struct RelationFixture {
    pub revision: u64,
    pub user: Option<SelfServiceUser>,
    pub owner: Option<SelfServiceUser>,
    pub inputs: Vec<Value>,
    pub replies: BTreeMap<Uuid, (Value, String, Value)>,
    pub lose_reply: bool,
    pub reject_proof: bool,
}
pub fn user(tenant: Uuid) -> SelfServiceUser {
    SelfServiceUser {
        tenant_id: tenant,
        instance_id: Uuid::from_u128(12),
        principal_id: Uuid::from_u128(13),
    }
}
pub fn response(
    state: &mut RelationFixture,
    tenant: Uuid,
    method: &Method,
    headers: &HeaderMap,
    input: Value,
) -> Response {
    let current = |state: &RelationFixture| json!({"revision":state.revision,"state":if state.user.is_some(){"active"}else{"unbound"},"user":state.user,"confirmedAt":if state.revision>0 {Some(1)}else{None}});
    if method == Method::GET {
        return axum::Json(current(state)).into_response();
    }
    let proof = headers
        .get("x-rss-user-session")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if method == Method::PUT && (state.reject_proof || proof.len() != 64) {
        return runtime_error(StatusCode::FORBIDDEN, ErrorCode::PermissionDenied);
    }
    let Ok(change) = serde_json::from_value::<CurrentUserChange>(input.clone()) else {
        return runtime_error(StatusCode::BAD_REQUEST, ErrorCode::MalformedRequest);
    };
    state
        .inputs
        .push(json!({"method":method.as_str(),"input":input.clone()}));
    let identity = format!("{}:{proof}", method.as_str());
    if let Some((old, old_identity, reply)) = state.replies.get(&change.operation_id) {
        return if old == &input && old_identity == &identity {
            axum::Json(reply.clone()).into_response()
        } else {
            runtime_error(StatusCode::CONFLICT, ErrorCode::OperationConflict)
        };
    }
    if change.expected_revision != state.revision {
        return runtime_error(StatusCode::CONFLICT, ErrorCode::OperationConflict);
    }
    state.revision += 1;
    state.user = if method == Method::PUT {
        Some(user(tenant))
    } else {
        None
    };
    if state.owner.is_none() && state.user.is_some() {
        state.owner = state.user.clone();
    }
    let value = current(state);
    state
        .replies
        .insert(change.operation_id, (input, identity, value.clone()));
    if std::mem::take(&mut state.lose_reply) {
        return runtime_error(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::ServiceUnavailable,
        );
    }
    axum::Json(value).into_response()
}

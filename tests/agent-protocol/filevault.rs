//! Client transport fixture. CMS is a bounded opaque carrier here; backend T2 owns decryption.
use super::*;
use rss_mdm_agent_wire::filevault as w;
#[derive(Default)]
pub struct Fixture {
    pub configuration: Option<w::Configuration>,
    pub pending_probe: Option<std::path::PathBuf>,
    pub uploads: Vec<w::Upload>,
    pub receipts: BTreeMap<Uuid, w::Receipt>,
    pub lose_ack: bool,
    pub wrong_ack: bool,
    pub fail_before_commit: bool,
    pub deny_status: bool,
    pub reject_upload: bool,
    pub config_reads: usize,
    pub status_reads: usize,
}
pub fn response(
    data: &mut Fixture,
    path: &str,
    value: Value,
    headers: &HeaderMap,
    authenticated: bool,
) -> Response {
    if !authenticated
        || headers.contains_key("x-rss-user-session")
        || headers.contains_key("cookie")
    {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    if path.ends_with("/configuration") {
        data.config_reads += 1;
        return axum::Json(data.configuration.clone().unwrap_or(w::Configuration {
            enabled: false,
            revision: 0,
            custody_id: None,
            custody_epoch: 0,
            recipient_id: None,
            recipient_certificate: None,
            material_revision: 0,
        }))
        .into_response();
    }
    if path.contains("/operations/") {
        data.status_reads += 1;
        if data.deny_status {
            return runtime_error(StatusCode::FORBIDDEN, ErrorCode::PermissionDenied);
        }
        let id = path
            .rsplit('/')
            .next()
            .and_then(|s| Uuid::parse_str(s).ok());
        return axum::Json(id.and_then(|id| data.receipts.get(&id)).cloned()).into_response();
    }
    let input: w::Upload = serde_json::from_value(value).unwrap();
    input.decode().unwrap();
    if let Some(path) = &data.pending_probe {
        let db =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        let body: Vec<u8> = db
            .query_row(
                "SELECT body FROM state WHERE key='filevault-custody'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let persisted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            persisted["pending"]["input"]["operationId"].as_str(),
            Some(input.operation_id.to_string().as_str())
        );
    }
    data.uploads.push(input.clone());
    if std::mem::take(&mut data.fail_before_commit) {
        return runtime_error(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::ServiceUnavailable,
        );
    }
    if data.reject_upload {
        return runtime_error(StatusCode::CONFLICT, ErrorCode::OperationConflict);
    }
    if !data.receipts.contains_key(&input.operation_id) {
        let config = data.configuration.as_mut().unwrap();
        assert_eq!(input.expected_revision, config.material_revision);
        config.material_revision += 1;
    }
    let receipt = data
        .receipts
        .entry(input.operation_id)
        .or_insert_with(|| w::Receipt {
            operation_id: input.operation_id,
            custody_id: input.custody_id,
            custody_epoch: input.custody_epoch,
            configuration_revision: input.configuration_revision,
            recipient_id: input.recipient_id.clone(),
            cms_digest: input.cms_digest.clone(),
            material: Uuid::new_v4(),
            version: 1,
            escrowed: true,
        })
        .clone();
    assert!(receipt.matches(&input));
    if std::mem::take(&mut data.lose_ack) {
        return runtime_error(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::ServiceUnavailable,
        );
    }
    let mut returned = receipt;
    if std::mem::take(&mut data.wrong_ack) {
        returned.custody_epoch += 1;
    }
    axum::Json(returned).into_response()
}

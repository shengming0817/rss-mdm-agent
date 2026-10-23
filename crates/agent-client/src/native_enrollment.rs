//! One durable native handoff per signed task attempt; OS effects remain with the execution journal.
use crate::{
    store::{decode, encode},
    wire, Client, Clock, Error, IdentityProvider, Start,
};
use rss_mdm_agent_wire::enroll::{NativeEnrollmentReceipt, NativeEnrollmentRequest};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    operation: Uuid,
    reference: String,
    receipt: Option<NativeEnrollmentReceipt>,
}
/// Short-lived OS input. Serialize only into protected executor staging, never a UI/log/journal.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEnrollmentMaterial {
    /// Server-provided, non-secret protocol coordinates.
    pub receipt: NativeEnrollmentReceipt,
    /// One-time password retained in native secret storage.
    pub password: wire::Secret,
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Last server-authorized native entry, used only to classify observed OS enrollment.
    pub fn native_enrollment_origin(&self) -> Result<Option<String>, Error> {
        self.store.get("native-enrollment-origin")
    }
    /// Exchange only a verified Start; retries retain the operation and protected password.
    pub async fn native_enrollment(
        &mut self,
        start: &Start,
    ) -> Result<NativeEnrollmentMaterial, Error> {
        self.validate_start(start)?;
        let wire::TaskPayload::Enrollment(task) = start.payload() else {
            return Err(Error::Protocol);
        };
        let entry = match &task.entry {
            wire::EnrollmentEntry::Windows { server } => server,
            wire::EnrollmentEntry::Macos { url } => url,
        };
        self.store.put("native-enrollment-origin", entry)?;
        let name = format!("native-enrollment/{}/{}", task.task_id, task.attempt_id);
        let mut pending = match self.store.get::<Pending>(&name)? {
            Some(value) => value,
            None => {
                let value = Pending {
                    operation: Uuid::new_v4(),
                    reference: format!("mdm-{}-{}", task.task_id, task.attempt_id),
                    receipt: None,
                };
                self.store.put(&name, &value)?;
                value
            }
        };
        let password = self.secrets.create_secret(&pending.reference)?;
        if pending.receipt.is_none() {
            let request = NativeEnrollmentRequest {
                operation_id: pending.operation,
                attempt_id: task.attempt_id,
                password: password.clone(),
            };
            let response = self
                .http
                .post(self.url(&format!("tasks/{}/enrollment", task.task_id))?)
                .header("content-type", "application/json")
                .body(encode(&request)?)
                .timeout(self.store.cfg.limits.request_timeout);
            let (status, bytes) = self
                .agent_json_transport(response, self.store.cfg.limits.response_bytes)
                .await?;
            if !status.is_success() {
                self.runtime_error(&bytes)?;
            }
            let receipt: NativeEnrollmentReceipt = decode(&bytes)?;
            validate(&receipt, task, self.now()?)?;
            pending.receipt = Some(receipt);
            self.store.put(&name, &pending)?;
        }
        let receipt = pending.receipt.ok_or(Error::Storage)?;
        validate(&receipt, task, self.now()?)?;
        Ok(NativeEnrollmentMaterial { receipt, password })
    }
}
fn validate(
    receipt: &NativeEnrollmentReceipt,
    task: &wire::EnrollmentTaskSpec,
    now: i64,
) -> Result<(), Error> {
    if receipt.enrollment_id.is_nil() || receipt.expires_at <= now || receipt.entry != task.entry {
        return Err(Error::Untrusted);
    }
    match &task.entry {
        wire::EnrollmentEntry::Windows { server } => {
            let server = url::Url::parse(server).map_err(|_| Error::Protocol)?;
            if receipt.profile_url.is_some()
                || receipt.username.as_deref()
                    != Some(
                        format!(
                            "{}@{}",
                            receipt.enrollment_id,
                            server.host_str().ok_or(Error::Protocol)?
                        )
                        .as_str(),
                    )
            {
                return Err(Error::Untrusted);
            }
        }
        wire::EnrollmentEntry::Macos { url } => {
            let origin = url::Url::parse(url)
                .map_err(|_| Error::Protocol)?
                .origin()
                .ascii_serialization();
            if receipt.username.is_some()
                || receipt.profile_url.as_deref()
                    != Some(
                        format!(
                            "{origin}/api/v1/enrollments/{}/profile",
                            receipt.enrollment_id
                        )
                        .as_str(),
                    )
            {
                return Err(Error::Untrusted);
            }
        }
    }
    Ok(())
}

impl NativeEnrollmentMaterial {
    /// Recheck an OS handoff against its signed task before touching the native entry.
    pub fn validate_for(&self, task: &wire::EnrollmentTaskSpec, now: i64) -> Result<(), Error> {
        validate(&self.receipt, task, now)
    }
    /// Download the existing Apple password-authorized profile over pinned HTTPS, without Agent mTLS.
    pub async fn profile(&self, ca_pem: &[u8]) -> Result<Vec<u8>, Error> {
        let url = self.receipt.profile_url.as_ref().ok_or(Error::Protocol)?;
        crate::validate_public_ca(ca_pem)?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .https_only(true)
            .tls_certs_only([
                reqwest::Certificate::from_pem(ca_pem).map_err(|_| Error::Configuration)?
            ])
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|_| Error::Configuration)?;
        let response = client
            .post(url)
            .json(&serde_json::json!({"password":self.password.expose()}))
            .send()
            .await
            .map_err(|_| Error::Unavailable)?;
        if !response.status().is_success() {
            return Err(Error::Denied);
        }
        crate::client::bounded(response, 512 * 1024).await
    }
}

//! Private consumer bindings; the fixed producer artifact is checked by protocol tests.
//! ref: reqwest v0.13.5 src/async_impl/request.rs (typed inputs, request construction).
use crate::{
    store::{decode, encode},
    Error,
};
use reqwest::{header::HeaderMap, Method, RequestBuilder, StatusCode};
use rss_mdm_agent_wire::{
    activities as a, connections, enroll as e, filevault as f, pki, platform_sso as s,
    runtime as w, task_trust,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{marker::PhantomData, time::Duration};

pub(crate) struct Spec {
    pub method: Method,
    pub path: &'static str,
    pub statuses: &'static [u16],
    pub media: Option<&'static str>,
}
pub(crate) trait Endpoint {
    type Input: Serialize + DeserializeOwned;
    type Output: DeserializeOwned;
    const SPEC: Spec;
}
// This small response is not exported by agent-wire. Its exact producer shape is
// checked against the fixed HTTP artifact, rather than declared as a shared DTO.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryUseAck {
    pub accepted: bool,
}
macro_rules! operations {
    ($($name:ident: $method:ident $path:literal [$($status:literal),+] $media:expr, $input:ty => $output:ty;)+) => {
        $(pub(crate) struct $name;
        impl Endpoint for $name {
            type Input = $input;
            type Output = $output;
            const SPEC: Spec = Spec { method: Method::$method, path: $path, statuses: &[$($status),+], media: $media };
        })+
        #[cfg(test)]
        pub(crate) fn specs() -> Vec<Spec> { vec![$($name::SPEC),+] }
    };
}
operations! {
    Report: POST "/api/v1/agent/runtime/reports" [202] Some("application/json"), w::ReportRequest => w::ReportAck;
    ReportStatus: GET "/api/v1/agent/runtime/reports/{id}" [200] Some("application/json"), () => w::ReportStatus;
    Claim: POST "/api/v1/agent/runtime/tasks/claim" [200] Some("application/json"), w::TaskClaimRequest => w::TaskClaimResponse;
    Event: POST "/api/v1/agent/runtime/tasks/{id}/events" [200] Some("application/json"), w::TaskEventRequest => w::TaskEventAck;
    TaskContent: GET "/api/v1/agent/runtime/tasks/{id}/content" [200,206] Some("application/octet-stream"), () => Vec<u8>;
    NativeEnrollment: POST "/api/v1/agent/runtime/tasks/{id}/enrollment" [200] Some("application/json"), e::NativeEnrollmentRequest => e::NativeEnrollmentReceipt;
    TaskTrust: GET "/api/v1/agent/runtime/task-trust" [200] Some("application/json"), () => task_trust::Trust;
    TaskTrustAck: POST "/api/v1/agent/runtime/task-trust" [200] Some("application/json"), task_trust::Acknowledgement => task_trust::Acknowledgement;
    TaskObservation: POST "/api/v1/agent/runtime/task-trust/observations" [200] Some("application/json"), task_trust::Observation => task_trust::Observation;
    PolicyEvent: POST "/api/v1/agent/runtime/policy-events" [202] Some("application/json"), w::PolicyEventRequest => w::PolicyEventAccepted;
    Directory: GET "/api/v1/agent/runtime/self-service" [200] Some("application/json"), () => w::SelfServiceDirectoryResponse;
    Resolve: POST "/api/v1/agent/runtime/self-service/resolve" [200] Some("application/json"), w::SelfServiceResolve => w::SelfServiceDirectoryItem;
    Invoke: POST "/api/v1/agent/runtime/self-service/invocations" [202] Some("application/json"), w::SelfServiceInvoke => w::SelfServiceAccepted;
    Invocation: GET "/api/v1/agent/runtime/self-service/invocations/{id}" [200] Some("application/json"), () => w::SelfServiceStatus;
    CurrentUser: GET "/api/v1/agent/runtime/current-user" [200] Some("application/json"), () => w::CurrentUser;
    BindUser: PUT "/api/v1/agent/runtime/current-user" [200] Some("application/json"), w::CurrentUserChange => w::CurrentUser;
    UnbindUser: DELETE "/api/v1/agent/runtime/current-user" [200] Some("application/json"), w::CurrentUserChange => w::CurrentUser;
    Activities: GET "/api/v1/agent/runtime/management-activities" [200] Some("application/json"), () => a::ManagementActivityPage;
    Changes: GET "/api/v1/agent/runtime/management-activities/changes" [200] Some("application/json"), () => a::ManagementActivityPage;
    Activity: GET "/api/v1/agent/runtime/management-activities/{id}" [200] Some("application/json"), () => a::ManagementActivityDetail;
    AccessUse: POST "/api/v1/agent/runtime/device-access/recovery-uses" [200] Some("application/json"), w::AccessRecoveryUse => RecoveryUseAck;
    AccessDirectory: GET "/api/v1/agent/runtime/device-access" [200] Some("application/json"), () => w::AccessDirectory;
    AccessBegin: POST "/api/v1/agent/runtime/device-access/begin" [200] Some("application/json"), w::AccessBegin => w::AccessChallenge;
    AccessConfirm: POST "/api/v1/agent/runtime/device-access/confirm" [200] Some("application/json"), w::AccessConfirm => w::AccessBindingStatus;
    PssoBootstrap: POST "/api/v1/agent/runtime/platform-sso/bootstrap" [200] Some("application/json"), s::Bootstrap => s::BootstrapResponse;
    PssoRegister: POST "/api/v1/agent/runtime/platform-sso/register" [200] Some("application/json"), s::Registration => s::RegistrationReceipt;
    PssoStatus: GET "/api/v1/agent/runtime/platform-sso/status" [200] Some("application/json"), () => s::Status;
    PssoRevoke: POST "/api/v1/agent/runtime/platform-sso/sources/{id}/revoke" [200] Some("application/json"), s::Revocation => s::RevocationReceipt;
    FilevaultConfiguration: GET "/api/v1/agent/runtime/filevault/configuration" [200] Some("application/json"), () => f::Configuration;
    FilevaultUpload: POST "/api/v1/agent/runtime/filevault/escrows" [200] Some("application/json"), f::Upload => f::Receipt;
    FilevaultReceipt: GET "/api/v1/agent/runtime/filevault/operations/{id}" [200] Some("application/json"), () => Option<f::Receipt>;
    TelemetryDirectory: GET "/api/v1/agent/runtime/telemetry/configurations" [200] Some("application/json"), () => w::TelemetryDirectory;
    TelemetryConfiguration: GET "/api/v1/agent/runtime/telemetry/configurations/{policy}/{version}" [200] Some("application/json"), () => w::SignedTelemetryConfiguration;
    TelemetryContent: GET "/api/v1/agent/runtime/telemetry/configurations/{policy}/{version}/content" [200,206] Some("application/octet-stream"), () => Vec<u8>;
    TelemetryUpload: POST "/api/v1/agent/runtime/telemetry/batches" [202] Some("application/json"), w::TelemetryUpload => w::TelemetryReceipt;
    TelemetryStatus: GET "/api/v1/agent/runtime/telemetry/batches/{id}" [200] Some("application/json"), () => w::TelemetryReceipt;
    TelemetryGap: POST "/api/v1/agent/runtime/telemetry/gaps" [202] Some("application/json"), w::TelemetryGapReport => w::TelemetryGapReceipt;
    TelemetryGapStatus: GET "/api/v1/agent/runtime/telemetry/gaps/{id}" [200] Some("application/json"), () => w::TelemetryGapReceipt;
    Grant: POST "/api/v1/agent/enroll/requests" [200] Some("application/json"), e::GrantRequest => e::GrantReceipt;
    Renewal: POST "/api/v1/agent/enroll/renewal-grants" [200] Some("application/json"), e::GrantRequest => e::GrantReceipt;
    RenewalStatus: POST "/api/v1/agent/enroll/renewal-grants/status" [200] Some("application/json"), e::RenewalStatusRequest => e::GrantReceipt;
    EnrollmentStatus: POST "/api/v1/agent/enroll/status" [200] Some("application/json"), e::StatusRequest => e::Progress;
    Certificate: POST "/api/v1/agent/enroll/certificates" [200] Some("application/json"), e::CertificateRequest => e::Candidate;
    Activate: POST "/api/v1/agent/enroll/activate" [200] Some("application/json"), e::ActivationRequest => e::ActivationReceipt;
    NativeGrant: POST "/api/v1/agent/enroll/native-grants" [200] Some("application/json"), e::NativeGrantRequest => e::GrantReceipt;
    Exchange: POST "/api/v1/agent/enroll/configurations/exchange" [200] Some("application/json"), e::configuration::Exchange => e::GrantReceipt;
    ExchangeStatus: POST "/api/v1/agent/enroll/configurations/status" [200] Some("application/json"), e::configuration::ExchangeStatus => e::GrantReceipt;
    Packages: GET "/api/v1/agent/enroll/packages" [200] Some("application/json"), () => Vec<e::configuration::Package>;
    Trust: GET "/api/v1/agent/enroll/trust" [200] Some("application/json"), () => pki::Trust;
    TrustAck: POST "/api/v1/agent/enroll/trust/acknowledgements" [204] None, pki::Acknowledgement => ();
    TrustObservation: POST "/api/v1/agent/enroll/trust/observations" [200] Some("application/json"), pki::Observation => pki::Observation;
    Connections: GET "/api/v1/agent/enroll/connections" [200] Some("application/json"), () => connections::SignedConfiguration;
    ConnectionsAck: POST "/api/v1/agent/enroll/connections/acknowledgements" [204] None, connections::Acknowledgement => ();
    ConnectionsFailure: POST "/api/v1/agent/enroll/connections/failures" [204] None, connections::FailureReport => ();
}

pub(crate) fn url<E: Endpoint>(
    origin: &url::Url,
    params: &[&str],
    query: Option<&str>,
) -> Result<url::Url, Error> {
    let mut url = origin.join("/").map_err(|_| Error::Configuration)?;
    let mut values = params.iter();
    {
        let mut segments = url.path_segments_mut().map_err(|_| Error::Configuration)?;
        segments.clear();
        for segment in E::SPEC.path.trim_start_matches('/').split('/') {
            if segment.starts_with('{') && segment.ends_with('}') {
                let value = values.next().ok_or(Error::Protocol)?;
                if value.is_empty() || matches!(*value, "." | "..") {
                    return Err(Error::Protocol);
                }
                segments.push(value);
            } else {
                segments.push(segment);
            }
        }
    }
    if values.next().is_some() {
        return Err(Error::Protocol);
    }
    url.set_query(query);
    Ok(url)
}
pub(crate) struct Call<E: Endpoint> {
    builder: RequestBuilder,
    marker: PhantomData<E>,
    body_bytes: usize,
}
impl<E: Endpoint> Call<E> {
    pub fn new(
        http: &reqwest::Client,
        origin: &url::Url,
        params: &[&str],
        query: Option<&str>,
        input: Option<&E::Input>,
    ) -> Result<Self, Error> {
        let mut builder = http.request(E::SPEC.method.clone(), url::<E>(origin, params, query)?);
        let bytes = input.map(encode).transpose()?;
        let body_bytes = bytes.as_ref().map_or(0, Vec::len);
        if let Some(bytes) = bytes {
            builder = builder
                .header("content-type", "application/json")
                .body(bytes);
        }
        Ok(Self {
            builder,
            marker: PhantomData,
            body_bytes,
        })
    }
    pub fn frozen(
        http: &reqwest::Client,
        origin: &url::Url,
        params: &[&str],
        bytes: &[u8],
    ) -> Result<Self, Error> {
        // Validate restored bytes against this endpoint's input without reserializing them.
        let _: E::Input = decode(bytes)?;
        let mut call = Self::new(http, origin, params, None, None)?;
        call.builder = call
            .builder
            .header("content-type", "application/json")
            .body(bytes.to_vec());
        call.body_bytes = bytes.len();
        Ok(call)
    }
    pub fn body_limit(self, max: usize) -> Result<Self, Error> {
        if self.body_bytes > max {
            return Err(Error::Capacity);
        }
        Ok(self)
    }
    pub fn header(mut self, name: &'static str, value: impl AsRef<str>) -> Self {
        self.builder = self.builder.header(name, value.as_ref());
        self
    }
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.builder = self.builder.timeout(timeout);
        self
    }
    pub async fn json(self, max: usize) -> Result<(StatusCode, Vec<u8>), Error> {
        let (status, bytes) = crate::client::json_transport(self.builder, max).await?;
        Self::check_status(status)?;
        if status.is_success() && E::SPEC.media.is_none() && !bytes.is_empty() {
            return Err(Error::Protocol);
        }
        Ok((status, bytes))
    }
    pub async fn send(self) -> Result<reqwest::Response, Error> {
        let response = self
            .builder
            .send()
            .await
            .map_err(crate::client::transport_error)?;
        Self::check_status(response.status())?;
        if response.status().is_success() {
            Self::check_media(response.headers())?;
        }
        Ok(response)
    }
    fn check_status(status: StatusCode) -> Result<(), Error> {
        if status.is_success() && !E::SPEC.statuses.contains(&status.as_u16()) {
            return Err(Error::Protocol);
        }
        Ok(())
    }
    fn check_media(headers: &HeaderMap) -> Result<(), Error> {
        if let Some(expected) = E::SPEC.media {
            if headers
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.split(';').next())
                .map(str::trim)
                != Some(expected)
            {
                return Err(Error::Protocol);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;

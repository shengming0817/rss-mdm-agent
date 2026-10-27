//! Registration is an independently authorized interface to the installation execution owner.
use crate::{deployment::Deployment, SystemClock};
use agent_client::{EnrollmentPhase, EnrollmentStatus, Error};
use execution_ipc::host::{
    CredentialStatus, Handler, Peer, PeerPolicy, RegistrationPhase, RegistrationStatus, Reply,
    Request,
};

pub(crate) fn project(value: EnrollmentStatus) -> RegistrationStatus {
    let phase = match value.phase {
        EnrollmentPhase::Unregistered => RegistrationPhase::Unregistered,
        EnrollmentPhase::Preparing => RegistrationPhase::Preparing,
        EnrollmentPhase::ConfigurationUnknown => RegistrationPhase::ConfigurationUnknown,
        EnrollmentPhase::AwaitingAuthorization => RegistrationPhase::AwaitingAuthorization,
        EnrollmentPhase::Authorized => RegistrationPhase::Authorized,
        EnrollmentPhase::Activating => RegistrationPhase::Activating,
        EnrollmentPhase::Active => RegistrationPhase::Active,
        EnrollmentPhase::IssuanceUnknown => RegistrationPhase::IssuanceUnknown,
        EnrollmentPhase::ActivationUnknown => RegistrationPhase::ActivationUnknown,
        EnrollmentPhase::Rejected => RegistrationPhase::Rejected,
        EnrollmentPhase::Paused => RegistrationPhase::Paused,
    };
    let credential = match value.failure {
        Some(rss_mdm_agent_wire::enroll::ErrorCode::CertificateExpired) => {
            CredentialStatus::Expired
        }
        Some(rss_mdm_agent_wire::enroll::ErrorCode::CredentialRevoked) => CredentialStatus::Revoked,
        Some(rss_mdm_agent_wire::enroll::ErrorCode::CredentialRetired) => CredentialStatus::Retired,
        _ if value.registered => CredentialStatus::Active,
        _ => CredentialStatus::Absent,
    };
    RegistrationStatus {
        task_trust: value
            .task_trust
            .map(|v| execution_ipc::host::TrustInstallationStatus {
                state: match v.state {
                    rss_mdm_agent_wire::task_trust::ObservationState::Received => {
                        execution_ipc::host::TrustInstallationState::Received
                    }
                    rss_mdm_agent_wire::task_trust::ObservationState::Applied => {
                        execution_ipc::host::TrustInstallationState::Applied
                    }
                    rss_mdm_agent_wire::task_trust::ObservationState::Failed => {
                        execution_ipc::host::TrustInstallationState::Failed
                    }
                    rss_mdm_agent_wire::task_trust::ObservationState::Unknown => {
                        execution_ipc::host::TrustInstallationState::Unknown
                    }
                },
                target_revision: v.target_revision,
                actual_revision: v.actual_revision,
                observed_at: v.observed_at,
                acknowledged: value.task_trust_acknowledged,
            }),
        pki_trust: value
            .pki_trust
            .map(|v| execution_ipc::host::TrustInstallationStatus {
                state: match v.state {
                    rss_mdm_agent_wire::pki::ObservationState::Received => {
                        execution_ipc::host::TrustInstallationState::Received
                    }
                    rss_mdm_agent_wire::pki::ObservationState::Applied => {
                        execution_ipc::host::TrustInstallationState::Applied
                    }
                    rss_mdm_agent_wire::pki::ObservationState::Failed => {
                        execution_ipc::host::TrustInstallationState::Failed
                    }
                    rss_mdm_agent_wire::pki::ObservationState::Unknown => {
                        execution_ipc::host::TrustInstallationState::Unknown
                    }
                },
                target_revision: v.target_revision,
                actual_revision: v.actual_revision,
                observed_at: v.observed_at,
                acknowledged: value.pki_trust_acknowledged,
            }),
        failure: value.failure.map(|code| match code {
            rss_mdm_agent_wire::enroll::ErrorCode::GrantExpired => {
                execution_ipc::host::RegistrationFailure::Expired
            }
            rss_mdm_agent_wire::enroll::ErrorCode::RegistrationLimit => {
                execution_ipc::host::RegistrationFailure::Capacity
            }
            rss_mdm_agent_wire::enroll::ErrorCode::ServiceUnavailable => {
                execution_ipc::host::RegistrationFailure::Unavailable
            }
            rss_mdm_agent_wire::enroll::ErrorCode::OperationConflict => {
                execution_ipc::host::RegistrationFailure::Conflict
            }
            _ => execution_ipc::host::RegistrationFailure::Denied,
        }),
        phase,
        grant_id: value.grant_id.map(|id| id.to_string()),
        expires_at: value.expires_at,
        public_key_fingerprint: value.public_key_fingerprint,
        registered: value.registered,
        credential,
    }
}
/// Authentication facade; the one service owner exists before enterprise registration.
pub(crate) struct RegistrationOwner {
    network: agent_client::Config,
    installer_format: Option<rss_mdm_agent_wire::enroll::configuration::InstallerFormat>,
    policy: PeerPolicy,
    registration_policy: PeerPolicy,
    owner: crate::service::ServiceHandle,
}
impl RegistrationOwner {
    pub(crate) fn new(deployment: Deployment) -> Result<Self, Error> {
        let exists = deployment
            .state_root
            .join("communication/communication.sqlite")
            .exists();
        let client = deployment.communication(!exists)?;
        Ok(Self {
            network: deployment.network()?,
            installer_format: crate::installation::installer_format(&deployment).ok(),
            policy: deployment.clients.clone(),
            registration_policy: deployment.registration_clients.clone(),
            owner: deployment.activate(client)?,
        })
    }
}
impl Handler for RegistrationOwner {
    fn session_fence(&self) -> Option<std::sync::Arc<dyn execution_ipc::host::SessionFence>> {
        self.owner.session_fence()
    }
    fn peer_policy(&self) -> Option<PeerPolicy> {
        let mut policy = self
            .owner
            .peer_policy()
            .unwrap_or_else(|| self.policy.clone());
        for image in &self.registration_policy.images {
            if !policy.images.iter().any(|v| {
                v.path == image.path && v.sha256 == image.sha256 && v.cdhash == image.cdhash
            }) {
                policy.images.push(image.clone());
            }
        }
        for subject in &self.registration_policy.subjects {
            if !policy.subjects.contains(subject) {
                policy.subjects.push(subject.clone());
            }
        }
        Some(policy)
    }
    fn register_helper(&mut self, peer: &Peer) -> Result<(), execution_app::Error> {
        self.owner.register_helper(peer)
    }
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply {
        if matches!(
            request,
            Request::RegistrationStatus {}
                | Request::RegistrationContext {}
                | Request::RequestRegistration {}
                | Request::ImportRegistration { .. }
        ) {
            if peer.authenticate(&self.registration_policy).is_err()
                || peer.session_binding().is_err()
            {
                return Reply::Rejected;
            }
            if let Request::RegistrationContext {} = request {
                return match self.owner.registration_status() {
                    Reply::RegistrationStatus { value } => Reply::RegistrationContext {
                        value: execution_ipc::host::RegistrationContext {
                            service_id: self.network.service_id.to_string(),
                            origin: self
                                .network
                                .connections
                                .configuration
                                .preferred_origin
                                .clone(),
                            tenant_id: self.network.tenant.to_string(),
                            status: value,
                        },
                    },
                    other => other,
                };
            }
            if let Request::RegistrationStatus {} = request {
                return self.owner.registration_status();
            }
            if let Request::ImportRegistration { file } = &request {
                use agent_client::Clock;
                if self.installer_format.is_none()
                    || SystemClock::new()
                        .and_then(|clock| clock.now())
                        .and_then(|now| {
                            agent_client::verify_configuration(file, &self.network, now)
                        })
                        .is_err()
                {
                    return Reply::Rejected;
                }
            }
            return self.owner.registration_request(request);
        }
        self.owner.handle(peer, request)
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        self.owner.tick()
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        self.owner.stop()
    }
}

#[cfg(all(test, target_os = "macos"))]
mod fence_tests {
    use super::*;
    #[tokio::test]
    async fn production_registration_facade_retains_the_original_owners_fast_revocation_lane() {
        let case = crate::service::product_gate_tests::Case::new(None).await;
        let policy = PeerPolicy {
            images: vec![case.service.core.config.interpreters[0].image.clone()],
            subjects: vec!["0".into()],
            interactive: false,
        };
        let network = case._server.config();
        let handle = case.service.spawn(policy.clone(), None).unwrap();
        let mut facade = RegistrationOwner {
            network,
            installer_format: None,
            policy: policy.clone(),
            registration_policy: policy,
            owner: handle,
        };
        let lane = facade.session_fence();
        let exposed = lane.is_some();
        drop(lane);
        // Stop the real owner even when the forwarding assertion fails.
        tokio::task::spawn_blocking(move || facade.stop())
            .await
            .unwrap()
            .unwrap();
        assert!(
            exposed,
            "the production facade must expose its owner's native fence"
        );
    }
}

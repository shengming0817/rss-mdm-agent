use crate::*;
use execution_app::*;
use execution_approval::ProfileApproval;
use execution_contract::*;
use execution_interaction::{Interaction, Reference, Spec};
use execution_lifecycle::{CommandEvent, ObservationEvent, ObservationVerifier};

impl JournalPort for Store {
    fn authority(&self) -> &Authority {
        &self.authority
    }
    fn input_limits(&self) -> ExecutionLimits {
        self.limits.input
    }
    fn backend_requests(
        &self,
        actor: &execution_contract::ActorId,
        host: &impl JournalHost,
    ) -> Result<Vec<BackendRequest>, JournalError> {
        self.sql_backend_requests(actor, host).map_err(Into::into)
    }
    fn backend_request(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Option<BackendRequest>, JournalError> {
        self.sql_backend_request(scope, host).map_err(Into::into)
    }
    fn record_backend_request(
        &mut self,
        scope: &Scope,
        expected: Option<&BackendRequest>,
        next: &BackendRequest,
        host: &impl JournalHost,
    ) -> Result<(), JournalError> {
        self.sql_record_backend_request(scope, expected, next, host)
            .map_err(Into::into)
    }
    fn contains_request(
        &self,
        request: &execution_contract::RequestId,
    ) -> Result<bool, JournalError> {
        self.sql_contains_request(request).map_err(Into::into)
    }
    fn record_software_progress(
        &mut self,
        scope: &Scope,
        facts: &SoftwareProgress,
        host: &impl JournalHost,
    ) -> Result<SoftwareProgress, JournalError> {
        self.sql_record_software_progress(scope, facts, host)
            .map_err(Into::into)
    }
    fn software_ownership(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Vec<execution_contract::SoftwareOwnership>, JournalError> {
        self.sql_software_ownership(scope, host).map_err(Into::into)
    }
    fn software_progress(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl JournalHost,
    ) -> Result<Option<SoftwareProgress>, JournalError> {
        self.sql_software_progress(scope, attempt, host)
            .map_err(Into::into)
    }
    fn delivery_evidence(
        &self,
        scope: &Scope,
        consumer: &Id,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<Vec<DeliveryEvidence>, JournalError> {
        self.sql_delivery_evidence(scope, consumer, limit, host)
            .map_err(Into::into)
    }
    fn pull_results(
        &self,
        scope: &Scope,
        consumer: &Id,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<Vec<Receipt>, JournalError> {
        self.sql_pull_results(scope, consumer, limit, host)
            .map_err(Into::into)
    }
    fn confirm(
        &mut self,
        scope: &Scope,
        consumer: &Id,
        event: &EventId,
        host: &impl JournalHost,
    ) -> Result<(), JournalError> {
        self.sql_confirm(scope, consumer, event, host)
            .map_err(Into::into)
    }
    fn audit(
        &self,
        scope: &Scope,
        operation: &OperationRequestId,
        host: &impl JournalHost,
    ) -> Result<AuditRecord, JournalError> {
        self.sql_audit(scope, operation, host).map_err(Into::into)
    }
    fn execution_by_request(
        &self,
        request: &execution_contract::RequestId,
        access: ExecutionAccess<'_>,
        host: &impl JournalHost,
    ) -> Result<ExecutionRecord, JournalError> {
        self.sql_execution_by_request(request, access, host)
            .map_err(Into::into)
    }
    fn execution_receipt(
        &self,
        scope: &Scope,
        op: &OperationRequestId,
        host: &impl JournalHost,
    ) -> Result<Option<Receipt>, JournalError> {
        self.sql_execution_receipt(scope, op, host)
            .map_err(Into::into)
    }
    fn open_execution(
        &mut self,
        op: &OperationRequestId,
        plan: &FrozenExecution,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError> {
        self.sql_open_execution(op, plan, host).map_err(Into::into)
    }
    fn device_execution_requests(
        &self,
        device: &execution_contract::DeviceId,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<ExecutionRequestPage, JournalError> {
        self.sql_device_execution_requests(device, after, limit, host)
            .map_err(Into::into)
    }
    fn execution_requests(
        &self,
        actor: &execution_contract::ActorId,
        device: &execution_contract::DeviceId,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<ExecutionRequestPage, JournalError> {
        self.sql_execution_requests(actor, device, after, limit, host)
            .map_err(Into::into)
    }
    fn apply_command(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &CommandEvent,
        bindings: &[ProfileApproval],
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError> {
        self.sql_apply_command(op, scope, event, bindings, host)
            .map_err(Into::into)
    }
    fn apply_observation(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &ObservationEvent,
        host: &impl JournalHost,
        verifier: &dyn ObservationVerifier,
    ) -> Result<CommitOutcome, JournalError> {
        self.sql_apply_observation(op, scope, event, host, verifier)
            .map_err(Into::into)
    }
    fn service_requests(
        &self,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<Vec<execution_contract::RequestId>, JournalError> {
        self.sql_service_requests(after, limit, host)
            .map_err(Into::into)
    }
    fn execution_confirmation_state(
        &self,
        input: &execution_contract::FrozenExecution,
        access: ExecutionAccess<'_>,
        host: &impl JournalHost,
    ) -> Result<Option<Interaction>, JournalError> {
        self.sql_execution_confirmation_state(input, access, host)
            .map_err(Into::into)
    }
    fn open_interaction(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        spec: &Spec,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError> {
        self.sql_open_interaction(op, scope, spec, host)
            .map_err(Into::into)
    }
    fn apply_interaction(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        id: &Reference,
        command: &execution_interaction::Command,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError> {
        self.sql_apply_interaction(op, scope, id, command, host)
            .map_err(Into::into)
    }
    fn interaction(
        &self,
        scope: &Scope,
        id: &Reference,
        host: &impl JournalHost,
    ) -> Result<Interaction, JournalError> {
        self.sql_interaction(scope, id, host).map_err(Into::into)
    }
    fn trust_revision(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Option<u64>, JournalError> {
        self.sql_trust_revision(scope, host).map_err(Into::into)
    }
    fn refresh_trust(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        expected: Option<u64>,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError> {
        self.sql_refresh_trust(op, scope, expected, host)
            .map_err(Into::into)
    }
    fn record_process(
        &mut self,
        scope: &Scope,
        facts: &ProcessEvidence,
        host: &impl JournalHost,
    ) -> Result<(), JournalError> {
        self.sql_record_process(scope, facts, host)
            .map_err(Into::into)
    }
    fn runner_evidence(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl JournalHost,
    ) -> Result<Option<ProcessEvidence>, JournalError> {
        self.sql_runner_evidence(scope, attempt, host)
            .map_err(Into::into)
    }
}

use crate::{host::Host, service::operation, *};
use execution_contract::{EventId, Id, RequestId};
use execution_interaction::{Command, Interaction, Kind, Reference, Spec};
use execution_sqlite::{AuditRecord, ExecutionAccess, OperationRequestId, Receipt, Scope};

impl<H: AppHost, R: RunnerPort> ExecutionApp<H, R> {
    /// Trusted device-owner presence lookup for an unsubmitted remote task. This is not
    /// exposed to UI/AI and absence is meaningful only in this bound authoritative journal.
    pub fn has_service_execution(
        &self,
        request: &RequestId,
        device: &execution_contract::DeviceId,
    ) -> Result<bool, Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Unbound);
        }
        if &self.binding.device != device {
            return Err(Error::Denied);
        }
        Ok(self.store.contains_request(request)?)
    }
    /// Device-owner delivery with independent evidence permission. Not a UI/AI endpoint.
    pub fn service_delivery(
        &self,
        request: &RequestId,
        consumer: &Id,
        limit: usize,
    ) -> Result<Vec<execution_sqlite::DeliveryEvidence>, Error> {
        let execution = self.load(None, request, ExecutionAccess::Delivery(consumer))?;
        let result = self.store.delivery_evidence(
            &Scope::from_input(execution.input()),
            consumer,
            limit,
            &self.adapter(None, None),
        )?;
        self.load(None, request, ExecutionAccess::Delivery(consumer))?;
        Ok(result)
    }
    /// Confirm precisely one service-consumer event after durable remote acceptance.
    pub fn service_confirm(
        &mut self,
        request: &RequestId,
        consumer: &Id,
        event: &EventId,
    ) -> Result<(), Error> {
        let execution = self.load(None, request, ExecutionAccess::Delivery(consumer))?;
        let host = Host::new(&self.host, &self.binding, &self.config, None);
        Ok(self.store.confirm(
            &Scope::from_input(execution.input()),
            consumer,
            event,
            &host,
        )?)
    }
    /// Open a bounded interaction bound by the service to the actual stored task. This records a
    /// wait reason only, not authorization or an automatic continuation. Host owns responder rights.
    pub fn open_interaction(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        id: Reference,
        kind: Kind,
        expires_at_unix_ms: u64,
    ) -> Result<Receipt, Error> {
        let context = Some(caller);
        let execution = self.load(context, request, ExecutionAccess::Interact)?;
        let plan = execution.input();
        let scope = Scope::from_input(plan);
        let op = operation(plan, "interaction-open", id.as_str())?;
        let spec = Spec {
            id,
            subject: scope.interaction_subject(),
            kind,
            expires_at_unix_ms,
        };
        let host =
            Host::new(&self.host, &self.binding, &self.config, context).with_input(Some(plan));
        Ok(self
            .store
            .open_interaction(&op, &scope, &spec, &host)?
            .receipt()
            .clone())
    }
    /// Read a pending or resolved interaction; no acknowledgement or execution is implied.
    pub fn interaction(
        &self,
        caller: &RequestContext,
        request: &RequestId,
        id: &Reference,
    ) -> Result<Interaction, Error> {
        let context = Some(caller);
        let execution = self.load(context, request, ExecutionAccess::Result)?;
        Ok(self.store.interaction(
            &Scope::from_input(execution.input()),
            id,
            &self.adapter(context, None),
        )?)
    }
    /// Answer/cancel/expire through C04/C18. Administrator answers carry references only and
    /// cannot grant approval; a future explicit attempt still needs independently verified C08 facts.
    pub fn respond(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        operation_id: &CommandId,
        id: &Reference,
        command: &Command,
    ) -> Result<Receipt, Error> {
        let context = Some(caller);
        let execution = self.load(context, request, ExecutionAccess::Interact)?;
        let plan = execution.input();
        let op = operation(plan, "interaction-response", operation_id.as_str())?;
        let host =
            Host::new(&self.host, &self.binding, &self.config, context).with_input(Some(plan));
        Ok(self
            .store
            .apply_interaction(&op, &Scope::from_input(plan), id, command, &host)?
            .receipt()
            .clone())
    }
    /// Pull at-least-once results. Processing/transport delivery is not durable acknowledgement.
    pub fn pull_results(
        &self,
        caller: &RequestContext,
        request: &RequestId,
        consumer: &Id,
        limit: usize,
    ) -> Result<Vec<Receipt>, Error> {
        let context = Some(caller);
        let execution = self.load(context, request, ExecutionAccess::Delivery(consumer))?;
        Ok(self.store.pull_results(
            &Scope::from_input(execution.input()),
            consumer,
            limit,
            &self.adapter(context, None),
        )?)
    }
    /// Confirm one exact delivered event after the consumer has durably handled it. Out-of-order
    /// confirmations cannot skip other events and never replay runner effects.
    pub fn confirm(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        consumer: &Id,
        event: &EventId,
    ) -> Result<(), Error> {
        let context = Some(caller);
        let execution = self.load(context, request, ExecutionAccess::Delivery(consumer))?;
        let host = Host::new(&self.host, &self.binding, &self.config, context);
        Ok(self.store.confirm(
            &Scope::from_input(execution.input()),
            consumer,
            event,
            &host,
        )?)
    }
    /// Privileged evidence with independent current ReadAudit authorization. Ordinary status or
    /// result delivery rights do not authorize reading policy, approval or actor audit details.
    pub fn audit(
        &self,
        caller: &RequestContext,
        request: &RequestId,
        operation: &OperationRequestId,
    ) -> Result<AuditRecord, Error> {
        let context = Some(caller);
        let execution = self.load(context, request, ExecutionAccess::Audit)?;
        Ok(self.store.audit(
            &Scope::from_input(execution.input()),
            operation,
            &self.adapter(context, None),
        )?)
    }
}

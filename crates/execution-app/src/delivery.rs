use crate::{host::Host, service::operation, *};
use execution_contract::{EventId, Id, RequestId};
use execution_interaction::{Command, Interaction, Kind, Reference, Spec};
use execution_sqlite::{AuditRecord, OperationRequestId, Receipt, Scope};

impl<H: AppHost, R: RunnerPort> ExecutionApp<H, R> {
    /// Open a bounded interaction bound by the service to the actual stored task. This records a
    /// wait reason only, not authorization or an automatic continuation. Host owns responder rights.
    pub fn open_interaction(
        &mut self,
        request: &RequestId,
        id: Reference,
        kind: Kind,
        expires_at_unix_ms: u64,
    ) -> Result<Receipt, Error> {
        let execution = self.load(request)?;
        let plan = execution.plan();
        let scope = Scope::from_plan(plan);
        let op = operation(plan, "interaction-open", id.as_str())?;
        let spec = Spec {
            id,
            subject: scope.interaction_subject(),
            kind,
            expires_at_unix_ms,
        };
        let host = Host {
            inner: &self.host,
            binding: &self.binding,
            config: self.config.active().ok(),
            plan: Some(plan),
            observation: None,
        };
        Ok(self
            .store
            .open_interaction(&op, &scope, &spec, &host)?
            .receipt()
            .clone())
    }
    /// Read a pending or resolved interaction; no acknowledgement or execution is implied.
    pub fn interaction(&self, request: &RequestId, id: &Reference) -> Result<Interaction, Error> {
        let execution = self.load(request)?;
        Ok(self
            .store
            .interaction(&Scope::from_plan(execution.plan()), id, &self.adapter(None))?)
    }
    /// Answer/cancel/expire through C04/C18. Administrator answers carry references only and
    /// cannot grant approval; a future explicit attempt still needs independently verified C08 facts.
    pub fn respond(
        &mut self,
        request: &RequestId,
        operation_id: &RequestId,
        id: &Reference,
        command: &Command,
    ) -> Result<Receipt, Error> {
        let execution = self.load(request)?;
        let plan = execution.plan();
        let op = operation(plan, "interaction-response", operation_id.as_str())?;
        let host = Host {
            inner: &self.host,
            binding: &self.binding,
            config: self.config.active().ok(),
            plan: Some(plan),
            observation: None,
        };
        Ok(self
            .store
            .apply_interaction(&op, &Scope::from_plan(plan), id, command, &host)?
            .receipt()
            .clone())
    }
    /// Pull at-least-once results. Processing/transport delivery is not durable acknowledgement.
    pub fn pull_results(
        &self,
        request: &RequestId,
        consumer: &Id,
        limit: usize,
    ) -> Result<Vec<Receipt>, Error> {
        let execution = self.load(request)?;
        Ok(self.store.pull_results(
            &Scope::from_plan(execution.plan()),
            consumer,
            limit,
            &self.adapter(None),
        )?)
    }
    /// Confirm one exact delivered event after the consumer has durably handled it. Out-of-order
    /// confirmations cannot skip other events and never replay runner effects.
    pub fn confirm(
        &mut self,
        request: &RequestId,
        consumer: &Id,
        event: &EventId,
    ) -> Result<(), Error> {
        let execution = self.load(request)?;
        let host = Host {
            inner: &self.host,
            binding: &self.binding,
            config: self.config.active().ok(),
            plan: None,
            observation: None,
        };
        Ok(self
            .store
            .confirm(&Scope::from_plan(execution.plan()), consumer, event, &host)?)
    }
    /// Privileged evidence with independent current ReadAudit authorization. Ordinary status or
    /// result delivery rights do not authorize reading policy, approval or actor audit details.
    pub fn audit(
        &self,
        request: &RequestId,
        operation: &OperationRequestId,
    ) -> Result<AuditRecord, Error> {
        let execution = self.load(request)?;
        Ok(self.store.audit(
            &Scope::from_plan(execution.plan()),
            operation,
            &self.adapter(None),
        )?)
    }
}

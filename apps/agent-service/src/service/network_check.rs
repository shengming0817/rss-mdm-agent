//! Authenticated finite network diagnostics; execution remains in the original app/journal.
use super::*;
use crate::network_check::{self, ACTION, PROFILE};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply};
fn metadata<T: serde::de::DeserializeOwned>(plan: &FrozenExecution, key: &str) -> Result<T, Error> {
    let Some(InputValue::Literal { value }) = plan.spec().request.parameters.get(key) else {
        return Err(Error::Untrusted);
    };
    serde_json::from_value(value.clone()).map_err(|_| Error::Untrusted)
}
impl Core {
    pub(super) fn network_operation(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        input: &NetworkCheckOperation,
    ) -> Result<Reply, Error> {
        self.data_context(command, context)?;
        if !matches!(origin, ClientOrigin::Desktop {}) {
            return Err(Error::Denied);
        }
        let request = match input {
            NetworkCheckOperation::Run { request, .. }
            | NetworkCheckOperation::Status { request }
            | NetworkCheckOperation::Cancel { request } => request,
            NetworkCheckOperation::Defaults => return Err(Error::Protocol),
        };
        let uuid = uuid::Uuid::parse_str(request.as_str()).map_err(|_| Error::Protocol)?;
        if uuid.is_nil() || uuid.to_string() != request.as_str() {
            return Err(Error::Protocol);
        }
        if self
            .app
            .has_service_execution(request, &self.host.root.device)
            .map_err(map_app_error)?
        {
            let plan = self.network_input(command, context, request)?;
            if let NetworkCheckOperation::Run { target, .. } = input {
                if metadata::<NetworkTarget>(&plan, "network-check-target")?
                    != network_check::normalize(target)?
                {
                    return Err(Error::Conflict);
                }
            }
            if matches!(input, NetworkCheckOperation::Cancel { .. }) {
                self.app
                    .cancel(&self.local_caller(), request)
                    .map_err(map_app_error)?;
            }
            return Ok(Reply::NetworkCheck {
                value: self.network_status(&plan)?,
            });
        }
        let NetworkCheckOperation::Run { target, .. } = input else {
            return Err(Error::NotFound);
        };
        if self.stopping || self.host.local_stopped.load(Ordering::Acquire) {
            return Err(Error::Denied);
        }
        let target = network_check::normalize(target)?;
        if let Some(original) = self
            .app
            .service_resource_owner(&network_check::resource(context, &target)?)
            .map_err(map_app_error)?
        {
            let plan = self.network_input(command, context, &original)?;
            return Ok(Reply::NetworkCheck {
                value: self.network_status(&plan)?,
            });
        }
        let helper = self.helpers.connect_peer(
            &command.subject,
            command.session,
            command.binding.as_ref().ok_or(Error::Denied)?,
        )?;
        let initiator = self.software_initiator(command, origin)?;
        let (plan, artifacts) = network_check::compile::prepare(
            target,
            request.clone(),
            &self.host.root,
            &self.host.local_actor,
            initiator,
            context,
            self.host.clock.millis()?,
            &self.config,
            &mut self.material_root,
            Some(helper),
        )?;
        self.host
            .materials
            .register(&plan, artifacts)
            .map_err(map_app_error)?;
        let permit = network_check::Permit::new(plan.clone())?;
        let fences = self.fences.clone();
        let key = policy::SessionKey::new(
            command.subject.clone(),
            command.binding.clone().ok_or(Error::Denied)?,
        );
        let _guard = fences.admit(&key, command.fence_epoch)?;
        *self
            .host
            .local_network
            .lock()
            .map_err(|_| Error::Unavailable)? = Some(permit);
        let submitted = self
            .app
            .request_execution(&self.local_caller(), &plan)
            .map_err(map_app_error);
        *self
            .host
            .local_network
            .lock()
            .map_err(|_| Error::Unavailable)? = None;
        submitted?;
        Ok(Reply::NetworkCheck {
            value: self.network_status(&plan)?,
        })
    }
    fn network_input(
        &self,
        command: &Command,
        context: &NativeContextRef,
        request: &RequestId,
    ) -> Result<FrozenExecution, Error> {
        let plan = self
            .app
            .frozen_input(&self.local_caller(), request)
            .map_err(map_app_error)?;
        if plan.spec().launch.interpreter.profile.id.as_str() != PROFILE
            || plan.spec().request.operation.action.as_str() != ACTION
        {
            return Err(Error::Denied);
        }
        self.data_context(command, context)?;
        crate::account_identity::verify_local_owner(&plan, &self.host.root, &command.subject)?;
        Ok(plan)
    }
    fn network_status(
        &mut self,
        plan: &FrozenExecution,
    ) -> Result<execution_app::NetworkCheckStatus, Error> {
        let request = &plan.spec().request.request_id;
        self.app.reconcile(request).map_err(map_app_error)?;
        self.finish_local_result(request)?;
        let status = self
            .app
            .status(&self.local_caller(), request)
            .map_err(map_app_error)?;
        let result = self
            .app
            .service_process_evidence(request)
            .map_err(map_app_error)?
            .filter(|p| {
                p.finished
                    && p.quiescent
                    && p.exit_code == Some(0)
                    && p.quality == OutputQuality::Complete
                    && p.content_digest == *plan.digest()
                    && serde_json::from_slice::<CollectionCompletion>(&p.stderr).is_ok_and(|r| {
                        r.closed
                            && r.input_sha256 == plan.spec().launch.artifact.sha256.as_str()
                            && crate::local_execution::hash(&p.stdout)
                                .is_ok_and(|h| h.as_str() == r.output_sha256)
                    })
            })
            .and_then(|p| serde_json::from_slice::<NetworkCheckResult>(&p.stdout).ok())
            .filter(|r| {
                metadata::<NetworkTarget>(plan, "network-check-target")
                    .ok()
                    .as_ref()
                    == Some(&r.target)
                    && r.tcp.len() <= 4
            });
        Ok(execution_app::NetworkCheckStatus { status, result })
    }
}

//! Trusted backend submission; HTTP waits borrow only communication and retain the sole owner.
use super::{host::BackendPermit, offered, plan};
use crate::{
    service::{helper_error, network, Command, Core, Selection},
    DeviceService,
};
use agent_client::{Error, IdentityProvider, Materials, Offer};
use execution_app::ExecutionStatus;
use execution_contract::BackendRequestState;
use rss_mdm_agent_wire::runtime as wire;
use std::sync::Arc;
pub(crate) async fn submit<S: IdentityProvider>(
    service: &mut DeviceService<S>,
    offer: Offer,
    materials: Materials,
    selection: Option<Selection>,
    commands: &mut tokio::sync::mpsc::Receiver<Command>,
) -> Result<ExecutionStatus, Error> {
    if let Some(selection) = &selection {
        let current = offered(&offer)?;
        if selection.task != current.task
            || selection.attempt != current.attempt
            || selection.revision != current.revision
        {
            return Err(Error::Conflict);
        }
    }
    if matches!(offer.payload(), wire::TaskPayload::Enrollment(_))
        && service.core.mdm.state != wire::MdmEnrollmentState::Unenrolled
    {
        return Err(Error::Denied);
    }
    service.core.available = None;
    let needs_user = match offer.payload() {
        wire::TaskPayload::DeviceAccess(_) => return Err(Error::Unsupported),
        wire::TaskPayload::Unmanage(_) => return Err(Error::Unsupported),
        wire::TaskPayload::Credential(_) => false,
        wire::TaskPayload::Collection(p) => {
            p.execution.identity == wire::ExecutionIdentity::LoggedInUser
        }
        wire::TaskPayload::Enrollment(_) => true,
        wire::TaskPayload::Script(p) => p.run_as == wire::ExecutionIdentity::LoggedInUser,
        wire::TaskPayload::Software(p) => p
            .steps
            .iter()
            .any(|step| matches!(step.target, wire::SoftwareExecutionTarget::User { .. })),
    };
    let login = if needs_user || selection.is_some() {
        Some(service.core.helpers.connect(selection.as_ref())?)
    } else {
        None
    };
    let delegate = if needs_user { login.clone() } else { None };
    let (preview, artifacts) = compile(
        &mut service.core,
        &offer,
        &materials,
        offer.payload(),
        delegate.clone(),
        super::enrollment::NativeMaterials::default(),
    )?;
    artifacts
        .inspect(&preview)
        .map_err(crate::error::app_error)?;
    if let Some(selection) = &selection {
        let record = service
            .core
            .app
            .backend_request(&service.core.caller(), &selection.request)
            .map_err(crate::error::app_error)?
            .ok_or(Error::Conflict)?;
        service.core.current_policy_access(&record)?;
    }
    let start = if selection.is_some() {
        network(
            service.client.start_user_initiated(&offer, &materials),
            &mut service.core,
            commands,
        )
        .await?
    } else {
        network(
            service.client.request_start(&offer, &materials),
            &mut service.core,
            commands,
        )
        .await?
    };
    service.client.validate_start(&start)?;
    if let Some(login) = &login {
        let account = execution_contract::OsAccountRef {
            platform: plan::platform()?,
            subject: plan::id(&login.context().subject)?,
        };
        login
            .verify_context(
                &execution_contract::RunAs::User {
                    account: account.clone(),
                },
                &execution_contract::SessionRequirement::ActiveUser {
                    account,
                    session: login.context().binding.clone(),
                },
            )
            .map_err(helper_error)?;
    }
    let material = if matches!(start.payload(), wire::TaskPayload::Enrollment(_)) {
        Some(
            network(
                service.client.native_enrollment(&start),
                &mut service.core,
                commands,
            )
            .await?,
        )
    } else {
        None
    };
    let profile = if matches!(start.payload(), wire::TaskPayload::Enrollment(p) if matches!(p.entry, wire::EnrollmentEntry::Macos { .. }))
    {
        let ca = service
            .client
            .configuration()
            .ca_pem
            .clone()
            .ok_or(Error::Configuration)?;
        Some(
            network(
                material.as_ref().ok_or(Error::Identity)?.profile(&ca),
                &mut service.core,
                commands,
            )
            .await?,
        )
    } else {
        None
    };
    dispatch_with_material(
        service,
        offer,
        materials,
        selection,
        start,
        delegate,
        super::enrollment::NativeMaterials {
            material,
            profile_bytes: profile,
        },
    )
}

// Single synchronous delivery boundary after HTTP Start and native helper verification.
// Explicit gate fixtures may exercise this boundary without claiming native OS authentication.
#[cfg(test)]
pub(crate) fn dispatch_started<S: IdentityProvider>(
    service: &mut DeviceService<S>,
    offer: Offer,
    materials: Materials,
    selection: Option<Selection>,
    start: agent_client::Start,
    delegate: Option<Arc<execution_ipc::helper::Connection>>,
) -> Result<ExecutionStatus, Error> {
    dispatch_with_material(
        service,
        offer,
        materials,
        selection,
        start,
        delegate,
        super::enrollment::NativeMaterials::default(),
    )
}
fn dispatch_with_material<S: IdentityProvider>(
    service: &mut DeviceService<S>,
    offer: Offer,
    materials: Materials,
    selection: Option<Selection>,
    start: agent_client::Start,
    delegate: Option<Arc<execution_ipc::helper::Connection>>,
    native: super::enrollment::NativeMaterials,
) -> Result<ExecutionStatus, Error> {
    if service
        .core
        .host
        .revoked
        .load(std::sync::atomic::Ordering::Acquire)
        || service.core.stopping
    {
        return Err(Error::Denied);
    }
    let record = if let Some(selection) = &selection {
        let current = service
            .core
            .app
            .backend_request(&service.core.caller(), &selection.request)
            .map_err(crate::error::app_error)?
            .ok_or(Error::Conflict)?;
        if current.state != BackendRequestState::Submitting
            || current.trigger != selection.trigger(&service.core.host.binding.device)?
        {
            return Err(Error::Denied);
        }
        service.core.current_policy_access(&current)?;
        Some(current)
    } else {
        None
    };
    let (mut plan, artifacts) = compile(
        &mut service.core,
        &offer,
        &materials,
        start.payload(),
        delegate,
        native,
    )?;
    if let Some(selection) = selection {
        let mut input = plan.spec().clone();
        let execution_contract::Initiator::Backend { trigger, .. } = &mut input.request.initiator
        else {
            return Err(Error::Untrusted);
        };
        let os_session = execution_contract::OsSessionRef {
            device: service.core.host.binding.device.clone(),
            account: execution_contract::OsAccountRef {
                platform: plan::platform()?,
                subject: plan::id(selection.subject)?,
            },
            session: selection.binding,
        };
        *trigger = match selection.origin {
            execution_ipc::host::ClientOrigin::Desktop {} => {
                execution_contract::BackendTrigger::Human { os_session }
            }
            execution_ipc::host::ClientOrigin::Ai {
                config,
                conversation,
                tool_call,
                context,
            } => execution_contract::BackendTrigger::Ai {
                os_session,
                config,
                conversation,
                tool_call,
                context,
            },
        };
        plan = execution_contract::FrozenExecution::freeze(input, &plan::storage_limits().input)
            .map_err(|_| Error::Untrusted)?;
    }
    let gate = super::gate::ProductGateProof::verify_start(
        &offer,
        &start,
        &plan,
        record.as_ref(),
        service.core.available_risk.as_ref(),
        service.core.host.clock.millis()?,
    )?;
    // Linearize revocation with the sole owner's irreversible runner delivery.
    // This guard spans synchronous admission only; no HTTP wait holds it.
    let admission = record
        .as_ref()
        .map(|record| service.core.policy_fence(record))
        .transpose()?
        .flatten();
    let _fence = admission
        .as_ref()
        .map(|(fences, subject, epoch)| fences.admit(subject, *epoch))
        .transpose()?;
    service
        .core
        .host
        .materials
        .register(&plan, artifacts)
        .map_err(crate::error::app_error)?;
    *service
        .core
        .host
        .current
        .lock()
        .map_err(|_| Error::Unavailable)? = Some(Arc::new(BackendPermit {
        plan: plan.clone(),
        start: start.clone(),
        gate,
    }));
    let caller = service.core.caller();
    let result = (|| {
        let prepared =
            service
                .bridge
                .prepare(&offer, &materials, &service.core.app, &caller, &plan)?;
        service.bridge.dispatch(
            &mut service.client,
            start,
            &materials,
            &mut service.core.app,
            prepared,
        )
    })();
    *service
        .core
        .host
        .current
        .lock()
        .map_err(|_| Error::Unavailable)? = None;
    if result.is_err()
        && matches!(
            service
                .core
                .app
                .frozen_input(&caller, &plan.spec().request.request_id),
            Err(execution_app::Error::NotFound)
        )
    {
        service
            .core
            .host
            .materials
            .retire(plan.digest())
            .map_err(crate::error::app_error)?;
    }
    result
}
fn compile(
    core: &mut Core,
    offer: &Offer,
    materials: &Materials,
    payload: &wire::TaskPayload,
    delegate: Option<Arc<execution_ipc::helper::Connection>>,
    native: super::enrollment::NativeMaterials,
) -> Result<
    (
        execution_contract::FrozenExecution,
        execution_runner::Artifacts,
    ),
    Error,
> {
    match payload {
        wire::TaskPayload::DeviceAccess(_) | wire::TaskPayload::Unmanage(_) => {
            Err(Error::Unsupported)
        }
        wire::TaskPayload::Credential(payload) => super::credential::compile(
            offer,
            materials,
            payload,
            &core.host.binding,
            &core.host.actor,
            (&core.config, &mut core.material_root),
            delegate,
        ),
        wire::TaskPayload::Collection(payload) => super::collection::compile(
            offer,
            materials,
            payload,
            &core.host.binding,
            &core.host.actor,
            (&core.config, &mut core.material_root),
            delegate,
        ),
        wire::TaskPayload::Enrollment(payload) => super::enrollment::compile(
            offer,
            materials,
            payload,
            (
                &core.host.binding,
                &core.host.actor,
                &mut core.material_root,
            ),
            delegate,
            native,
        ),
        wire::TaskPayload::Software(payload) => crate::backend::software::compile(
            offer,
            materials,
            payload,
            &core.host.binding,
            &core.host.actor,
            (&core.config, &mut core.material_root),
            delegate,
        ),
        wire::TaskPayload::Script(payload) => {
            let work_root = delegate
                .as_ref()
                .map_or(&core.config.work_root, |h| &h.context().work_root);
            let file = materials.files().first().ok_or(Error::Untrusted)?;
            let content = core
                .material_root
                .publish(
                    delegate.as_ref().map(|h| h.context().subject.as_str()),
                    file.reader()?,
                    &execution_contract::Digest::new(plan::hex(&payload.content.sha256))
                        .map_err(|_| Error::Protocol)?,
                    payload.content.length,
                )
                .map_err(crate::error::app_error)?;
            plan::script(
                offer,
                materials,
                payload,
                (&core.host.binding, &core.host.actor),
                &core.config.interpreters,
                (work_root, &content),
                delegate.clone(),
            )
        }
    }
}

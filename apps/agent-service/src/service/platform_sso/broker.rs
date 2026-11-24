//! The desktop authorizes one employee, live console and registered RP. The signed
//! extension redeems that selection against the original browser cookie and server challenge.
use super::*;
use execution_ipc::device_access::{EmployeePortal, Response as EmployeeReply};
use rss_mdm_agent_wire::{enterprise as broker, runtime as access};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

struct Selection {
    binding: Uuid,
    device: rss_mdm_agent_wire::DeviceId,
    portal: EmployeePortal,
    session: Uuid,
    current: Box<dyn Fn() -> bool + Send + Sync>,
    until: Instant,
    operation: Option<Uuid>,
}
static SELECTED: Mutex<Option<Selection>> = Mutex::new(None);

pub(crate) fn select(
    directory: EmployeeReply,
    binding: Uuid,
    session: Uuid,
    employee: &access::SelfServiceUser,
    current: impl Fn() -> bool + Send + Sync + 'static,
) -> Result<EmployeeReply, Error> {
    let EmployeeReply::OwnBindings {
        device, bindings, ..
    } = directory
    else {
        return Err(Error::Identity);
    };
    if !bindings.iter().any(|b| b.id == binding && b.configuration.enabled
        && b.applied_revision == Some(b.revision) && matches!(&b.effect,Some(access::AccessEffect::Applied{account,..}) if Some(account)==b.account.as_ref())
        && matches!(b.account, Some(access::AccessAccount::Macos{..}))) || !current() {
        return Err(Error::Denied);
    }
    let portal = EmployeePortal::configured(employee.tenant_id, employee.instance_id, None)
        .map_err(|_| Error::Configuration)?
        .ok_or(Error::Configuration)?;
    let launch_url = portal
        .application
        .as_ref()
        .ok_or(Error::Configuration)?
        .launch_url
        .to_string();
    *SELECTED.lock().map_err(|_| Error::Storage)? = Some(Selection {
        binding,
        device,
        portal,
        session,
        current: Box::new(current),
        until: Instant::now() + Duration::from_secs(120),
        operation: None,
    });
    Ok(EmployeeReply::Application { launch_url })
}

pub(super) async fn challenge(
    owner: &mut (impl Owner + ?Sized),
    command: &Command,
    operation: Uuid,
    cookie: &access::Secret,
) -> Result<Response, Error> {
    let scope = w::Kind::User;
    let mode = Some(w::AuthenticationMode::SecureEnclave);
    let context = owner.context(command, scope, true, mode).await?;
    let binding = context.binding.ok_or(Error::Identity)?;
    let session = super::super::login_binding_id(command.binding.as_ref().ok_or(Error::Identity)?);
    let admit = || -> Result<EmployeePortal, Error> {
        let mut lock = SELECTED.lock().map_err(|_| Error::Storage)?;
        let selected = lock.as_mut().ok_or(Error::Denied)?;
        if selected.until <= Instant::now()
            || !(selected.current)()
            || selected.binding != binding
            || selected.session != session
            || selected.portal.tenant != context.tenant
            || selected.portal.origin != context.origin
            || (context.authority == w::RegistrationTrust::DaDevicePop
                && selected.device != context.device_id)
            || selected.operation.is_some_and(|op| op != operation)
        {
            return Err(Error::Denied);
        }
        selected.operation = Some(operation);
        Ok(selected.portal.clone())
    };
    let portal = admit()?;
    let begin = broker::Begin {
        operation_id: operation,
        binding_id: binding,
        relying_party: portal
            .application
            .as_ref()
            .ok_or(Error::Configuration)?
            .relying_party
            .clone(),
    };
    let challenge = agent_client::enterprise::challenge(
        &portal.origin,
        portal.ca_pem.as_deref(),
        portal.tenant,
        cookie,
        &begin,
    )
    .await?;
    if challenge.local_session.session_id != session
        || challenge.expires_at <= (crate::wall()? / 1000) as i64
        || !matches!(&challenge.local_session.account,access::AccessAccount::Macos{generated_uid} if Some(generated_uid.to_string()) == context.generated_uid)
        || context != owner.context(command, scope, true, mode).await?
    {
        return Err(Error::Identity);
    }
    admit()?;
    Ok(Response::BrokerChallenge { challenge })
}

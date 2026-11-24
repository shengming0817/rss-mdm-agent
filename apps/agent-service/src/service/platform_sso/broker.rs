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
impl Selection {
    fn admit(
        &mut self,
        session: Uuid,
        operation: Uuid,
        now: Instant,
        confirmed: bool,
    ) -> Result<(), Error> {
        if self.until <= now
            || !(self.current)()
            || self.session != session
            || self.operation.is_some_and(|selected| selected != operation)
        {
            return Err(Error::Denied);
        }
        if confirmed {
            self.operation = Some(operation);
        }
        Ok(())
    }
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
    let admit = |confirmed: bool| -> Result<EmployeePortal, Error> {
        let mut lock = SELECTED.lock().map_err(|_| Error::Storage)?;
        let selected = lock.as_mut().ok_or(Error::Denied)?;
        if selected.binding != binding
            || selected.portal.tenant != context.tenant
            || selected.portal.origin != context.origin
            || (context.authority == w::RegistrationTrust::DaDevicePop
                && selected.device != context.device_id)
        {
            return Err(Error::Denied);
        }
        selected.admit(session, operation, Instant::now(), confirmed)?;
        Ok(selected.portal.clone())
    };
    let portal = admit(false)?;
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
    admit(true)?;
    Ok(Response::BrokerChallenge { challenge })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_consumed_only_by_the_authenticated_transaction_and_current_console() {
        let now = Instant::now();
        let session = Uuid::new_v4();
        let op = Uuid::new_v4();
        let other = Uuid::new_v4();
        let live = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let observed = live.clone();
        let mut selection = Selection {
            binding: Uuid::new_v4(), device: rss_mdm_agent_wire::DeviceId::new_v4(),
            portal: serde_json::from_value(serde_json::json!({"origin":"https://example.test","tenant":Uuid::new_v4(),"instance":Uuid::new_v4(),"product":"mdm"})).unwrap(),
            session, current: Box::new(move || observed.load(std::sync::atomic::Ordering::Acquire)),
            until: now + Duration::from_secs(120), operation: None
        };
        assert!(selection.admit(session, other, now, false).is_ok());
        assert!(selection.admit(session, op, now, true).is_ok());
        assert!(selection.admit(session, op, now, true).is_ok());
        assert!(selection.admit(session, other, now, true).is_err());
        assert!(selection.admit(Uuid::new_v4(), op, now, false).is_err());
        assert!(selection
            .admit(session, op, selection.until, false)
            .is_err());
        live.store(false, std::sync::atomic::Ordering::Release);
        assert!(selection.admit(session, op, now, false).is_err());
    }
}

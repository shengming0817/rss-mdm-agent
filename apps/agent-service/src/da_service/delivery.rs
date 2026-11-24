//! One bounded native step per current DA permit. Escrow and native confirmation are distinct.
use super::{login::Login, *};
use crate::{access_worker as access, credential_worker as credential};
use credential::state;
use device_access::{Channel, Registration, Store};
use execution_contract::{Digest, Platform, RunAs, SessionRequirement};
use rss_mdm_agent_wire::{
    da::{AccountObservation as Observation, CredentialEvent, CredentialState},
    runtime as w,
};

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Attempt {
    renew_from: Option<i64>,
    started: bool,
    permit: Option<w::AccessTaskSpec>,
}
fn now() -> Result<i64, Error> {
    Ok((crate::wall()? / 1000) as i64)
}

pub(super) async fn drive(
    d: &Deployment,
    client: &da::Client<DeviceSecrets, SystemClock>,
    login: &Login,
    enable: bool,
) -> Result<(), Error> {
    let root = access::root(&d.execution);
    let mut store = Store::open(&root, d.installation_id).map_err(|_| Error::Storage)?;
    let ops = client.operations()?;
    let r = ops.registration();
    let owner = Registration {
        channel: Channel::Da,
        tenant: r.tenant,
        device: r.device,
    };
    store
        .bind_registration(owner.clone())
        .map_err(|_| Error::Identity)?;
    let account = account(d, &ops, &root, login, enable).await;
    let credentials = credentials(d, &ops).await;
    for usage in store.recovery_uses(&owner).map_err(|_| Error::Storage)? {
        ops.recovery_use(&usage).await?;
        store
            .acknowledge_recovery_use(&owner, usage.operation_id)
            .map_err(|_| Error::Storage)?;
    }
    account.and(credentials)
}

async fn account(
    d: &Deployment,
    ops: &da::Operations<'_>,
    root: &Path,
    login: &Login,
    enable: bool,
) -> Result<(), Error> {
    ops.replay_account().await?;
    ops.retire_account(|id, attempt| {
        let key = format!("{id}.{attempt}.da-attempt");
        if let Some(record) = state::read_optional::<Attempt>(root, &key)? {
            let t = record.permit.ok_or(Error::Storage)?;
            if (t.task_id, t.attempt_id) != (id, attempt) {
                return Err(Error::Identity);
            }
            access::retire_native(root, d.installation_id, &t)?;
            state::remove(root, &key)?;
        }
        Ok(())
    })?;
    let Some(o) = ops.account().await? else {
        return login.offer(None);
    };
    let key = format!("{}.{}.da-attempt", o.task_id, o.attempt_id);
    let mut attempt: Attempt = state::read_optional(root, &key)?.unwrap_or_default();
    let event = attempt
        .renew_from
        .map_or(Observation::Received, |expires_at| {
            Observation::NotStarted { expires_at }
        });
    let ack = ops.account_event(&o, event).await?;
    let Some(t) = ack.permit else {
        login.offer(None)?;
        if let Some(old) = &attempt.permit {
            if let Some(p) = access::prepared(root, old)? {
                ops.account_event(&o, Observation::Prepared(p)).await?;
            }
            if let Some(result) = access::observe_da(root, d.installation_id, old)? {
                ops.account_event(&o, Observation::Result(result)).await?;
                return Ok(());
            }
        }
        if let Some(exp) = ack
            .issued_expires_at
            .filter(|v| *v <= now().unwrap_or(i64::MIN))
        {
            if !attempt.started {
                attempt.renew_from = Some(exp);
                state::write(root, &key, &attempt)?;
                ops.account_event(&o, Observation::NotStarted { expires_at: exp })
                    .await?;
            } else if let Some(t) = &attempt.permit {
                if access::create_absent(root, t)? {
                    ops.account_event(&o, Observation::CreateAbsent { expires_at: exp })
                        .await?;
                    return Ok(());
                }
                ops.account_event(
                    &o,
                    Observation::Result(access::report(t, w::AccessEffect::Unknown)),
                )
                .await?;
            }
        }
        return Ok(());
    };
    t.validate()?;
    let r = ops.registration();
    if (t.tenant_id, t.device_id) != (r.tenant, r.device)
        || t.platform != w::TaskPlatform::Windows
        || t.architecture != w::TaskArchitecture::X86_64
        || !cfg!(target_arch = "x86_64")
        || t.permit != w::TaskPermit::Start
        || (t.task_id, t.attempt_id, t.binding_id, t.definition_digest)
            != (o.task_id, o.attempt_id, o.binding_id, o.definition_digest)
        || Some(t.expires_at) != ack.issued_expires_at
        || t.expires_at <= now()?
        || t.deadline <= now()?
    {
        return Err(Error::Untrusted);
    }
    match &t.action {
        w::AccessAction::Apply {
            account:
                w::AccessAccount::Windows {
                    authority: w::AccessWindowsAuthority::Local,
                    ..
                },
            configuration,
            ..
        }
        | w::AccessAction::Create { configuration, .. }
            if matches!(
                t.windows,
                Some(
                    w::AccessWindowsPlan::Initialize { .. } | w::AccessWindowsPlan::Preserve { .. }
                )
            ) && (!configuration.enabled || enable) =>
        {
            ()
        }
        w::AccessAction::Unbind {
            account:
                w::AccessAccount::Windows {
                    authority: w::AccessWindowsAuthority::Local,
                    ..
                },
            ..
        } if t.windows.is_none() => (),
        _ => {
            login.offer(None)?;
            return Err(Error::Denied);
        }
    }
    attempt.permit = Some(t.clone());
    state::write(root, &key, &attempt)?;
    if let Some(result) = access::stored_result(root, &t)? {
        login.offer(None)?;
        ops.account_event(&o, Observation::Result(result)).await?;
        return Ok(());
    }
    let needs_setup = matches!(t.windows, Some(w::AccessWindowsPlan::Initialize { .. }));
    if needs_setup && !attempt.started {
        login.offer(Some(&t))?;
    } else {
        login.offer(None)?;
    }
    let input = access::Input {
        channel: Channel::Da,
        task: t.clone(),
        root: root.to_owned(),
        installation: d.installation_id,
        image_sha256: Digest::new(d.service.sha256.clone()).map_err(|_| Error::Identity)?,
        run_as: RunAs::System {
            platform: Platform::Windows,
        },
        session: SessionRequirement::NotRequired {},
    };
    let step = access::drive_da(&input, || {
        let password = if needs_setup && !attempt.started {
            login.input(&t)?
        } else {
            None
        };
        if !attempt.started {
            attempt.started = true;
            state::write(root, &key, &attempt)?;
        }
        login.offer(None)?;
        Ok(password)
    });
    let mut result = match step {
        Ok(Some(v)) => v,
        Ok(None) | Err(Error::Denied) => return Ok(()),
        Err(e) => return Err(e),
    };

    if let Some(p) = access::prepared(root, &t)? {
        ops.account_event(&o, Observation::Prepared(p)).await?;
    }
    if let Some(escrow) = access::pending(root, &t)? {
        let receipt = ops
            .account_event(&o, Observation::Escrow(escrow.clone()))
            .await?
            .receipt
            .ok_or(Error::Protocol)?;
        access::acknowledge(root, &t, &escrow, vec![receipt])?;
        // Revalidate Start against current authority after the network custody barrier.
        if matches!(result.effect, w::AccessEffect::Unknown) {
            let event = attempt
                .renew_from
                .map_or(Observation::Received, |expires_at| {
                    Observation::NotStarted { expires_at }
                });
            if ops.account_event(&o, event).await?.permit.as_ref() != Some(&t)
                || t.expires_at <= now()?
            {
                return Err(Error::Expired);
            }
            let Some(next) = access::drive_da(&input, || Ok(None))? else {
                return Ok(());
            };
            result = next;
        }
    }
    ops.account_event(&o, Observation::Result(result)).await?;
    Ok(())
}

async fn credentials(d: &Deployment, ops: &da::Operations<'_>) -> Result<(), Error> {
    ops.replay_credential().await?;
    ops.retire_credential(|t| credential::retire(&credential::root(&d.execution), t))?;
    let Some(t) = ops.credential().await? else {
        return Ok(());
    };
    if !cfg!(target_arch = "x86_64") || t.expires_at <= now()? || t.deadline <= now()? {
        return Err(Error::Expired);
    }
    let ack = ops
        .credential_event(
            &t,
            CredentialEvent::Received {
                attempt_id: t.attempt_id,
                definition_digest: t.definition_digest,
            },
        )
        .await?;
    if ack.received_at.is_none()
        || !matches!(
            ack.state,
            CredentialState::Accepted
                | CredentialState::PendingApplication
                | CredentialState::Reconciling
        )
    {
        return Err(Error::Denied);
    }
    let root = credential::prepare_root(&d.execution)?;
    let input = credential::Input {
        channel: Channel::Da,
        task: t.clone(),
        root: root.clone(),
        image_sha256: Digest::new(d.service.sha256.clone()).map_err(|_| Error::Identity)?,
        run_as: RunAs::System {
            platform: Platform::Windows,
        },
        session: SessionRequirement::NotRequired {},
    };
    let result = credential::drive_da(&input);
    let Some(escrow) = credential::pending(&root, &t)? else {
        return result.and(Err(Error::Unavailable));
    };
    let custody = ops
        .credential_event(
            &t,
            CredentialEvent::Escrow {
                attempt_id: t.attempt_id,
                value: escrow.clone(),
            },
        )
        .await?;
    let receipt = custody.receipt.ok_or(Error::Protocol)?;
    if custody.state != CredentialState::PendingApplication
        || escrow.materials.len() != 1
        || receipt.subject != escrow.materials[0].subject
    {
        return Err(Error::Untrusted);
    }
    receipt.validate()?;
    credential::acknowledge(&root, &t, &escrow, std::slice::from_ref(&receipt))?;
    if t.expires_at <= now()? {
        return Err(Error::Expired);
    }
    let actual = credential::drive_da(&input)?;
    if !actual.complete
        || actual.failure.is_some()
        || actual.definition_digest != t.definition_digest
        || actual.items.len() != 1
        || actual.items[0].subject != receipt.subject
        || actual.items[0].outcome != w::CredentialOutcome::Verified
    {
        return Err(Error::Unavailable);
    }
    let settled = ops
        .credential_event(
            &t,
            CredentialEvent::Applied {
                attempt_id: t.attempt_id,
                definition_digest: t.definition_digest,
                receipt: receipt.clone(),
            },
        )
        .await?;
    if settled.state != CredentialState::Applied || settled.receipt.as_ref() != Some(&receipt) {
        return Err(Error::Conflict);
    }
    Ok(())
}

//! Desktop input and original-attempt settlement on the independent DA device listener.
use super::*;
use crate::{access_worker as access, credential_worker::state};
use access::macos::{
    self,
    creation::{self, Evidence, Outcome},
};
use rss_mdm_agent_wire::{
    da::{AccountEffect, AccountObservation as Event, AccountOffer, AccountSettlement},
    runtime as w,
};
use std::path::PathBuf;
mod input;
pub(crate) use input::{Employee, Presence, Session, Setup};

type Ids = (Uuid, Uuid);
fn ids(t: &w::AccessTaskSpec) -> Ids {
    (t.task_id, t.attempt_id)
}
fn seconds() -> Result<i64, Error> {
    Ok((crate::wall()? / 1000) as i64)
}

fn validate(
    registration: &rss_mdm_agent_wire::da::Registration,
    offer: &AccountOffer,
    ack: &AccountSettlement,
    t: &w::AccessTaskSpec,
    previous: Option<&w::AccessTaskSpec>,
    now: i64,
) -> Result<(), Error> {
    t.validate()?;
    if (registration.tenant, registration.device) != (t.tenant_id, t.device_id)
        || (
            offer.task_id,
            offer.attempt_id,
            offer.binding_id,
            offer.definition_digest,
        ) != (t.task_id, t.attempt_id, t.binding_id, t.definition_digest)
        || (t.platform, t.architecture, t.permit)
            != (
                w::TaskPlatform::Macos,
                w::TaskArchitecture::Aarch64,
                w::TaskPermit::Start,
            )
        || t.windows.is_some()
        || t.expires_at <= now
        || t.deadline <= now
        || ack.issued_expires_at != Some(t.expires_at)
    {
        return Err(Error::Untrusted);
    }
    if let Some(old) = previous {
        let mut renewed = old.clone();
        renewed.expires_at = t.expires_at;
        renewed.deadline = t.deadline;
        if renewed != *t {
            return Err(Error::Conflict);
        }
    }
    Ok(())
}

fn result(
    t: &w::AccessTaskSpec,
    effect: impl Into<AccountEffect>,
) -> rss_mdm_agent_wire::da::AccountResult {
    w::AccessResult {
        binding_id: t.binding_id,
        binding_generation: t.binding_generation,
        revision: t.revision,
        definition_digest: t.definition_digest,
        effect: effect.into(),
    }
}

fn created(
    t: &w::AccessTaskSpec,
    a: macos::NativeAccount,
) -> Result<rss_mdm_agent_wire::da::AccountResult, Error> {
    let actual = macos::facts(&a);
    let generated_uid = Uuid::parse_str(&a.generated_uid).map_err(|_| Error::Identity)?;
    Ok(result(
        t,
        AccountEffect::MacAccountCreated {
            generated_uid,
            role: if a.administrator {
                w::AccessRole::Administrator
            } else {
                w::AccessRole::Standard
            },
            secure_token: actual.secure_token,
            volume_owner: actual.volume_owner,
            filevault_unlock: actual.filevault_unlock,
        },
    ))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| Error::Unavailable)?
}
#[derive(Clone)]
struct Journal {
    root: PathBuf,
    native: PathBuf,
}
impl Journal {
    fn create(
        &self,
        spec: &w::AccessTaskSpec,
        account_name: &str,
        setup: &Setup,
        employee: &Employee,
    ) -> Result<Step, Error> {
        let _lock = state::lock(&self.native)?;
        if !employee.permits(spec) || spec.expires_at <= seconds()? {
            return Err(Error::Denied);
        }
        match creation::create(&self.root, spec, account_name, || {
            Ok(input::NativeInput {
                setup,
                task: spec,
                employee,
            })
        }) {
            Ok(Outcome::Created(a)) => Ok(Step::Created(a)),
            Ok(Outcome::NameConflict) => {
                let r = result(
                    spec,
                    w::AccessEffect::Failed {
                        reason: w::AccessFailure::NameConflict,
                    },
                );
                self.save(spec, &r)?;
                Ok(Step::Effect(r))
            }
            Err(e)
                if matches!(
                    creation::evidence(&self.root, spec, account_name)?,
                    Evidence::Unstarted
                ) =>
            {
                Err(e)
            }
            Err(_) => Ok(Step::Effect(result(spec, w::AccessEffect::Unknown))),
        }
    }
    fn new(d: &Deployment, device: rss_mdm_agent_wire::DeviceId) -> Result<Self, Error> {
        let root = d.da_root()?.join(format!("account-{device}"));
        let native = access::root(&d.execution);
        platform_private_storage::directory(&root)?;
        platform_private_storage::directory(&d.execution.material_root)?;
        platform_private_storage::directory(&native)?;
        Ok(Self { root, native })
    }
    fn saved(
        &self,
        t: &w::AccessTaskSpec,
    ) -> Result<Option<rss_mdm_agent_wire::da::AccountResult>, Error> {
        let value: Option<rss_mdm_agent_wire::da::AccountResult> =
            state::read_optional(&self.root, &access::name(t, "result"))?;
        if value.as_ref().is_some_and(|v| {
            (
                v.binding_id,
                v.binding_generation,
                v.revision,
                v.definition_digest,
            ) != (
                t.binding_id,
                t.binding_generation,
                t.revision,
                t.definition_digest,
            )
        }) {
            return Err(Error::Identity);
        }
        Ok(value)
    }
    fn save(
        &self,
        t: &w::AccessTaskSpec,
        value: &rss_mdm_agent_wire::da::AccountResult,
    ) -> Result<(), Error> {
        state::write(&self.root, &access::name(t, "result"), value)
    }
    fn evidence(&self, t: &w::AccessTaskSpec) -> Result<Evidence, Error> {
        let _lock = state::lock(&self.native)?;
        match &t.action {
            w::AccessAction::Create { name, .. } => creation::evidence(&self.root, t, name),
            _ => Ok(Evidence::Unstarted),
        }
    }
    fn cleanup(
        &self,
        id: Uuid,
        attempt_id: Uuid,
        done: da::AccountCompletion,
    ) -> Result<(), Error> {
        let _lock = state::lock(&self.native)?;
        if done == da::AccountCompletion::Rejected {
            let key = attempt::key(id, attempt_id);
            if let Some(record) = state::read_optional::<attempt::Attempt>(&self.root, &key)? {
                if !record.started {
                    if let Some(t) = record.permit {
                        if self.saved(&t)?.is_some()
                            || self.root.join(access::name(&t, "creation")).try_exists()?
                        {
                            return Ok(());
                        }
                    }
                    state::remove(&self.root, &key)?;
                }
            }
            return Ok(());
        }
        attempt::retire(&self.root, id, attempt_id, done, |t| {
            state::remove(&self.root, &access::name(t, "keep"))?;
            state::remove(&self.root, &access::name(t, "creation"))?;
            state::remove(&self.root, &access::name(t, "result"))
        })
    }
}

fn keep(
    journal: &Journal,
    service: Uuid,
    t: &w::AccessTaskSpec,
    mutate: bool,
) -> Result<w::AccessEffect, Error> {
    use crate::service::platform_sso::{binding_name, save_binding};
    let w::AccessAction::Unbind {
        account: w::AccessAccount::Macos { generated_uid },
        disposition: w::AccessAccountDisposition::Keep,
    } = &t.action
    else {
        return Ok(w::AccessEffect::Failed {
            reason: w::AccessFailure::Unsupported,
        });
    };
    let root = &journal.native;
    let _lock = state::lock(root)?;
    let key = binding_name(&generated_uid.to_string())?;
    if let Some(mut b) = state::read_optional::<platform_sso::LocalBinding>(root, &key)? {
        if b.authority != rss_mdm_agent_wire::platform_sso::RegistrationTrust::DaDevicePop
            || (b.installation, b.tenant, b.device_id, b.binding)
                != (service, t.tenant_id, t.device_id, t.binding_id)
            || b.binding_generation > t.binding_generation
            || b.revision > t.revision
        {
            return Ok(w::AccessEffect::Failed {
                reason: w::AccessFailure::AccountChanged,
            });
        }
        if b.enabled || b.revision != t.revision || b.binding_generation != t.binding_generation {
            let proof_key = access::name(t, "keep");
            let proof = state::read_optional::<(w::AccessTaskSpec, platform_sso::LocalBinding)>(
                &journal.root,
                &proof_key,
            )?;
            let authorized = mutate && seconds()? < t.expires_at;
            if let Some((original, mut prior)) = proof {
                prior.enabled = b.enabled;
                if original != *t || prior != b {
                    return Err(Error::Identity);
                }
            } else if authorized {
                state::write(&journal.root, &proof_key, &(t, &b))?;
            } else {
                return Ok(w::AccessEffect::Unknown);
            }
            if !authorized && b.enabled {
                return Ok(w::AccessEffect::Unknown);
            }
            b.enabled = false;
            b.binding_generation = t.binding_generation;
            b.revision = t.revision;
            let settlement = if authorized {
                save_binding(root, &b)
            } else {
                crate::service::platform_sso::finish_disabled_binding(root, &b)
            };
            match settlement {
                Ok(()) => (),
                Err(Error::Unavailable) => return Ok(w::AccessEffect::Unknown),
                Err(e) => return Err(e),
            }
        }
    }
    Ok(w::AccessEffect::Unbound {
        disposition: w::AccessAccountDisposition::Keep,
    })
}

fn verify_installation(d: &Deployment, native: &Path) -> Result<(), Error> {
    let actual = Deployment::metadata(&Deployment::default_path()?)?;
    actual.require_service()?;
    if unsafe { libc::geteuid() } != 0
        || actual.installation_id != d.installation_id
        || actual.da_root()? != d.da_root()?
        || access::root(&actual.execution) != native
        || actual.modules.da.backend()? != d.modules.da.backend()?
    {
        return Err(Error::Identity);
    }
    Ok(())
}

enum Step {
    Effect(rss_mdm_agent_wire::da::AccountResult),
    Created(macos::NativeAccount),
}

pub(crate) async fn drive(
    d: &Deployment,
    client: &da::Client<DeviceSecrets, SystemClock>,
    setup: &Arc<Setup>,
) -> Result<(), Error> {
    if !cfg!(target_arch = "aarch64") {
        return Ok(());
    }
    let ops = client.operations()?;
    let registration = ops.registration();
    let journal = Journal::new(d, registration.device)?;
    ops.replay_account().await?;
    ops.retire_account(|task, attempt, done| {
        journal.cleanup(task, attempt, done)?;
        setup.withdraw((task, attempt))
    })?;
    let input = setup.confirmed()?;
    let Some(offer) = ops
        .account(w::TaskPlatform::Macos, input.as_ref().map(|(t, _)| ids(t)))
        .await?
    else {
        return Ok(());
    };
    let id = (offer.task_id, offer.attempt_id);
    let key = attempt::key(id.0, id.1);
    let mut record: attempt::Attempt =
        state::read_optional(&journal.root, &key)?.unwrap_or_default();
    let request = record
        .renew_from
        .map_or(Event::Received, |expires_at| Event::NotStarted {
            expires_at,
        });
    let ack = match ops.account_event(&offer, request.clone()).await {
        Ok(ack) => ack,
        Err(e) => {
            if matches!(
                e,
                Error::Conflict
                    | Error::Denied
                    | Error::NotFound
                    | Error::Identity
                    | Error::CredentialRevoked
            ) {
                setup.withdraw(id)?;
            }
            return Err(e);
        }
    };
    let permit = ack.permit.as_ref();
    if let Some(t) = permit {
        validate(
            registration,
            &offer,
            &ack,
            t,
            record.permit.as_ref(),
            seconds()?,
        )?;
        record.permit = Some(t.clone());
        state::write(&journal.root, &key, &record)?;
    } else {
        setup.withdraw(id)?;
    }
    let retained = record.permit.clone();
    let task = retained.as_ref();
    if let Some(t) = task {
        if let Some(r) = journal.saved(t)? {
            ops.account_event(&offer, Event::Result(r)).await?;
            return Ok(());
        }
    }
    let evidence = match task {
        Some(t) => {
            let j = journal.clone();
            let t = t.clone();
            blocking(move || j.evidence(&t)).await?
        }
        None => Evidence::Unstarted,
    };
    let expires = ack
        .issued_expires_at
        .filter(|v| seconds().is_ok_and(|now| *v <= now));
    if matches!(evidence, Evidence::Unstarted) && permit.is_none() && !record.started {
        if let Some(expires_at) = expires {
            record.renew_from = Some(expires_at);
            record.started = false;
            state::write(&journal.root, &key, &record)?;
            ops.account_event(&offer, Event::NotStarted { expires_at })
                .await?;
        }
        return Ok(());
    }
    let Some(t) = task else {
        return Err(Error::Protocol);
    };
    let step = match evidence {
        Evidence::Created(a) => Step::Created(a),
        Evidence::Absent if expires.is_some() => {
            ops.account_event(
                &offer,
                Event::CreateAbsent {
                    expires_at: expires.ok_or(Error::Protocol)?,
                },
            )
            .await?;
            return Ok(());
        }
        Evidence::Absent | Evidence::Unknown => Step::Effect(result(t, w::AccessEffect::Unknown)),
        Evidence::Unstarted => {
            if let w::AccessAction::Create { name, .. } = &t.action {
                setup.offer(t)?;
                let Some((_, mut employee)) = input.filter(|(confirmed, _)| confirmed == t) else {
                    return Ok(());
                };
                employee.verify(t).await?;
                if ops
                    .account_event(&offer, request.clone())
                    .await?
                    .permit
                    .as_ref()
                    != Some(t)
                {
                    setup.withdraw(id)?;
                    return Ok(());
                }
                verify_installation(d, &journal.native)?;
                let j = journal.clone();
                let spec = t.clone();
                let account_name = name.clone();
                let setup = setup.clone();
                let actor = employee.clone();
                blocking(move || j.create(&spec, &account_name, &setup, &actor)).await?
            } else {
                // Only the explicit disposition permit can change this DA-owned binding.
                let may_start = permit.is_some();
                let spec = t.clone();
                let local = journal.clone();
                let service = registration.service_id;
                if may_start && !record.started {
                    record.started = true;
                    state::write(&journal.root, &key, &record)?;
                }
                Step::Effect(result(
                    t,
                    blocking(move || keep(&local, service, &spec, may_start)).await?,
                ))
            }
        }
    };
    setup.withdraw(id)?;
    let r = match step {
        Step::Effect(r) => r,
        Step::Created(a) => {
            let spec = t.clone();
            blocking(move || match macos::inspect(&a.generated_uid, false) {
                Ok(current) => created(&spec, current),
                Err(_) => Ok(result(&spec, w::AccessEffect::Unknown)),
            })
            .await?
        }
    };
    if !matches!(r.effect, AccountEffect::Access(w::AccessEffect::Unknown)) {
        journal.save(t, &r)?;
    }
    ops.account_event(&offer, Event::Result(r)).await?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn task() -> w::AccessTaskSpec {
        let now = (crate::wall().unwrap() / 1000) as i64;
        w::AccessTaskSpec {
            wire_version: 1,
            tenant_id: Uuid::new_v4(),
            device_id: Uuid::new_v4().to_string().parse().unwrap(),
            platform: w::TaskPlatform::Macos,
            architecture: w::TaskArchitecture::Aarch64,
            task_id: Uuid::new_v4(),
            attempt_id: Uuid::new_v4(),
            permit: w::TaskPermit::Start,
            expires_at: now + 300,
            deadline: now + 300,
            binding_id: Uuid::new_v4(),
            instance_id: Uuid::new_v4(),
            principal_id: Uuid::new_v4(),
            binding_generation: 1,
            factor_generation: 1,
            revision: 1,
            definition_digest: [5; 32],
            action: w::AccessAction::Create {
                name: "employee".into(),
                configuration: w::AccessConfiguration {
                    mac: Some(Default::default()),
                    windows: w::AccessWindowsConfiguration::Disabled,
                    self_enrollment: false,
                    enabled: true,
                    role: Some(w::AccessRole::Administrator),
                },
            },
            windows: None,
            authorization: w::TaskAuthorization::Policy {
                policy_id: Uuid::new_v4(),
                policy_version: Uuid::new_v4(),
            },
        }
    }

    #[test]
    fn keep_requires_the_completed_generation_even_after_the_original_marked_start() {
        let root = std::env::temp_dir().join(format!("rss-da-keep-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let journal = Journal {
            root: root.clone(),
            native: root.clone(),
        };
        let mut t = task();
        t.revision = 2;
        t.binding_generation = 2;
        let uid = Uuid::new_v4();
        let service = Uuid::new_v4();
        t.action = w::AccessAction::Unbind {
            account: w::AccessAccount::Macos { generated_uid: uid },
            disposition: w::AccessAccountDisposition::Keep,
        };
        let b = platform_sso::LocalBinding {
            authority: rss_mdm_agent_wire::platform_sso::RegistrationTrust::DaDevicePop,
            installation: service,
            tenant: t.tenant_id,
            device_id: t.device_id,
            binding: t.binding_id,
            binding_generation: 1,
            revision: 1,
            generated_uid: uid.to_string(),
            enabled: false,
            configuration: Default::default(),
        };
        let key = crate::service::platform_sso::binding_name(&uid.to_string()).unwrap();
        state::write(&root, &key, &b).unwrap();
        assert_eq!(
            keep(&journal, service, &t, false).unwrap(),
            w::AccessEffect::Unknown
        );
        let mut prior = b.clone();
        prior.enabled = true;
        state::write(&root, &access::name(&t, "keep"), &(&t, &prior)).unwrap();
        assert!(matches!(
            keep(&journal, service, &t, false).unwrap(),
            w::AccessEffect::Unbound { .. }
        ));
        assert!(matches!(
            keep(&journal, service, &t, true).unwrap(),
            w::AccessEffect::Unbound { .. }
        ));
        assert!(matches!(
            keep(&journal, service, &t, false).unwrap(),
            w::AccessEffect::Unbound { .. }
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn renewed_start_changes_only_deadlines_and_remains_bound_to_the_original_device() {
        let t = task();
        let offer = AccountOffer {
            task_id: t.task_id,
            attempt_id: t.attempt_id,
            binding_id: t.binding_id,
            definition_digest: t.definition_digest,
            operation_id: Uuid::new_v4(),
        };
        let mut registration = rss_mdm_agent_wire::da::Registration {
            service_id: Uuid::new_v4(),
            tenant: t.tenant_id,
            device: t.device_id,
            credential: Uuid::new_v4(),
            issuance_id: Uuid::new_v4(),
            operation_id: Uuid::new_v4(),
        };
        let ack = AccountSettlement {
            event_id: Uuid::new_v4(),
            issued_expires_at: Some(t.expires_at),
            permit: Some(t.clone()),
            receipt: None,
        };
        let now = seconds().unwrap();
        assert!(validate(&registration, &offer, &ack, &t, None, now).is_ok());
        for update in [0, 1, 2, 3, 4] {
            let mut old = t.clone();
            match update {
                0 => old.binding_generation += 1,
                1 => old.principal_id = Uuid::new_v4(),
                2 => old.revision += 1,
                3 => old.attempt_id = Uuid::new_v4(),
                _ => old.definition_digest = [8; 32],
            }
            assert_eq!(
                validate(&registration, &offer, &ack, &t, Some(&old), now),
                Err(Error::Conflict)
            );
        }
        assert_eq!(
            validate(&registration, &offer, &ack, &t, None, t.expires_at),
            Err(Error::Untrusted)
        );
        registration.tenant = Uuid::new_v4();
        assert_eq!(
            validate(&registration, &offer, &ack, &t, None, now),
            Err(Error::Untrusted)
        );
    }
}

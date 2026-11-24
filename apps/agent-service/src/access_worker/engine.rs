use super::*;
use device_access::{Clock, Protector, Store};
use execution_ipc::device_access::WindowsComponentState as Component;
use w::{
    AccessAccount as Account, AccessAction as Action, AccessEffect as Effect,
    AccessFailure as Failure, AccessWindowsPlan as Plan,
};
use zeroize::Zeroizing;

pub(super) trait Native: Protector {
    fn hello_prepare(
        &self,
        record: &device_access::hello::HelloRecord,
    ) -> Result<Option<bool>, w::AccessFailure>;
    fn hello(
        &mut self,
        record: &device_access::hello::HelloRecord,
        mutate: bool,
    ) -> Result<(w::AccessWhfbFacts, bool), w::AccessFailure>;
    fn disposition(
        &self,
        account: &w::AccessAccount,
        disposition: w::AccessAccountDisposition,
    ) -> Result<bool, w::AccessFailure>;
    fn identity(&self, sid: &str) -> Result<(w::AccessWindowsAuthority, String, String), Failure>;
    fn dispose(
        &mut self,
        account: &w::AccessAccount,
        disposition: w::AccessAccountDisposition,
    ) -> Result<(), Failure>;
    fn integration_ready(&self, root: &Path, installation: uuid::Uuid) -> Result<Component, Error>;
    fn clock(&self) -> Result<Clock, Error>;
    fn password(&mut self) -> Result<Zeroizing<String>, Error>;
    fn create(&mut self, name: &str, password: &str) -> Result<String, Failure>;
    fn inspect(&self, sid: &str) -> Result<w::AccessRole, Failure>;
    fn role(&mut self, sid: &str, role: w::AccessRole) -> Result<(), Failure>;
    fn enable(&mut self, sid: &str) -> Result<(), Failure>;
}
#[derive(Serialize, Deserialize)]
pub(super) struct Creation {
    pub(super) task: w::AccessTaskSpec,
    pub(super) sid: Option<String>,
}
fn storage(_: device_access::Error) -> Error {
    Error::Storage
}
pub(super) fn execute(input: &Input, native: &mut impl Native) -> Result<w::AccessResult, Error> {
    let lock = state::lock(&input.root)?;
    execute_locked(input, native, &lock)
}
pub(super) fn execute_locked(
    input: &Input,
    native: &mut impl Native,
    _lock: &std::fs::File,
) -> Result<w::AccessResult, Error> {
    let task = &input.task;
    task.validate()?;
    if let Some(result) = stored_result(&input.root, task)? {
        if complete(task, &result.effect) || matches!(result.effect, Effect::Failed { .. }) {
            return Ok(result);
        }
    }
    let outcome = apply(input, native);
    let result = match outcome {
        Ok(effect) => report(task, effect),
        Err(Error::Expired) if input.channel == device_access::Channel::Da => {
            return Err(Error::Expired)
        }
        Err(_) => report(task, Effect::Unknown),
    };
    // An uncertain account or escrow operation remains recoverable; never freeze Unknown as Applied.
    if !matches!(result.effect, Effect::Unknown) {
        state::write(&input.root, &name(task, "result"), &result)?;
    }
    Ok(result)
}
fn require_window(input: &Input) -> Result<(), Error> {
    if input.channel == device_access::Channel::Da {
        let now = (crate::wall()? / 1000) as i64;
        if input.task.expires_at <= now || input.task.deadline <= now {
            return Err(Error::Expired);
        }
    }
    Ok(())
}
/// The exclusive worker lock spans this publication and the following native call.
pub(super) fn publish_creation(
    input: &Input,
    marker: &str,
    check: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    state::write(
        &input.root,
        marker,
        &Creation {
            task: input.task.clone(),
            sid: None,
        },
    )?;
    if let Err(error) = check() {
        // We are still on the pre-dispatch side. Removing the intent is a durable
        // proof for renewal. A process interruption leaves the normal uncertain intent.
        state::remove(&input.root, marker)?;
        return Err(error);
    }
    Ok(())
}
fn apply(input: &Input, native: &mut impl Native) -> Result<Effect, Error> {
    let task = &input.task;
    let mut store = Store::open(&input.root, input.installation).map_err(storage)?;
    if matches!(task.windows, Some(Plan::Whfb { .. })) {
        return super::hello::apply(input, native, &mut store);
    }
    let enabled = match &task.action {
        Action::Apply { configuration, .. } | Action::Create { configuration, .. } => {
            configuration.enabled
        }
        _ => false,
    };
    // Even disabling and unbinding must target the storage consumed by this installation.
    let ready = match native.integration_ready(&input.root, input.installation) {
        Ok(ready) => ready,
        Err(Error::Identity) => {
            return Ok(Effect::Failed {
                reason: Failure::AccountChanged,
            })
        }
        Err(_) => {
            return Ok(Effect::Failed {
                reason: Failure::NativeFailure,
            })
        }
    };
    if enabled && matches!(ready, Component::RebootRequired | Component::Unavailable) {
        return Ok(Effect::Unknown);
    }
    if enabled && component_health(ready).is_err() {
        return Ok(Effect::Failed {
            reason: Failure::Unsupported,
        });
    }
    if let Action::Unbind {
        disposition,
        account: account @ Account::Windows { .. },
    } = &task.action
    {
        require_window(input)?;
        store.revoke_task(input.channel, task).map_err(storage)?;
        require_window(input)?;
        return Ok(match native.dispose(account, *disposition) {
            Ok(()) => Effect::Unbound {
                disposition: *disposition,
            },
            Err(Failure::NativeFailure) => Effect::Unknown,
            Err(reason) => Effect::Failed { reason },
        });
    }
    store.admit_task(input.channel, task).map_err(storage)?;
    // Require already confirmed setup before touching an OS account.
    if matches!(task.windows, Some(Plan::Initialize { .. })) {
        store
            .candidate(
                device_access::CandidateKey::for_task(input.channel, task),
                native,
            )
            .map_err(storage)?;
    }
    let (sid, configuration, created) = match &task.action {
        Action::Apply {
            account: Account::Windows { sid, .. },
            configuration,
        } => (sid.clone(), configuration, false),
        Action::Create {
            name: account_name,
            configuration,
        } => {
            let marker = name(task, "creation");
            let sid = match state::read_optional::<Creation>(&input.root, &marker)? {
                Some(previous) => {
                    if previous.task != *task {
                        return Err(Error::Untrusted);
                    }
                    // Never adopt a name after crash between NetUserAdd and durable SID recording.
                    previous.sid.ok_or(Error::Conflict)?
                }
                None => {
                    require_window(input)?;
                    let password = native.password()?;
                    publish_creation(input, &marker, || require_window(input))?;
                    let sid = match native.create(account_name, &password) {
                        Ok(sid) => sid,
                        Err(Failure::NativeFailure) => return Err(Error::Unavailable),
                        Err(reason) => return Ok(Effect::Failed { reason }),
                    };
                    state::write(
                        &input.root,
                        &marker,
                        &Creation {
                            task: task.clone(),
                            sid: Some(sid.clone()),
                        },
                    )?;
                    sid
                }
            };
            (sid, configuration, true)
        }
        _ => return Err(Error::Unsupported),
    };
    let account = Account::Windows {
        sid: sid.clone(),
        authority: native.identity(&sid).map_err(|_| Error::Identity)?.0,
    };
    account.validate()?;
    state::write(
        &input.root,
        &name(task, "prepared"),
        &w::AccessPrepared {
            binding_id: task.binding_id,
            binding_generation: task.binding_generation,
            revision: task.revision,
            definition_digest: task.definition_digest,
            account: account.clone(),
        },
    )?;
    let facts = match task.windows.as_ref().ok_or(Error::Protocol)? {
        Plan::Whfb { .. } => return Err(Error::Protocol),
        Plan::Preserve { facts } => {
            require_window(input)?;
            store
                .configure(
                    &authority(input.channel, task, &sid, native)?,
                    task.revision,
                    configuration.enabled,
                    facts,
                )
                .map_err(storage)?;
            facts.clone()
        }
        Plan::Initialize {
            recipient,
            expected_revision,
        } => {
            let candidate = store
                .candidate(
                    device_access::CandidateKey::for_task(input.channel, task),
                    native,
                )
                .map_err(storage)?;
            let subject = w::CredentialSubject::RssLogin {
                binding: task.binding_id,
                binding_generation: task.binding_generation,
                sid: sid.clone(),
            };
            let escrow = match pending(&input.root, task)? {
                Some(escrow) => escrow,
                None => {
                    let envelope = crate::credential_worker::envelope::encrypt_material(
                        recipient,
                        &subject,
                        *expected_revision,
                        task.material_aad(&subject, *expected_revision)?,
                        candidate.recovery_secret(),
                    )?;
                    let escrow = w::CredentialEscrow {
                        definition_digest: task.definition_digest,
                        materials: vec![envelope],
                    };
                    state::write(&input.root, &name(task, "escrow"), &escrow)?;
                    escrow
                }
            };
            escrow.validate()?;
            if escrow.definition_digest != task.definition_digest
                || escrow.materials.len() != 1
                || escrow.materials[0].subject != subject
                || escrow.materials[0].expected_revision != *expected_revision
            {
                return Err(Error::Untrusted);
            }
            use sha2::{Digest, Sha256};
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&escrow).map_err(|_| Error::Protocol)?)
            );
            let receipt = loop {
                if let Some(ack) = state::read_optional::<Ack>(&input.root, &name(task, "ack"))? {
                    if ack.digest != digest
                        || ack.receipts.len() != 1
                        || ack.receipts[0].subject != subject
                    {
                        return Err(Error::Untrusted);
                    }
                    ack.receipts[0].validate()?;
                    break ack.receipts[0].clone();
                }
                // The independent DA owner settles escrow between bounded native steps.
                if input.channel == device_access::Channel::Da {
                    return Err(Error::Unavailable);
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            };
            let facts = w::AccessRssFacts {
                factor_generation: task.factor_generation,
                recovery: receipt,
            };
            require_window(input)?;
            store
                .activate(
                    authority(input.channel, task, &sid, native)?,
                    task.revision,
                    configuration.enabled,
                    device_access::CandidateKey::for_task(input.channel, task),
                    facts.clone(),
                    native.clock()?,
                    native,
                )
                .map_err(storage)?;
            facts
        }
    };
    if let Some(role) = configuration.role.or(if created {
        Some(w::AccessRole::Standard)
    } else {
        None
    }) {
        require_window(input)?;
        if native.role(&sid, role).is_err() {
            return Ok(Effect::Unknown);
        }
    }
    require_window(input)?;
    if created && native.enable(&sid).is_err() {
        return Ok(Effect::Unknown);
    }
    let role = native.inspect(&sid).map_err(|_| Error::Unavailable)?;
    if configuration.role.is_some_and(|expected| expected != role)
        || store
            .facts(&authority(input.channel, task, &sid, native)?)
            .map_err(storage)?
            != facts
    {
        return Ok(Effect::Unknown);
    }
    Ok(Effect::Applied {
        account,
        role,
        mac: None,
        windows: Some(w::AccessWindowsFacts::RssFactor(facts)),
    })
}

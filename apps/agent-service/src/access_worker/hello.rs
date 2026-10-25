//! Native Hello task application shares the existing worker, store and result transport.
use super::*;
use device_access::hello::HelloRecord;
use w::{AccessAction as Action, AccessEffect as Effect, AccessWindowsFacts as Facts};

pub(super) fn apply(
    input: &Input,
    native: &mut impl engine::Native,
    store: &mut device_access::Store,
) -> Result<Effect, Error> {
    let task = &input.task;
    let account = match &task.action {
        Action::Apply { account, .. } | Action::Unbind { account, .. } => account.clone(),
        _ => return Err(Error::Protocol),
    };
    let w::AccessAccount::Windows { sid, .. } = &account else {
        return Err(Error::Protocol);
    };
    let record = store
        .prepare_hello(task, authority(task, sid, native)?)
        .map_err(|_| Error::Identity)?;
    let before = match native.hello_prepare(&record) {
        Ok(value) => value,
        Err(reason) => return Ok(Effect::Failed { reason }),
    };
    let record = store
        .begin_hello_policy(&record, before)
        .map_err(|_| Error::Storage)?;
    if native.clock()?.utc >= task.deadline {
        return Ok(Effect::Unknown);
    }
    let (facts, owned) = match native.hello(&record, true) {
        Ok(facts) => facts,
        // The intent is durable: a failed call may already have changed native state.
        Err(_) => return Ok(Effect::Unknown),
    };
    let facts = store
        .observe_hello(&record, facts, owned)
        .map_err(|_| Error::Storage)?;
    match &task.action {
        Action::Unbind { disposition, .. } => {
            native
                .dispose(&account, *disposition)
                .map_err(|_| Error::Unavailable)?;
            ()
        }
        Action::Apply { configuration, .. } => {
            if let Some(role) = configuration.role {
                native.role(sid, role).map_err(|_| Error::Unavailable)?;
            }
            ()
        }
        _ => return Err(Error::Protocol),
    };
    effect(task, &record, facts, native)
}
pub(super) fn effect(
    task: &w::AccessTaskSpec,
    record: &HelloRecord,
    facts: w::AccessWhfbFacts,
    native: &impl engine::Native,
) -> Result<Effect, Error> {
    match &task.action {
        Action::Apply {
            account,
            configuration,
        } => {
            let role = native
                .inspect(&record.authority.sid)
                .map_err(|_| Error::Unavailable)?;
            if configuration.role.is_some_and(|expected| expected != role) {
                return Ok(Effect::Unknown);
            }
            Ok(Effect::Applied {
                account: account.clone(),
                role,
                mac: None,
                windows: Some(Facts::Whfb(facts)),
            })
        }
        Action::Unbind {
            account,
            disposition,
        } if native
            .disposition(account, *disposition)
            .map_err(|_| Error::Unavailable)? =>
        {
            Ok(Effect::WhfbRetired {
                account: account.clone(),
                disposition: *disposition,
                facts,
            })
        }
        _ => Ok(Effect::Unknown),
    }
}
/// Observation refresh never reapplies policy, changes roles, or retries a destructive operation.
#[cfg(windows)]
pub(crate) fn refresh(
    root: &Path,
    installation: uuid::Uuid,
    task: &w::AccessTaskSpec,
) -> Result<(), Error> {
    if !matches!(task.windows, Some(w::AccessWindowsPlan::Whfb { .. })) {
        return Ok(());
    }
    let _lock = state::lock(root)?;
    if stored_result(root, task)?
        .is_some_and(|r| complete(task, &r.effect) || matches!(r.effect, Effect::Failed { .. }))
    {
        return Ok(());
    }
    let mut native = super::windows::NativeWindows {
        root: root.into(),
        installation,
    };
    let mut store = device_access::Store::open(root, installation).map_err(|_| Error::Storage)?;
    let account = match &task.action {
        Action::Apply { account, .. } | Action::Unbind { account, .. } => account,
        _ => return Err(Error::Protocol),
    };
    let w::AccessAccount::Windows { sid, .. } = account else {
        return Err(Error::Protocol);
    };
    let Some(record) = store.hello(sid).map_err(|_| Error::Storage)? else {
        return Ok(());
    };
    if record.revision != task.revision
        || record.definition_digest != task.definition_digest
        || record.authority.binding != task.binding_id
    {
        return Ok(());
    }
    // Recheck identity even after the original worker has exited.
    if authority(task, sid, &native)? != record.authority {
        return Err(Error::Identity);
    }
    let (facts, owned) =
        engine::Native::hello(&mut native, &record, false).map_err(|_| Error::Unavailable)?;
    let facts = store
        .observe_hello(&record, facts, owned)
        .map_err(|_| Error::Storage)?;
    let effect = effect(task, &record, facts, &native)?;
    state::write(root, &name(task, "result"), &report(task, effect))
}

/// Accepted unmanage keeps observations running without policy or destructive retries.
#[cfg(windows)]
pub(crate) fn drain(root: &Path, installation: uuid::Uuid) -> Result<bool, Error> {
    if !root.exists() {
        return Ok(true);
    }
    let _lock = state::lock(root)?;
    let mut store = device_access::Store::open(root, installation).map_err(|_| Error::Storage)?;
    let mut native = super::windows::NativeWindows {
        root: root.into(),
        installation,
    };
    for record in store.hello_records().map_err(|_| Error::Storage)? {
        if !record.retired {
            continue;
        }
        if let Ok((facts, owned)) = engine::Native::hello(&mut native, &record, false) {
            store
                .observe_hello(&record, facts, owned)
                .map_err(|_| Error::Storage)?;
        }
    }
    store.hello_cleanup_complete().map_err(|_| Error::Storage)
}

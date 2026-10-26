use super::*;
use state::{Phase, Record};
use w::{CredentialFailure as Failure, CredentialOperation as Op, CredentialSubject as Subject};
fn native<T>(value: Result<T, Failure>) -> Result<T, Error> {
    value.map_err(|_| Error::Unavailable)
}
// An issued account candidate has a stable SID before authentication. Escrow it even
// when local logon policy prevents verification; only locate after ACK proves the effect.
// BitLocker must still discover the newly created protector before it can name material.
fn candidate(n: &mut impl Native, op: &Op, secret: &str) -> Result<Option<Subject>, Error> {
    match op {
        Op::RotateLocalAdministrator { sid } => {
            Ok(Some(Subject::LocalAdministrator { sid: sid.clone() }))
        }
        _ => native(n.locate(op, secret)),
    }
}
fn save(input: &Input, record: &Record) -> Result<(), Error> {
    state::write(&input.root, &state::name(&record.task, "state"), record)
}
fn bound(a: &w::CredentialTaskSpec, b: &w::CredentialTaskSpec) -> bool {
    a.tenant_id == b.tenant_id
        && a.device_id == b.device_id
        && a.registration_id == b.registration_id
        && a.generation == b.generation
}
fn account_key(task: &w::CredentialTaskSpec, sid: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{}-{}-{:x}.account",
        task.registration_id,
        task.generation,
        Sha256::digest(sid)
    )
}
pub(super) fn execute(
    input: &Input,
    native_os: &mut impl Native,
) -> Result<w::CredentialResult, Error> {
    let _lock = state::lock(&input.root)?; // also proves a referenced original worker has stopped
    let task = &input.task;
    task.validate()?;
    if let Some(result) =
        state::read_optional::<w::CredentialResult>(&input.root, &state::name(task, "result"))?
    {
        if result.definition_digest != task.definition_digest {
            return Err(Error::Untrusted);
        }
        return Ok(result);
    }
    let outcome = execute_inner(input, native_os).and_then(|mut result| {
        if let Some(escrow) = pending(&input.root, task)? {
            for material in escrow.materials {
                if !result
                    .items
                    .iter()
                    .any(|item| item.subject == material.subject)
                {
                    result.items.push(w::CredentialItemResult {
                        subject: material.subject,
                        outcome: w::CredentialOutcome::Unknown,
                    });
                    result.complete = false;
                    result.failure = Some(Failure::NativeFailure);
                }
            }
        }
        Ok(result)
    });
    // Only complete evidence is frozen. Unexpected failure retains the protected original state.
    if let Ok(result) = &outcome {
        state::write(&input.root, &state::name(task, "result"), result)?;
    }
    outcome
}
fn execute_inner(input: &Input, n: &mut impl Native) -> Result<w::CredentialResult, Error> {
    let task = &input.task;
    if matches!(
        task.operation,
        Op::RotateRssLogin { .. } | Op::VerifyRssLogin { .. }
    ) {
        #[cfg(windows)]
        {
            return super::rss::execute_system(input, n);
        }
        #[cfg(not(windows))]
        {
            return Err(Error::Unsupported);
        }
    }
    let mut recovery = None;
    let mut record: Option<Record> = None;
    let mut failure = None;
    let materials = if let Some(reference) = &task.recovery {
        let mut original = task.clone();
        original.task_id = reference.task_id;
        original.attempt_id = reference.attempt_id;
        let Some(old) =
            state::read_optional::<Record>(&input.root, &state::name(&original, "state"))?
        else {
            return Ok(failed(task, Failure::RecoveryState));
        };
        let matches = match (&task.operation, &old.task.operation) {
            (
                Op::VerifyBitlocker {
                    volume: a,
                    protector: b,
                },
                Op::RotateBitlocker {
                    volume: c,
                    protector: d,
                },
            ) => a == c && b == d,
            (Op::VerifyLocalAdministrator { sid: a }, Op::RotateLocalAdministrator { sid: b }) => {
                a == b
            }
            _ => false,
        };
        if !bound(task, &old.task) || !matches {
            return Err(Error::Untrusted);
        }
        if old.phase == Phase::Prepared {
            return Ok(w::CredentialResult {
                definition_digest: task.definition_digest,
                recovery: Some(w::CredentialRecoveryOutcome::NotAttempted),
                failure: None,
                complete: true,
                items: vec![],
            });
        }
        let secret = state::secret(n, &old)?;
        let Some(subject) = candidate(n, &old.task.operation, &secret)? else {
            return Ok(failed(task, Failure::RecoveryState));
        };
        recovery = Some(w::CredentialRecoveryOutcome::Applied);
        record = Some(old);
        vec![Material { subject, secret }]
    } else {
        match &task.operation {
            Op::RotateRssLogin { .. } | Op::VerifyRssLogin { .. } => {
                return Err(Error::Unsupported)
            }
            Op::CollectBitlocker => {
                let inventory = match n.collect() {
                    Ok(value) => value,
                    Err(error) => return Ok(failed(task, error)),
                };
                failure = inventory.failure;
                inventory.materials
            }
            Op::VerifyBitlocker { volume, protector } => match n.bitlocker(volume, *protector) {
                Ok(value) => vec![value],
                Err(error) => return Ok(failed(task, error)),
            },
            Op::VerifyLocalAdministrator { sid } => {
                let Some(old) =
                    state::read_optional::<Record>(&input.root, &account_key(task, sid))?
                else {
                    return Ok(failed(task, Failure::RecoveryState));
                };
                if !bound(task, &old.task)
                    || old.subject.as_ref()
                        != Some(&Subject::LocalAdministrator { sid: sid.clone() })
                {
                    return Err(Error::Untrusted);
                }
                let secret = state::secret(n, &old)?;
                let Some(subject) = native(n.locate(&task.operation, &secret))? else {
                    return Ok(failed(task, Failure::NativeFailure));
                };
                vec![Material { subject, secret }]
            }
            Op::RotateBitlocker { .. } | Op::RotateLocalAdministrator { .. } => {
                let old = state::read_optional::<Record>(&input.root, &state::name(task, "state"))?;
                let mut current = match old {
                    Some(old) => {
                        if old.task != *task {
                            return Err(Error::Untrusted);
                        }
                        old
                    }
                    None => {
                        // Validate the selected old protector before allocating mutation state.
                        if let Op::RotateBitlocker { volume, protector } = &task.operation {
                            if let Err(error) = n.bitlocker(volume, *protector) {
                                return Ok(failed(task, error));
                            }
                        }
                        let secret = envelope::password(matches!(
                            task.operation,
                            Op::RotateBitlocker { .. }
                        ))?;
                        let current = Record {
                            task: task.clone(),
                            phase: Phase::Prepared,
                            protected: state::seal(n, task, &secret)?,
                            subject: None,
                        };
                        save(input, &current)?;
                        current
                    }
                };
                let secret = state::secret(n, &current)?;
                let mut rejected = None;
                if current.phase == Phase::Prepared {
                    current.phase = Phase::Issued;
                    save(input, &current)?;
                    // Once Issued is durable, even an ambiguous native failure can only be observed.
                    rejected = n.apply(&task.operation, &secret).err();
                    if let Some(
                        failure @ (Failure::PermissionDenied
                        | Failure::OwnerConflict
                        | Failure::TargetMissing
                        | Failure::Unsupported),
                    ) = rejected
                    {
                        return Ok(failed(task, failure));
                    }
                }
                let Some(subject) = candidate(n, &task.operation, &secret)? else {
                    let failure = match rejected {
                        Some(
                            value @ (Failure::PermissionDenied
                            | Failure::OwnerConflict
                            | Failure::TargetMissing
                            | Failure::Unsupported),
                        ) => value,
                        _ => Failure::NativeFailure,
                    };
                    return Ok(failed(task, failure));
                };
                current.subject = Some(subject.clone());
                save(input, &current)?;
                record = Some(current);
                vec![Material { subject, secret }]
            }
        }
    };
    if materials.is_empty() {
        return Ok(failed(task, failure.unwrap_or(Failure::NoMaterials)));
    }
    if materials.len() > 512 {
        return Err(Error::Capacity);
    }
    let escrow_name = state::name(task, "escrow");
    let escrow = match state::read_optional::<w::CredentialEscrow>(&input.root, &escrow_name)? {
        Some(value) => {
            if value.definition_digest != task.definition_digest
                || value
                    .materials
                    .iter()
                    .map(|v| &v.subject)
                    .collect::<Vec<_>>()
                    != materials.iter().map(|m| &m.subject).collect::<Vec<_>>()
            {
                return Err(Error::Conflict);
            }
            value
        }
        None => {
            let value = w::CredentialEscrow {
                definition_digest: task.definition_digest,
                materials: materials
                    .iter()
                    .map(|m| envelope::encrypt(task, m))
                    .collect::<Result<_, _>>()?,
            };
            value.validate()?;
            state::write(&input.root, &escrow_name, &value)?;
            value
        }
    };
    let digest = envelope::digest(&escrow)?;
    loop {
        if state::read_optional::<String>(&input.root, &state::name(task, "ack"))?.as_ref()
            == Some(&digest)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if let Some(current) = &mut record {
        let secret = state::secret(n, current)?;
        let actual = native(n.locate(&current.task.operation, &secret))?;
        if actual.as_ref() != Some(&materials[0].subject) {
            return failure_with_materials(&input.root, task, Failure::NativeFailure);
        }
        if let Op::RotateBitlocker { volume, protector } = &current.task.operation {
            if actual
                == Some(Subject::Bitlocker {
                    volume: volume.clone(),
                    protector: *protector,
                })
            {
                return Err(Error::Untrusted);
            }
            native(n.remove(volume, *protector))?;
        }
        current.phase = Phase::Finished;
        current.subject = actual;
        save(input, current)?;
        if let Op::RotateLocalAdministrator { sid } = &current.task.operation {
            let key = account_key(task, sid);
            let previous = state::read_optional::<Record>(&input.root, &key)?;
            state::write(&input.root, &key, current)?;
            if let Some(mut previous) = previous.filter(|p| p.task.task_id != current.task.task_id)
            {
                previous.protected.clear();
                state::remove(&input.root, &state::name(&previous.task, "state"))?;
            }
        }
    }
    Ok(w::CredentialResult {
        definition_digest: task.definition_digest,
        recovery,
        failure,
        complete: failure.is_none(),
        items: materials
            .into_iter()
            .map(|m| w::CredentialItemResult {
                subject: m.subject,
                outcome: w::CredentialOutcome::Verified,
            })
            .collect(),
    })
}

pub(super) fn result(
    input: &Input,
    native_os: &mut impl Native,
) -> Result<w::CredentialResult, Error> {
    match execute(input, native_os) {
        Ok(result) => Ok(result),
        Err(error) => failure_with_materials(
            &input.root,
            &input.task,
            if error == Error::Unavailable {
                Failure::NativeFailure
            } else {
                Failure::RecoveryState
            },
        ),
    }
}

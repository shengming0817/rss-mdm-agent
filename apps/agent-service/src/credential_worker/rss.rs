//! RSS recovery rotation uses the existing credential task escrow barrier and offline factor owner.
use super::*;
use state::{Phase, Record};
use w::{CredentialFailure as Failure, CredentialOperation as Op, CredentialSubject as Subject};
struct Protection<'a, N>(&'a N);
impl<N: Native> device_access::Protector for Protection<'_, N> {
    fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>, device_access::Error> {
        self.0
            .protect(bytes)
            .map_err(|_| device_access::Error::Storage)
    }
    fn open(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, device_access::Error> {
        self.0
            .unprotect(bytes)
            .map_err(|_| device_access::Error::Storage)
    }
}
#[cfg(windows)]
pub(super) fn execute_system(
    input: &Input,
    native: &impl Native,
) -> Result<w::CredentialResult, Error> {
    let config = device_access::windows::installation().map_err(|_| Error::Storage)?;
    if input.root.parent().map(|p| p.join("device-access")) != Some(config.root.clone()) {
        return Err(Error::Untrusted);
    }
    let mut store =
        device_access::Store::open(&config.root, config.id).map_err(|_| Error::Storage)?;
    execute(input, native, &mut store)
}
pub(super) fn execute(
    input: &Input,
    native: &impl Native,
    store: &mut device_access::Store,
) -> Result<w::CredentialResult, Error> {
    let task = &input.task;
    let (binding, binding_generation, sid) = match &task.operation {
        Op::RotateRssLogin {
            binding,
            binding_generation,
            sid,
        }
        | Op::VerifyRssLogin {
            binding,
            binding_generation,
            sid,
        } => (*binding, *binding_generation, sid.clone()),
        _ => return Err(Error::Unsupported),
    };
    let subject = Subject::RssLogin {
        binding,
        binding_generation,
        sid,
    };
    let protection = Protection(native);
    let (mut record, secret, recovery) = if let Some(reference) = &task.recovery {
        let mut original = task.clone();
        original.task_id = reference.task_id;
        original.attempt_id = reference.attempt_id;
        let record: Record = state::read_optional(&input.root, &state::name(&original, "state"))?
            .ok_or(Error::Storage)?;
        if record.task.tenant_id != task.tenant_id
            || record.task.device_id != task.device_id
            || record.task.registration_id != task.registration_id
            || record.task.generation != task.generation
            || record.subject.as_ref() != Some(&subject)
            || !matches!(record.task.operation, Op::RotateRssLogin { .. })
        {
            return Err(Error::Untrusted);
        }
        if record.phase == Phase::Prepared {
            return Ok(w::CredentialResult {
                definition_digest: task.definition_digest,
                recovery: Some(w::CredentialRecoveryOutcome::NotAttempted),
                failure: None,
                complete: true,
                items: vec![],
            });
        }
        let secret = state::secret(native, &record)?;
        let receipts: Vec<w::CredentialReceipt> =
            state::read_optional(&input.root, &state::name(&record.task, "receipts"))?
                .ok_or(Error::Storage)?;
        if receipts.len() != 1
            || !store
                .verify_recovery_material(task, &subject, &secret, &receipts[0])
                .map_err(|_| Error::Storage)?
        {
            return failure_with_materials(&input.root, task, Failure::RecoveryState);
        }
        (record, secret, Some(w::CredentialRecoveryOutcome::Applied))
    } else if matches!(task.operation, Op::VerifyRssLogin { .. }) {
        let (secret, _) = store
            .recovery_material(task, &subject, &protection)
            .map_err(|_| Error::Storage)?;
        let record = Record {
            task: task.clone(),
            phase: Phase::Finished,
            protected: state::seal(native, task, &secret)?,
            subject: Some(subject.clone()),
        };
        (record, secret, None)
    } else {
        let current = state::read_optional::<Record>(&input.root, &state::name(task, "state"))?;
        let record = if let Some(current) = current {
            if current.task != *task || current.subject.as_ref() != Some(&subject) {
                return Err(Error::Untrusted);
            }
            current
        } else {
            // Confirm the exact association still exists before allocating a new candidate.
            store
                .recovery_material(task, &subject, &protection)
                .map_err(|_| Error::Storage)?;
            use base64::Engine;
            let random = Zeroizing::new(platform_credentials::random_bytes(32)?);
            let secret =
                Zeroizing::new(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&*random));
            let current = Record {
                task: task.clone(),
                phase: Phase::Prepared,
                protected: state::seal(native, task, &secret)?,
                subject: Some(subject.clone()),
            };
            state::write(&input.root, &state::name(task, "state"), &current)?;
            current
        };
        let secret = state::secret(native, &record)?;
        (record, secret, None)
    };
    let material = Material {
        subject: subject.clone(),
        secret,
    };
    let escrow = match pending(&input.root, task)? {
        Some(value) => value,
        None => {
            let value = w::CredentialEscrow {
                definition_digest: task.definition_digest,
                materials: vec![envelope::encrypt(task, &material)?],
            };
            state::write(&input.root, &state::name(task, "escrow"), &value)?;
            value
        }
    };
    if escrow.definition_digest != task.definition_digest
        || escrow.materials.len() != 1
        || escrow.materials[0].subject != subject
    {
        return Err(Error::Untrusted);
    }
    let digest = envelope::digest(&escrow)?;
    loop {
        if state::read_optional::<String>(&input.root, &state::name(task, "ack"))?.as_ref()
            == Some(&digest)
        {
            break;
        }
        if crate::wall()? / 1000 >= task.deadline as u64 {
            return Err(Error::Expired);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let receipts: Vec<w::CredentialReceipt> =
        state::read_optional(&input.root, &state::name(task, "receipts"))?.ok_or(Error::Storage)?;
    if receipts.len() != 1 || receipts[0].subject != subject {
        return Err(Error::Untrusted);
    }
    if matches!(task.operation, Op::RotateRssLogin { .. }) {
        record.phase = Phase::Issued;
        state::write(&input.root, &state::name(task, "state"), &record)?;
        store
            .rotate_recovery(
                task,
                &subject,
                &material.secret,
                receipts[0].clone(),
                &protection,
            )
            .map_err(|_| Error::Storage)?;
    }
    if !store
        .verify_recovery_material(task, &subject, &material.secret, &receipts[0])
        .map_err(|_| Error::Storage)?
    {
        return failure_with_materials(&input.root, task, Failure::RecoveryState);
    }
    record.phase = Phase::Finished;
    state::write(&input.root, &state::name(&record.task, "state"), &record)?;
    Ok(w::CredentialResult {
        definition_digest: task.definition_digest,
        recovery,
        failure: None,
        complete: true,
        items: vec![w::CredentialItemResult {
            subject,
            outcome: w::CredentialOutcome::Verified,
        }],
    })
}

//! Protected start evidence and exact terminal cleanup shared by the DA delivery worker.
use super::*;
use crate::credential_worker::state;
use rss_mdm_agent_wire::runtime as w;

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Attempt {
    pub renew_from: Option<i64>,
    pub started: bool,
    pub permit: Option<w::AccessTaskSpec>,
}
pub(super) fn key(task: Uuid, attempt: Uuid) -> String {
    format!("{task}.{attempt}.da-attempt")
}
pub(super) fn retire(
    root: &Path,
    id: Uuid,
    sequence: Uuid,
    completion: da::AccountCompletion,
    clean: impl FnOnce(&w::AccessTaskSpec) -> Result<(), Error>,
) -> Result<(), Error> {
    let key = key(id, sequence);
    let Some(record) = state::read_optional::<Attempt>(root, &key)? else {
        return Ok(());
    };
    let never_started = match completion {
        da::AccountCompletion::NotStarted { expires_at } => {
            if record.started || record.renew_from != Some(expires_at) {
                return Err(Error::Conflict);
            }
            true
        }
        da::AccountCompletion::Observed => false,
    };
    if let Some(t) = record.permit {
        if (t.task_id, t.attempt_id) != (id, sequence) {
            return Err(Error::Identity);
        }
        clean(&t)?;
    } else if !never_started {
        return Err(Error::Storage);
    }
    state::remove(root, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_lost_first_start_can_retire_only_its_confirmed_unstarted_attempt() {
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("da-clean-{}", Uuid::new_v4()));
        platform_private_storage::directory(&path).unwrap();
        let task = Uuid::new_v4();
        let attempt = Uuid::new_v4();
        let other = Uuid::new_v4();
        let record = Attempt {
            renew_from: Some(31),
            started: false,
            permit: None,
        };
        state::write(&path, &key(task, attempt), &record).unwrap();
        state::write(&path, &key(task, other), &record).unwrap();
        let no_native =
            |_: &w::AccessTaskSpec| -> Result<(), Error> { panic!("no local grant was received") };
        for completion in [
            da::AccountCompletion::Observed,
            da::AccountCompletion::NotStarted { expires_at: 32 },
        ] {
            assert!(retire(&path, task, attempt, completion, no_native).is_err());
        }
        let completion = da::AccountCompletion::NotStarted { expires_at: 31 };
        retire(&path, task, attempt, completion, no_native).unwrap();
        retire(&path, task, attempt, completion, no_native).unwrap();
        assert!(state::read_optional::<Attempt>(&path, &key(task, attempt))
            .unwrap()
            .is_none());
        assert!(state::read_optional::<Attempt>(&path, &key(task, other))
            .unwrap()
            .is_some());
        std::fs::remove_dir_all(path).unwrap();
    }
}

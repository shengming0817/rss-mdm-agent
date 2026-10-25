//! Native Hello metadata in the existing protected store; never PINs or private keys.
use super::*;
use rss_mdm_agent_wire::runtime as w;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloRecord {
    pub authority: Authority,
    pub revision: i64,
    pub definition_digest: [u8; 32],
    pub enabled: bool,
    pub retired: bool,
    pub previous_key: Option<Uuid>,
    pub last_key: Option<Uuid>,
    pub policy_owned: bool,
    pub observation: Option<w::AccessWhfbFacts>,
}
fn load_hello(conn: &Connection, sid: &str) -> Result<Option<HelloRecord>, Error> {
    let bytes: Option<Vec<u8>> = conn
        .query_row("SELECT body FROM hello_bindings WHERE sid=?1", [sid], |r| {
            r.get(0)
        })
        .optional()?;
    bytes
        .map(|bytes| {
            if bytes.len() > 16384 {
                return Err(Error::Invalid);
            }
            let value: HelloRecord = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
            value.authority.validate()?;
            if value.authority.sid != sid
                || value.revision < 1
                || value.definition_digest == [0; 32]
                || !matches!(
                    value.authority.source,
                    w::AccessWindowsAuthority::Entra { .. }
                )
            {
                return Err(Error::Invalid);
            }
            if let Some(f) = &value.observation {
                f.validate(&AccessAccount::Windows {
                    sid: sid.into(),
                    authority: value.authority.source.clone(),
                })
                .map_err(|_| Error::Invalid)?;
                if f.factor_generation != value.authority.factor_generation {
                    return Err(Error::Invalid);
                }
            }
            Ok(value)
        })
        .transpose()
}
fn save_hello(conn: &Connection, value: &HelloRecord) -> Result<(), Error> {
    let bytes = encode(value)?;
    if bytes.len() > 16384 {
        return Err(Error::Invalid);
    }
    conn.execute("INSERT INTO hello_bindings VALUES(?1,?2) ON CONFLICT(sid) DO UPDATE SET body=excluded.body", params![value.authority.sid,bytes])?;
    Ok(())
}
impl Store {
    /// Only the service's authenticated registration can admit a signed native task.
    pub fn prepare_hello(
        &mut self,
        task: &w::AccessTaskSpec,
        authority: Authority,
    ) -> Result<HelloRecord, Error> {
        task.validate().map_err(|_| Error::Invalid)?;
        authority.validate()?;
        let Some(w::AccessWindowsPlan::Whfb {
            retire,
            previous_key,
        }) = &task.windows
        else {
            return Err(Error::Invalid);
        };
        if task.permit != w::TaskPermit::Start
            || authority.tenant != task.tenant_id
            || authority.device != task.device_id
            || authority.registration != task.registration_id
            || authority.registration_generation != task.generation
            || authority.binding != task.binding_id
            || authority.binding_generation != task.binding_generation
            || authority.factor_generation != task.factor_generation
            || authority.instance != task.instance_id
            || authority.principal != task.principal_id
        {
            return Err(Error::Changed);
        }
        let expected = AccessAccount::Windows {
            sid: authority.sid.clone(),
            authority: authority.source.clone(),
        };
        let enabled = match &task.action {
            w::AccessAction::Apply {
                account,
                configuration,
            } if account == &expected => configuration.enabled,
            w::AccessAction::Unbind { account, .. } if account == &expected => false,
            _ => return Err(Error::Invalid),
        };
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        registration::require(&tx, &authority)?;
        if !retire {
            registration::admit(&tx, &authority)?;
        }
        if let Some(rss) = load(&tx, &authority.sid)? {
            if !rss.revoked {
                return Err(Error::Changed);
            }
        }
        let old = load_hello(&tx, &authority.sid)?;
        if let Some(old) = &old {
            let same = old.authority.registration() == authority.registration()
                && old.authority.binding == authority.binding;
            if same {
                if old.authority.instance != authority.instance
                    || old.authority.principal != authority.principal
                    || old.authority.source != authority.source
                    || old.authority.join != authority.join
                    || old.authority.binding_generation > authority.binding_generation
                    || old.authority.factor_generation > authority.factor_generation
                    || old.revision > task.revision
                    || (old.revision == task.revision
                        && old.definition_digest != task.definition_digest)
                    || (old.retired
                        && !retire
                        && old.authority.binding_generation >= authority.binding_generation)
                {
                    return Err(Error::Changed);
                }
            } else if !old.retired
                || !old
                    .observation
                    .as_ref()
                    .is_some_and(|f| f.registration == w::AccessObservation::Absent)
            {
                return Err(Error::Changed);
            }
        }
        let previous_key = (*previous_key).or_else(|| {
            old.as_ref().and_then(|old| {
                if old.authority.binding != authority.binding
                    || old.authority.factor_generation < authority.factor_generation
                {
                    old.last_key.or(old.previous_key)
                } else {
                    old.previous_key
                }
            })
        });
        let policy_owned = old.as_ref().is_some_and(|v| {
            v.policy_owned
                && v.authority.source == authority.source
                && v.authority.registration() == authority.registration()
        });
        let mut value = HelloRecord {
            authority,
            revision: task.revision,
            definition_digest: task.definition_digest,
            enabled,
            retired: *retire,
            previous_key,
            last_key: old.as_ref().and_then(|v| v.last_key),
            policy_owned,
            observation: None,
        };
        if let Some(old) = old {
            if old.revision == task.revision && old.definition_digest == task.definition_digest {
                value.observation = old.observation;
            }
        }
        if *retire {
            registration::record_revocation(&tx, &value.authority, value.revision)?;
        }
        save_hello(&tx, &value)?;
        tx.commit()?;
        Ok(value)
    }
    /// Full retirement may erase metadata only after policy and native credential cleanup.
    /// Ordinary package removal must not call a native key deletion operation.
    pub fn hello_cleanup_complete(&self) -> Result<bool, Error> {
        self.check_files()?;
        let sids = self
            .conn
            .prepare("SELECT sid FROM hello_bindings")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for sid in sids {
            let record = load_hello(&self.conn, &sid)?.ok_or(Error::Invalid)?;
            if !record.retired
                || !record.observation.as_ref().is_some_and(|facts| {
                    facts.policy == w::AccessObservation::Absent
                        && facts.registration == w::AccessObservation::Absent
                })
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// Read retained ownership; callers still establish current OS identity independently.
    pub fn hello(&self, sid: &str) -> Result<Option<HelloRecord>, Error> {
        self.check_files()?;
        let value = load_hello(&self.conn, sid)?;
        if let Some(value) = &value {
            registration::require(&self.conn, &value.authority)?;
        }
        Ok(value)
    }
    /// Observations cannot change task authority or revive a retired binding.
    pub fn observe_hello(
        &mut self,
        expected: &HelloRecord,
        mut facts: w::AccessWhfbFacts,
        policy_owned: bool,
    ) -> Result<w::AccessWhfbFacts, Error> {
        facts
            .validate(&AccessAccount::Windows {
                sid: expected.authority.sid.clone(),
                authority: expected.authority.source.clone(),
            })
            .map_err(|_| Error::Invalid)?;
        if facts.factor_generation != expected.authority.factor_generation {
            return Err(Error::Changed);
        }
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        registration::require(&tx, &expected.authority)?;
        let mut current = load_hello(&tx, &expected.authority.sid)?.ok_or(Error::Changed)?;
        if current.authority != expected.authority
            || current.revision != expected.revision
            || current.definition_digest != expected.definition_digest
            || current
                .observation
                .as_ref()
                .is_some_and(|old| old.observed_at > facts.observed_at)
        {
            return Err(Error::Changed);
        }
        if !current.retired
            && facts.registration == w::AccessObservation::Present
            && current.previous_key.is_some()
            && facts.key_id == current.previous_key
        {
            facts.registration = w::AccessObservation::Unknown;
            facts.hardware_key = w::AccessObservation::Unknown;
        }
        if facts.registration == w::AccessObservation::Present {
            current.last_key = facts.key_id;
        }
        current.policy_owned |= policy_owned;
        current.observation = Some(facts.clone());
        save_hello(&tx, &current)?;
        tx.commit()?;
        Ok(facts)
    }
}
pub(super) fn retire_all(conn: &Connection) -> Result<(), Error> {
    let sids = conn
        .prepare("SELECT sid FROM hello_bindings")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for sid in sids {
        let mut value = load_hello(conn, &sid)?.ok_or(Error::Invalid)?;
        registration::record_revocation(conn, &value.authority, value.revision)?;
        value.retired = true;
        value.enabled = false;
        save_hello(conn, &value)?;
    }
    Ok(())
}

pub(super) fn require_rss(conn: &Connection, sid: &str) -> Result<(), Error> {
    if load_hello(conn, sid)?.is_some_and(|r| {
        !r.retired
            || !r
                .observation
                .as_ref()
                .is_some_and(|f| f.registration == w::AccessObservation::Absent)
    }) {
        Err(Error::Changed)
    } else {
        Ok(())
    }
}

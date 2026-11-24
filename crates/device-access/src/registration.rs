//! One durable admission fence shared by service writers and the offline provider.
use super::*;

/// Authenticated transport that owns an offline binding within this installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Channel {
    Mdm = 1,
    Da = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub channel: Channel,
    pub tenant: Uuid,
    pub device: rss_mdm_agent_wire::DeviceId,
}
/// Identity of the exact setup attempt within the installation-bound store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateKey {
    pub owner: Registration,
    pub binding: Uuid,
    pub task: Uuid,
    pub attempt: Uuid,
}
impl CandidateKey {
    pub fn for_task(channel: Channel, t: &rss_mdm_agent_wire::runtime::AccessTaskSpec) -> Self {
        Self {
            owner: Registration {
                channel,
                tenant: t.tenant_id,
                device: t.device_id,
            },
            binding: t.binding_id,
            task: t.task_id,
            attempt: t.attempt_id,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    registration: Registration,
    retired: bool,
}
impl Authority {
    pub fn registration(&self) -> Registration {
        Registration {
            channel: self.channel,
            tenant: self.tenant,
            device: self.device,
        }
    }
}
impl Registration {
    fn validate(&self) -> Result<(), Error> {
        if self.tenant.is_nil() {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
fn current(conn: &Connection, channel: Channel) -> Result<Option<Current>, Error> {
    let bytes: Option<Vec<u8>> = conn
        .query_row(
            "SELECT body FROM registration WHERE channel=?1",
            [channel as u8],
            |r| r.get(0),
        )
        .optional()?;
    bytes
        .map(|b| {
            if b.len() > 4096 {
                return Err(Error::Invalid);
            }
            let value: Current = serde_json::from_slice(&b).map_err(|_| Error::Invalid)?;
            value.registration.validate()?;
            if value.registration.channel != channel {
                return Err(Error::Invalid);
            }
            Ok(value)
        })
        .transpose()
}
pub(super) fn require(conn: &Connection, authority: &Authority) -> Result<(), Error> {
    require_registration(conn, authority.registration())
}
pub(super) fn require_registration(conn: &Connection, owner: Registration) -> Result<(), Error> {
    let value = current(conn, owner.channel)?.ok_or(Error::Changed)?;
    if value.retired || value.registration != owner {
        return Err(Error::Changed);
    }
    Ok(())
}
pub(super) fn record_revocation(
    conn: &Connection,
    authority: &Authority,
    revision: i64,
) -> Result<(), Error> {
    let previous: Option<(i64, i64)> = conn
        .query_row(
            "SELECT generation,revision FROM revoked WHERE binding=?1 AND owner=?2",
            params![
                authority.binding.to_string(),
                encode(&authority.registration())?
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if previous.is_some_and(|(generation, rev)| {
        generation > authority.binding_generation
            || (generation == authority.binding_generation && rev > revision)
    }) {
        return Err(Error::Changed);
    }
    conn.execute("INSERT INTO revoked VALUES(?4,?1,?2,?3) ON CONFLICT(owner,binding) DO UPDATE SET generation=excluded.generation,revision=excluded.revision", params![authority.binding.to_string(),authority.binding_generation,revision,encode(&authority.registration())?])?;
    Ok(())
}
pub(super) fn admit(conn: &Connection, authority: &Authority) -> Result<(), Error> {
    require(conn, authority)?;
    let generation: Option<i64> = conn
        .query_row(
            "SELECT generation FROM revoked WHERE binding=?1 AND owner=?2",
            params![
                authority.binding.to_string(),
                encode(&authority.registration())?
            ],
            |r| r.get(0),
        )
        .optional()?;
    if generation.is_some_and(|g| g >= authority.binding_generation) {
        return Err(Error::Changed);
    }
    Ok(())
}
impl Store {
    /// Admission precedes account mutation and recovery escrow, not merely factor activation.
    pub fn admit_task(
        &self,
        channel: Channel,
        task: &rss_mdm_agent_wire::runtime::AccessTaskSpec,
    ) -> Result<(), Error> {
        task.validate().map_err(|_| Error::Invalid)?;
        if !matches!(
            task.windows,
            Some(rss_mdm_agent_wire::runtime::AccessWindowsPlan::Whfb { .. })
        ) {
            if let rss_mdm_agent_wire::runtime::AccessAction::Apply {
                account: AccessAccount::Windows { sid, .. },
                ..
            } = &task.action
            {
                super::hello::require_rss(&self.conn, sid)?;
                if load(&self.conn, sid)?.is_some_and(|r| {
                    !r.revoked
                        && (r.authority.channel != channel
                            || r.authority.binding != task.binding_id)
                }) {
                    return Err(Error::Changed);
                }
            }
        }
        self.check_files()?;
        let value = current(&self.conn, channel)?.ok_or(Error::Changed)?;
        if value.retired
            || value.registration
                != (Registration {
                    channel,
                    tenant: task.tenant_id,
                    device: task.device_id,
                })
        {
            return Err(Error::Changed);
        }
        let generation: Option<i64> = self
            .conn
            .query_row(
                "SELECT generation FROM revoked WHERE binding=?1 AND owner=?2",
                params![task.binding_id.to_string(), encode(&value.registration)?],
                |r| r.get(0),
            )
            .optional()?;
        if generation.is_some_and(|g| g >= task.binding_generation) {
            return Err(Error::Changed);
        }
        Ok(())
    }
    /// A changed join is persisted before accepting a newly authenticated native session.
    pub fn observe_join(&mut self, fingerprint: &str, now: i64) -> Result<i64, Error> {
        if fingerprint.is_empty()
            || fingerprint.len() > 512
            || fingerprint.chars().any(char::is_control)
            || now <= 0
        {
            return Err(Error::Invalid);
        }
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<(String, i64)> = tx
            .query_row(
                "SELECT fingerprint,observed FROM join_epoch WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((same, observed)) = &old {
            if *observed <= 0 || now < *observed {
                return Err(Error::Invalid);
            }
            if same == fingerprint {
                return Ok(*observed);
            }
        }
        if old.is_some() {
            retire_records(&tx, None)?;
        }
        tx.execute("INSERT INTO join_epoch VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET fingerprint=excluded.fingerprint,observed=excluded.observed", params![fingerprint, now])?;
        tx.commit()?;
        Ok(now)
    }
    pub fn require_join_session(
        &mut self,
        fingerprint: &str,
        logon: i64,
        now: i64,
    ) -> Result<(), Error> {
        let observed = self.observe_join(fingerprint, now)?;
        if logon < observed || logon > now {
            return Err(Error::Changed);
        }
        Ok(())
    }
    /// Signed Unbind closes a binding even if its Initialize has not arrived yet.
    pub fn revoke_task(
        &mut self,
        channel: Channel,
        task: &rss_mdm_agent_wire::runtime::AccessTaskSpec,
    ) -> Result<(), Error> {
        use rss_mdm_agent_wire::runtime::{AccessAccount, AccessAction, TaskPermit};
        task.validate().map_err(|_| Error::Invalid)?;
        let AccessAction::Unbind {
            account: AccessAccount::Windows { sid, authority },
            ..
        } = &task.action
        else {
            return Err(Error::Invalid);
        };
        super::hello::require_rss(&self.conn, sid)?;
        if task.permit != TaskPermit::Start {
            return Err(Error::Invalid);
        }
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = current(&tx, channel)?.ok_or(Error::Changed)?;
        if current.retired
            || current.registration
                != (Registration {
                    channel,
                    tenant: task.tenant_id,
                    device: task.device_id,
                })
        {
            return Err(Error::Changed);
        }
        let previous: Option<(i64, i64)> = tx
            .query_row(
                "SELECT generation,revision FROM revoked WHERE binding=?1 AND owner=?2",
                params![task.binding_id.to_string(), encode(&current.registration)?],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if previous.is_some_and(|(generation, revision)| {
            generation > task.binding_generation
                || (generation == task.binding_generation && revision > task.revision)
        }) {
            return Err(Error::Changed);
        }
        if let Some(mut record) = load(&tx, sid)? {
            let a = &record.authority;
            if a.registration() != current.registration
                || a.instance != task.instance_id
                || a.principal != task.principal_id
                || &a.source != authority
                || a.binding != task.binding_id
                || a.binding_generation > task.binding_generation
                || record.revision > task.revision
                || (record.revision == task.revision && !record.revoked)
            {
                return Err(Error::Changed);
            }
            record.revision = task.revision;
            record.revoked = true;
            record.enabled = false;
            save(&tx, &record)?;
        }
        tx.execute("INSERT INTO revoked VALUES(?4,?1,?2,?3) ON CONFLICT(owner,binding) DO UPDATE SET generation=excluded.generation,revision=excluded.revision", params![task.binding_id.to_string(),task.binding_generation,task.revision,encode(&current.registration)?])?;
        tx.commit()?;
        Ok(())
    }
    /// Read-only recovery of an interrupted DA result. Require the exact locally
    /// committed revision; observing the same SID later cannot settle this attempt.
    pub fn observed_da_task(
        &self,
        task: &rss_mdm_agent_wire::runtime::AccessTaskSpec,
        sid: &str,
    ) -> Result<Option<(Authority, Option<AccessRssFacts>)>, Error> {
        use rss_mdm_agent_wire::runtime::AccessAction;
        self.check_files()?;
        let Some(record) = load(&self.conn, sid)? else {
            return Ok(None);
        };
        let a = record.authority;
        if a.channel != Channel::Da
            || a.tenant != task.tenant_id
            || a.device != task.device_id
            || a.principal != task.principal_id
            || a.instance != task.instance_id
            || a.binding != task.binding_id
            || record.revision != task.revision
        {
            return Ok(None);
        }
        require(&self.conn, &a)?;
        match &task.action {
            AccessAction::Unbind { .. }
                if record.revoked && a.binding_generation <= task.binding_generation =>
            {
                Ok(Some((a, None)))
            }
            AccessAction::Create { configuration, .. }
            | AccessAction::Apply { configuration, .. }
                if !record.revoked
                    && record.enabled == configuration.enabled
                    && a.binding_generation == task.binding_generation
                    && a.factor_generation == task.factor_generation =>
            {
                Ok(Some((a, Some(record.facts))))
            }
            _ => Ok(None),
        }
    }
    /// Native execution can inspect an exact disabled binding to complete its authorized removal.
    pub fn stored_authority(&self, sid: &str) -> Result<Option<Authority>, Error> {
        self.check_files()?;
        let Some(record) = load(&self.conn, sid)? else {
            return self.hello(sid).map(|r| r.map(|r| r.authority));
        };
        let value = current(&self.conn, record.authority.channel)?.ok_or(Error::Changed)?;
        if value.retired || value.registration != record.authority.registration() {
            return Ok(None);
        }
        Ok(Some(record.authority))
    }
    /// Called only with the service's durable, authenticated registration; never from a task.
    pub fn bind_registration(&mut self, registration: Registration) -> Result<(), Error> {
        registration.validate()?;
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = current(&tx, registration.channel)? {
            if old.registration == registration {
                return if old.retired {
                    Err(Error::Changed)
                } else {
                    Ok(())
                };
            }
            if old.registration.tenant != registration.tenant {
                return Err(Error::Changed);
            }
            retire_records(&tx, Some(&old.registration))?;
        }
        tx.execute("INSERT INTO registration VALUES(?1,?2) ON CONFLICT(channel) DO UPDATE SET body=excluded.body", params![registration.channel as u8, encode(&Current { registration, retired: false })?])?;
        tx.commit()?;
        Ok(())
    }
    /// A confirmed retirement cannot be undone by reopening or replaying the same registration.
    pub fn retire_registration(&mut self, channel: Channel) -> Result<(), Error> {
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(mut value) = current(&tx, channel)? {
            retire_records(&tx, Some(&value.registration))?;
            value.retired = true;
            tx.execute(
                "UPDATE registration SET body=?1 WHERE channel=?2",
                params![encode(&value)?, channel as u8],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Read back the current DA device's configured account classes from the offline provider's
    /// live admission records. A machine with no matching bindings cannot confirm application.
    pub fn confirms_rss_policy(
        &self,
        channel: Channel,
        tenant: Uuid,
        device: rss_mdm_agent_wire::DeviceId,
        enabled: bool,
        classes: &[rss_mdm_agent_wire::runtime::AccessWindowsKind],
    ) -> Result<bool, Error> {
        self.check_files()?;
        let Some(registration) = current(&self.conn, channel)? else {
            return Ok(false);
        };
        if registration.retired
            || registration.registration
                != (Registration {
                    channel,
                    tenant,
                    device,
                })
        {
            return Ok(false);
        }
        let sids = self
            .conn
            .prepare("SELECT sid FROM bindings LIMIT 1025")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if sids.len() > 1024 {
            return Err(Error::Storage);
        }
        let mut observed = std::collections::BTreeSet::new();
        for sid in sids {
            let record = load(&self.conn, &sid)?.ok_or(Error::Storage)?;
            if record.authority.registration() != registration.registration {
                continue;
            }
            let kind = record.authority.source.kind();
            if self.enabled(&sid)? != (enabled && classes.contains(&kind)) {
                return Ok(false);
            }
            if classes.contains(&kind) {
                observed.insert(kind);
            }
        }
        Ok(classes.iter().all(|class| observed.contains(class)))
    }
    /// Returns only an active, current binding; callers still recheck during factor consumption.
    pub fn authority(&self, sid: &str) -> Result<Option<Authority>, Error> {
        self.check_files()?;
        let Some(record) = load(&self.conn, sid)? else {
            return Ok(None);
        };
        if !record.enabled || record.revoked {
            return Ok(None);
        }
        require(&self.conn, &record.authority)?;
        Ok(Some(record.authority))
    }
}
fn retire_records(conn: &Connection, registration: Option<&Registration>) -> Result<(), Error> {
    super::hello::retire_all(conn, registration)?;
    if let Some(owner) = registration {
        conn.execute("DELETE FROM candidates WHERE owner=?1", [encode(owner)?])?;
    }
    let sids = conn
        .prepare("SELECT sid FROM bindings")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for sid in sids {
        let mut record = load(conn, &sid)?.ok_or(Error::Changed)?;
        if registration.is_some_and(|r| *r != record.authority.registration()) {
            continue;
        }
        if registration.is_none()
            && !matches!(
                record.authority.source,
                rss_mdm_agent_wire::runtime::AccessWindowsAuthority::Entra { .. }
            )
        {
            continue;
        }
        record_revocation(conn, &record.authority, record.revision)?;
        record.enabled = false;
        record.revoked = true;
        save(conn, &record)?;
    }
    Ok(())
}

//! One durable admission fence shared by service writers and the offline provider.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub tenant: Uuid,
    pub device: String,
    pub id: Uuid,
    pub generation: u64,
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
            tenant: self.tenant,
            device: self.device.clone(),
            id: self.registration,
            generation: self.registration_generation,
        }
    }
}
impl Registration {
    fn validate(&self) -> Result<(), Error> {
        if self.tenant.is_nil()
            || self.id.is_nil()
            || self.generation == 0
            || self.generation > i64::MAX as u64
            || self.device.is_empty()
            || self.device.len() > 256
            || self.device.chars().any(char::is_control)
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
fn current(conn: &Connection) -> Result<Option<Current>, Error> {
    let bytes: Option<Vec<u8>> = conn
        .query_row("SELECT body FROM registration WHERE id=1", [], |r| r.get(0))
        .optional()?;
    bytes
        .map(|b| {
            if b.len() > 4096 {
                return Err(Error::Invalid);
            }
            let value: Current = serde_json::from_slice(&b).map_err(|_| Error::Invalid)?;
            value.registration.validate()?;
            Ok(value)
        })
        .transpose()
}
pub(super) fn require(conn: &Connection, authority: &Authority) -> Result<(), Error> {
    let value = current(conn)?.ok_or(Error::Changed)?;
    if value.retired || value.registration != authority.registration() {
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
            "SELECT generation,revision FROM revoked WHERE binding=?1",
            [authority.binding.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if previous.is_some_and(|(generation, rev)| {
        generation > authority.binding_generation
            || (generation == authority.binding_generation && rev > revision)
    }) {
        return Err(Error::Changed);
    }
    conn.execute("INSERT INTO revoked VALUES(?1,?2,?3) ON CONFLICT(binding) DO UPDATE SET generation=excluded.generation,revision=excluded.revision", params![authority.binding.to_string(),authority.binding_generation,revision])?;
    Ok(())
}
pub(super) fn admit(conn: &Connection, authority: &Authority) -> Result<(), Error> {
    require(conn, authority)?;
    let generation: Option<i64> = conn
        .query_row(
            "SELECT generation FROM revoked WHERE binding=?1",
            [authority.binding.to_string()],
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
            }
        }
        self.check_files()?;
        let value = current(&self.conn)?.ok_or(Error::Changed)?;
        if value.retired
            || value.registration
                != (Registration {
                    tenant: task.tenant_id,
                    device: task.device_id.clone(),
                    id: task.registration_id,
                    generation: task.generation,
                })
        {
            return Err(Error::Changed);
        }
        let generation: Option<i64> = self
            .conn
            .query_row(
                "SELECT generation FROM revoked WHERE binding=?1",
                [task.binding_id.to_string()],
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
            retire_records(&tx, true)?;
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
        let current = current(&tx)?.ok_or(Error::Changed)?;
        if current.retired
            || current.registration
                != (Registration {
                    tenant: task.tenant_id,
                    device: task.device_id.clone(),
                    id: task.registration_id,
                    generation: task.generation,
                })
        {
            return Err(Error::Changed);
        }
        let previous: Option<(i64, i64)> = tx
            .query_row(
                "SELECT generation,revision FROM revoked WHERE binding=?1",
                [task.binding_id.to_string()],
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
        tx.execute("INSERT INTO revoked VALUES(?1,?2,?3) ON CONFLICT(binding) DO UPDATE SET generation=excluded.generation,revision=excluded.revision", params![task.binding_id.to_string(),task.binding_generation,task.revision])?;
        tx.commit()?;
        Ok(())
    }
    /// Native execution can inspect an exact disabled binding to complete its authorized removal.
    pub fn stored_authority(&self, sid: &str) -> Result<Option<Authority>, Error> {
        self.check_files()?;
        let Some(record) = load(&self.conn, sid)? else {
            return self.hello(sid).map(|r| r.map(|r| r.authority));
        };
        let value = current(&self.conn)?.ok_or(Error::Changed)?;
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
        if let Some(old) = current(&tx)? {
            if old.registration == registration {
                return if old.retired {
                    Err(Error::Changed)
                } else {
                    Ok(())
                };
            }
            if old.registration.tenant != registration.tenant
                || old.registration.device != registration.device
                || registration.generation <= old.registration.generation
            {
                return Err(Error::Changed);
            }
            retire_records(&tx, false)?;
        }
        tx.execute("INSERT INTO registration VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET body=excluded.body", [encode(&Current { registration, retired: false })?])?;
        tx.commit()?;
        Ok(())
    }
    /// A confirmed retirement cannot be undone by reopening or replaying the same registration.
    pub fn retire_registration(&mut self) -> Result<(), Error> {
        self.check_files()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(mut value) = current(&tx)? {
            retire_records(&tx, false)?;
            value.retired = true;
            tx.execute(
                "UPDATE registration SET body=?1 WHERE id=1",
                [encode(&value)?],
            )?;
        }
        tx.commit()?;
        Ok(())
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
fn retire_records(conn: &Connection, cloud_only: bool) -> Result<(), Error> {
    super::hello::retire_all(conn)?;
    let sids = conn
        .prepare("SELECT sid FROM bindings")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for sid in sids {
        let mut record = load(conn, &sid)?.ok_or(Error::Changed)?;
        if cloud_only
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

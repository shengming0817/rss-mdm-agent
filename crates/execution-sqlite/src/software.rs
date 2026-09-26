use crate::{
    database::bounded_blob,
    execution::load_execution,
    journal::{authorize, decode, encode},
    *,
};
use execution_contract::{
    AttemptId, FrozenPlan, Ownership, SoftwareEvidence, SoftwareProvenance, SoftwareState,
};
use execution_lifecycle::{EffectAssessment, Observation};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

pub(crate) fn claim(
    conn: &Connection,
    plan: &FrozenPlan,
    attempt: &AttemptId,
) -> Result<(), Error> {
    if let Some(s) = plan.spec().execution.software() {
        for key in s.lock_keys() {
            let held: Option<String> = conn
                .query_row(
                    "SELECT attempt_id FROM software_claims WHERE resource=?1",
                    [&key],
                    |r| r.get(0),
                )
                .optional()?;
            if held.as_deref().is_some_and(|a| a != attempt.as_str()) {
                return Err(Error::Busy);
            }
            conn.execute(
                "INSERT OR IGNORE INTO software_claims VALUES(?1,?2)",
                params![key, attempt.as_str()],
            )?;
        }
    }
    Ok(())
}
fn facts(
    conn: &Connection,
    attempt: &AttemptId,
    limits: Limits,
) -> Result<Option<SoftwareEvidence>, Error> {
    let bytes: Option<Vec<u8>> = conn
        .query_row(
            &format!(
                "SELECT {} FROM software_evidence WHERE attempt_id=?1",
                bounded_blob("body", limits.max_record_bytes)
            ),
            [attempt.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    bytes
        .map(|b| decode(&b, limits.max_record_bytes))
        .transpose()
}
pub(crate) fn settle(
    conn: &Connection,
    plan: &FrozenPlan,
    next: &execution_lifecycle::Snapshot,
    limits: Limits,
) -> Result<(), Error> {
    let Some(s) = plan.spec().execution.software() else {
        return Ok(());
    };
    let Some(a) = &next.attempt else {
        return Ok(());
    };
    if a.termination.is_none() {
        return Ok(());
    }
    let never = a
        .termination
        .as_ref()
        .is_some_and(|f| matches!(f.observation, Observation::NeverDispatched { .. }));
    let assessment = a.assessment.as_ref().and_then(|f| match f.observation {
        Observation::Effect { assessment } => Some(assessment),
        _ => None,
    });
    if !never
        && !matches!(
            assessment,
            Some(
                EffectAssessment::Satisfied
                    | EffectAssessment::NoEffect
                    | EffectAssessment::NotSatisfied
            )
        )
    {
        return Ok(());
    }
    if assessment == Some(EffectAssessment::Satisfied) {
        let evidence = facts(conn, &a.id, limits)?.ok_or(Error::Conflict)?;
        if evidence.plan_digest != *plan.digest()
            || !s.satisfied(&evidence.detected)
            || evidence.restart_required
        {
            return Err(Error::Conflict);
        }
        let resource = &s.lock_keys()[1];
        match &evidence.detected {
            SoftwareState::Absent {} => {
                conn.execute(
                    "DELETE FROM software_ownership WHERE resource=?1",
                    [resource],
                )?;
            }
            SoftwareState::Present { .. } => {
                let existing =
                    ownership(conn, plan, limits)?.ownership == Ownership::OrganizationManaged;
                if existing || matches!(evidence.before, Some(SoftwareState::Absent {})) {
                    conn.execute("INSERT INTO software_ownership VALUES(?1,?2,?3,?4) ON CONFLICT(resource) DO UPDATE SET attempt_id=excluded.attempt_id,authority=excluded.authority,package=excluded.package", params![resource,a.id.as_str(),encode(&plan.spec().request.authority,limits.max_record_bytes)?,encode(&s.package,limits.max_record_bytes)?])?;
                }
            }
            SoftwareState::Unknown {} => return Err(Error::Conflict),
        }
    }
    conn.execute(
        "DELETE FROM software_claims WHERE attempt_id=?1",
        [a.id.as_str()],
    )?;
    Ok(())
}
fn ownership(
    conn: &Connection,
    plan: &FrozenPlan,
    limits: Limits,
) -> Result<SoftwareProvenance, Error> {
    let s = plan
        .spec()
        .execution
        .software()
        .ok_or(Error::InvalidInput)?;
    let row: Option<(Vec<u8>, Vec<u8>, String)> = conn
        .query_row(
            &format!(
                "SELECT {},{},attempt_id FROM software_ownership WHERE resource=?1",
                bounded_blob("authority", limits.max_record_bytes),
                bounded_blob("package", limits.max_record_bytes)
            ),
            [&s.lock_keys()[1]],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    Ok(match row {
        Some((a, p, id))
            if a == encode(&plan.spec().request.authority, limits.max_record_bytes)?
                && p == encode(&s.package, limits.max_record_bytes)? =>
        {
            SoftwareProvenance {
                ownership: Ownership::OrganizationManaged,
                state: facts(
                    conn,
                    &AttemptId::new(id).map_err(|_| Error::Corrupt)?,
                    limits,
                )?
                .map(|f| f.detected),
            }
        }
        _ => SoftwareProvenance {
            ownership: Ownership::UserExisting,
            state: None,
        },
    })
}
impl Store {
    /// Read protected provenance for this exact software scope; presence never creates ownership.
    pub fn software_ownership(
        &self,
        scope: &Scope,
        host: &impl Host,
    ) -> Result<SoftwareProvenance, Error> {
        let tx = self.read(scope, Access::RunnerFact, None, host)?;
        let (plan, _, _) = load_execution(&tx, scope, self.limits)?;
        ownership(&tx, &plan, self.limits)
    }
    /// Persist independent software detection even when process termination remains unknown.
    pub fn record_software(
        &mut self,
        scope: &Scope,
        value: &SoftwareEvidence,
        host: &impl Host,
    ) -> Result<(), Error> {
        self.check_scope(scope)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        authorize(host, Access::RunnerFact, scope, None)?;
        let (plan, execution, _) = load_execution(&tx, scope, self.limits)?;
        let a = execution
            .snapshot()
            .attempt
            .as_ref()
            .ok_or(Error::Conflict)?;
        let s = plan
            .spec()
            .execution
            .software()
            .ok_or(Error::InvalidInput)?;
        if a.id != value.attempt_id
            || a.runner != value.runner
            || *plan.digest() != value.plan_digest
        {
            return Err(Error::InvalidInput);
        }
        for state in value.before.iter().chain(std::iter::once(&value.detected)) {
            if matches!(state, SoftwareState::Present { version } if !s.detection.versions.iter().any(|p| &p.version == version))
            {
                return Err(Error::InvalidInput);
            }
        }
        let mut value = value.clone();
        if let Some(old) = facts(&tx, &a.id, self.limits)? {
            if old.before.is_some() && value.before.is_some() && old.before != value.before {
                return Err(Error::Conflict);
            }
            value.before = old.before.clone().or(value.before);
            value.restart_required |= old.restart_required;
            if a.assessment.as_ref().is_some_and(|o| {
                matches!(
                    o.observation,
                    Observation::Effect {
                        assessment: EffectAssessment::Satisfied
                            | EffectAssessment::NoEffect
                            | EffectAssessment::NotSatisfied
                    }
                )
            }) && value != old
            {
                return Err(Error::Conflict);
            }
        }
        tx.execute("INSERT INTO software_evidence VALUES(?1,?2) ON CONFLICT(attempt_id) DO UPDATE SET body=excluded.body",params![a.id.as_str(),encode(&value,self.limits.max_record_bytes)?])?;
        tx.commit().map_err(|_| Error::OperationCommitUnknown)
    }
    /// Read independent observations under the same scope authorization as process evidence.
    pub fn software_evidence(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl Host,
    ) -> Result<Option<SoftwareEvidence>, Error> {
        let tx = self.read(scope, Access::RunnerFact, None, host)?;
        let (plan, execution, _) = load_execution(&tx, scope, self.limits)?;
        if execution
            .snapshot()
            .attempt
            .as_ref()
            .is_none_or(|a| &a.id != attempt)
        {
            return Err(Error::Conflict);
        }
        let value = facts(&tx, attempt, self.limits)?;
        if value
            .as_ref()
            .is_some_and(|v| v.plan_digest != *plan.digest())
        {
            return Err(Error::Corrupt);
        }
        Ok(value)
    }
}

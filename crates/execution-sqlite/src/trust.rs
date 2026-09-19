use crate::database::bounded_blob;
use crate::{journal::*, *};
use execution_approval::{
    ApprovalFacts, ApprovalRecord, ApprovalStatus, ApprovalVerifier, ProfileApproval,
    VerificationError,
};
use execution_contract::{AttemptId, FrozenPlan, VersionedRef};
use rusqlite::{params, Connection, OptionalExtension};

pub(crate) struct Head {
    pub revision: u64,
    pub authorization: VersionedRef,
    pub approval: VersionedRef,
    pub digest: String,
    pub until: u64,
}
pub(crate) fn head(
    conn: &Connection,
    scope: &Scope,
    limits: Limits,
) -> Result<Option<Head>, Error> {
    let row = conn
        .query_row(
            &format!(
                "SELECT revision,{},{},approval_digest,fresh_until FROM trust_heads WHERE scope=?1",
                bounded_blob("authorization_revision", limits.max_record_bytes),
                bounded_blob("approval_revision", limits.max_record_bytes)
            ),
            [scope.key()],
            |r| {
                Ok((
                    r.get::<_, u64>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, u64>(4)?,
                ))
            },
        )
        .optional()?;
    row.map(|(revision, a, p, digest, until)| {
        Ok(Head {
            revision,
            authorization: decode(&a, limits.max_record_bytes)?,
            approval: decode(&p, limits.max_record_bytes)?,
            digest,
            until,
        })
    })
    .transpose()
}
impl Store {
    /// Read the protected CAS revision for a subsequent trust refresh. Requires trust-management
    /// access, independently of result/audit access; absence means the first refresh uses None.
    pub fn trust_revision(&self, scope: &Scope, host: &impl Host) -> Result<Option<u64>, Error> {
        let tx = self.read(scope, Access::ManageTrust, None, host)?;
        Ok(head(&tx, scope, self.limits)?.map(|head| head.revision))
    }

    /// Atomically install a complete freshly verified snapshot using expected local head revision.
    /// Definitions are immutable; refresh cannot import, reset or refund local consumption counters.
    pub fn refresh_trust(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        expected: Option<u64>,
        host: &impl Host,
    ) -> Result<CommitOutcome, Error> {
        let mut w = match self.start(
            op,
            scope,
            OperationKind::Trust,
            &expected,
            Access::ManageTrust,
            host,
        )? {
            Start::Replay(r) => return Ok(CommitOutcome::AlreadyCommitted(r)),
            Start::New(w) => w,
        };
        let old = head(&w.tx, scope, w.limits)?;
        if old.as_ref().map(|h| h.revision) != expected {
            return Err(Error::Conflict);
        }
        let mut snapshot = host.trusted_snapshot(scope)?;
        w.refresh_time(host)?;
        validate_snapshot(&mut snapshot, scope, w.now, w.limits)?;
        let digest = hash(&snapshot.approvals)?;
        if old
            .as_ref()
            .is_some_and(|h| h.approval == snapshot.approval_revision && h.digest != digest)
        {
            return Err(Error::Conflict);
        }
        retain_version(
            &w,
            "authorization",
            &snapshot.authorization_revision,
            old.as_ref().map(|h| &h.authorization),
        )?;
        retain_version(
            &w,
            "approval",
            &snapshot.approval_revision,
            old.as_ref().map(|h| &h.approval),
        )?;
        w.tx.execute(
            "UPDATE approval_heads SET status='unknown' WHERE scope=?1",
            [scope.key()],
        )?;
        for entry in &snapshot.approvals {
            install(&w, entry)?;
        }
        let revision = expected
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(Error::Capacity)?;
        w.tx.execute("INSERT INTO trust_heads VALUES(?1,?2,?3,?4,?5,?6)
            ON CONFLICT(scope) DO UPDATE SET revision=excluded.revision,authorization_revision=excluded.authorization_revision,
            approval_revision=excluded.approval_revision,approval_digest=excluded.approval_digest,fresh_until=excluded.fresh_until",
            params![scope.key(), integer(revision)?, w.bounded(&snapshot.authorization_revision)?, w.bounded(&snapshot.approval_revision)?,
                digest, integer(snapshot.fresh_until_unix_ms)?])?;
        let mut audit = empty_audit(AuditReason::TrustRefreshed);
        audit.trust = Some(TrustAudit {
            authorization_revision: snapshot.authorization_revision,
            approval_revision: snapshot.approval_revision,
            fresh_until_unix_ms: snapshot.fresh_until_unix_ms,
        });
        w.finish(Outcome::Changed, revision, audit)
    }
}
fn validate_snapshot(
    snapshot: &mut TrustSnapshot,
    scope: &Scope,
    now: u64,
    limits: Limits,
) -> Result<(), Error> {
    if snapshot.fresh_until_unix_ms <= now || snapshot.approvals.len() > limits.max_approvals {
        return Err(Error::Trust);
    }
    integer(snapshot.fresh_until_unix_ms)?;
    snapshot.approvals.sort_by(|a, b| {
        a.definition
            .reference
            .id
            .as_str()
            .cmp(b.definition.reference.id.as_str())
    });
    let mut previous = None;
    for entry in &mut snapshot.approvals {
        let d = &mut entry.definition;
        if previous == Some(&d.reference.id)
            || d.plan_id != scope.plan_id
            || d.max_uses == 0
            || d.profiles.is_empty()
            || d.profiles.len() > limits.max_approvals
            || d.validity.expires_at_unix_ms <= d.validity.not_before_unix_ms
        {
            return Err(Error::Trust);
        }
        previous = Some(&d.reference.id);
        d.profiles
            .sort_by(|a, b| (&a.id, &a.revision).cmp(&(&b.id, &b.revision)));
        if d.profiles.windows(2).any(|p| p[0] == p[1]) {
            return Err(Error::Trust);
        }
    }
    encode(&snapshot.approvals, limits.max_record_bytes)?;
    Ok(())
}
fn retain_version(
    w: &Write<'_>,
    kind: &str,
    version: &VersionedRef,
    current: Option<&VersionedRef>,
) -> Result<(), Error> {
    if current == Some(version) {
        return Ok(());
    }
    // A previous source revision can never become current again, even after record cleanup.
    w.tx.execute(
        "INSERT INTO trust_versions VALUES(?1,?2,?3)",
        params![w.scope.key(), kind, w.bounded(version)?],
    )?;
    Ok(())
}
fn install(w: &Write<'_>, entry: &TrustedApproval) -> Result<(), Error> {
    let d = &entry.definition;
    let bytes = w.bounded(d)?;
    let old: Option<Vec<u8>> =
        w.tx.query_row(
            &format!(
                "SELECT {} FROM approvals WHERE scope=?1 AND record_id=?2 AND version=?3",
                bounded_blob("definition", w.limits.max_record_bytes)
            ),
            params![
                w.scope.key(),
                d.reference.id.as_str(),
                d.reference.revision.as_str()
            ],
            |r| r.get(0),
        )
        .optional()?;
    match old {
        Some(old) if old != bytes => return Err(Error::Conflict),
        Some(_) => {}
        None => {
            w.tx.execute(
                "INSERT INTO approvals VALUES(?1,?2,?3,?4)",
                params![
                    w.scope.key(),
                    d.reference.id.as_str(),
                    d.reference.revision.as_str(),
                    bytes
                ],
            )?;
            w.tx.execute(
                "INSERT INTO approval_usage VALUES(?1,?2,?3,0,0)",
                params![
                    w.scope.key(),
                    d.reference.id.as_str(),
                    d.reference.revision.as_str()
                ],
            )?;
        }
    }
    w.tx.execute("INSERT INTO approval_heads VALUES(?1,?2,?3,?4) ON CONFLICT(scope,record_id) DO UPDATE SET version=excluded.version,status=excluded.status",
        params![w.scope.key(), d.reference.id.as_str(), d.reference.revision.as_str(), match entry.state { ApprovalState::Active=>"active", ApprovalState::Revoked=>"revoked", ApprovalState::Unknown=>"unknown" }])?;
    Ok(())
}
pub(crate) struct StoredApprovals<'a> {
    pub conn: &'a Connection,
    pub scope: &'a Scope,
    pub limits: Limits,
    pub now: u64,
    pub clock: &'a dyn Fn() -> Result<u64, Error>,
    pub head: &'a Head,
}
impl StoredApprovals<'_> {
    pub fn audit_records(
        &self,
        bindings: &[ProfileApproval],
    ) -> Result<Vec<ProtectedApprovalAudit>, Error> {
        let mut refs: Vec<_> = bindings.iter().map(|b| &b.record).collect();
        refs.sort_by(|a, b| (&a.id, &a.revision).cmp(&(&b.id, &b.revision)));
        refs.dedup();
        let mut result = Vec::new();
        for reference in refs {
            let r = match self.record(reference) {
                Ok(r) => r,
                Err(Error::NotFound) => continue,
                Err(e) => return Err(e),
            };
            result.push(ProtectedApprovalAudit {
                definition: ApprovalDefinition {
                    reference: r.reference,
                    approver: r.approver,
                    plan_id: r.plan_id,
                    plan_digest: r.plan_digest,
                    profiles: r.profiles,
                    validity: r.validity,
                    max_uses: r.max_uses,
                },
                state: match r.status {
                    ApprovalStatus::Active => ApprovalState::Active,
                    ApprovalStatus::Revoked => ApprovalState::Revoked,
                    ApprovalStatus::Unknown => ApprovalState::Unknown,
                },
                used: r.used,
                consumption_revision: r.consumption_revision,
            });
        }
        Ok(result)
    }

    pub fn record(&self, reference: &VersionedRef) -> Result<ApprovalRecord, Error> {
        let (bytes, status, used, revision): (Vec<u8>, String, u32, u64) = self.conn.query_row(
            &format!("SELECT {},h.status,u.used,u.revision FROM approvals a
             JOIN approval_heads h ON h.scope=a.scope AND h.record_id=a.record_id AND h.version=a.version
             JOIN approval_usage u ON u.scope=a.scope AND u.record_id=a.record_id AND u.version=a.version
             WHERE a.scope=?1 AND a.record_id=?2 AND a.version=?3", bounded_blob("a.definition", self.limits.max_record_bytes)),
             params![self.scope.key(), reference.id.as_str(), reference.revision.as_str()],
             |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let d: ApprovalDefinition = decode(&bytes, self.limits.max_record_bytes)?;
        if &d.reference != reference {
            return Err(Error::Corrupt);
        }
        Ok(ApprovalRecord {
            reference: d.reference,
            approver: d.approver,
            plan_id: d.plan_id,
            plan_digest: d.plan_digest,
            profiles: d.profiles,
            validity: d.validity,
            status: match status.as_str() {
                "active" => ApprovalStatus::Active,
                "revoked" => ApprovalStatus::Revoked,
                "unknown" => ApprovalStatus::Unknown,
                _ => return Err(Error::Corrupt),
            },
            max_uses: d.max_uses,
            used,
            consumption_revision: revision,
        })
    }
}
impl ApprovalVerifier for StoredApprovals<'_> {
    fn verify(
        &self,
        plan: &FrozenPlan,
        refs: &[VersionedRef],
    ) -> Result<ApprovalFacts, VerificationError> {
        let now = (self.clock)().map_err(|_| VerificationError::Clock)?;
        if now < self.now {
            return Err(VerificationError::Clock);
        }
        if Scope::from_plan(plan) != *self.scope
            || refs.len() > self.limits.max_approvals
            || now >= self.head.until
        {
            return Err(VerificationError::Unavailable);
        }
        let records = refs
            .iter()
            .map(|r| self.record(r).map_err(|_| VerificationError::Unavailable))
            .collect::<Result<_, _>>()?;
        Ok(ApprovalFacts {
            authority: self.scope.authority.clone(),
            policy: plan.spec().policy.clone(),
            verification_revision: self.head.approval.clone(),
            now_unix_ms: now,
            fresh_until_unix_ms: self.head.until,
            records,
        })
    }
}
pub(crate) fn check_gate(
    w: &Write<'_>,
    plan: &FrozenPlan,
    attempt: &AttemptId,
    bindings: &[ProfileApproval],
    gate: &AdmissionGate,
    h: &Head,
) -> bool {
    use execution_admission::DecisionOutcome;
    use execution_approval::ApprovalOutcome;
    let a = &gate.admission;
    let p = &gate.approval;
    if a.plan_id() != &plan.spec().plan_id
        || a.plan_digest() != plan.digest()
        || a.attempt_id() != attempt
        || !a
            .validity()
            .is_some_and(|v| v.is_current(w.now, &h.authorization))
        || !p.valid_for_commit(plan, attempt, w.now, &h.authorization)
        || w.now >= h.until
        || a.validity() != p.admission_validity()
    {
        return false;
    }
    match (a.outcome(), p.outcome()) {
        (DecisionOutcome::Allowed, ApprovalOutcome::NotRequired) => {
            p.consumptions().is_empty() && bindings.is_empty()
        }
        (DecisionOutcome::ApprovalRequired { profiles }, ApprovalOutcome::Satisfied) => {
            profiles.len() == p.bindings().len()
                && bindings.len() == p.bindings().len()
                && profiles
                    .iter()
                    .all(|r| p.bindings().iter().any(|b| &b.profile == r))
                && bindings.iter().all(|b| p.bindings().contains(b))
                && p.bindings().iter().all(|b| bindings.contains(b))
        }
        _ => false,
    }
}
pub(crate) fn consume(
    w: &Write<'_>,
    plan: &FrozenPlan,
    attempt: &AttemptId,
    gate: &AdmissionGate,
    h: &Head,
) -> Result<Vec<ConsumptionAudit>, Error> {
    let provider = StoredApprovals {
        conn: &w.tx,
        scope: &w.scope,
        limits: w.limits,
        now: w.now,
        clock: &|| Ok(w.now),
        head: h,
    };
    let mut audits = Vec::new();
    for intent in gate.approval.consumptions() {
        let record = provider.record(intent.approval())?;
        if record.status != ApprovalStatus::Active
            || record.plan_id != plan.spec().plan_id
            || &record.plan_digest != plan.digest()
            || record.used != intent.expected_uses()
            || record.consumption_revision != intent.expected_consumption_revision()
            || record.used >= record.max_uses
            || record.validity.not_before_unix_ms > w.now
            || w.now >= record.validity.expires_at_unix_ms
            || intent.verification_revision() != &h.approval
            || intent.plan_id() != &plan.spec().plan_id
            || intent.plan_digest() != plan.digest()
            || intent.attempt_id() != attempt
            || w.now >= intent.valid_until_unix_ms()
        {
            return Err(Error::Trust);
        }
        let after = record.used.checked_add(1).ok_or(Error::Capacity)?;
        let revision = record
            .consumption_revision
            .checked_add(1)
            .ok_or(Error::Capacity)?;
        let n = w.tx.execute("UPDATE approval_usage SET used=?1,revision=?2 WHERE scope=?3 AND record_id=?4 AND version=?5 AND used=?6 AND revision=?7",
            params![after, integer(revision)?, w.scope.key(), record.reference.id.as_str(), record.reference.revision.as_str(), record.used, integer(record.consumption_revision)?])?;
        if n != 1 {
            return Err(Error::Conflict);
        }
        w.tx.execute(
            "INSERT INTO approval_consumptions VALUES(?1,?2,?3,?4)",
            params![
                attempt.as_str(),
                w.scope.key(),
                record.reference.id.as_str(),
                record.reference.revision.as_str()
            ],
        )?;
        audits.push(ConsumptionAudit {
            approval: record.reference,
            approver: record.approver,
            used_before: record.used,
            used_after: after,
            revision_before: record.consumption_revision,
            revision_after: revision,
            verification_revision: h.approval.clone(),
        });
    }
    Ok(audits)
}

//! Bounded device-owned loss snapshots. ref: rusqlite src/transaction.rs.
use super::*;

const MAX_PENDING_REASONS: i64 = 640;
const MAX_REPORTS: i64 = 64;

pub(super) fn install(c: &Connection) -> Result<(), Error> {
    let tx = c.unchecked_transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS gap_pending(device TEXT NOT NULL,reason TEXT NOT NULL,count INTEGER NOT NULL,first INTEGER NOT NULL,last INTEGER NOT NULL,PRIMARY KEY(device,reason));
        CREATE TABLE IF NOT EXISTS gap_reports(id TEXT PRIMARY KEY,device TEXT NOT NULL,body BLOB NOT NULL,digest TEXT NOT NULL,expires INTEGER NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,due INTEGER NOT NULL DEFAULT 0);")?;
    tx.commit()?;
    Ok(())
}
pub(super) fn reason_key(reason: GapReason) -> Result<String, Error> {
    serde_json::to_value(reason)
        .map_err(|_| Error::Protocol)?
        .as_str()
        .map(str::to_owned)
        .ok_or(Error::Protocol)
}
pub(super) fn record(
    c: &Connection,
    device_id: rss_mdm_agent_wire::DeviceId,
    reason: GapReason,
    count: u64,
    first: i64,
    last: i64,
) -> Result<(), Error> {
    if last == i64::MAX
    {
        return Err(Error::Protocol);
    }
    // Lifetime local evidence is independent of transmission and never cleared by receipt.
    super::gap(c, reason, count, first, last)?;
    if count == 0 {
        return Ok(());
    }
    let key = reason_key(reason)?;
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM gap_pending WHERE device=?1 AND reason=?2)",params![device_id.to_string(),key],|r|r.get(0))?;
    let rows: i64 = c.query_row("SELECT count(*) FROM gap_pending", [], |r| r.get(0))?;
    // If reserved remote-diagnostic space is exhausted, retain only bounded local evidence.
    if !exists && rows >= MAX_PENDING_REASONS {
        return Ok(());
    }
    c.execute("INSERT INTO gap_pending VALUES(?1,?2,?3,?4,?5) ON CONFLICT(device,reason) DO UPDATE SET count=min(9223372036854775807-gap_pending.count,excluded.count)+gap_pending.count,first=min(gap_pending.first,excluded.first),last=max(gap_pending.last,excluded.last)",params![device_id.to_string(),key,count,first,last])?;
    Ok(())
}
pub(super) fn record_object(
    c: &Connection,
    table: &str,
    id: &str,
    reason: GapReason,
    count: u64,
    first: i64,
    last: i64,
) -> Result<(), Error> {
    let owner: String = c.query_row(&format!("SELECT device FROM {table} WHERE id=?1"),[id],|r|r.get(0))?;
    record(c,rss_mdm_agent_wire::DeviceId::parse(&owner).map_err(|_| Error::Storage)?,reason,count,first,last)

}

/// An immutable report restored from private storage; retries cannot recreate its identity.
pub struct PendingGap {
    report: w::TelemetryGapReport,
    body: Vec<u8>,
    attempts: u32,
}
impl PendingGap {
    /// Stable report identity.
    pub fn id(&self) -> Uuid {
        self.report.report_id
    }
    /// Complete body size for shared upload pacing.
    pub fn bytes(&self) -> usize {
        self.body.len()
    }
    /// Whether a preceding send may have committed remotely.
    pub fn attempted(&self) -> bool {
        self.attempts != 0
    }
    pub(crate) fn body(&self) -> &[u8] {
        &self.body
    }
    pub(crate) fn belongs_to(&self, r: &w::BindingProfile) -> bool {
        self.report.device_id == r.device_id
    }
    pub(crate) fn expires(&self) -> i64 {
        self.report.created_at + w::TELEMETRY_GAP_WINDOW_MICROS
    }
}
impl Spool {
    /// Bind new loss to its original device at the moment it is recorded.
    #[allow(
        clippy::too_many_arguments,
        reason = "original device and loss interval are recorded atomically"
    )]
    pub fn gap_for(
        &mut self,
        device_id: rss_mdm_agent_wire::DeviceId,
        reason: GapReason,
        count: u64,
        first: i64,
        last: i64,
    ) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        record(&tx, device_id, reason, count, first, last)?;
        tx.commit()?;
        Ok(())
    }
    /// Atomically freeze at most ten reasons. New loss never modifies an existing report.
    pub fn freeze_gaps(
        &mut self,
        device_id: rss_mdm_agent_wire::DeviceId,
        created_at: i64,
    ) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        // Old identities are terminal locally; never rebind them to the current device.
        tx.execute(
            "DELETE FROM gap_reports WHERE expires<=?1 OR device<>?2",
            params![created_at, device_id.to_string()],
        )?;
        tx.execute(
            "DELETE FROM gap_pending WHERE device<>?1",
            params![device_id.to_string()],
        )?;
        let n: i64 = tx.query_row("SELECT count(*) FROM gap_reports", [], |r| r.get(0))?;
        if n >= MAX_REPORTS {
            tx.commit()?;
            return Ok(());
        }
        let rows=tx.prepare("SELECT reason,count,first,last FROM gap_pending WHERE device=?1 ORDER BY reason")?.query_map(params![device_id.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?)))?.collect::<Result<Vec<_>,_>>()?;
        if rows.is_empty() {
            tx.commit()?;
            return Ok(());
        }
        let gaps = rows
            .into_iter()
            .map(|(reason, count, first, last)| {
                Ok(w::TelemetryGap {
                    reason: serde_json::from_value(serde_json::Value::String(reason))
                        .map_err(|_| Error::Storage)?,
                    count,
                    first,
                    last,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let report = w::TelemetryGapReport {
            report_id: Uuid::new_v4(),
            device_id,
            created_at,
            gaps,
        };
        report.validate()?;
        // A frozen transport body is not a signing canonicalization domain. Preserve
        // exact i64/u64 values; JCS converts integers through f64 and can round them.
        let body = serde_json::to_vec(&report).map_err(|_| Error::Protocol)?;
        tx.execute("INSERT INTO gap_reports(id,device,body,digest,expires) VALUES(?1,?2,?3,?4,?5)",params![report.report_id.to_string(),device_id.to_string(),body,hash(&body),created_at+w::TELEMETRY_GAP_WINDOW_MICROS])?;
        tx.execute(
            "DELETE FROM gap_pending WHERE device=?1",
            params![device_id.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Read due work only for the current device; expiry never renews creation time.
    pub fn pending_gap(
        &self,
        device_id: rss_mdm_agent_wire::DeviceId,
        now: i64,
    ) -> Result<Option<PendingGap>, Error> {
        let row=self.conn.query_row("SELECT id,body,digest,attempts FROM gap_reports WHERE device=?1 AND due<=?2 AND expires>?3 ORDER BY rowid LIMIT 1",params![device_id.to_string(),now/1_000_000,now],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,String>(2)?,r.get::<_,u32>(3)?))).optional()?;
        row.map(|(id, body, digest, attempts)| {
            if body.len() > w::TELEMETRY_GAP_MAX_BYTES || hash(&body) != digest {
                return Err(Error::Storage);
            }
            let report: w::TelemetryGapReport = decode(&body)?;
            report.validate()?;
            if report.report_id.to_string() != id
                || report.device_id != device_id
            {
                return Err(Error::Storage);
            }
            Ok(PendingGap {
                report,
                body,
                attempts,
            })
        })
        .transpose()
    }
    /// Commit uncertain-attempt state before network I/O.
    pub fn attempting_gap(&mut self, p: &PendingGap, now: i64) -> Result<(), Error> {
        self.conn.execute(
            "UPDATE gap_reports SET attempts=min(attempts+1,31),due=?2 WHERE id=?1 AND digest=?3",
            params![p.id().to_string(), now.saturating_add(2), hash(p.body())],
        )?;
        Ok(())
    }
    /// Preserve the report and respect bounded server Retry-After.
    pub fn retry_gap(&mut self, p: &PendingGap, now: i64, delay: Option<u64>) -> Result<(), Error> {
        let backoff = (1u64 << p.attempts.min(10)).min(900) + u64::from(p.id().as_bytes()[0]) % 7;
        self.conn.execute(
            "UPDATE gap_reports SET due=?2 WHERE id=?1 AND digest=?3",
            params![
                p.id().to_string(),
                now.saturating_add(delay.unwrap_or(0).max(backoff).min(86400) as i64),
                hash(p.body())
            ],
        )?;
        Ok(())
    }
    /// Delete only the exact report confirmed by a matching durable receipt.
    pub fn acknowledge_gap(
        &mut self,
        p: &PendingGap,
        r: &w::TelemetryGapReceipt,
    ) -> Result<(), Error> {
        if r.report_id != p.id() || r.received_at < 0 || r.received_at == i64::MAX {
            return Err(Error::Protocol);
        }
        self.reject_gap(p)
    }
    /// Terminal rejection retains lifetime local evidence and produces no further loss report.
    pub fn reject_gap(&mut self, p: &PendingGap) -> Result<(), Error> {
        self.conn.execute(
            "DELETE FROM gap_reports WHERE id=?1 AND digest=?2",
            params![p.id().to_string(), hash(p.body())],
        )?;
        Ok(())
    }
}

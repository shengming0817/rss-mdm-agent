use crate::{
    database::bounded_blob,
    journal::{authorize, decode, encode},
    *,
};
use execution_contract::BackendRequest;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
impl Store {
    /// List bounded preparation records without creating another task queue.
    pub(crate) fn sql_backend_requests(
        &self,
        actor: &execution_contract::ActorId,
        host: &impl JournalHost,
    ) -> Result<Vec<BackendRequest>, Error> {
        crate::database::ensure_current(&self.conn, &self.authority, self.limits)?;
        let mut query = self.conn.prepare(
            "SELECT request_id FROM backend_requests WHERE actor=?1 ORDER BY rowid DESC LIMIT ?2",
        )?;
        let ids = query
            .query_map(params![actor.as_str(), self.limits.max_batch], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                let scope = Scope {
                    authority: self.authority.clone(),
                    actor: actor.clone(),
                    request_id: execution_contract::RequestId::new(id)
                        .map_err(|_| Error::Corrupt)?,
                };
                self.sql_backend_request(&scope, host)?
                    .ok_or(Error::Corrupt)
            })
            .collect()
    }
    /// Read a pre-execution intent from the same protected journal and actor namespace.
    pub(crate) fn sql_backend_request(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Option<BackendRequest>, Error> {
        let tx = self.read(scope, Access::ReadResult, None, host)?;
        read(&tx, &scope.interaction_subject().as_str(), self.limits)
    }
    /// Compare-and-append a preparation transition; it creates no execution attempt or grant.
    pub(crate) fn sql_record_backend_request(
        &mut self,
        scope: &Scope,
        expected: Option<&BackendRequest>,
        next: &BackendRequest,
        host: &impl JournalHost,
    ) -> Result<(), Error> {
        self.check_scope(scope)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        authorize(host, Access::Execute, scope, None)?;
        use execution_contract::BackendRequestState as S;
        let transition = match expected.map(|p| p.state) {
            None => matches!(next.state, S::Proposed | S::Selected),
            Some(S::Proposed) => matches!(next.state, S::Selected | S::Cancelled | S::Failed),
            Some(S::Selected) => matches!(next.state, S::Submitting | S::Cancelled | S::Failed),
            Some(S::Submitting) => matches!(next.state, S::Cancelled | S::Failed),
            Some(S::Cancelled | S::Failed) => false,
        };
        if !transition
            || !next.offer.user_initiated
            || matches!(
                next.trigger,
                execution_contract::BackendTrigger::Automatic {}
            )
            || (next.state == S::Failed) != next.failure.is_some() && next.state != S::Cancelled
        {
            return Err(Error::InvalidInput);
        }
        if next.offer.request != scope.request_id
            || next.revision != expected.map_or(1, |p| p.revision.saturating_add(1))
        {
            return Err(Error::InvalidInput);
        }
        let key = scope.interaction_subject();
        let previous = read(&tx, key.as_str(), self.limits)?;
        if previous.as_ref() != expected {
            return Err(Error::Conflict);
        }
        if expected.is_some_and(|p| p.offer != next.offer || p.trigger != next.trigger) {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO backend_requests VALUES(?1,?2,?3,?4) ON CONFLICT(scope) DO UPDATE SET body=excluded.body", params![key.as_str(), scope.actor.as_str(), scope.request_id.as_str(), encode(next, self.limits.max_record_bytes)?])?;
        tx.commit().map_err(|_| Error::OperationCommitUnknown)
    }
}
fn read(
    conn: &rusqlite::Connection,
    key: &str,
    limits: Limits,
) -> Result<Option<BackendRequest>, Error> {
    let body: Option<Vec<u8>> = conn
        .query_row(
            &format!(
                "SELECT {} FROM backend_requests WHERE scope=?1",
                bounded_blob("body", limits.max_record_bytes)
            ),
            [key],
            |r| r.get(0),
        )
        .optional()?;
    body.map(|bytes| decode(&bytes, limits.max_record_bytes))
        .transpose()
}

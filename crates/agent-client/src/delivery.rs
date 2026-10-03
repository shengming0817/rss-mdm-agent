//! Durable communication associations and frozen transport; no execution application access.
use crate::{
    store::{decode, encode, hash},
    wire, Client, Clock, Error, SecretProvider, Start,
};
use execution_contract::{ActorId, AttemptId, Digest, EventId, RequestId};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Binding {
    pub(crate) task: Uuid,
    pub(crate) attempt: Uuid,
    pub(crate) request: RequestId,
    pub(crate) digest: execution_contract::Digest,
    pub(crate) actor: execution_contract::ActorId,
    pub(crate) local_attempt: Option<execution_contract::AttemptId>,
}

/// Immutable coordinates persisted by the communication owner; not execution authority.
#[derive(Clone)]
pub struct Association(Binding);
impl Association {
    /// Exact remote task.
    pub fn task(&self) -> Uuid {
        self.0.task
    }
    /// Exact remote attempt.
    pub fn attempt(&self) -> Uuid {
        self.0.attempt
    }
    /// Original local request.
    pub fn request(&self) -> &RequestId {
        &self.0.request
    }
    /// Original frozen plan digest.
    pub fn digest(&self) -> &Digest {
        &self.0.digest
    }
    /// Original local actor.
    pub fn actor(&self) -> &ActorId {
        &self.0.actor
    }
    /// Journal attempt recorded after admission.
    pub fn local_attempt(&self) -> Option<&AttemptId> {
        self.0.local_attempt.as_ref()
    }
}
/// A persisted immutable result request with its original source events.
pub struct FrozenResult {
    owner: Uuid,
    association: Association,
    key: String,
    request: wire::TaskEventRequest,
    events: Vec<EventId>,
}
impl FrozenResult {
    /// Original source events; this list alone does not prove remote acknowledgement.
    pub fn events(&self) -> &[EventId] {
        &self.events
    }
}
/// A result acknowledged durably by this communication owner.
/// Private construction and no Clone/Deserialize prevent invented confirmation facts.
pub struct ResultAck {
    owner: Uuid,
    association: Association,
    projection: String,
    result: Option<FrozenResult>,
}
impl ResultAck {
    /// Coordinates of the original acknowledged execution association.
    pub fn association(&self) -> &Association {
        &self.association
    }
    /// Original frozen source events, when the result still awaits local confirmation.
    /// A retained terminal projection authorizes no invented source event list.
    pub fn events(&self) -> &[EventId] {
        self.result.as_ref().map_or(&[], |result| result.events())
    }
}
/// Frozen cancellation for an offer the trusted host has proven never entered its journal.
pub struct FrozenAbandonment {
    owner: Uuid,
    task: Uuid,
    attempt: Uuid,
    key: String,
    request: wire::TaskEventRequest,
}
/// Durable cancellation acknowledgement; not proof that a local execution is absent.
pub struct AbandonmentAck(FrozenAbandonment);
impl<S: SecretProvider, C: Clock> Client<S, C> {
    /// Read original coordinates without granting Start or execution authority.
    pub fn association(&self, task: Uuid) -> Result<Option<Association>, Error> {
        self.store
            .get::<Binding>(&format!("binding/{task}"))
            .map(|v| v.map(Association))
    }
    fn check_association(&self, association: &Association) -> Result<(), Error> {
        let current = self
            .association(association.task())?
            .ok_or(Error::Conflict)?;
        if current.0 != association.0 {
            return Err(Error::Conflict);
        }
        let offer = self.retained_offer(association.task())?;
        if offer.attempt_id() != association.attempt() {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    /// Reserve exact host coordinates before local admission, only with a valid original Start.
    pub fn reserve_association(
        &mut self,
        start: &Start,
        request: &RequestId,
        digest: &Digest,
        actor: &ActorId,
    ) -> Result<Association, Error> {
        self.validate_start(start)?;
        let task = start.payload().task_id();
        let candidate = Binding {
            task,
            attempt: start.payload().attempt_id(),
            request: request.clone(),
            digest: digest.clone(),
            actor: actor.clone(),
            local_attempt: None,
        };
        if let Some(old) = self.association(task)? {
            if old.0.attempt != candidate.attempt
                || old.0.request != candidate.request
                || old.0.digest != candidate.digest
                || old.0.actor != candidate.actor
            {
                return Err(Error::Conflict);
            }
            return Ok(old);
        }
        self.store.put(&format!("binding/{task}"), &candidate)?;
        Ok(Association(candidate))
    }
    /// Complete an interrupted association handoff; this cannot dispatch execution.
    pub fn bind_local_attempt(
        &mut self,
        association: &Association,
        attempt: Option<&AttemptId>,
    ) -> Result<Association, Error> {
        self.check_association(association)?;
        if association.local_attempt().is_some() && association.local_attempt() != attempt {
            return Err(Error::Conflict);
        }
        let mut binding = association.0.clone();
        binding.local_attempt = attempt.cloned();
        self.store
            .put(&format!("binding/{}", binding.task), &binding)?;
        Ok(Association(binding))
    }
    /// Read historical verified input for recovery or reporting; it grants no fresh Start.
    pub fn retained_offer(&self, task: Uuid) -> Result<crate::Offer, Error> {
        self.stored_offer(task)
    }
    /// Check current communication identity and reliable time before preparing new transport work.
    pub fn check_delivery(&self) -> Result<(), Error> {
        self.active()?;
        self.now()?;
        Ok(())
    }
    /// Recover the exact frozen request after interruption, without projecting newer evidence.
    pub fn pending_result(&self, association: &Association) -> Result<Option<FrozenResult>, Error> {
        self.check_association(association)?;
        let row: Option<(String, Vec<u8>, String)> = self.store.conn.query_row(
            "SELECT key,CASE WHEN typeof(body)='blob' AND length(body)<=1114112 THEN body END,source FROM requests WHERE task=?1 AND source IS NOT NULL ORDER BY rowid LIMIT 1",
            [association.task().to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        row.map(|(key, body, source)| {
            let request: wire::TaskEventRequest = decode(&body)?;
            let events: Vec<EventId> = decode(source.as_bytes())?;
            if request.attempt_id() != association.attempt() || events.is_empty() {
                return Err(Error::Conflict);
            }
            Ok(FrozenResult {
                owner: self.delivery_owner,
                association: association.clone(),
                key,
                request,
                events,
            })
        })
        .transpose()
    }
    /// Freeze one terminal wire projection and its exact original source identities.
    pub fn freeze_result(
        &mut self,
        association: &Association,
        candidate: &EventId,
        events: &[EventId],
        event: wire::TaskEvent,
    ) -> Result<FrozenResult, Error> {
        self.check_delivery()?;
        self.check_association(association)?;
        if events.is_empty()
            || !events.contains(candidate)
            || events.len() > 64
            || events
                .iter()
                .enumerate()
                .any(|(i, v)| events[..i].contains(v))
            || !matches!(
                event,
                wire::TaskEvent::Result(_)
                    | wire::TaskEvent::SoftwareResult(_)
                    | wire::TaskEvent::Cancelled
            )
        {
            return Err(Error::Protocol);
        }
        if self.acknowledged_result(association)?.is_some()
            || self.pending_result(association)?.is_some()
        {
            return Err(Error::Conflict);
        }
        let key = format!("result/{}/{}", association.task(), candidate.as_str());
        let source = String::from_utf8(encode(&events)?).map_err(|_| Error::Protocol)?;
        let request = self.delivery_request(
            &key,
            association.task(),
            association.attempt(),
            event,
            &source,
        )?;
        Ok(FrozenResult {
            owner: self.delivery_owner,
            association: association.clone(),
            key,
            request,
            events: events.to_vec(),
        })
    }
    fn check_result(&self, result: &FrozenResult) -> Result<bool, Error> {
        if result.owner != self.delivery_owner {
            return Err(Error::Conflict);
        }
        self.check_association(&result.association)?;
        let (body, source, accepted): (Vec<u8>, String, bool) = self.store.conn.query_row(
            "SELECT body,source,accepted FROM requests WHERE key=?1 AND task=?2",
            params![result.key, result.association.task().to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if body != encode(&result.request)? || source.as_bytes() != encode(&result.events)? {
            return Err(Error::Conflict);
        }
        Ok(accepted)
    }
    /// Send only transport work and return evidence after the ACK/projection transaction commits.
    pub async fn send_result(&mut self, result: FrozenResult) -> Result<ResultAck, Error> {
        if !self.check_result(&result)? {
            self.check_delivery()?;
            self.send_output_chunks(result.association.task(), &result.request)
                .await?;
            let ack = self
                .send_event(result.association.task(), &result.request)
                .await?;
            if ack.permit().is_some() {
                return Err(Error::Protocol);
            }
            let tx = self.store.conn.unchecked_transaction()?;
            if tx.execute(
                "UPDATE requests SET accepted=1 WHERE key=?1 AND task=?2",
                params![result.key, result.association.task().to_string()],
            )? != 1
            {
                return Err(Error::Conflict);
            }
            tx.execute(
                "INSERT INTO state VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
                params![
                    format!("projection/{}", result.association.task()),
                    encode(&hash(&encode(result.request.event())?))?
                ],
            )?;
            tx.commit()?;
        }
        let projection = hash(&encode(result.request.event())?);
        let ack = ResultAck {
            owner: self.delivery_owner,
            association: result.association.clone(),
            projection,
            result: Some(result),
        };
        self.validate_result_ack(&ack)?;
        Ok(ack)
    }
    /// Restore a retained terminal projection; it never represents a new HTTP result.
    pub fn acknowledged_result(
        &self,
        association: &Association,
    ) -> Result<Option<ResultAck>, Error> {
        self.check_association(association)?;
        let projection: Option<String> = self
            .store
            .get(&format!("projection/{}", association.task()))?;
        projection
            .map(|projection| {
                if projection.len() != 64 || !projection.bytes().all(|v| v.is_ascii_hexdigit()) {
                    return Err(Error::Storage);
                }
                Ok(ResultAck {
                    owner: self.delivery_owner,
                    association: association.clone(),
                    projection,
                    result: None,
                })
            })
            .transpose()
    }
    /// Recheck durable acknowledgement before the trusted backend confirms original journal facts.
    pub fn validate_result_ack(&self, ack: &ResultAck) -> Result<(), Error> {
        if ack.owner != self.delivery_owner {
            return Err(Error::Conflict);
        }
        self.check_association(&ack.association)?;
        if self
            .store
            .get::<String>(&format!("projection/{}", ack.association.task()))?
            .as_ref()
            != Some(&ack.projection)
        {
            return Err(Error::Conflict);
        }
        if let Some(result) = &ack.result {
            if !self.check_result(result)? {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    }
    /// Retire only the frozen transport after the host confirmed every original source event.
    pub fn retire_result(&mut self, ack: ResultAck) -> Result<(), Error> {
        self.validate_result_ack(&ack)?;
        let result = ack.result.ok_or(Error::Conflict)?;
        let prefix = format!(
            "chunk/{}/{}/",
            result.association.task(),
            result.association.attempt()
        );
        let tx = self.store.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM requests WHERE task=?1 AND substr(key,1,length(?2))=?2",
            params![result.association.task().to_string(), prefix],
        )?;
        tx.execute("DELETE FROM requests WHERE key=?1", [result.key])?;
        tx.commit()?;
        Ok(())
    }
    /// Release settled communication state using a durable terminal result, after journal checks.
    pub fn release_result(&mut self, ack: ResultAck) -> Result<(), Error> {
        self.validate_result_ack(&ack)?;
        let pending: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM requests WHERE task=?1 AND source IS NOT NULL)",
            [ack.association.task().to_string()],
            |r| r.get(0),
        )?;
        if pending {
            return Err(Error::Conflict);
        }
        self.release(ack.association.task())
    }
    /// Freeze cancellation only; the backend must independently prove journal absence.
    pub fn freeze_abandonment(&mut self, task: Uuid) -> Result<FrozenAbandonment, Error> {
        let offer = self.retained_offer(task)?;
        let attempt = offer.attempt_id();
        let key = format!("abandon/{task}/{attempt}");
        let request = self.event_request(&key, task, attempt, wire::TaskEvent::Cancelled, None)?;
        Ok(FrozenAbandonment {
            owner: self.delivery_owner,
            task,
            attempt,
            key,
            request,
        })
    }
    fn check_abandonment(&self, pending: &FrozenAbandonment) -> Result<bool, Error> {
        if pending.owner != self.delivery_owner
            || self.retained_offer(pending.task)?.attempt_id() != pending.attempt
        {
            return Err(Error::Conflict);
        }
        let (body, accepted): (Vec<u8>, bool) = self.store.conn.query_row(
            "SELECT body,accepted FROM requests WHERE key=?1 AND task=?2",
            params![pending.key, pending.task.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if body != encode(&pending.request)? {
            return Err(Error::Conflict);
        }
        Ok(accepted)
    }
    /// Return a cancellation receipt only after the original ACK is durably stored.
    pub async fn send_abandonment(
        &mut self,
        pending: FrozenAbandonment,
    ) -> Result<AbandonmentAck, Error> {
        if !self.check_abandonment(&pending)? {
            let ack = self.send_event(pending.task, &pending.request).await?;
            if ack.permit().is_some() {
                return Err(Error::Protocol);
            }
            if self.store.conn.execute(
                "UPDATE requests SET accepted=1 WHERE key=?1 AND task=?2",
                params![pending.key, pending.task.to_string()],
            )? != 1
            {
                return Err(Error::Conflict);
            }
        }
        Ok(AbandonmentAck(pending))
    }
    /// Release the same acknowledged cancellation after a second authoritative absence check.
    pub fn release_abandonment(&mut self, ack: AbandonmentAck) -> Result<(), Error> {
        if !self.check_abandonment(&ack.0)? {
            return Err(Error::Conflict);
        }
        self.release(ack.0.task)
    }
}

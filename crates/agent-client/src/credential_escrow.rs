//! Nonterminal encrypted delivery. Only a committed acknowledgement permits native cleanup.
use crate::{wire, Client, Clock, Error, IdentityProvider};
use rusqlite::params;
use uuid::Uuid;
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Freeze encrypted materials in the existing communication journal and replay the same
    /// operation until the server acknowledges durable escrow. Does not retire execution.
    pub async fn escrow_credentials(
        &mut self,
        task: Uuid,
        escrow: wire::CredentialEscrow,
    ) -> Result<Vec<wire::CredentialReceipt>, Error> {
        self.active()?;
        escrow.validate()?;
        let offer = self.retained_offer(task)?;
        let (definition_digest, attempt_id) = match offer.payload() {
            wire::TaskPayload::Credential(spec) => (spec.definition_digest, spec.attempt_id),
            wire::TaskPayload::DeviceAccess(spec) => (spec.definition_digest, spec.attempt_id),
            _ => return Err(Error::Protocol),
        };
        if escrow.definition_digest != definition_digest || self.association(task)?.is_none() {
            return Err(Error::Untrusted);
        }
        let key = format!("credential-escrow/{task}/{}", attempt_id);
        let subjects: Vec<_> = escrow.materials.iter().map(|m| m.subject.clone()).collect();
        let receipt_key = format!("{key}/receipt");
        let request = self.event_request(
            &key,
            task,
            attempt_id,
            wire::TaskEvent::CredentialEscrow(escrow),
            None,
        )?;
        let accepted: bool = self.store.conn.query_row(
            "SELECT accepted FROM requests WHERE key=?1",
            [&key],
            |r| r.get(0),
        )?;
        if !accepted {
            let ack = self.send_event(task, &request).await?;
            if ack.permit().is_some()
                || ack.credentials().len() != subjects.len()
                || subjects
                    .iter()
                    .any(|s| !ack.credentials().iter().any(|r| &r.subject == s))
            {
                return Err(Error::Protocol);
            }
            let tx = self.store.conn.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO state VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
                params![receipt_key, crate::store::encode(&ack.credentials())?],
            )?;
            tx.execute(
                "UPDATE requests SET accepted=1 WHERE key=?1 AND task=?2",
                params![key, task.to_string()],
            )?;
            tx.commit()?;
        }
        self.store.get(&receipt_key)?.ok_or(Error::Storage)
    }
}

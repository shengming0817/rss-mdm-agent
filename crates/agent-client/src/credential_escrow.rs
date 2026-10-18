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
    ) -> Result<(), Error> {
        self.active()?;
        escrow.validate()?;
        let offer = self.retained_offer(task)?;
        let wire::TaskPayload::Credential(spec) = offer.payload() else {
            return Err(Error::Protocol);
        };
        if escrow.definition_digest != spec.definition_digest || self.association(task)?.is_none() {
            return Err(Error::Untrusted);
        }
        let key = format!("credential-escrow/{task}/{}", spec.attempt_id);
        let request = self.event_request(
            &key,
            task,
            spec.attempt_id,
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
            if ack.permit().is_some() {
                return Err(Error::Protocol);
            }
            self.store.conn.execute(
                "UPDATE requests SET accepted=1 WHERE key=?1 AND task=?2",
                params![key, task.to_string()],
            )?;
        }
        Ok(())
    }
}

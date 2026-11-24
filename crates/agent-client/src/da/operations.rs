//! Windows delivery on the DA listener. Its single writer retains the offer and exact events.
use super::*;
use crate::http as h;
use rss_mdm_agent_wire::runtime as w;
use serde::de::DeserializeOwned;

/// An mTLS session bound to the current registration and its single writer.
pub struct Operations<'a> {
    http: reqwest::Client,
    origin: &'a url::Url,
    db: &'a rusqlite::Connection,
    registration: wire::Registration,
}
impl<S: Credentials, C: Clock> Client<S, C> {
    /// Open the current credential for device operations. Never use the Agent identity here.
    pub fn operations(&self) -> Result<Operations<'_>, Error> {
        let active = self.active()?.ok_or(Error::Identity)?;
        let trust = VerifiedTrust::device(
            read(&self.journal.db, "pki")?.ok_or(Error::Identity)?,
            self.backend.instance,
            self.backend.tenant,
            pki::Purpose::DaDevice,
            self.now()?,
        )?;
        let (_, _, device) = crate::tls::device_validity(
            &active.chain,
            &active.key,
            &self.secrets,
            self.now()?,
            &trust,
        )?;
        if device != active.receipt.device {
            return Err(Error::Identity);
        }
        Ok(Operations {
            http: self.http(Some((&active.key, &active.chain)))?,
            origin: &self.origin,
            db: &self.journal.db,
            registration: active.receipt,
        })
    }

    /// Recover an exact administrative withdrawal, including when this key is denied on normal routes.
    pub async fn revocation(&self) -> Result<Option<wire::RevocationReceipt>, Error> {
        let Some(active) = self.active()? else {
            return Ok(None);
        };
        if let Some(original) = read::<wire::RevocationReceipt>(&self.journal.db, "revocation")? {
            let r = &active.receipt;
            if (
                original.service_id,
                original.tenant,
                original.device,
                original.credential,
            ) == (r.service_id, r.tenant, r.device, r.credential)
            {
                return Ok(Some(original));
            }
        }
        let http = self.http(Some((&active.key, &active.chain)))?;
        let response = Call::<h::DaRevocation>::new(&http, &self.origin, &[], None, None)?
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        if matches!(
            response.status(),
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::UNAUTHORIZED
        ) {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(Error::Denied);
        }
        let receipt: wire::RevocationReceipt =
            crate::store::decode(&crate::client::bounded(response, 4096).await?)?;
        let r = active.receipt;
        if receipt.operation_id.is_nil()
            || (
                receipt.service_id,
                receipt.tenant,
                receipt.device,
                receipt.credential,
            ) != (r.service_id, r.tenant, r.device, r.credential)
        {
            return Err(Error::Identity);
        }
        write(&self.journal.db, "revocation", &receipt)?;
        Ok(Some(receipt))
    }

    /// Called only after the native owner durably fenced this registration.
    pub fn retire(&self, receipt: &wire::RevocationReceipt) -> Result<(), Error> {
        let active = self.active()?.ok_or(Error::Identity)?;
        if read::<wire::RevocationReceipt>(&self.journal.db, "revocation")?.as_ref()
            != Some(receipt)
            || (
                receipt.service_id,
                receipt.tenant,
                receipt.device,
                receipt.credential,
            ) != (
                active.receipt.service_id,
                active.receipt.tenant,
                active.receipt.device,
                active.receipt.credential,
            )
        {
            return Err(Error::Identity);
        }
        let tx = self.journal.db.unchecked_transaction()?;
        let mut release: Vec<(String, bool)> = read(&tx, "release")?.unwrap_or_default();
        release.push((active.key, true));
        write(&tx, "release", &release)?;
        tx.execute(
            "DELETE FROM state WHERE name IN ('active','applied-proof','effective-digest')",
            [],
        )?;
        tx.commit()?;
        self.release_settled()
    }
}
impl Operations<'_> {
    /// Identity actually authenticated by this device session.
    pub fn registration(&self) -> &wire::Registration {
        &self.registration
    }

    async fn send<E: Endpoint>(
        &self,
        params: &[&str],
        input: Option<&E::Input>,
    ) -> Result<E::Output, Error> {
        let response = Call::<E>::new(&self.http, self.origin, params, None, input)?
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 | 403 => Error::Denied,
                404 => Error::NotFound,
                409 => Error::Conflict,
                _ => Error::Unavailable,
            });
        }
        crate::store::decode(&crate::client::bounded(response, 262144).await?)
    }
    fn key(&self, item: &str) -> String {
        format!("windows/{}/{item}", self.registration.device)
    }
    fn save<T: Serialize>(&self, item: &str, value: &T) -> Result<(), Error> {
        write(self.db, &self.key(item), value)
    }
    fn load<T: DeserializeOwned>(&self, item: &str) -> Result<Option<T>, Error> {
        read(self.db, &self.key(item))
    }
    fn finish_replay<T: Serialize, U>(
        &self,
        key: &str,
        event: &T,
        result: Result<U, Error>,
    ) -> Result<(), Error> {
        match result {
            Ok(_) => Ok(()),
            Err(reason @ (Error::Denied | Error::Conflict | Error::NotFound)) => {
                // Keep the original rejected event. A withdrawn attempt must not starve cleanup.
                let tx = self.db.unchecked_transaction()?;
                let digest = crate::store::hash(&crate::store::encode(event)?);
                write(
                    &tx,
                    &self.key(&format!("rejected/{key}/{digest}")),
                    &(event, reason),
                )?;
                tx.execute("DELETE FROM state WHERE name=?1", [self.key(key)])?;
                tx.commit()?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    /// Journal a server offer before the received event can be sent.
    pub async fn account(&self) -> Result<Option<wire::AccountOffer>, Error> {
        let offer = self.send::<h::DaAccountNext>(&[], None).await?;
        if let Some(o) = &offer {
            if [o.operation_id, o.task_id, o.attempt_id, o.binding_id]
                .iter()
                .any(Uuid::is_nil)
            {
                return Err(Error::Protocol);
            }
            let key = format!("offer/{}/{}", o.task_id, o.attempt_id);
            if self
                .load::<wire::AccountOffer>(&key)?
                .is_some_and(|old| old != *o)
            {
                return Err(Error::Conflict);
            }
            self.save(&key, o)?;
        }
        Ok(offer)
    }
    /// Settle the exact device event, retaining its key even across lost responses.
    pub async fn account_event(
        &self,
        o: &wire::AccountOffer,
        event: wire::AccountObservation,
    ) -> Result<wire::AccountSettlement, Error> {
        if self
            .load::<wire::AccountOffer>(&format!("offer/{}/{}", o.task_id, o.attempt_id))?
            .as_ref()
            != Some(o)
        {
            return Err(Error::Identity);
        }
        let key = format!(
            "event/{}/{}/{}",
            o.task_id,
            o.attempt_id,
            crate::store::hash(&crate::store::encode(&event)?)
        );
        let event_id = self.load::<Uuid>(&key)?.unwrap_or_else(Uuid::new_v4);
        self.save(&key, &event_id)?;
        let body = wire::AccountEvent {
            event_id,
            attempt_id: o.attempt_id,
            definition_digest: o.definition_digest,
            event,
        };
        self.save("account-pending", &(o.task_id, &body))?;
        self.send_account(o.task_id, &body).await
    }
    async fn send_account(
        &self,
        task: Uuid,
        body: &wire::AccountEvent,
    ) -> Result<wire::AccountSettlement, Error> {
        let ack = self
            .send::<h::DaAccountEvent>(&[&task.to_string()], Some(body))
            .await?;
        if ack.event_id != body.event_id {
            return Err(Error::Protocol);
        }
        let tx = self.db.unchecked_transaction()?;
        write(
            &tx,
            &self.key(&format!("account-receipt/{}", body.event_id)),
            &ack,
        )?;
        tx.execute(
            "DELETE FROM state WHERE name=?1",
            [self.key("account-pending")],
        )?;
        tx.commit()?;
        Ok(ack)
    }
    /// A previously sent result can settle even after it disappears from the claim queue.
    pub async fn replay_account(&self) -> Result<(), Error> {
        if let Some((task, body)) = self.load::<(Uuid, wire::AccountEvent)>("account-pending")? {
            self.finish_replay(
                "account-pending",
                &(task, &body),
                self.send_account(task, &body).await,
            )?;
        }
        Ok(())
    }
    /// Retrieve the exact authorized RSS recovery operation; other worker capabilities are excluded.
    pub async fn credential(&self) -> Result<Option<w::CredentialTaskSpec>, Error> {
        let task = self.send::<h::DaCredentialNext>(&[], None).await?;
        if let Some(t) = &task {
            t.validate()?;
            if t.tenant_id != self.registration.tenant
                || t.device_id != self.registration.device
                || t.platform != w::TaskPlatform::Windows
                || t.architecture != w::TaskArchitecture::X86_64
                || t.permit != w::TaskPermit::Start
                || !matches!(
                    t.operation,
                    w::CredentialOperation::RotateRssLogin { .. }
                        | w::CredentialOperation::VerifyRssLogin { .. }
                )
            {
                return Err(Error::Untrusted);
            }
            self.save(&format!("credential/{}/{}", t.task_id, t.attempt_id), t)?;
        }
        Ok(task)
    }
    /// Persist the exact request before attempting settlement.
    pub async fn credential_event(
        &self,
        task: &w::CredentialTaskSpec,
        event: wire::CredentialEvent,
    ) -> Result<wire::CredentialSettlement, Error> {
        if self
            .load::<w::CredentialTaskSpec>(&format!(
                "credential/{}/{}",
                task.task_id, task.attempt_id
            ))?
            .as_ref()
            != Some(task)
        {
            return Err(Error::Identity);
        }
        self.save("credential-pending", &(task.task_id, &event))?;
        self.send_credential(task.task_id, &event).await
    }
    async fn send_credential(
        &self,
        id: Uuid,
        event: &wire::CredentialEvent,
    ) -> Result<wire::CredentialSettlement, Error> {
        let ack = self
            .send::<h::DaCredentialEvent>(&[&id.to_string()], Some(event))
            .await?;
        if ack.operation_id != id {
            return Err(Error::Protocol);
        }
        let tx = self.db.unchecked_transaction()?;
        write(&tx, &self.key(&format!("credential-receipt/{id}")), &ack)?;
        tx.execute(
            "DELETE FROM state WHERE name=?1",
            [self.key("credential-pending")],
        )?;
        tx.commit()?;
        Ok(ack)
    }
    /// Reconcile an uncertain operation before polling for another.
    pub async fn replay_credential(&self) -> Result<(), Error> {
        if let Some((id, event)) =
            self.load::<(Uuid, wire::CredentialEvent)>("credential-pending")?
        {
            self.finish_replay(
                "credential-pending",
                &(id, &event),
                self.send_credential(id, &event).await,
            )?;
        }
        Ok(())
    }
    /// Acknowledge one original, durably recorded offline use.
    pub async fn recovery_use(&self, usage: &w::AccessRecoveryUse) -> Result<(), Error> {
        usage.validate()?;
        if !self
            .send::<h::DaAccessUse>(&[], Some(usage))
            .await?
            .accepted
        {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}

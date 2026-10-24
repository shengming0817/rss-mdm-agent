//! Device Access uses the existing device-authenticated runtime and event journal.
use crate::{store::encode, wire, Client, Clock, Error, IdentityProvider};
use reqwest::Method;
use uuid::Uuid;
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Upload a durable offline usage fact; transport acknowledgement never grants login access.
    pub async fn access_recovery_use(&self, input: &wire::AccessRecoveryUse) -> Result<(), Error> {
        input.validate()?;
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Ack {
            accepted: bool,
        }
        let ack: Ack = self
            .self_service_json(
                None,
                Method::POST,
                "device-access/recovery-uses",
                Some(encode(input)?),
            )
            .await?;
        if !ack.accepted {
            return Err(Error::Protocol);
        }
        Ok(())
    }
    /// Employee policy discovery; the native host supplies its retained employee proof.
    pub async fn access_directory(
        &self,
        proof: &str,
        cursor: Option<&str>,
    ) -> Result<wire::AccessDirectory, Error> {
        let mut path = "device-access?limit=100".to_owned();
        if let Some(cursor) = cursor {
            if cursor.is_empty() || cursor.len() > 4096 || cursor.chars().any(char::is_control) {
                return Err(Error::Protocol);
            }
            let query = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("cursor", cursor)
                .finish();
            path.push('&');
            path.push_str(&query);
        }
        self.self_service_json(Some(proof), Method::GET, &path, None)
            .await
    }
    /// Begin first self-enrollment. The server chooses the account from native-session evidence.
    pub async fn access_begin(
        &self,
        proof: &str,
        input: &wire::AccessBegin,
    ) -> Result<wire::AccessChallenge, Error> {
        self.self_service_json(
            Some(proof),
            Method::POST,
            "device-access/begin",
            Some(encode(input)?),
        )
        .await
    }
    /// Confirm only after the native host rechecks the exact live console login.
    pub async fn access_confirm(
        &self,
        proof: &str,
        input: &wire::AccessConfirm,
    ) -> Result<wire::AccessBindingStatus, Error> {
        input.local_session.validate()?;
        self.self_service_json(
            Some(proof),
            Method::POST,
            "device-access/confirm",
            Some(encode(input)?),
        )
        .await
    }
    /// Persist and replay an actual native account observation without claiming Applied.
    pub async fn access_prepared(
        &mut self,
        task: Uuid,
        prepared: wire::AccessPrepared,
    ) -> Result<(), Error> {
        self.active()?;
        let offer = self.retained_offer(task)?;
        let wire::TaskPayload::DeviceAccess(spec) = offer.payload() else {
            return Err(Error::Protocol);
        };
        prepared.account.validate()?;
        if prepared.binding_id != spec.binding_id
            || prepared.binding_generation != spec.binding_generation
            || prepared.revision != spec.revision
            || prepared.definition_digest != spec.definition_digest
            || prepared.account.platform() != spec.platform
            || self.association(task)?.is_none()
        {
            return Err(Error::Untrusted);
        }
        let key = format!("access-prepared/{task}/{}", spec.attempt_id);
        let request = self.event_request(
            &key,
            task,
            spec.attempt_id,
            wire::TaskEvent::DeviceAccessPrepared(prepared),
            None,
        )?;
        let ack = self.send_event(task, &request).await?;
        if ack.permit().is_some() || !ack.credentials().is_empty() {
            return Err(Error::Protocol);
        }
        self.store
            .conn
            .execute("UPDATE requests SET accepted=1 WHERE key=?1", [key])?;
        Ok(())
    }
}

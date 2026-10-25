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

/// Completion uses the actual platform and mechanism frozen into the signed task.
pub fn device_access_complete(task: &wire::AccessTaskSpec, effect: &wire::AccessEffect) -> bool {
    match &task.action {
        wire::AccessAction::Apply { configuration, .. }
        | wire::AccessAction::Create { configuration, .. } => {
            effect.complete(configuration, task.platform)
        }
        wire::AccessAction::Unbind { disposition, .. } => {
            let configuration = wire::AccessConfiguration {
                mac: None,
                windows: if matches!(task.windows, Some(wire::AccessWindowsPlan::Whfb { .. })) {
                    wire::AccessWindowsConfiguration::Whfb
                } else {
                    wire::AccessWindowsConfiguration::Disabled
                },
                self_enrollment: false,
                enabled: false,
                role: None,
            };
            matches!(effect,wire::AccessEffect::Unbound{disposition:actual}|wire::AccessEffect::WhfbRetired{disposition:actual,..} if actual==disposition)
                && effect.complete(&configuration, task.platform)
        }
    }
}

pub(crate) fn validate_report(
    task: &wire::AccessTaskSpec,
    report: &wire::AccessResult,
) -> Result<(), Error> {
    report.validate_for(task.platform)?;
    if report.binding_id != task.binding_id
        || report.binding_generation != task.binding_generation
        || report.revision != task.revision
        || report.definition_digest != task.definition_digest
    {
        return Err(Error::Untrusted);
    }
    let account = match &report.effect {
        wire::AccessEffect::Applied { account, .. }
        | wire::AccessEffect::WhfbRetired { account, .. } => Some(account),
        _ => None,
    };
    if let Some(account) = account {
        match &task.action {
            wire::AccessAction::Apply {
                account: expected, ..
            }
            | wire::AccessAction::Unbind {
                account: expected, ..
            } if account != expected => return Err(Error::Untrusted),
            _ => (),
        }
    }
    Ok(())
}
fn observation_digest(event: &wire::TaskEvent) -> Result<String, Error> {
    let mut event = event.clone();
    if let wire::TaskEvent::DeviceAccessResult(report) = &mut event {
        match &mut report.effect {
            wire::AccessEffect::Applied {
                windows: Some(wire::AccessWindowsFacts::Whfb(f)),
                ..
            }
            | wire::AccessEffect::WhfbRetired { facts: f, .. } => f.observed_at = 0,
            _ => (),
        }
    }
    Ok(crate::store::hash(&encode(&event)?))
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Send changed, nonterminal native observations through the existing frozen request journal.
    /// Only one unacknowledged observation is retained; it is replayed before any newer fact.
    pub async fn access_observation(
        &mut self,
        id: Uuid,
        report: wire::AccessResult,
    ) -> Result<(), Error> {
        use rusqlite::{params, OptionalExtension};
        self.check_delivery()?;
        let retained = self.retained_offer(id)?;
        let wire::TaskPayload::DeviceAccess(task) = retained.payload() else {
            return Err(Error::Protocol);
        };
        if !matches!(task.windows, Some(wire::AccessWindowsPlan::Whfb { .. }))
            || self
                .association(id)?
                .is_none_or(|v| v.local_attempt().is_none())
        {
            return Err(Error::Untrusted);
        }
        validate_report(task, &report)?;
        let terminal = device_access_complete(task, &report.effect)
            || matches!(report.effect, wire::AccessEffect::Failed { .. });
        let attempt = task.attempt_id;
        let key = format!("access-observation/{id}");
        let last = format!("access-observation-last/{id}");
        let pending: Option<Vec<u8>> = self
            .store
            .conn
            .query_row(
                "SELECT body FROM requests WHERE key=?1 AND task=?2",
                params![key, id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let event = wire::TaskEvent::DeviceAccessResult(report);
        let request = if let Some(bytes) = pending {
            crate::store::decode::<wire::TaskEventRequest>(&bytes)?
        } else {
            if terminal
                || self.store.get::<String>(&last)?.as_ref() == Some(&observation_digest(&event)?)
            {
                return Ok(());
            }
            self.event_request(&key, id, attempt, event, None)?
        };
        if request.attempt_id() != attempt {
            return Err(Error::Untrusted);
        }
        let wire::TaskEvent::DeviceAccessResult(value) = request.event() else {
            return Err(Error::Protocol);
        };
        validate_report(task, value)?;
        let ack = self.send_event(id, &request).await?;
        if ack.permit().is_some() || !ack.credentials().is_empty() {
            return Err(Error::Protocol);
        }
        let tx = self.store.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO state VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
            params![last, encode(&observation_digest(request.event())?)?],
        )?;
        tx.execute("DELETE FROM requests WHERE key=?1", [key])?;
        tx.commit()?;
        Ok(())
    }
}

//! Periodic device reads owned by the existing service; HTTP and sampling advance independently.
use agent_client::{
    telemetry::{Configuration, Failure, GapReason, Limits, Pending, PendingGap, Spool},
    Client, Clock, Error, IdentityProvider,
};
use rss_mdm_agent_wire::runtime as w;
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod collector;
mod environment;
pub(crate) mod mapping;
pub(crate) mod performance;

struct Slot {
    configuration: Configuration,
    due: Instant,
    upload: Instant,
    content: Option<Vec<u8>>,
    content_at: Instant,
}
pub(crate) struct Telemetry {
    spool: Spool,
    collector: collector::Collector,
    environment: environment::Environment,
    slots: BTreeMap<uuid::Uuid, Slot>,
    refresh: VecDeque<(uuid::Uuid, uuid::Uuid)>,
    directory_at: Instant,
    request_at: Instant,
    send_at: Instant,
    newest: bool,
    network_turn: usize,
    registration: w::BindingProfile,
    last_time: Option<(Instant, i64)>,
    summary_at: Instant,
    pub(crate) summary: serde_json::Value,
}
pub(crate) enum Action {
    Directory,
    Configuration(uuid::Uuid, uuid::Uuid),
    Content(Box<Configuration>),
    Send(Pending),
    SendGap(PendingGap),
}
pub(crate) enum Event {
    Directory(w::TelemetryDirectory),
    Configuration(Configuration),
    Content(Configuration, Vec<u8>),
    Receipt(w::TelemetryReceipt),
    GapReceipt(w::TelemetryGapReceipt),
}
impl Telemetry {
    pub(crate) fn data_sources(&self) -> Result<std::collections::BTreeSet<String>, Error> {
        self.slots
            .values()
            .map(|s| {
                Ok(format!(
                    "telemetry:{}",
                    crate::collected_data::digest(s.configuration.payload())?.as_str()
                ))
            })
            .collect()
    }
    pub fn open<S: IdentityProvider, C: Clock>(client: &Client<S, C>) -> Result<Self, Error> {
        let spool = client.telemetry_spool(Limits::production())?;
        let mut value = Self {
            spool,
            collector: collector::Collector::new()?,
            environment: environment::Environment::new(),
            slots: BTreeMap::new(),
            refresh: VecDeque::new(),
            directory_at: Instant::now(),
            request_at: Instant::now(),
            send_at: Instant::now(),
            newest: false,
            network_turn: 0,
            registration: client.registration()?,
            last_time: None,
            summary_at: Instant::now(),
            summary: serde_json::Value::Null,
        };
        for signed in value.spool.configurations()? {
            if let Ok(c) = client.verify_telemetry(signed) {
                value.install(c)?;
            }
        }
        Ok(value)
    }
    fn install(&mut self, c: Configuration) -> Result<(), Error> {
        let p = c.payload();
        if let w::TelemetryCollector::Script { output_schema, .. } = &p.collector {
            mapping::admit_schema(output_schema)?;
        }
        self.spool.install(&c)?;
        if let Some(slot) = self
            .slots
            .get_mut(&p.policy)
            .filter(|s| s.configuration.payload().binding == p.binding)
        {
            slot.configuration = c;

            return Ok(());
        }
        if self
            .collector
            .configuration()
            .is_some_and(|v| v.payload().policy == p.policy)
        {
            self.collector.cancel()?;
        }
        let jitter = u64::from_le_bytes(
            p.policy.as_bytes()[..8]
                .try_into()
                .map_err(|_| Error::Protocol)?,
        ) ^ u64::from_le_bytes(
            p.registration_id.as_bytes()[..8]
                .try_into()
                .map_err(|_| Error::Protocol)?,
        );
        let now = Instant::now();
        let due = now + Duration::from_millis(jitter % u64::from(p.sample_millis.max(1)));
        self.slots.insert(
            p.policy,
            Slot {
                due,
                upload: now + Duration::from_millis(u64::from(p.upload_millis)),
                configuration: c,
                content: None,
                content_at: now,
            },
        );
        Ok(())
    }
    /// Called from every owner reconciliation, including while unrelated HTTP is pending.
    pub fn step(&mut self, core: &mut crate::service::Core) -> Result<(), Error> {
        let now = Instant::now();
        let at = timestamp()?;
        let environment = self.environment.observe(self.spool.root());
        let live = core.host.identity_ready() && !core.stopping;
        if let Some((before, wall)) = self.last_time {
            if at < wall
                || (at - wall).abs_diff(before.elapsed().as_micros().min(i64::MAX as u128) as i64)
                    > 2_000_000
            {
                self.collector.cancel()?;
                self.spool.gap_for(
                    self.registration.registration_id,
                    self.registration.generation,
                    if at < wall {
                        GapReason::Clock
                    } else {
                        GapReason::Suspended
                    },
                    1,
                    wall.min(at),
                    wall.max(at),
                )?;
                for slot in self.slots.values_mut() {
                    slot.due = now
                        + Duration::from_millis(u64::from(
                            slot.configuration.payload().normal_sample_millis,
                        ));
                }
                self.last_time = Some((now, at));
                return Ok(());
            }
        }
        self.last_time = Some((now, at));
        if !live
            || self
                .collector
                .configuration()
                .is_some_and(|c| c.payload().expires_at <= at / 1_000_000)
        {
            self.collector.cancel()?;
        }
        if let Some((active, evidence)) = self.collector.take()? {
            self.complete(
                &active.configuration,
                active.observed,
                &evidence,
                live,
                at,
                core,
            )?;
        }

        if live && !self.collector.active() {
            let policy = self
                .slots
                .iter()
                .filter(|(_, s)| s.due <= now)
                .min_by_key(|(id, s)| (s.due, **id))
                .map(|(id, _)| *id);
            if let Some(policy) = policy {
                let slot = self.slots.get_mut(&policy).ok_or(Error::Storage)?;
                let p = slot.configuration.payload();
                let cadence = cadence(
                    p.sample_millis,
                    p.normal_sample_millis,
                    p.boost_expires_at,
                    environment.boost,
                    at / 1_000_000,
                );
                let elapsed = now.saturating_duration_since(slot.due).as_millis();
                if elapsed >= u128::from(cadence) {
                    self.spool.gap_for(
                        p.registration_id,
                        p.generation,
                        GapReason::Suspended,
                        (elapsed / u128::from(cadence)).min(i64::MAX as u128) as u64,
                        at.saturating_sub((elapsed * 1000).min(at as u128) as i64),
                        at,
                    )?;
                }
                slot.due = now + Duration::from_millis(u64::from(cadence));
                let allowed = environment.collect && at / 1_000_000 < p.expires_at;
                if !allowed {
                    self.spool.gap_for(
                        p.registration_id,
                        p.generation,
                        if at / 1_000_000 >= p.expires_at {
                            GapReason::Lease
                        } else if environment.capacity {
                            GapReason::Capacity
                        } else {
                            GapReason::Power
                        },
                        1,
                        at,
                        at,
                    )?;
                } else if matches!(p.collector, w::TelemetryCollector::Native { .. })
                    || slot.content.is_some()
                {
                    if let Err(e) = self.collector.start(
                        slot.configuration.clone(),
                        slot.content.clone(),
                        core,
                        at,
                    ) {
                        self.spool.gap_for(
                            p.registration_id,
                            p.generation,
                            if e == Error::Capacity {
                                GapReason::Capacity
                            } else {
                                GapReason::Collection
                            },
                            1,
                            at,
                            at,
                        )?;
                    }
                } else {
                    self.spool.gap_for(
                        p.registration_id,
                        p.generation,
                        GapReason::Collection,
                        1,
                        at,
                        at,
                    )?;
                }
            }
        }
        for slot in self.slots.values_mut() {
            if now >= slot.upload {
                self.spool.freeze_all(&slot.configuration)?;
                slot.upload = now
                    + Duration::from_millis(u64::from(slot.configuration.payload().upload_millis));
            }
        }
        if now >= self.summary_at {
            self.spool.prune(
                at / 1_000_000,
                self.registration.registration_id,
                self.registration.generation,
            )?;
            self.summary =
                serde_json::to_value(self.spool.summary()?).map_err(|_| Error::Storage)?;
            self.summary_at = now + Duration::from_secs(1);
        }
        Ok(())
    }
    fn complete(
        &mut self,
        configuration: &Configuration,
        observed: i64,
        evidence: &execution_contract::ProcessEvidence,
        live: bool,
        at: i64,
        core: &mut crate::service::Core,
    ) -> Result<(), Error> {
        let p = configuration.payload();
        let current = live
            && self
                .slots
                .get(&p.policy)
                .is_some_and(|s| s.configuration.payload().binding == p.binding)
            && at / 1_000_000 < p.expires_at;
        if current {
            let result = mapping::readings(p, evidence, at).and_then(|values| {
                self.spool
                    .append(configuration, observed, values, &performance::boot()?)
            });
            self.project_local(
                core,
                configuration,
                evidence,
                at,
                result.as_ref().ok().copied(),
            );
            match result {
                Ok(_) => (),
                Err(e) => {
                    self.spool.gap_for(
                        p.registration_id,
                        p.generation,
                        if e == Error::Capacity {
                            GapReason::Capacity
                        } else {
                            GapReason::Collection
                        },
                        1,
                        observed.min(at),
                        observed.max(at),
                    )?;
                }
            }
        } else {
            self.spool.gap_for(
                p.registration_id,
                p.generation,
                GapReason::Configuration,
                1,
                observed.min(at),
                observed.max(at),
            )?;
        }
        Ok(())
    }
    pub fn action(&mut self) -> Result<Option<Action>, Error> {
        let now = Instant::now();
        if now < self.request_at {
            return Ok(None);
        }
        self.request_at = now + Duration::from_millis(100);
        if now >= self.directory_at {
            self.directory_at = now + Duration::from_secs(60);
            return Ok(Some(Action::Directory));
        }
        for _ in 0..4 {
            let turn = self.network_turn;
            self.network_turn = (turn + 1) % 4;
            match turn {
                0 => {
                    if let Some((p, v)) = self.refresh.pop_front() {
                        return Ok(Some(Action::Configuration(p, v)));
                    }
                }
                1 => {
                    if let Some(slot) = self.slots.values_mut().find(|s| {
                        s.content_at <= now
                            && s.content.is_none()
                            && matches!(
                                s.configuration.payload().collector,
                                w::TelemetryCollector::Script { .. }
                            )
                            && s.configuration.payload().expires_at
                                > timestamp().unwrap_or(i64::MAX) / 1_000_000
                    }) {
                        slot.content_at = now + Duration::from_secs(60);
                        return Ok(Some(Action::Content(Box::new(slot.configuration.clone()))));
                    }
                }
                2 => {
                    if now >= self.send_at && self.environment.observe(self.spool.root()).upload {
                        if let Some(p) =
                            self.spool.pending(timestamp()? / 1_000_000, self.newest)?
                        {
                            self.newest = !self.newest;
                            self.send_at = now
                                + Duration::from_secs_f64((p.bytes() as f64 / 65536.0).max(0.05));
                            self.spool.attempting(&p, timestamp()? / 1_000_000)?;
                            return Ok(Some(Action::Send(p)));
                        }
                    }
                }
                _ => {
                    if now >= self.send_at && self.environment.observe(self.spool.root()).upload {
                        let at = timestamp()?;
                        self.spool.freeze_gaps(
                            self.registration.registration_id,
                            self.registration.generation,
                            at,
                        )?;
                        if let Some(p) = self.spool.pending_gap(
                            self.registration.registration_id,
                            self.registration.generation,
                            at,
                        )? {
                            self.send_at = now
                                + Duration::from_secs_f64((p.bytes() as f64 / 65536.0).max(0.05));
                            self.spool.attempting_gap(&p, at / 1_000_000)?;
                            return Ok(Some(Action::SendGap(p)));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
    pub fn apply(&mut self, action: &Action, result: Result<Event, Failure>) -> Result<(), Error> {
        match result {
            Ok(Event::Directory(d)) => {
                for slot in self.slots.values() {
                    if !d.assignments.iter().any(|a| {
                        a.supported
                            && a.policy == slot.configuration.payload().policy
                            && a.version == slot.configuration.payload().policy_version
                    }) {
                        self.spool.freeze_all(&slot.configuration)?;
                    }
                }
                self.spool.retain_configurations(&d)?;
                self.slots.retain(|id, s| {
                    d.assignments.iter().any(|a| {
                        a.supported
                            && a.policy == *id
                            && a.version == s.configuration.payload().policy_version
                    })
                });
                if self
                    .collector
                    .configuration()
                    .is_some_and(|c| !self.slots.contains_key(&c.payload().policy))
                {
                    self.collector.cancel()?;
                }
                self.refresh.retain(|(p, v)| {
                    d.assignments
                        .iter()
                        .any(|a| a.supported && a.policy == *p && a.version == *v)
                });
                for a in d.assignments.into_iter().filter(|a| a.supported) {
                    if !self.refresh.contains(&(a.policy, a.version)) {
                        self.refresh.push_back((a.policy, a.version));
                    }
                }
            }
            Ok(Event::Configuration(c)) => self.install(c)?,
            Ok(Event::Content(c, bytes)) => {
                let used: usize = self
                    .slots
                    .values()
                    .filter_map(|s| s.content.as_ref())
                    .map(Vec::len)
                    .sum();
                if used.saturating_add(bytes.len()) > 8 * 1024 * 1024 {
                    return Err(Error::Capacity);
                }
                if let Some(slot) = self
                    .slots
                    .get_mut(&c.payload().policy)
                    .filter(|s| s.configuration.payload().binding == c.payload().binding)
                {
                    slot.content = Some(bytes);
                }
            }
            Ok(Event::Receipt(r)) => {
                let Action::Send(p) = action else {
                    return Err(Error::Protocol);
                };
                self.spool.acknowledge(p, &r)?;
            }
            Ok(Event::GapReceipt(r)) => {
                let Action::SendGap(p) = action else {
                    return Err(Error::Protocol);
                };
                self.spool.acknowledge_gap(p, &r)?;
            }
            Err(f) => {
                if let Action::SendGap(p) = action {
                    if matches!(
                        f.error,
                        Error::Conflict
                            | Error::Protocol
                            | Error::Denied
                            | Error::Untrusted
                            | Error::Expired
                    ) {
                        self.spool.reject_gap(p)?;
                    } else {
                        self.spool
                            .retry_gap(p, timestamp()? / 1_000_000, f.retry_after)?;
                    }
                }
                if let Action::Send(p) = action {
                    if matches!(
                        f.error,
                        Error::Conflict | Error::Protocol | Error::Denied | Error::Untrusted
                    ) {
                        self.spool.reject(p, GapReason::Rejected)?;
                    } else {
                        self.spool
                            .retry(p, timestamp()? / 1_000_000, f.retry_after)?;
                    }
                }
                self.request_at = Instant::now()
                    + Duration::from_secs(f.retry_after.unwrap_or(5).clamp(1, 86400));
            }
        }
        Ok(())
    }
    fn project_local(
        &self,
        core: &mut crate::service::Core,
        configuration: &Configuration,
        evidence: &execution_contract::ProcessEvidence,
        at: i64,
        observation: Option<uuid::Uuid>,
    ) {
        let normalized = observation
            .and_then(|id| self.spool.observation(configuration, id).ok())
            .map(|o| o.values);
        if let Err(error) = core.periodic_data(configuration.payload(), evidence, at, normalized) {
            eprintln!("agent_collected_data: {error}");
        }
    }
    pub fn shutdown(&mut self, core: &mut crate::service::Core) -> Result<(), Error> {
        self.collector.shutdown()?;
        if let Some((active, evidence)) = self.collector.take()? {
            let now = timestamp()?;
            let result = mapping::readings(active.configuration.payload(), &evidence, now)
                .and_then(|values| {
                    self.spool.append(
                        &active.configuration,
                        active.observed,
                        values,
                        &performance::boot()?,
                    )
                });
            self.project_local(
                core,
                &active.configuration,
                &evidence,
                now,
                result.as_ref().ok().copied(),
            );
            match result {
                Ok(_) => (),
                Err(_) => self.spool.gap_for(
                    active.configuration.payload().registration_id,
                    active.configuration.payload().generation,
                    GapReason::Collection,
                    1,
                    active.observed,
                    now.max(active.observed),
                )?,
            }
        }
        for slot in self.slots.values() {
            self.spool.freeze_all(&slot.configuration)?;
        }
        Ok(())
    }
}
pub(crate) async fn request<S: IdentityProvider, C: Clock>(
    client: &Client<S, C>,
    action: &Action,
) -> Result<Event, Failure> {
    match action {
        Action::Directory => client.telemetry_directory().await.map(Event::Directory),
        Action::Configuration(p, v) => client
            .telemetry_configuration(*p, *v)
            .await
            .map(Event::Configuration),
        Action::Content(c) => client
            .telemetry_content(c)
            .await
            .map(|b| Event::Content((**c).clone(), b)),
        Action::SendGap(p) => {
            if p.attempted() {
                if let Some(r) = client.telemetry_gap_status(p).await? {
                    return Ok(Event::GapReceipt(r));
                }
            }
            client.telemetry_gap_submit(p).await.map(Event::GapReceipt)
        }
        Action::Send(p) => {
            if p.attempted() {
                if let Some(r) = client.telemetry_status(p).await? {
                    return Ok(Event::Receipt(r));
                }
            }
            client.telemetry_submit(p).await.map(Event::Receipt)
        }
    }
}
fn timestamp() -> Result<i64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Clock)?
        .as_micros()
        .try_into()
        .map_err(|_| Error::Clock)
}

fn cadence(boost: u32, normal: u32, until: Option<i64>, local_boost: bool, now: i64) -> u32 {
    if !local_boost || until.is_some_and(|until| now >= until) {
        normal
    } else {
        boost
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boost_expires_at_boundary_and_local_power_can_only_reduce_it() {
        assert_eq!(cadence(500, 3000, Some(10), true, 9), 500);
        assert_eq!(cadence(500, 3000, Some(10), true, 10), 3000);
        assert_eq!(cadence(500, 3000, Some(10), false, 9), 3000);
        assert_eq!(cadence(3000, 3000, None, true, 10), 3000);
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "telemetry/tests.rs"]
mod behavior_tests;

//! Per-login process mechanism. The system service owns every business intent and durable fact.
use crate::{Artifacts, MaterialRegistry, NativeRunner};
use execution_app::{DispatchOutcome, Error, RunnerPort};
use execution_contract::*;
use execution_ipc::{
    helper::{Command, Envelope, Reply},
    host,
};
use std::path::PathBuf;

/// A physical process owner for one actual user login; it has no device secrets or database.
pub struct Helper {
    physical: std::collections::BTreeMap<
        (AttemptId, u32, SoftwarePhase, u32),
        crate::runner::invocation::PhysicalInvocation,
    >,
    retired: std::collections::BTreeMap<(AttemptId, u32, SoftwarePhase, u32), (Digest, u64)>,
    clock_watermark: u64,
    capacity: usize,
    limits: ExecutionLimits,
    policy: host::PeerPolicy,
    work_root: PathBuf,
    subject: String,
    session: u32,
    binding: Id,
    materials: MaterialRegistry,
    runner: NativeRunner,
}
impl Helper {
    /// Open one physical helper in the current login; no device store or credentials are opened.
    pub fn new(
        policy: host::PeerPolicy,
        work_root: PathBuf,
        capacity: usize,
        limits: ExecutionLimits,
    ) -> Result<Self, Error> {
        policy.validate()?;
        let subject = host::current_subject()?;
        let session = host::current_session()?;
        if subject == "0" || subject == "S-1-5-18" || session == 0 {
            return Err(Error::Unbound);
        }
        platform_private_storage::directory(&work_root).map_err(|_| Error::Storage)?;
        let work_root = work_root.canonicalize().map_err(|_| Error::Storage)?;
        let materials = MaterialRegistry::new(capacity)?;
        let runner = NativeRunner::with_materials(
            Id::new("native-user-helper").expect("constant"),
            materials.clone(),
            capacity,
        )?;
        Ok(Self {
            physical: Default::default(),
            retired: Default::default(),
            clock_watermark: 0,
            capacity,
            limits,
            policy,
            work_root,
            subject,
            session,
            binding: host::current_session_binding()?,
            materials,
            runner,
        })
    }
    fn input(&self, input: ExecutionInput) -> Result<FrozenExecution, Error> {
        if host::current_subject()? != self.subject || host::current_session()? != self.session {
            return Err(Error::Unbound);
        }
        if !matches!(&input.request.authority, Authority::Enterprise { .. })
            || !matches!(&input.run_as, RunAs::User { account } if account.subject.as_str() == self.subject)
            || !matches!(&input.session_requirement, SessionRequirement::ActiveUser { account, session } if account.subject.as_str() == self.subject && session == &self.binding)
            || input.launch.cwd != self.work_root.to_str().ok_or(Error::Configuration)?
        {
            return Err(Error::Denied);
        }
        FrozenExecution::freeze(input, &self.limits).map_err(|_| Error::InvalidInput)
    }
    fn artifacts(&self, interpreter: PathBuf, content: PathBuf) -> Artifacts {
        Artifacts {
            program: Vec::new(),
            delegate: None,
            interpreter,
            content,
            work_root: self.work_root.clone(),
            controlled_input: None,
            #[cfg(test)]
            fixture_owned: false,
        }
    }
    fn command(
        &mut self,
        connection: &host::SystemConnection<'_>,
        command: Command,
    ) -> Result<Reply, Error> {
        Ok(match command {
            Command::Invoke {
                input,
                attempt,
                step,
                phase,
                cleanup_sequence,
                interpreter,
                content,
                timeout_ms,
                output_bytes,
                first_start,
                before,
            } => {
                if cleanup_sequence > 3 || (cleanup_sequence > 0 && phase != SoftwarePhase::Cleanup)
                {
                    return Err(Error::Denied);
                }
                let (plan, invocation) = self.invocation(*input, step, phase)?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| Error::Clock)?
                    .as_millis()
                    .try_into()
                    .map_err(|_| Error::Clock)?;
                if now < self.clock_watermark {
                    return Err(Error::Clock);
                }
                self.clock_watermark = now;
                if cleanup_sequence == 0 && now >= plan.spec().validity.expires_at_unix_ms {
                    return Err(Error::Clock);
                }
                // An expired invocation can never be accepted again, even after its tombstone
                // is reclaimed. The watermark prevents a clock rollback reopening that window.
                self.retired.retain(|_, (_, expiry)| *expiry > now);
                let key = (attempt.clone(), step, phase, cleanup_sequence);
                if let Some((digest, _)) = self.retired.get(&key) {
                    return if digest == plan.digest() {
                        Ok(Reply::Acknowledged)
                    } else {
                        Err(Error::Conflict)
                    };
                }
                if let Some(owner) = self.physical.get(&key) {
                    return if &owner.digest == plan.digest() {
                        Ok(Reply::Submitted)
                    } else {
                        Err(Error::Conflict)
                    };
                }
                if self.physical.len() >= self.capacity
                    || self.retired.len().saturating_add(self.physical.len()) >= 4096
                {
                    return Err(Error::Capacity);
                }
                let mut source = self.artifacts(interpreter, content);
                if matches!(invocation.launch.stdin, StandardInput::Controlled { .. })
                    && invocation.launch.interpreter.profile.id.as_str() == "native-software-worker"
                {
                    source.controlled_input =
                        Some(std::sync::Arc::new(crate::runner::program::BeforeInput {
                            digest: plan.digest().clone(),
                            attempt: attempt.clone(),
                            step,
                            state: before.ok_or(Error::Denied)?,
                        }));
                }
                let owner = crate::runner::invocation::PhysicalInvocation::start(
                    plan,
                    attempt,
                    source,
                    invocation,
                    timeout_ms,
                    output_bytes,
                    first_start,
                )?;
                self.physical.insert(key, owner);
                Reply::Submitted
            }
            Command::InvocationEvidence {
                input,
                attempt,
                step,
                phase,
                cleanup_sequence,
            } => {
                let (plan, _) = self.invocation(*input, step, phase)?;
                let process = match self.physical.get(&(attempt, step, phase, cleanup_sequence)) {
                    Some(owner) if &owner.digest == plan.digest() => owner
                        .facts
                        .lock()
                        .map_err(|_| Error::Unavailable)?
                        .clone()
                        .map(Box::new),
                    Some(_) => return Err(Error::Denied),
                    None => None,
                };
                Reply::Evidence { process }
            }
            Command::InvocationStop {
                input,
                attempt,
                step,
                phase,
                cleanup_sequence,
            } => {
                let (plan, _) = self.invocation(*input, step, phase)?;
                let owner = self
                    .physical
                    .get(&(attempt, step, phase, cleanup_sequence))
                    .ok_or(Error::NotFound)?;
                if &owner.digest != plan.digest() {
                    return Err(Error::Denied);
                }
                owner
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Release);
                Reply::StopRequested
            }
            Command::InvocationAck {
                input,
                attempt,
                step,
                phase,
                cleanup_sequence,
            } => {
                let (plan, _) = self.invocation(*input, step, phase)?;
                let key = (attempt, step, phase, cleanup_sequence);
                if let Some(owner) = self.physical.get(&key) {
                    if &owner.digest != plan.digest()
                        || !owner
                            .facts
                            .lock()
                            .map_err(|_| Error::Unavailable)?
                            .as_ref()
                            .is_some_and(|f| f.finished)
                    {
                        return Err(Error::Conflict);
                    }
                    if self.retired.len() >= 4096 {
                        return Err(Error::Capacity);
                    }
                    self.retired.insert(
                        key.clone(),
                        (
                            owner.digest.clone(),
                            plan.spec().validity.expires_at_unix_ms,
                        ),
                    );
                    self.physical.remove(&key);
                }
                Reply::Acknowledged
            }
            Command::Ready if host::active_login() => Reply::Ready {
                subject: self.subject.clone(),
                session: self.session,
                binding: self.binding.clone(),
                work_root: self.work_root.clone(),
            },
            Command::Ready => return Err(Error::Unbound),
            Command::Inspect {
                input,
                interpreter,
                content,
            } => {
                let input = self.input(*input)?;
                self.artifacts(interpreter, content).inspect(&input)?;
                Reply::Prepared
            }
            Command::Start {
                input,
                interpreter,
                content,
                attempt,
                timeout_ms,
                start_before_ms,
                output_bytes,
            } => {
                let input = self.input(*input)?;
                match self
                    .materials
                    .register(&input, self.artifacts(interpreter, content))
                {
                    Ok(()) => (),
                    Err(Error::Conflict) => self.materials.inspect(&input)?,
                    Err(error) => return Err(error),
                }
                match self.runner.execute_delegated(
                    connection,
                    &input,
                    &attempt,
                    timeout_ms,
                    start_before_ms,
                    output_bytes,
                )? {
                    DispatchOutcome::Accepted | DispatchOutcome::NeverDispatched => {
                        Reply::Submitted
                    }
                    DispatchOutcome::OutcomeUnknown => Reply::Unavailable,
                }
            }
            Command::Evidence { input, attempt } => {
                let input = self.input(*input)?;
                Reply::Evidence {
                    process: self.runner.evidence(&input, &attempt)?.map(Box::new),
                }
            }
            Command::Acknowledge { input, process } => {
                let input = self.input(*input)?;
                self.runner.acknowledge_capture(&input, &process)?;
                self.materials.retire(&input)?;
                Reply::Acknowledged
            }
            Command::Stop { input, attempt } => {
                let input = self.input(*input)?;
                self.runner.stop(&input, &attempt)?;
                Reply::StopRequested
            }
        })
    }
    fn invocation(
        &self,
        input: ExecutionInput,
        step: u32,
        phase: SoftwarePhase,
    ) -> Result<(FrozenExecution, SoftwareInvocation), Error> {
        if host::current_subject()? != self.subject
            || host::current_session()? != self.session
            || !matches!(input.request.authority, Authority::Enterprise { .. })
        {
            return Err(Error::Unbound);
        }
        let plan = FrozenExecution::freeze(input, &self.limits).map_err(|_| Error::InvalidInput)?;
        let invocation = plan
            .spec()
            .execution
            .software_program()
            .and_then(|p| p.invocation(step as usize, phase))
            .ok_or(Error::InvalidInput)?
            .clone();
        if !matches!(&invocation.run_as, RunAs::User { account } if account.subject.as_str() == self.subject)
            || !matches!(&invocation.session_requirement, SessionRequirement::ActiveUser { session, .. } if session == &self.binding)
        {
            return Err(Error::Denied);
        }
        Ok((plan, invocation))
    }
}
impl host::Handler for Helper {
    fn helper_active(&self) -> bool {
        host::active_login() && host::current_session().is_ok_and(|session| session == self.session)
    }
    fn peer_policy(&self) -> Option<host::PeerPolicy> {
        Some(self.policy.clone())
    }
    fn handle(&mut self, _: &host::Peer, _: host::Request) -> host::Reply {
        host::Reply::Rejected
    }
    fn handle_wire(&mut self, peer: &host::Peer, bytes: &[u8]) -> Vec<u8> {
        let reply = (|| {
            let policy = self.policy.clone();
            let connection = peer.system_connection(&policy)?;
            let envelope: Envelope =
                serde_json::from_slice(bytes).map_err(|_| Error::InvalidInput)?;
            if envelope.version != 2 {
                return Err(Error::InvalidInput);
            }
            self.command(&connection, envelope.command)
        })();
        serde_json::to_vec(&match reply {
            Ok(reply) => reply,
            Err(Error::Capacity) => Reply::Capacity,
            Err(Error::Unavailable | Error::Storage | Error::OutcomeUnknown) => Reply::Unavailable,
            Err(_) => Reply::Rejected,
        })
        .unwrap_or_else(|_| b"{\"kind\":\"unavailable\"}".to_vec())
    }
    fn tick(&mut self) -> Result<(), Error> {
        if host::current_subject()? != self.subject || host::current_session()? != self.session {
            return Err(Error::Unbound);
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<(), Error> {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        for owner in self.physical.values() {
            owner
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
        let runner = self.runner.shutdown_until(until);
        loop {
            let mut ended = true;
            for owner in self.physical.values() {
                ended &= owner
                    .facts
                    .lock()
                    .map_err(|_| Error::Unavailable)?
                    .as_ref()
                    .is_some_and(|f| f.finished && f.quiescent);
            }
            if ended {
                return runner;
            }
            if std::time::Instant::now() >= until {
                return Err(Error::OutcomeUnknown);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    use execution_ipc::host::Handler;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    #[test]
    fn shutdown_cancels_every_unresolved_physical_owner_and_does_not_claim_success() {
        let materials = MaterialRegistry::new(2).unwrap();
        let mut helper = Helper {
            physical: Default::default(),
            retired: Default::default(),
            clock_watermark: 0,
            capacity: 2,
            limits: execution_app::test_execution_limits(),
            policy: host::PeerPolicy {
                images: vec![],
                subjects: vec![],
                interactive: true,
            },
            work_root: PathBuf::new(),
            subject: "fixture".into(),
            session: 1,
            binding: Id::new("fixture").unwrap(),
            runner: NativeRunner::with_materials(Id::new("fixture").unwrap(), materials.clone(), 2)
                .unwrap(),
            materials,
        };
        let flags = [
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        ];
        for (step, flag) in flags.iter().enumerate() {
            helper.physical.insert(
                (
                    AttemptId::new("attempt").unwrap(),
                    step as u32,
                    SoftwarePhase::Mutation,
                    0,
                ),
                crate::runner::invocation::PhysicalInvocation {
                    digest: Digest::new("ab".repeat(32)).unwrap(),
                    cancel: flag.clone(),
                    facts: Arc::new(Mutex::new(None)),
                },
            );
        }
        let started = std::time::Instant::now();
        assert!(matches!(helper.stop(), Err(Error::OutcomeUnknown)));
        assert!(flags.iter().all(|f| f.load(Ordering::Acquire)));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        helper.physical.clear();
        assert!(helper.stop().is_ok());
    }
}

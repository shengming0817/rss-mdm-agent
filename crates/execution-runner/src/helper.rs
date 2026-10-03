//! Per-login process mechanism. The system service owns every business intent and durable fact.
use crate::{host, Artifacts, MaterialRegistry, NativeRunner};
use execution_app::{DispatchOutcome, Error, RunnerPort};
use execution_contract::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Envelope {
    pub version: u8,
    pub command: Command,
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Command {
    Invoke {
        input: Box<ExecutionInput>,
        attempt: AttemptId,
        step: u32,
        phase: SoftwarePhase,
        cleanup_sequence: u32,
        interpreter: PathBuf,
        content: PathBuf,
        timeout_ms: u64,
        output_bytes: u64,
        first_start: Option<u64>,
        before: Option<SoftwareState>,
    },
    InvocationEvidence {
        input: Box<ExecutionInput>,
        attempt: AttemptId,
        step: u32,
        phase: SoftwarePhase,
        cleanup_sequence: u32,
    },
    InvocationStop {
        input: Box<ExecutionInput>,
        attempt: AttemptId,
        step: u32,
        phase: SoftwarePhase,
        cleanup_sequence: u32,
    },
    InvocationAck {
        input: Box<ExecutionInput>,
        attempt: AttemptId,
        step: u32,
        phase: SoftwarePhase,
        cleanup_sequence: u32,
    },
    Ready,
    Inspect {
        input: Box<ExecutionInput>,
        interpreter: PathBuf,
        content: PathBuf,
    },
    Start {
        input: Box<ExecutionInput>,
        interpreter: PathBuf,
        content: PathBuf,
        attempt: AttemptId,
        timeout_ms: u64,
        start_before_ms: u64,
        output_bytes: u64,
    },
    Evidence {
        input: Box<ExecutionInput>,
        attempt: AttemptId,
    },
    Acknowledge {
        input: Box<ExecutionInput>,
        process: Box<ProcessEvidence>,
    },
    Stop {
        input: Box<ExecutionInput>,
        attempt: AttemptId,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Reply {
    Ready {
        subject: String,
        session: u32,
        binding: Id,
        work_root: PathBuf,
    },
    Prepared,
    Submitted,
    Evidence {
        process: Option<Box<ProcessEvidence>>,
    },
    Acknowledged,
    StopRequested,
    Rejected,
    Capacity,
    Unavailable,
}
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

/// Observed login and private work directory of a pinned helper; no enterprise permission.
#[derive(Clone)]
pub struct UserContext {
    /// Actual native OS subject.
    pub subject: String,
    /// Exact native login session.
    pub session: u32,
    /// Kernel boot and authentication-session identity, never a reused WTS number.
    pub binding: Id,
    /// The helper's validated private working directory.
    pub work_root: PathBuf,
}
/// Concrete native connection to one observed helper. The system runner remains execution owner.
pub struct Connection {
    policy: host::PeerPolicy,
    context: UserContext,
}
impl Connection {
    /// Connect only from the actual system account and verify the registered helper's reply.
    pub fn connect(policy: host::PeerPolicy, subject: String, session: u32) -> Result<Self, Error> {
        let system = if cfg!(windows) { "S-1-5-18" } else { "0" };
        if host::current_subject()? != system
            || subject == system
            || session == 0
            || !policy.interactive
            || policy.subjects != [subject.clone()]
        {
            return Err(Error::Denied);
        }
        policy.validate()?;
        let mut connection = Self {
            policy,
            context: UserContext {
                subject,
                session,
                binding: Id::new("unbound").expect("constant"),
                work_root: PathBuf::new(),
            },
        };
        match connection.exchange(Command::Ready)? {
            Reply::Ready {
                subject,
                session,
                binding,
                work_root,
            } if subject == connection.context.subject
                && session == connection.context.session
                && work_root.is_absolute() =>
            {
                connection.context.binding = binding;
                connection.context.work_root = work_root;
                Ok(connection)
            }
            _ => Err(Error::Unbound),
        }
    }
    /// The actual helper context, fixed for this connection; never inferred from request JSON.
    pub fn context(&self) -> &UserContext {
        &self.context
    }
    /// Recheck the actual authenticated helper login immediately before execution admission.
    pub fn verify_context(
        &self,
        run_as: &RunAs,
        session: &SessionRequirement,
    ) -> Result<(), Error> {
        if !matches!(run_as, RunAs::User { account } if account.subject.as_str() == self.context.subject)
            || !matches!(session, SessionRequirement::ActiveUser { session, .. } if session == &self.context.binding)
        {
            return Err(Error::Unbound);
        }
        match self.exchange(Command::Ready)? {
            Reply::Ready {
                subject,
                session,
                binding,
                work_root,
            } if subject == self.context.subject
                && session == self.context.session
                && binding == self.context.binding
                && work_root == self.context.work_root =>
            {
                Ok(())
            }
            _ => Err(Error::Unbound),
        }
    }
    pub(crate) fn exchange(&self, command: Command) -> Result<Reply, Error> {
        let bytes = serde_json::to_vec(&Envelope {
            version: 2,
            command,
        })
        .map_err(|_| Error::InvalidInput)?;
        #[cfg(target_os = "macos")]
        let reply = crate::macos_service::query_helper(
            &bytes,
            self.context.subject.parse().map_err(|_| Error::Unbound)?,
            self.context.session,
            &self.policy,
        )?;
        #[cfg(windows)]
        let reply = crate::windows_service::query_session(
            &bytes,
            &self.context.subject,
            self.context.session,
            &self.policy,
        )?;
        #[cfg(not(any(target_os = "macos", windows)))]
        let reply: Vec<u8> = {
            let _ = bytes;
            return Err(Error::Unsupported);
        };
        match serde_json::from_slice(&reply).map_err(|_| Error::InvalidInput)? {
            Reply::Rejected => Err(Error::Denied),
            Reply::Capacity => Err(Error::Capacity),
            Reply::Unavailable => Err(Error::Unavailable),
            reply => Ok(reply),
        }
    }
    pub(crate) fn inspect(
        &self,
        plan: &FrozenExecution,
        artifacts: &Artifacts,
    ) -> Result<(), Error> {
        if !matches!(&plan.spec().run_as, RunAs::User { account } if account.subject.as_str() == self.context.subject)
            || !matches!(&plan.spec().session_requirement, SessionRequirement::ActiveUser { session, .. } if session == &self.context.binding)
            || artifacts.work_root != self.context.work_root
        {
            return Err(Error::Denied);
        }
        match self.exchange(Command::Inspect {
            input: Box::new(plan.spec().clone()),
            interpreter: artifacts.interpreter.clone(),
            content: artifacts.content.clone(),
        })? {
            Reply::Prepared => Ok(()),
            _ => Err(Error::InvalidInput),
        }
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    use crate::host::Handler;
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

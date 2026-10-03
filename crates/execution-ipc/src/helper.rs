//! Authenticated per-login protocol and client, without a physical executor or journal.
use crate::host;
use execution_app::Error;
use execution_contract::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
/// Current helper wire version; older envelopes are rejected without negotiation.
pub const VERSION: u8 = 2;
/// One command from the authenticated system service to a pinned user helper.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    /// Exact helper protocol version.
    pub version: u8,
    /// Physical operation; enterprise admission remains in the system service.
    pub command: Command,
}
/// Physical helper operations within one authenticated native login.
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Command {
    /// Start one step of an admitted software program.
    Invoke {
        /// Frozen execution specification from the system service.
        input: Box<ExecutionInput>,
        /// Original attempt identity, reused for recovery and deduplication.
        attempt: AttemptId,
        /// Ordered software step index.
        step: u32,
        /// Execute, verify or cleanup phase of this step.
        phase: SoftwarePhase,
        /// Bounded cleanup retry sequence; zero for the initial invocation.
        cleanup_sequence: u32,
        /// Protected interpreter artifact path.
        interpreter: PathBuf,
        /// Protected content artifact path.
        content: PathBuf,
        /// Maximum physical execution duration.
        timeout_ms: u64,
        /// Maximum captured output bytes.
        output_bytes: u64,
        /// Optional first-start deadline in device-wide monotonic milliseconds.
        first_start: Option<u64>,
        /// Independent pre-execution software state for controlled worker input.
        before: Option<SoftwareState>,
    },
    /// Read physical evidence for an existing software-step invocation.
    InvocationEvidence {
        /// Frozen specification used to validate the invocation digest.
        input: Box<ExecutionInput>,
        /// Existing attempt identity.
        attempt: AttemptId,
        /// Ordered software step index.
        step: u32,
        /// Invocation phase.
        phase: SoftwarePhase,
        /// Cleanup sequence that identifies the invocation.
        cleanup_sequence: u32,
    },
    /// Request cancellation of an existing software-step invocation.
    InvocationStop {
        /// Frozen specification used to validate the invocation digest.
        input: Box<ExecutionInput>,
        /// Existing attempt identity.
        attempt: AttemptId,
        /// Ordered software step index.
        step: u32,
        /// Invocation phase.
        phase: SoftwarePhase,
        /// Cleanup sequence that identifies the invocation.
        cleanup_sequence: u32,
    },
    /// Retire a completed software invocation after the service captures its evidence.
    InvocationAck {
        /// Frozen specification used to validate the invocation digest.
        input: Box<ExecutionInput>,
        /// Existing attempt identity.
        attempt: AttemptId,
        /// Ordered software step index.
        step: u32,
        /// Invocation phase.
        phase: SoftwarePhase,
        /// Cleanup sequence that identifies the invocation.
        cleanup_sequence: u32,
    },
    /// Observe the helper's active login and private working directory.
    Ready,
    /// Validate physical artifacts before admitting a process.
    Inspect {
        /// Frozen execution specification.
        input: Box<ExecutionInput>,
        /// Protected interpreter artifact path.
        interpreter: PathBuf,
        /// Protected content artifact path.
        content: PathBuf,
    },
    /// Start a single admitted process, deduplicated by its original attempt.
    Start {
        /// Frozen execution specification.
        input: Box<ExecutionInput>,
        /// Protected interpreter artifact path.
        interpreter: PathBuf,
        /// Protected content artifact path.
        content: PathBuf,
        /// Original attempt identity.
        attempt: AttemptId,
        /// Maximum physical execution duration.
        timeout_ms: u64,
        /// Latest permitted start in device-wide monotonic milliseconds.
        start_before_ms: u64,
        /// Maximum captured output bytes.
        output_bytes: u64,
    },
    /// Read evidence for a single-process attempt without dispatching it again.
    Evidence {
        /// Frozen specification used to validate the attempt digest.
        input: Box<ExecutionInput>,
        /// Existing attempt identity.
        attempt: AttemptId,
    },
    /// Release captured process resources after the service journals their evidence.
    Acknowledge {
        /// Frozen specification used to validate captured evidence.
        input: Box<ExecutionInput>,
        /// Evidence already captured by the service.
        process: Box<ProcessEvidence>,
    },
    /// Request cancellation of an existing single-process attempt.
    Stop {
        /// Frozen specification used to validate the attempt digest.
        input: Box<ExecutionInput>,
        /// Existing attempt identity.
        attempt: AttemptId,
    },
}
/// Bounded physical facts or acknowledgement; no durable application state is owned here.
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Reply {
    /// Facts of the helper's current native login.
    Ready {
        /// Actual OS subject.
        subject: String,
        /// Native login session number.
        session: u32,
        /// Kernel boot and authentication-session identity.
        binding: Id,
        /// Validated private working directory.
        work_root: PathBuf,
    },
    /// Physical artifact validation succeeded.
    Prepared,
    /// Request accepted; this does not assert process completion.
    Submitted,
    /// Observed physical process state, if an owner is still available.
    Evidence {
        /// Facts retained for the original attempt.
        process: Option<Box<ProcessEvidence>>,
    },
    /// Evidence acknowledged and retained resources eligible for retirement.
    Acknowledged,
    /// Cancellation requested; this does not assert process exit.
    StopRequested,
    /// Malformed, unauthorized or inconsistent request.
    Rejected,
    /// Physical ownership or retained-evidence limit reached.
    Capacity,
    /// Outcome cannot be confirmed; query the original attempt.
    Unavailable,
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
    /// Send a bounded command to the same pinned helper and classify its physical reply.
    pub fn exchange(&self, command: Command) -> Result<Reply, Error> {
        let bytes = serde_json::to_vec(&Envelope {
            version: VERSION,
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
}

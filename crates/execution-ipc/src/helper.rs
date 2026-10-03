//! Authenticated per-login protocol and client, without a physical executor or journal.
use crate::host;
use execution_app::Error;
use execution_contract::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
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
pub enum Command {
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
pub enum Reply {
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
    pub fn exchange(&self, command: Command) -> Result<Reply, Error> {
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
}

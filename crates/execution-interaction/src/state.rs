use crate::*;

/// Commands concern this interaction only. The host authenticates/authorizes the responder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Submit a response for the immutable waiting reason.
    Answer {
        /// Stable command identity for retries.
        id: Reference,
        /// Untrusted response data.
        response: Response,
    },
    /// Cancel this interaction, without terminating any task.
    Cancel {
        /// Stable cancellation identity for retries.
        id: Reference,
    },
    /// Observe whether the exclusive deadline elapsed.
    CheckExpiry,
}
/// Outcome independent of whether any state changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// An answer was proposed for commitment.
    Answered,
    /// A cancellation was proposed for commitment.
    Cancelled,
    /// Expiry was proposed for commitment; it never means consent.
    Expired,
    /// Identical command already committed.
    Duplicate,
    /// Another terminal result already won.
    Late,
    /// No transition is due before the deadline.
    NotDue,
}
/// Candidate conditional write. This is not proof that persistence succeeded.
#[derive(Debug, Clone)]
pub struct Transition {
    /// Compare this revision together with the interaction ID in protected storage.
    pub expected_revision: u64,
    /// Candidate next state; becomes authoritative only after the conditional write succeeds.
    pub next: Interaction,
}
/// A proposed transition, or a duplicate/late/not-due outcome without any write.
#[derive(Debug, Clone)]
pub struct Evaluation {
    /// Stable command result.
    pub outcome: Outcome,
    /// Optional conditional update; no I/O is performed by this crate.
    pub transition: Option<Transition>,
}
/// Validated immutable state. There is no Deserialize or task-lifecycle effect.
/// ```compile_fail
/// let _: execution_interaction::Interaction = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct Interaction {
    snapshot: Snapshot,
    limits: Limits,
}
impl Interaction {
    /// Open pending state at a caller-supplied UTC Unix millisecond time.
    pub fn open(
        spec: Spec,
        opened_at_unix_ms: u64,
        limits: Limits,
    ) -> Result<Self, InteractionError> {
        Self::restore(
            Snapshot {
                version: 1,
                spec,
                opened_at_unix_ms,
                revision: 0,
                status: Status::Pending,
            },
            limits,
        )
    }
    /// Bounded decoding of the sole current snapshot format, followed by invariant validation.
    pub fn decode(bytes: &[u8], limits: Limits) -> Result<Self, InteractionError> {
        validate_limits(limits)?;
        if bytes.len() > limits.max_snapshot_bytes {
            return Err(InteractionError::Limit);
        }
        let snapshot = serde_json::from_slice(bytes).map_err(|_| InteractionError::Snapshot)?;
        Self::restore(snapshot, limits)
    }
    /// Validate a storage DTO. Caller must establish storage integrity and subject authorization.
    pub fn restore(snapshot: Snapshot, limits: Limits) -> Result<Self, InteractionError> {
        validate_limits(limits)?;
        let deadline = snapshot.spec.expires_at_unix_ms;
        let opened = snapshot.opened_at_unix_ms;
        if snapshot.version != 1 || deadline <= opened {
            return Err(InteractionError::Snapshot);
        }
        if deadline - opened > limits.max_lifetime_ms {
            return Err(InteractionError::Limit);
        }
        let valid = match &snapshot.status {
            Status::Pending => snapshot.revision == 0,
            Status::Answered {
                response,
                at_unix_ms,
                ..
            } => {
                snapshot.revision == 1
                    && (opened..deadline).contains(at_unix_ms)
                    && snapshot.spec.kind.accepts(response)
            }
            Status::Cancelled { at_unix_ms, .. } => {
                snapshot.revision == 1 && (opened..deadline).contains(at_unix_ms)
            }
            Status::Expired { at_unix_ms } => snapshot.revision == 1 && *at_unix_ms >= deadline,
        };
        if !valid {
            return Err(InteractionError::Snapshot);
        }
        if serde_json::to_vec(&snapshot)
            .map_err(|_| InteractionError::Snapshot)?
            .len()
            > limits.max_snapshot_bytes
        {
            return Err(InteractionError::Limit);
        }
        if matches!(snapshot.status, Status::Pending) {
            // Reject waits whose configured envelope cannot ever store their terminal result.
            let longest = Reference::new("x".repeat(128)).expect("valid bounded reference");
            let response = match snapshot.spec.kind {
                Kind::UserConfirmation { .. } => Response::Confirmation { accepted: false },
                Kind::PrivacyConsent { .. } => Response::PrivacyConsent { accepted: false },
                Kind::AdministratorAuthorization { .. } => Response::AdministratorDecision {
                    record: longest.clone(),
                },
                Kind::ParameterInput { .. } => Response::ParameterSubmission {
                    submission: longest.clone(),
                },
                Kind::MaintenanceWindow { .. } => Response::MaintenanceSelection {
                    selection: longest.clone(),
                },
                Kind::RestartPrompt { .. } => Response::RestartSelection {
                    selection: longest.clone(),
                },
            };
            let mut terminal = snapshot.clone();
            terminal.revision = 1;
            for status in [
                Status::Answered {
                    id: longest.clone(),
                    response,
                    at_unix_ms: u64::MAX,
                },
                Status::Cancelled {
                    id: longest,
                    at_unix_ms: u64::MAX,
                },
                Status::Expired {
                    at_unix_ms: u64::MAX,
                },
            ] {
                terminal.status = status;
                if serde_json::to_vec(&terminal)
                    .map_err(|_| InteractionError::Snapshot)?
                    .len()
                    > limits.max_snapshot_bytes
                {
                    return Err(InteractionError::Limit);
                }
            }
        }
        Ok(Self { snapshot, limits })
    }
    /// Read the sole persistence representation; mutating a clone requires validation again.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    /// Pure evaluation. `now` must be trusted host time, never the responder's backdated timestamp.
    /// The host supplies reliable time; unchanged observations do not establish a durable clock watermark.
    pub fn evaluate(&self, command: Command, now: u64) -> Result<Evaluation, InteractionError> {
        let s = &self.snapshot;
        let previous_time = match s.status {
            Status::Pending => s.opened_at_unix_ms,
            Status::Answered { at_unix_ms, .. }
            | Status::Cancelled { at_unix_ms, .. }
            | Status::Expired { at_unix_ms } => at_unix_ms,
        };
        if now < previous_time {
            return Err(InteractionError::Clock);
        }
        let unchanged = |outcome| {
            Ok(Evaluation {
                outcome,
                transition: None,
            })
        };
        if !matches!(s.status, Status::Pending) {
            let (committed_id, repeated) = match &s.status {
                Status::Answered { id, response, .. } => (
                    Some(id),
                    command
                        == (Command::Answer {
                            id: id.clone(),
                            response: response.clone(),
                        }),
                ),
                Status::Cancelled { id, .. } => {
                    (Some(id), command == (Command::Cancel { id: id.clone() }))
                }
                Status::Expired { .. } => (None, command == Command::CheckExpiry),
                Status::Pending => unreachable!(),
            };
            if repeated {
                return unchanged(Outcome::Duplicate);
            }
            let incoming_id = match &command {
                Command::Answer { id, .. } | Command::Cancel { id } => Some(id),
                Command::CheckExpiry => None,
            };
            if committed_id.is_some() && committed_id == incoming_id {
                return Err(InteractionError::IdempotencyConflict);
            }
            return unchanged(Outcome::Late);
        }
        let (status, outcome) = if now >= s.spec.expires_at_unix_ms {
            (Status::Expired { at_unix_ms: now }, Outcome::Expired)
        } else {
            match command {
                Command::CheckExpiry => return unchanged(Outcome::NotDue),
                Command::Cancel { id } => (
                    Status::Cancelled {
                        id,
                        at_unix_ms: now,
                    },
                    Outcome::Cancelled,
                ),
                Command::Answer { id, response } => {
                    if !s.spec.kind.accepts(&response) {
                        return Err(InteractionError::ResponseKind);
                    }
                    (
                        Status::Answered {
                            id,
                            response,
                            at_unix_ms: now,
                        },
                        Outcome::Answered,
                    )
                }
            }
        };
        let mut next = s.clone();
        next.revision = 1;
        next.status = status;
        Ok(Evaluation {
            outcome,
            transition: Some(Transition {
                expected_revision: s.revision,
                next: Self::restore(next, self.limits)?,
            }),
        })
    }
}
fn validate_limits(limits: Limits) -> Result<(), InteractionError> {
    if limits.max_snapshot_bytes == 0 || limits.max_lifetime_ms == 0 {
        Err(InteractionError::Configuration)
    } else {
        Ok(())
    }
}

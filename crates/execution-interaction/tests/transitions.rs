use execution_interaction::*;
fn reference(s: &str) -> Reference {
    Reference::new(s).unwrap()
}
fn limits() -> Limits {
    Limits {
        max_snapshot_bytes: 4096,
        max_lifetime_ms: 1000,
    }
}
fn open(kind: Kind) -> Interaction {
    Interaction::open(
        Spec {
            id: reference("i-1"),
            subject: reference("authority-task-plan-1"),
            kind,
            expires_at_unix_ms: 200,
        },
        100,
        limits(),
    )
    .unwrap()
}
fn answer(id: &str, response: Response) -> Command {
    Command::Answer {
        id: reference(id),
        response,
    }
}
fn confirmation() -> Interaction {
    open(Kind::UserConfirmation {
        purpose: ConfirmationPurpose::CloseApplication,
    })
}
#[test]
fn all_kinds_finish_only_as_answers() {
    let cases = [
        (
            Kind::UserConfirmation {
                purpose: ConfirmationPurpose::Continue,
            },
            Response::Confirmation { accepted: true },
        ),
        (
            Kind::PrivacyConsent {
                scope: reference("scope-1"),
            },
            Response::PrivacyConsent { accepted: false },
        ),
        (
            Kind::AdministratorAuthorization {
                request: reference("approval-request-1"),
            },
            Response::AdministratorDecision {
                record: reference("unverified-record-1"),
            },
        ),
        (
            Kind::ParameterInput {
                schema: reference("schema-1"),
            },
            Response::ParameterSubmission {
                submission: reference("submission-1"),
            },
        ),
        (
            Kind::MaintenanceWindow {
                options: reference("windows-1"),
            },
            Response::MaintenanceSelection {
                selection: reference("window-1"),
            },
        ),
        (
            Kind::RestartPrompt {
                options: reference("restart-options-1"),
            },
            Response::RestartSelection {
                selection: reference("later-1"),
            },
        ),
    ];
    for (kind, response) in cases {
        let interaction = open(kind);
        let evaluated = interaction.evaluate(answer("r-1", response), 199).unwrap();
        let transition = evaluated.transition.unwrap();
        assert_eq!(transition.expected_revision, 0);
        assert_eq!(transition.next.snapshot().revision, 1);
        assert!(matches!(
            transition.next.snapshot().status,
            Status::Answered { .. }
        ));
        assert_eq!(evaluated.outcome, Outcome::Answered);
    }
}
#[test]
fn invalid_answer_does_not_win_and_deadline_has_priority() {
    let i = confirmation();
    assert_eq!(
        i.evaluate(
            answer("bad", Response::PrivacyConsent { accepted: true }),
            150
        )
        .unwrap_err(),
        InteractionError::ResponseKind
    );
    assert_eq!(i.snapshot().revision, 0);
    assert_eq!(
        i.evaluate(Command::CheckExpiry, 199).unwrap().outcome,
        Outcome::NotDue
    );
    for command in [
        answer("yes", Response::Confirmation { accepted: true }),
        Command::Cancel {
            id: reference("cancel"),
        },
        Command::CheckExpiry,
    ] {
        let result = i.evaluate(command, 200).unwrap();
        assert_eq!(result.outcome, Outcome::Expired);
        assert!(matches!(
            result.transition.unwrap().next.snapshot().status,
            Status::Expired { .. }
        ));
    }
    assert_eq!(
        i.evaluate(Command::CheckExpiry, 99).unwrap_err(),
        InteractionError::Clock
    );
}
#[test]
fn competing_commands_cannot_rewrite_the_committed_winner() {
    let i = confirmation();
    let a = answer("r-1", Response::Confirmation { accepted: true });
    let b = answer("r-2", Response::Confirmation { accepted: false });
    let cancel = Command::Cancel {
        id: reference("c-1"),
    };
    for (first, second) in [
        (a.clone(), b.clone()),
        (b, a.clone()),
        (a.clone(), cancel.clone()),
        (cancel, a.clone()),
    ] {
        let first_transition = i.evaluate(first.clone(), 150).unwrap().transition.unwrap();
        let competitor = i.evaluate(second.clone(), 150).unwrap().transition.unwrap();
        assert_eq!(
            competitor.expected_revision,
            first_transition.expected_revision
        );
        let committed = first_transition.next;
        assert_ne!(
            competitor.expected_revision,
            committed.snapshot().revision,
            "CAS rejects a stale transition"
        );
        assert_eq!(
            committed.evaluate(first, 151).unwrap().outcome,
            Outcome::Duplicate
        );
        let late = committed.evaluate(second, 151).unwrap();
        assert_eq!(late.outcome, Outcome::Late);
        assert!(late.transition.is_none());
    }
    let done = i.evaluate(a, 150).unwrap().transition.unwrap().next;
    assert_eq!(
        done.evaluate(
            answer("r-1", Response::Confirmation { accepted: false }),
            151
        )
        .unwrap_err(),
        InteractionError::IdempotencyConflict
    );
}
#[test]
fn snapshots_restore_pending_and_validate_terminal_invariants() {
    let i = confirmation();
    let encoded = serde_json::to_vec(i.snapshot()).unwrap();
    let restored = Interaction::decode(&encoded, limits()).unwrap();
    assert_eq!(restored.snapshot(), i.snapshot());
    let finished = restored
        .evaluate(Command::Cancel { id: reference("c") }, 150)
        .unwrap()
        .transition
        .unwrap()
        .next;
    assert!(Interaction::restore(finished.snapshot().clone(), limits()).is_ok());
    let mut tampered = finished.snapshot().clone();
    tampered.revision = 0;
    assert_eq!(
        Interaction::restore(tampered, limits()).unwrap_err(),
        InteractionError::Snapshot
    );
    let mut tampered = finished.snapshot().clone();
    tampered.status = Status::Cancelled {
        id: reference("c"),
        at_unix_ms: 200,
    };
    assert!(Interaction::restore(tampered, limits()).is_err());
    let mut bad = i.snapshot().clone();
    bad.version = 2;
    assert!(Interaction::restore(bad, limits()).is_err());
    assert_eq!(
        Interaction::decode(
            &encoded,
            Limits {
                max_snapshot_bytes: 1,
                ..limits()
            }
        )
        .unwrap_err(),
        InteractionError::Limit
    );
    assert!(Reference::new("user secret\n").is_err());
    assert!(Reference::new("x".repeat(129)).is_err());
}

#[test]
fn pending_state_reserves_space_for_every_terminal_response() {
    let i = confirmation();
    let pending_bytes = serde_json::to_vec(i.snapshot()).unwrap().len();
    let small = Limits {
        max_snapshot_bytes: pending_bytes,
        ..limits()
    };
    assert!(
        Interaction::open(i.snapshot().spec.clone(), 100, small).is_err(),
        "must not create a wait that cannot record expiry or an answer"
    );
}

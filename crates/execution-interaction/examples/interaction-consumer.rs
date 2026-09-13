use execution_interaction::*;
fn reference(value: &str) -> Reference {
    Reference::new(value).unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limits = Limits {
        max_snapshot_bytes: 4096,
        max_lifetime_ms: 1000,
    };
    let interaction = Interaction::open(
        Spec {
            id: reference("interaction-1"),
            subject: reference("test-authority-task-plan-1"),
            kind: Kind::AdministratorAuthorization {
                request: reference("approval-request-1"),
            },
            expires_at_unix_ms: 200,
        },
        100,
        limits,
    )?;
    let restored = Interaction::decode(&serde_json::to_vec(interaction.snapshot())?, limits)?;
    let answer = Command::Answer {
        id: reference("answer-1"),
        response: Response::AdministratorDecision {
            record: reference("unverified-decision-1"),
        },
    };
    let transition = restored.evaluate(answer.clone(), 150)?.transition.unwrap();
    assert_eq!(transition.expected_revision, 0);
    assert_eq!(
        transition.next.evaluate(answer, 151)?.outcome,
        Outcome::Duplicate
    );
    assert_eq!(
        transition
            .next
            .evaluate(
                Command::Cancel {
                    id: reference("cancel-1")
                },
                151
            )?
            .outcome,
        Outcome::Late
    );
    assert_eq!(
        restored.evaluate(Command::CheckExpiry, 200)?.outcome,
        Outcome::Expired
    );
    println!("execution-interaction: fixed test state only; no authorization, task cancellation or persistent CAS performed");
    Ok(())
}

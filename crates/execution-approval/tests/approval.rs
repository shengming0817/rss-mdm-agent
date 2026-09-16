use execution_admission::{
    decide, AdmissionLimits, AuthorityFacts, AuthorityVerifier, DecisionOutcome, DelegationFacts,
    Rule, RuleEffect, SubjectFacts,
};
use execution_approval::*;
use execution_contract::*;
use std::cell::Cell;

fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn reference(s: &str) -> VersionedRef {
    VersionedRef {
        id: id(s),
        revision: id("1"),
    }
}
fn plan() -> FrozenPlan {
    let limits = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_attempts: 3,
    };
    let bytes = include_bytes!("../../execution-contract/tests/fixtures/plan.json");
    FrozenPlan::freeze(decode_plan(bytes, &limits).unwrap(), &limits).unwrap()
}
struct Policy(FrozenPlan, Vec<RuleEffect>);
impl AuthorityVerifier for Policy {
    fn verify(
        &self,
        _: &FrozenPlan,
    ) -> Result<AuthorityFacts, execution_admission::VerificationError> {
        let s = self.0.spec();
        Ok(AuthorityFacts {
            subject: SubjectFacts {
                authority: s.request.authority.clone(),
                actor: s.request.actor.clone(),
                validity: s.validity,
                budget: s.budget,
            },
            delegation: s.request.delegation.as_ref().map(|r| DelegationFacts {
                reference: r.clone(),
                scope: self.0.clone(),
            }),
            policy: s.policy.clone(),
            rules: self
                .1
                .iter()
                .enumerate()
                .map(|(i, effect)| Rule {
                    id: id(&format!("rule-{i}")),
                    template: self.0.clone(),
                    effect: effect.clone(),
                })
                .collect(),
            now_unix_ms: 1500,
        })
    }
}
fn decision(p: &FrozenPlan, effects: Vec<RuleEffect>) -> execution_admission::AdmissionDecision {
    decide(
        p,
        &Policy(p.clone(), effects),
        AdmissionLimits { max_rules: 16 },
    )
}
struct Verifier {
    facts: ApprovalFacts,
    calls: Cell<usize>,
    fail: bool,
}
impl ApprovalVerifier for Verifier {
    fn verify(
        &self,
        _: &FrozenPlan,
        _: &[VersionedRef],
    ) -> Result<ApprovalFacts, VerificationError> {
        self.calls.set(self.calls.get() + 1);
        if self.fail {
            Err(VerificationError::Signature)
        } else {
            Ok(self.facts.clone())
        }
    }
}
fn verifier(p: &FrozenPlan) -> Verifier {
    Verifier {
        calls: Cell::new(0),
        fail: false,
        facts: ApprovalFacts {
            authority: p.spec().request.authority.clone(),
            policy: p.spec().policy.clone(),
            verification_revision: reference("authority-epoch"),
            now_unix_ms: 1500,
            fresh_until_unix_ms: 1900,
            records: vec![ApprovalRecord {
                reference: reference("grant"),
                approver: ActorId::new("approver").unwrap(),
                plan_id: p.spec().plan_id.clone(),
                plan_digest: p.digest().clone(),
                profiles: vec![reference("admin"), reference("security")],
                validity: p.spec().validity,
                status: ApprovalStatus::Active,
                max_uses: 2,
                used: 0,
                consumption_revision: 0,
            }],
        },
    }
}
fn check(
    p: &FrozenPlan,
    d: &execution_admission::AdmissionDecision,
    bindings: &[ProfileApproval],
    v: &Verifier,
) -> ApprovalDecision {
    evaluate(
        p,
        d,
        &AttemptId::new("attempt-1").unwrap(),
        bindings,
        v,
        ApprovalLimits {
            max_profiles: 8,
            max_records: 8,
        },
    )
}
fn bindings() -> Vec<ProfileApproval> {
    ["admin", "security"]
        .iter()
        .map(|s| ProfileApproval {
            profile: reference(s),
            record: reference("grant"),
        })
        .collect()
}
fn required(p: &FrozenPlan) -> execution_admission::AdmissionDecision {
    decision(
        p,
        ["admin", "security"]
            .iter()
            .map(|s| RuleEffect::ApprovalRequired {
                profile: reference(s),
            })
            .collect(),
    )
}
#[test]
fn allowed_and_denied_never_verify_or_consume_approval() {
    let p = plan();
    let mut v = verifier(&p);
    v.fail = true;
    for (effect, expected) in [
        (RuleEffect::Allow, ApprovalOutcome::NotRequired),
        (
            RuleEffect::Deny,
            ApprovalOutcome::Rejected(Reason::AdmissionDenied),
        ),
    ] {
        let r = check(&p, &decision(&p, vec![effect]), &[], &v);
        assert_eq!(r.outcome(), &expected);
        assert!(r.consumptions().is_empty());
    }
    assert_eq!(v.calls.get(), 0);
}

#[test]
fn human_and_ai_share_the_same_approval_outcomes() {
    let p = plan();
    let Initiator::Human { os_session } = p.spec().request.initiator.clone() else {
        panic!("human fixture")
    };
    let mut spec = p.spec().clone();
    spec.request.initiator = Initiator::Ai {
        provider: id("test-provider"),
        os_session,
        provider_account: ProviderAccountRef {
            account: id("test-account"),
            config: reference("config"),
        },
        conversation: id("conversation"),
        tool_call: id("call"),
    };
    let limits = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_attempts: 3,
    };
    let ai = FrozenPlan::freeze(spec, &limits).unwrap();
    for p in [p, ai] {
        let v = verifier(&p);
        assert_eq!(
            check(&p, &decision(&p, vec![RuleEffect::Allow]), &[], &v).outcome(),
            &ApprovalOutcome::NotRequired
        );
        assert_eq!(
            check(&p, &required(&p), &bindings(), &v).outcome(),
            &ApprovalOutcome::Satisfied
        );
        assert_eq!(
            check(&p, &decision(&p, vec![RuleEffect::Deny]), &bindings(), &v).outcome(),
            &ApprovalOutcome::Rejected(Reason::AdmissionDenied)
        );
    }
}

#[test]
fn every_trusted_verifier_failure_is_preserved_without_consumption() {
    struct Reject(VerificationError);
    impl ApprovalVerifier for Reject {
        fn verify(
            &self,
            _: &FrozenPlan,
            _: &[VersionedRef],
        ) -> Result<ApprovalFacts, VerificationError> {
            Err(self.0)
        }
    }
    let p = plan();
    let d = required(&p);
    for error in [
        VerificationError::Signature,
        VerificationError::Issuer,
        VerificationError::Unavailable,
        VerificationError::Clock,
        VerificationError::Revocation,
        VerificationError::Policy,
    ] {
        let result = evaluate(
            &p,
            &d,
            &AttemptId::new("a").unwrap(),
            &bindings(),
            &Reject(error),
            ApprovalLimits {
                max_profiles: 8,
                max_records: 8,
            },
        );
        assert_eq!(
            result.outcome(),
            &ApprovalOutcome::Rejected(Reason::Verification(error))
        );
        assert!(result.consumptions().is_empty());
    }
}
#[test]
fn every_profile_is_required_but_one_record_is_consumed_only_once() {
    let p = plan();
    let v = verifier(&p);
    let d = required(&p);
    assert!(matches!(
        d.outcome(),
        DecisionOutcome::ApprovalRequired { .. }
    ));
    let r = check(&p, &d, &bindings(), &v);
    assert_eq!(r.outcome(), &ApprovalOutcome::Satisfied);
    assert_eq!(r.consumptions().len(), 1);
    let intent = &r.consumptions()[0];
    assert_eq!(intent.approval(), &reference("grant"));
    assert_eq!(intent.expected_consumption_revision(), 0);
    assert_eq!(intent.expected_uses(), 0);
    assert_eq!(intent.valid_until_unix_ms(), 1900);
    assert_eq!(intent.attempt_id().as_str(), "attempt-1");
    assert_eq!(check(&p, &d, &bindings(), &v), r);
    assert_eq!(v.facts.records[0].used, 0);
    for bs in [
        bindings()[..1].to_vec(),
        vec![bindings()[0].clone(), bindings()[0].clone()],
    ] {
        assert!(matches!(
            check(&p, &d, &bs, &v).outcome(),
            ApprovalOutcome::Rejected(_)
        ));
    }
}
#[test]
fn ordinary_references_do_not_bypass_signature_verification() {
    let p = plan();
    let mut v = verifier(&p);
    v.fail = true;
    assert_eq!(
        check(&p, &required(&p), &bindings(), &v).outcome(),
        &ApprovalOutcome::Rejected(Reason::Verification(VerificationError::Signature))
    );
}
#[test]
fn invalid_facts_reject_the_entire_batch_without_consumption() {
    let p = plan();
    let d = required(&p);
    for case in 0..12 {
        let mut v = verifier(&p);
        match case {
            0 => v.facts.records[0].status = ApprovalStatus::Revoked,
            1 => v.facts.records[0].status = ApprovalStatus::Unknown,
            2 => v.facts.records[0].used = 2,
            3 => v.facts.now_unix_ms = 2000,
            4 => v.facts.fresh_until_unix_ms = 1500,
            5 => v.facts.records[0].plan_id = PlanId::new("different").unwrap(),
            6 => v.facts.records[0].plan_digest = Digest::new("ab".repeat(32)).unwrap(),
            7 => v.facts.records[0].profiles.pop().map(|_| ()).unwrap(),
            8 => v.facts.records[0].reference = reference("unknown"),
            9 => v.facts.policy = reference("other-policy"),
            10 => v.facts.records[0].consumption_revision = u64::MAX,
            _ => v.facts.records.push(v.facts.records[0].clone()),
        }
        let r = check(&p, &d, &bindings(), &v);
        assert!(
            matches!(r.outcome(), ApprovalOutcome::Rejected(_)),
            "case {case}"
        );
        assert!(r.consumptions().is_empty());
    }
}
#[test]
fn a_decision_for_another_plan_cannot_be_reused_even_if_allowed() {
    let p = plan();
    let mut spec = p.spec().clone();
    spec.plan_id = PlanId::new("other").unwrap();
    let limits = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_attempts: 3,
    };
    let other = FrozenPlan::freeze(spec, &limits).unwrap();
    let v = verifier(&p);
    assert_eq!(
        check(&p, &decision(&other, vec![RuleEffect::Allow]), &[], &v).outcome(),
        &ApprovalOutcome::Rejected(Reason::PlanMismatch)
    );
    assert_eq!(v.calls.get(), 0);
}

#[test]
fn separate_records_are_all_or_nothing_and_namespaces_must_match() {
    let p = plan();
    let d = required(&p);
    let mut v = verifier(&p);
    v.facts.records[0].profiles = vec![reference("admin")];
    let mut second = v.facts.records[0].clone();
    second.reference = reference("second");
    second.profiles = vec![reference("security")];
    v.facts.records.push(second);
    let mut bs = bindings();
    bs[1].record = reference("second");
    assert_eq!(check(&p, &d, &bs, &v).consumptions().len(), 2);
    v.facts.records[1].status = ApprovalStatus::Revoked;
    assert!(check(&p, &d, &bs, &v).consumptions().is_empty());
    let mut v = verifier(&p);
    v.facts.authority = Authority::Test {
        id: id("other-authority"),
    };
    assert_eq!(
        check(&p, &d, &bindings(), &v).outcome(),
        &ApprovalOutcome::Rejected(Reason::Context)
    );
    let mut bs = bindings();
    bs[1].record.revision = id("2");
    assert_eq!(
        check(&p, &d, &bs, &verifier(&p)).outcome(),
        &ApprovalOutcome::Rejected(Reason::Record)
    );
}

// A deterministic transaction model, not SQLite/concurrency integration evidence.
#[test]
fn consumption_and_attempt_intent_commit_together_and_replay_does_not_charge() {
    use execution_lifecycle as lifecycle;
    struct NoEvidence;
    impl lifecycle::ObservationVerifier for NoEvidence {
        fn verify(
            &self,
            _: &FrozenPlan,
            _: &AttemptId,
            _: &EvidenceRef,
            _: u64,
        ) -> Result<lifecycle::ObservationFacts, lifecycle::ObservationError> {
            Err(lifecycle::ObservationError::Unavailable)
        }
    }
    struct Store {
        state: lifecycle::Execution,
        used: u32,
        revision: u64,
    }
    impl Store {
        fn commit(
            &mut self,
            decision: &ApprovalDecision,
            transition: &lifecycle::Transition,
            now: u64,
            fail: bool,
        ) -> bool {
            let next = transition.next.snapshot();
            let Some(attempt) = next.attempt.as_ref() else {
                return false;
            };
            if decision.outcome() != &ApprovalOutcome::Satisfied
                || self.state.snapshot().revision != transition.expected_revision
                || decision.plan_id() != &next.plan_id
                || decision.plan_digest() != &next.plan_digest
                || decision.attempt_id() != &attempt.id
                || decision.consumptions().len() != 1
            {
                return false;
            }
            let i = &decision.consumptions()[0];
            if i.expected_uses() != self.used
                || i.expected_consumption_revision() != self.revision
                || i.verification_revision() != &reference("authority-epoch")
                || now >= i.valid_until_unix_ms()
                || fail
            {
                return false;
            }
            self.used += 1;
            self.revision += 1;
            self.state = transition.next.clone();
            true
        }
    }
    let p = plan();
    let d = required(&p);
    let v = verifier(&p);
    let approval = check(&p, &d, &bindings(), &v);
    let mut s = lifecycle::Execution::open(
        p.clone(),
        1000,
        lifecycle::Limits {
            max_snapshot_bytes: 16384,
        },
    )
    .unwrap();
    let prepare = lifecycle::Event {
        id: EventId::new("prepare").unwrap(),
        expected_revision: 0,
        command: lifecycle::Command::Prepare,
    };
    s = s
        .evaluate(prepare, 1100, &NoEvidence)
        .unwrap()
        .transition
        .unwrap()
        .next;
    let event = lifecycle::Event {
        id: EventId::new("intent").unwrap(),
        expected_revision: s.snapshot().revision,
        command: lifecycle::Command::BeginAttempt {
            attempt_id: AttemptId::new("attempt-1").unwrap(),
            runner: id("test-runner"),
            mode: lifecycle::ExecutionMode::Test,
        },
    };
    let candidate = s
        .evaluate(event.clone(), 1500, &NoEvidence)
        .unwrap()
        .transition
        .unwrap();
    let mut store = Store {
        state: s,
        used: 0,
        revision: 0,
    };
    assert!(!store.commit(&approval, &candidate, 1500, true));
    assert_eq!(store.used, 0);
    assert_eq!(store.state.snapshot().attempts, 0);
    assert_eq!(store.state.snapshot().first_attempt_at_unix_ms, None);
    assert!(!store.commit(&approval, &candidate, 1900, false));
    assert!(store.commit(&approval, &candidate, 1500, false));
    assert_eq!(store.used, 1);
    assert_eq!(store.state.snapshot().attempts, 1);
    assert_eq!(store.state.snapshot().first_attempt_at_unix_ms, Some(1500));
    assert!(!store.commit(&approval, &candidate, 1500, false));
    let replay = store.state.evaluate(event, 1600, &NoEvidence).unwrap();
    assert_eq!(replay.outcome, lifecycle::EventOutcome::Duplicate);
    assert!(replay.transition.is_none());
    assert_eq!(store.used, 1);
    // The transaction charged an admitted intent even though no runner was dispatched.
    assert_eq!(store.state.phase(), lifecycle::Phase::Starting);
}

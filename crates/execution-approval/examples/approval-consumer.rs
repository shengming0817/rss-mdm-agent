use execution_admission::{
    decide, AdmissionLimits, AuthorityFacts, AuthorityVerifier, DelegationFacts, Rule, RuleEffect,
    SubjectFacts,
};
use execution_approval::*;
use execution_contract::*;

// Explicit test fixture ports; never use these as production adapters.
struct TestPorts {
    plan: FrozenPlan,
    effect: RuleEffect,
}
fn reference(s: &str) -> VersionedRef {
    VersionedRef {
        id: Id::new(s).unwrap(),
        revision: Id::new("1").unwrap(),
    }
}
impl AuthorityVerifier for TestPorts {
    fn verify(
        &self,
        plan: &FrozenPlan,
        _: &AttemptId,
    ) -> Result<AuthorityFacts, execution_admission::VerificationError> {
        if !matches!(plan.spec().request.authority, Authority::Test { .. }) {
            return Err(execution_admission::VerificationError::Subject);
        }
        let s = self.plan.spec();
        Ok(AuthorityFacts {
            subject: SubjectFacts {
                authority: s.request.authority.clone(),
                actor: s.request.actor.clone(),
                validity: s.validity,
                budget: s.budget,
            },
            delegation: s.request.delegation.as_ref().map(|r| DelegationFacts {
                reference: r.clone(),
                scope: self.plan.clone(),
            }),
            policy: s.policy.clone(),
            rules: vec![Rule {
                id: Id::new("test-rule").unwrap(),
                template: self.plan.clone(),
                effect: self.effect.clone(),
            }],
            now_unix_ms: s.validity.not_before_unix_ms,
            verification_revision: s.policy.clone(),
            fresh_until_unix_ms: s.validity.expires_at_unix_ms,
        })
    }
}
impl ApprovalVerifier for TestPorts {
    fn verify(
        &self,
        plan: &FrozenPlan,
        records: &[VersionedRef],
    ) -> Result<ApprovalFacts, VerificationError> {
        if !matches!(plan.spec().request.authority, Authority::Test { .. })
            || records != [reference("test-grant")]
        {
            return Err(VerificationError::Issuer);
        }
        let s = self.plan.spec();
        Ok(ApprovalFacts {
            authority: s.request.authority.clone(),
            policy: s.policy.clone(),
            verification_revision: reference("test-epoch"),
            now_unix_ms: s.validity.not_before_unix_ms,
            fresh_until_unix_ms: s.validity.expires_at_unix_ms,
            records: vec![ApprovalRecord {
                reference: reference("test-grant"),
                approver: ActorId::new("test-approver").unwrap(),
                plan_id: s.plan_id.clone(),
                plan_digest: self.plan.digest().clone(),
                profiles: vec![reference("test-admin")],
                validity: s.validity,
                status: ApprovalStatus::Active,
                max_uses: 1,
                used: 0,
                consumption_revision: 0,
            }],
        })
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(std::env::args().nth(1).ok_or("test plan path required")?)?;
    let limits = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_stdin_bytes: 65536,
        max_attempts: 3,
    };
    let plan = FrozenPlan::freeze(decode_plan(&bytes, &limits)?, &limits)?;
    let mut ports = TestPorts {
        plan: plan.clone(),
        effect: RuleEffect::ApprovalRequired {
            profile: reference("test-admin"),
        },
    };
    let bindings = [ProfileApproval {
        profile: reference("test-admin"),
        record: reference("test-grant"),
    }];
    let attempt = AttemptId::new("attempt-1")?;
    let bounds = ApprovalLimits {
        max_profiles: 8,
        max_records: 8,
    };
    let admission = decide(&plan, &attempt, &ports, AdmissionLimits { max_rules: 8 });
    let result = evaluate(&plan, &admission, &bindings, &ports, bounds);
    assert_eq!(result.outcome(), &ApprovalOutcome::Satisfied);
    assert_eq!(result.consumptions().len(), 1);
    assert_eq!(result.consumptions()[0].attempt_id(), &attempt);
    ports.effect = RuleEffect::Allow;
    let admission = decide(&plan, &attempt, &ports, AdmissionLimits { max_rules: 8 });
    let allowed = evaluate(&plan, &admission, &[], &ports, bounds);
    assert_eq!(allowed.outcome(), &ApprovalOutcome::NotRequired);
    assert!(allowed.consumptions().is_empty());
    ports.effect = RuleEffect::Deny;
    let admission = decide(&plan, &attempt, &ports, AdmissionLimits { max_rules: 8 });
    assert_eq!(
        evaluate(&plan, &admission, &bindings, &ports, bounds).outcome(),
        &ApprovalOutcome::Rejected(Reason::AdmissionDenied)
    );
    println!("execution-approval: test ports only; applicability is not signing, atomic consumption or execution");
    Ok(())
}

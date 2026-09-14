use execution_admission::Rule;
use execution_admission::*;
use execution_contract::*;

// Explicit fixture authority only. It must never be used as a production authentication adapter.
struct TestAuthority {
    template: FrozenPlan,
    effect: RuleEffect,
}
impl AuthorityVerifier for TestAuthority {
    fn verify(&self, plan: &FrozenPlan) -> Result<AuthorityFacts, VerificationError> {
        if !matches!(plan.spec().request.authority, Authority::Test { .. })
            || !matches!(
                self.template.spec().request.authority,
                Authority::Test { .. }
            )
        {
            return Err(VerificationError::Subject);
        }
        let s = self.template.spec();
        Ok(AuthorityFacts {
            subject: SubjectFacts {
                authority: s.request.authority.clone(),
                actor: s.request.actor.clone(),
                validity: s.validity,
                budget: s.budget,
            },
            delegation: s
                .request
                .delegation
                .as_ref()
                .map(|reference| DelegationFacts {
                    reference: reference.clone(),
                    scope: self.template.clone(),
                }),
            policy: s.policy.clone(),
            rules: vec![Rule {
                id: Id::new("fixed-test-rule").unwrap(),
                template: self.template.clone(),
                effect: self.effect.clone(),
            }],
            now_unix_ms: s.validity.not_before_unix_ms,
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
        max_attempts: 3,
    };
    let plan = FrozenPlan::freeze(decode_plan(&bytes, &limits)?, &limits)?;
    let mut authority = TestAuthority {
        template: plan.clone(),
        effect: RuleEffect::Allow,
    };
    let bounds = AdmissionLimits { max_rules: 16 };
    assert_eq!(
        decide(&plan, &authority, bounds).outcome(),
        &DecisionOutcome::Allowed
    );
    authority.effect = RuleEffect::ApprovalRequired {
        profile: VersionedRef {
            id: Id::new("test-admin")?,
            revision: Id::new("1")?,
        },
    };
    assert!(matches!(
        decide(&plan, &authority, bounds).outcome(),
        DecisionOutcome::ApprovalRequired { .. }
    ));
    authority.effect = RuleEffect::Deny;
    assert_eq!(
        decide(&plan, &authority, bounds).outcome(),
        &DecisionOutcome::Denied
    );
    authority.effect = RuleEffect::Allow;
    let mut changed = plan.spec().clone();
    changed.launch.argv.push("--unapproved".into());
    let changed = FrozenPlan::freeze(changed, &limits)?;
    assert_eq!(
        decide(&changed, &authority, bounds).outcome(),
        &DecisionOutcome::Denied
    );
    println!("execution-admission: explicit test authority only; no real identity, approval signing/consumption or execution permit");
    Ok(())
}

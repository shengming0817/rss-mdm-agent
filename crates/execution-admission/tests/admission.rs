use execution_admission::Rule;
use execution_admission::*;
use execution_contract::*;
fn id(v: &str) -> Id {
    Id::new(v).unwrap()
}
fn limits() -> ExecutionLimits {
    ExecutionLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_stdin_bytes: 65536,
        max_attempts: 3,
    }
}
fn plan() -> FrozenExecution {
    FrozenExecution::freeze(
        decode_execution(
            include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
            &limits(),
        )
        .unwrap(),
        &limits(),
    )
    .unwrap()
}
#[derive(Clone)]
struct TestAuthority(Result<AuthorityFacts, VerificationError>);
impl AuthorityVerifier for TestAuthority {
    fn verify(
        &self,
        _: &FrozenExecution,
        _: &AttemptId,
    ) -> Result<AuthorityFacts, VerificationError> {
        self.0.clone()
    }
}
fn authority(p: &FrozenExecution) -> TestAuthority {
    let s = p.spec();
    TestAuthority(Ok(AuthorityFacts {
        verified_origin: s.request.initiator.clone(),
        risk: Some(execution_admission::RiskLevel::One),
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
                scope: p.clone(),
            }),
        policy: s.policy.clone(),
        rules: vec![Rule {
            id: id("allow-1"),
            template: p.clone(),
            effect: RuleEffect::Allow,
        }],
        now_unix_ms: 1500,
        verification_revision: VersionedRef {
            id: id("authority-epoch"),
            revision: id("1"),
        },
        fresh_until_unix_ms: 1950,
    }))
}
fn decide_for(p: &FrozenExecution, a: &TestAuthority) -> AdmissionDecision {
    decide(
        p,
        &AttemptId::new("attempt-1").unwrap(),
        a,
        AdmissionLimits { max_rules: 32 },
    )
}
fn facts(a: &mut TestAuthority) -> &mut AuthorityFacts {
    a.0.as_mut().unwrap()
}
#[test]
fn each_attempt_obtains_current_authority_and_an_exclusive_deadline() {
    use std::cell::Cell;
    struct ChangingAuthority {
        inner: TestAuthority,
        calls: Cell<usize>,
    }
    impl AuthorityVerifier for ChangingAuthority {
        fn verify(
            &self,
            _: &FrozenExecution,
            attempt: &AttemptId,
        ) -> Result<AuthorityFacts, VerificationError> {
            self.calls.set(self.calls.get() + 1);
            assert_eq!(attempt.as_str(), format!("attempt-{}", self.calls.get()));
            if self.calls.get() == 1 {
                self.inner.0.clone()
            } else {
                Err(VerificationError::Revocation)
            }
        }
    }
    let p = plan();
    let a = ChangingAuthority {
        inner: authority(&p),
        calls: Cell::new(0),
    };
    let first = decide(
        &p,
        &AttemptId::new("attempt-1").unwrap(),
        &a,
        AdmissionLimits { max_rules: 32 },
    );
    let second = decide(
        &p,
        &AttemptId::new("attempt-2").unwrap(),
        &a,
        AdmissionLimits { max_rules: 32 },
    );
    assert_eq!(first.outcome(), &DecisionOutcome::Allowed);
    assert_eq!(second.outcome(), &DecisionOutcome::Denied);
    assert!(second.validity().is_none());
    let validity = first.validity().unwrap();
    assert!(validity.is_current(1949, validity.revision()));
    assert!(!validity.is_current(1950, validity.revision()));
    for deadline in [0, 1500, 2500] {
        let mut a = authority(&p);
        facts(&mut a).fresh_until_unix_ms = deadline;
        let d = decide_for(&p, &a);
        if deadline <= 1500 {
            assert_eq!(d.reason(), Reason::StaleVerification);
            assert!(d.validity().is_none());
        } else {
            assert_eq!(
                d.validity().unwrap().valid_until_unix_ms(),
                p.spec().validity.expires_at_unix_ms
            );
        }
    }
}
#[test]
fn sources_share_actor_permissions_but_have_distinct_plan_bindings() {
    let p = plan();
    let a = authority(&p);
    assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Allowed);
    let mut spec = p.spec().clone();
    let os = match &spec.request.initiator {
        Initiator::Human { os_session } => os_session.clone(),
        _ => unreachable!(),
    };
    spec.request.initiator = Initiator::Ai {
        provider: id("ai"),
        os_session: os,
        config: VersionedRef {
            id: id("cfg"),
            revision: id("1"),
        },
        conversation: id("chat"),
        tool_call: id("call"),
    };
    let ai = FrozenExecution::freeze(spec, &limits()).unwrap();
    let result = decide_for(&ai, &authority(&ai));
    assert_eq!(result.outcome(), &DecisionOutcome::Allowed);
    assert_eq!(result.content_digest(), ai.digest());
    assert_ne!(result.content_digest(), p.digest());
    let mut spec = ai.spec().clone();
    spec.request.initiator = Initiator::Policy {
        policy: spec.policy.clone(),
    };
    let policy = FrozenExecution::freeze(spec, &limits()).unwrap();
    assert_eq!(
        decide_for(&policy, &authority(&policy)).outcome(),
        &DecisionOutcome::Allowed
    );
}
#[test]
fn unavailable_context_policy_or_time_always_denies() {
    let p = plan();
    for failure in [
        VerificationError::Subject,
        VerificationError::Policy,
        VerificationError::Delegation,
        VerificationError::Clock,
        VerificationError::Revocation,
    ] {
        assert_eq!(
            decide_for(&p, &TestAuthority(Err(failure))).outcome(),
            &DecisionOutcome::Denied
        );
    }
    let mut a = authority(&p);
    facts(&mut a).rules.clear();
    assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Denied);
    facts(&mut a).policy.revision = id("changed");
    assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Denied);
    assert_eq!(
        decide(
            &p,
            &AttemptId::new("attempt-1").unwrap(),
            &a,
            AdmissionLimits { max_rules: 0 }
        )
        .outcome(),
        &DecisionOutcome::Denied
    );
}
#[test]
fn subjects_delegations_and_windows_are_intersections() {
    let p = plan();
    for change in 0..9 {
        let mut a = authority(&p);
        let f = facts(&mut a);
        match change {
            0 => f.subject.actor = ActorId::new("other").unwrap(),
            1 => {
                f.subject.authority = Authority::Enterprise {
                    id: id("test-authority"),
                    tenant: id("other"),
                }
            }
            2 => f.delegation = None,
            3 => f.delegation.as_mut().unwrap().reference.revision = id("other"),
            4 => f.now_unix_ms = 2000,
            5 => f.now_unix_ms = 999,
            6 => f.subject.validity.expires_at_unix_ms = 1999,
            7 => {
                let mut s = f.delegation.as_ref().unwrap().scope.spec().clone();
                s.request.target.device = DeviceId::new("other").unwrap();
                f.delegation.as_mut().unwrap().scope =
                    FrozenExecution::freeze(s, &limits()).unwrap();
            }
            _ => f.subject.budget.total_output_bytes -= 1,
        }
        assert_eq!(
            decide_for(&p, &a).outcome(),
            &DecisionOutcome::Denied,
            "case {change}"
        );
    }
    let mut s = p.spec().clone();
    s.request.delegation = None;
    let direct = FrozenExecution::freeze(s, &limits()).unwrap();
    let mut a = authority(&p);
    assert_eq!(decide_for(&direct, &a).outcome(), &DecisionOutcome::Denied);
    facts(&mut a).delegation = None;
    assert_eq!(decide_for(&direct, &a).outcome(), &DecisionOutcome::Allowed);
}
#[test]
fn no_launch_parameter_identity_or_constraint_substitution_is_allowed() {
    let mut direct = plan().spec().clone();
    direct.request.delegation = None;
    let p = FrozenExecution::freeze(direct, &limits()).unwrap();
    let a = authority(&p);
    for case in 0..21 {
        let mut s = p.spec().clone();
        match case {
            0 => s.launch.artifact.resource.revision = id("other"),
            1 => s.launch.artifact.sha256 = Digest::new("34".repeat(32)).unwrap(),
            2 => s.launch.interpreter.artifact.resource.revision = id("other"),
            3 => s.launch.argv.push(execution_contract::LaunchArg::Literal {
                value: "--unsafe".into(),
            }),
            4 => s.launch.cwd = "/other".into(),
            5 => {
                s.launch.env.clear();
            }
            6 => {
                s.request.parameters.clear();
            }
            7 => {
                let execution_contract::IsolationPolicy::Restricted {
                    require_sandbox, ..
                } = &mut s.constraints
                else {
                    panic!("restricted fixture")
                };
                *require_sandbox = false;
            }
            8 => {
                let execution_contract::IsolationPolicy::Restricted {
                    allow_child_processes,
                    ..
                } = &mut s.constraints
                else {
                    panic!("restricted fixture")
                };
                *allow_child_processes = true;
            }
            9 => {
                let execution_contract::IsolationPolicy::Restricted { write_paths, .. } =
                    &mut s.constraints
                else {
                    panic!("restricted fixture")
                };
                write_paths.push("/other".into());
            }
            10 => s.request.operation.action = id("other"),
            11 => s.request.operation.resource.revision = id("other"),
            12 => s.request.target.device = DeviceId::new("other").unwrap(),
            13 => s.request.target.scope = TargetScope::Device {},
            14 => {
                s.run_as = RunAs::System {
                    platform: Platform::Linux,
                }
            }
            15 => {
                s.session_requirement = SessionRequirement::ActiveUser {
                    account: OsAccountRef {
                        platform: Platform::Linux,
                        subject: id("user"),
                    },
                }
            }
            16 => {
                let execution_contract::IsolationPolicy::Restricted { read_paths, .. } =
                    &mut s.constraints
                else {
                    panic!("restricted fixture")
                };
                read_paths.clear();
            }
            17 => s.launch.interpreter.profile.revision = id("2"),
            18 => s.launch.output.stdout = TextEncoding::Utf16Le,
            19 => s.launch.artifact_encoding = ArtifactEncoding::Utf8Bom,
            _ => {
                s.launch.stdin = StandardInput::Controlled {
                    reference: s.policy.clone(),
                    encoding: TextEncoding::Utf8,
                    max_bytes: 10,
                }
            }
        }
        let altered = FrozenExecution::freeze(s, &limits()).unwrap();
        assert_eq!(
            decide_for(&altered, &a).outcome(),
            &DecisionOutcome::Denied,
            "case {case}"
        );
    }
}
#[test]
fn rule_priority_is_order_independent_and_approval_is_not_denial() {
    let mut spec = plan().spec().clone();
    spec.request.initiator = Initiator::Policy {
        policy: spec.policy.clone(),
    };
    let p = FrozenExecution::freeze(spec, &limits()).unwrap();
    let mut a = authority(&p);
    let profile = VersionedRef {
        id: id("admin"),
        revision: id("1"),
    };
    facts(&mut a).rules.push(Rule {
        id: id("approval"),
        template: p.clone(),
        effect: RuleEffect::ApprovalRequired {
            profile: profile.clone(),
        },
    });
    let result = decide_for(&p, &a);
    assert_eq!(
        result.outcome(),
        &DecisionOutcome::ApprovalRequired {
            profiles: vec![profile]
        }
    );
    facts(&mut a).rules.reverse();
    assert_eq!(decide_for(&p, &a), result);
    facts(&mut a).rules.push(Rule {
        id: id("deny"),
        template: p.clone(),
        effect: RuleEffect::Deny,
    });
    assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Denied);
}
#[test]
fn each_budget_and_rule_window_is_enforced_without_permission_splicing() {
    let p = plan();
    for axis in 0..3 {
        let mut a = authority(&p);
        let f = facts(&mut a);
        match axis {
            0 => f.subject.budget.total_timeout_ms -= 1,
            1 => f.subject.budget.total_output_bytes -= 1,
            _ => f.subject.budget.max_attempts = 0,
        };
        assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Denied);
    }
    let mut a = authority(&p);
    let mut low = p.spec().clone();
    low.budget.total_timeout_ms -= 1;
    facts(&mut a).rules[0].template = FrozenExecution::freeze(low, &limits()).unwrap();
    let mut low = p.spec().clone();
    low.budget.total_output_bytes -= 1;
    facts(&mut a).rules.push(Rule {
        id: id("other-rule"),
        template: FrozenExecution::freeze(low, &limits()).unwrap(),
        effect: RuleEffect::Allow,
    });
    assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Denied);
    let mut a = authority(&p);
    let mut short = p.spec().clone();
    short.validity.expires_at_unix_ms = 1999;
    facts(&mut a).rules[0].template = FrozenExecution::freeze(short, &limits()).unwrap();
    assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Denied);
}

#[test]
fn active_denial_cannot_be_evaded_using_the_denial_templates_budget() {
    let p = plan();
    let mut a = authority(&p);
    let mut narrower = p.spec().clone();
    narrower.budget.total_timeout_ms = 1;
    narrower.validity.expires_at_unix_ms = 1600;
    facts(&mut a).rules.push(Rule {
        id: id("deny"),
        template: FrozenExecution::freeze(narrower, &limits()).unwrap(),
        effect: RuleEffect::Deny,
    });
    assert_eq!(decide_for(&p, &a).reason(), Reason::ExplicitDeny);
    facts(&mut a).now_unix_ms = 1600;
    assert_eq!(decide_for(&p, &a).outcome(), &DecisionOutcome::Allowed);
}
#[test]
fn invalid_rules_and_multiple_approval_profiles_are_not_silently_ignored() {
    let mut spec = plan().spec().clone();
    spec.request.initiator = Initiator::Policy {
        policy: spec.policy.clone(),
    };
    let p = FrozenExecution::freeze(spec, &limits()).unwrap();
    let mut a = authority(&p);
    let duplicate = facts(&mut a).rules[0].clone();
    facts(&mut a).rules.push(duplicate);
    assert_eq!(decide_for(&p, &a).reason(), Reason::InvalidPolicy);
    let mut a = authority(&p);
    let mut stale = p.spec().clone();
    stale.policy.revision = id("stale");
    facts(&mut a).rules[0].template = FrozenExecution::freeze(stale, &limits()).unwrap();
    assert_eq!(decide_for(&p, &a).reason(), Reason::InvalidPolicy);
    let mut a = authority(&p);
    for (key, profile) in [("r-b", "privacy"), ("r-a", "admin"), ("r-c", "admin")] {
        facts(&mut a).rules.push(Rule {
            id: id(key),
            template: p.clone(),
            effect: RuleEffect::ApprovalRequired {
                profile: VersionedRef {
                    id: id(profile),
                    revision: id("1"),
                },
            },
        });
    }
    let d = decide_for(&p, &a);
    assert_eq!(
        d.outcome(),
        &DecisionOutcome::ApprovalRequired {
            profiles: vec![
                VersionedRef {
                    id: id("admin"),
                    revision: id("1")
                },
                VersionedRef {
                    id: id("privacy"),
                    revision: id("1")
                }
            ]
        }
    );
    assert_eq!(
        d.rule_ids().iter().map(Id::as_str).collect::<Vec<_>>(),
        vec!["allow-1", "r-a", "r-b", "r-c"]
    );
    assert_eq!(
        decide(
            &p,
            &AttemptId::new("attempt-1").unwrap(),
            &a,
            AdmissionLimits { max_rules: 1 }
        )
        .reason(),
        Reason::Limit
    );
}

#[test]
fn trusted_origin_and_numeric_risk_choose_the_confirmation_gate() {
    let original = plan();
    let human = original.spec().request.initiator.clone();
    let ai = Initiator::Ai {
        os_session: match &human {
            Initiator::Human { os_session } => os_session.clone(),
            _ => panic!("human fixture"),
        },
        provider: id("provider"),
        config: VersionedRef {
            id: id("config"),
            revision: id("1"),
        },
        conversation: id("conversation"),
        tool_call: id("call"),
    };
    for origin in [human.clone(), ai.clone()] {
        for risk in [
            Some(RiskLevel::Zero),
            Some(RiskLevel::One),
            Some(RiskLevel::Two),
            Some(RiskLevel::Three),
            None,
        ] {
            let mut spec = original.spec().clone();
            spec.request.initiator = origin.clone();
            let p = FrozenExecution::freeze(spec, &limits()).unwrap();
            let mut a = authority(&p);
            facts(&mut a).verified_origin = origin.clone();
            facts(&mut a).risk = risk;
            let result = decide_for(&p, &a);
            let expected = match (&origin, risk) {
                (Initiator::Human { .. }, _) => ExecutionGate::Confirmation,
                (_, Some(RiskLevel::Zero | RiskLevel::One)) => ExecutionGate::Direct,
                (_, Some(RiskLevel::Two)) => ExecutionGate::Confirmation,
                _ => ExecutionGate::Blocked,
            };
            assert_eq!(result.execution_gate(), expected);
        }
    }
    let mut a = authority(&original);
    facts(&mut a).verified_origin = ai;
    assert_eq!(
        decide_for(&original, &a).outcome(),
        &DecisionOutcome::Denied
    );
}

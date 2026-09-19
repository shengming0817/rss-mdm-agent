#![allow(dead_code)]
use execution_admission::Rule;
use execution_admission::*;
use execution_app::*;
use execution_approval::ProfileApproval;
use execution_capability::*;
use execution_contract::*;
use execution_sqlite::*;
use std::sync::{Arc, Mutex};

pub fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
pub fn request(value: &str) -> RequestId {
    RequestId::new(value).unwrap()
}
pub fn reference(value: &str) -> VersionedRef {
    VersionedRef {
        id: id(value),
        revision: id("1"),
    }
}
pub fn plan() -> FrozenPlan {
    FrozenPlan::freeze(
        decode_plan(
            include_bytes!("../../../execution-contract/tests/fixtures/plan.json"),
            &test_store_limits().plan,
        )
        .unwrap(),
        &test_store_limits().plan,
    )
    .unwrap()
}
pub struct Database {
    pub path: std::path::PathBuf,
    root: std::path::PathBuf,
}
impl Database {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "execution-app-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            path: root.join("execution.db"),
            root,
        }
    }
    pub fn sql(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.path).unwrap()
    }
    pub fn count(&self, table: &str) -> u64 {
        self.sql()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub struct State {
    pub accesses: Option<Vec<Access>>,
    pub consumer: Option<Id>,
    pub capability_error: Option<execution_app::Error>,
    pub capability_hook: Option<(usize, Arc<dyn Fn() + Send + Sync>)>,
    pub read_hook: Option<Arc<dyn Fn() + Send + Sync>>,
    pub runner_facts: bool,
    pub bound: bool,
    pub actor: ActorId,
    pub now: u64,
    pub allow: bool,
    pub read: bool,
    pub audit: bool,
    pub approval: bool,
    pub capability: bool,
    pub capabilities: usize,
    pub block_capability_on: usize,
    pub admissions: usize,
    pub config_audits: Vec<ConfigChange>,
    pub grants: Vec<TrustedApproval>,
    pub policy_epoch: VersionedRef,
    pub approval_epoch: VersionedRef,
}
#[derive(Clone)]
pub struct TestHost {
    pub template: FrozenPlan,
    pub state: Arc<Mutex<State>>,
}
impl TestHost {
    pub fn new() -> Self {
        let template = plan();
        Self {
            state: Arc::new(Mutex::new(State {
                accesses: None,
                consumer: None,
                capability_error: None,
                capability_hook: None,
                read_hook: None,
                runner_facts: true,
                bound: true,
                actor: template.spec().request.actor.clone(),
                now: 1000,
                allow: true,
                read: true,
                audit: true,
                approval: false,
                capability: true,
                capabilities: 0,
                block_capability_on: usize::MAX,
                admissions: 0,
                config_audits: vec![],
                grants: vec![],
                policy_epoch: reference("authority-epoch"),
                approval_epoch: reference("approval-epoch"),
            })),
            template,
        }
    }
    pub fn grant(&self, uses: u32) {
        let mut s = self.state.lock().unwrap();
        s.grants = vec![TrustedApproval {
            state: ApprovalState::Active,
            definition: ApprovalDefinition {
                reference: reference("approval"),
                approver: ActorId::new("approver").unwrap(),
                plan_id: self.template.spec().plan_id.clone(),
                plan_digest: self.template.digest().clone(),
                profiles: vec![reference("profile")],
                validity: self.template.spec().validity,
                max_uses: uses,
            },
        }];
        s.approval_epoch = reference("approval-granted");
    }
}
impl AuthorityVerifier for TestHost {
    fn verify(&self, _: &FrozenPlan, _: &AttemptId) -> Result<AuthorityFacts, VerificationError> {
        let mut s = self.state.lock().unwrap();
        s.admissions += 1;
        let p = self.template.spec();
        Ok(AuthorityFacts {
            subject: SubjectFacts {
                authority: p.request.authority.clone(),
                actor: s.actor.clone(),
                validity: p.validity,
                budget: p.budget,
            },
            delegation: p
                .request
                .delegation
                .clone()
                .map(|reference| DelegationFacts {
                    reference,
                    scope: self.template.clone(),
                }),
            policy: p.policy.clone(),
            rules: vec![Rule {
                id: id("fixture-rule"),
                template: self.template.clone(),
                effect: if !s.allow {
                    RuleEffect::Deny
                } else if s.approval {
                    RuleEffect::ApprovalRequired {
                        profile: reference("profile"),
                    }
                } else {
                    RuleEffect::Allow
                },
            }],
            now_unix_ms: s.now,
            verification_revision: s.policy_epoch.clone(),
            fresh_until_unix_ms: 2000,
        })
    }
}
impl AppHost for TestHost {
    fn binding(&self) -> Result<Binding, execution_app::Error> {
        let s = self.state.lock().unwrap();
        if !s.bound {
            return Err(execution_app::Error::Unbound);
        }
        Ok(Binding {
            authority: self.template.spec().request.authority.clone(),
            actor: s.actor.clone(),
            device: self.template.spec().request.target.device.clone(),
        })
    }
    fn authorize(&self, r: AccessRequest<'_>) -> Result<(), execution_sqlite::Error> {
        let mut s = self.state.lock().unwrap();
        if !s.bound
            || r.scope.authority != self.template.spec().request.authority
            || r.scope.actor != s.actor
            || (r.access == Access::ReadResult && !s.read)
            || (r.access == Access::ReadAudit && !s.audit)
            || (r.access == Access::RunnerFact && !s.runner_facts)
            || s.accesses
                .as_ref()
                .is_some_and(|grants| !grants.contains(&r.access))
            || (r.access == Access::Deliver
                && s.consumer.as_ref().is_some_and(|id| r.consumer != Some(id)))
        {
            return Err(execution_sqlite::Error::Denied);
        }
        let hook = if matches!(
            r.access,
            Access::ReadResult | Access::Execute | Access::RunnerFact
        ) {
            s.read_hook.take()
        } else {
            None
        };
        drop(s);
        if let Some(hook) = hook {
            hook();
        }
        Ok(())
    }
    fn reliable_now(&self) -> Result<u64, execution_sqlite::Error> {
        Ok(self.state.lock().unwrap().now)
    }
    fn capabilities(&self, _: &FrozenPlan) -> Result<CapabilitySnapshot, execution_app::Error> {
        let mut s = self.state.lock().unwrap();
        s.capabilities += 1;
        let hook = if s
            .capability_hook
            .as_ref()
            .is_some_and(|(count, _)| *count == s.capabilities)
        {
            s.capability_hook.take().map(|(_, hook)| hook)
        } else {
            None
        };
        let available = s.capability && s.capabilities < s.block_capability_on;
        let p = self.template.spec();
        fn inventory<T>(value: Vec<T>, available: bool) -> Inventory<T> {
            Inventory {
                complete: true,
                entries: value
                    .into_iter()
                    .map(|capability| Entry {
                        capability,
                        availability: if available {
                            Availability::Available
                        } else {
                            Availability::Blocked
                        },
                    })
                    .collect(),
            }
        }
        let snapshot = CapabilitySnapshot {
            verified_at_unix_ms: s.now,
            fresh_until_unix_ms: 2000,
            environment: EnvironmentSnapshot {
                authority: p.request.authority.clone(),
                device: p.request.target.device.clone(),
                source: reference("fixture-capabilities"),
                platform: Some(p.request.target.platform),
                interpreters: inventory(vec![p.launch.interpreter.clone()], available),
                launch_io: inventory(
                    vec![
                        LaunchIoCapability::ControlledStdin(TextEncoding::Utf8),
                        LaunchIoCapability::CapturedText(TextEncoding::Utf8),
                    ],
                    available,
                ),
                run_as: inventory(vec![p.run_as.clone()], available),
                user_sessions: inventory(
                    match &p.run_as {
                        RunAs::User { account } => vec![account.clone()],
                        _ => vec![],
                    },
                    available,
                ),
                isolation: inventory(
                    vec![
                        Isolation::NetworkDenied,
                        Isolation::NetworkAllowlist,
                        Isolation::ReadPaths,
                        Isolation::WritePaths,
                        Isolation::ChildProcessesDenied,
                        Isolation::Sandbox,
                    ],
                    available,
                ),
            },
        };
        drop(s);
        if let Some(hook) = hook {
            hook();
        }
        if let Some(error) = self.state.lock().unwrap().capability_error {
            return Err(error);
        }
        Ok(snapshot)
    }
    fn trusted_snapshot(&self, _: &FrozenPlan) -> Result<TrustSnapshot, execution_sqlite::Error> {
        let s = self.state.lock().unwrap();
        Ok(TrustSnapshot {
            authorization_revision: s.policy_epoch.clone(),
            approval_revision: s.approval_epoch.clone(),
            fresh_until_unix_ms: 2000,
            approvals: s.grants.clone(),
        })
    }
    fn approval_bindings(
        &self,
        _: &FrozenPlan,
    ) -> Result<Vec<ProfileApproval>, execution_app::Error> {
        let s = self.state.lock().unwrap();
        Ok(if s.approval && !s.grants.is_empty() {
            vec![ProfileApproval {
                profile: reference("profile"),
                record: reference("approval"),
            }]
        } else {
            vec![]
        })
    }
    fn configuration_change(&self, change: &ConfigChange) -> Result<(), execution_app::Error> {
        self.state
            .lock()
            .unwrap()
            .config_audits
            .push(change.clone());
        Ok(())
    }
}

use super::execution::{catalog, now};
use crate::self_service::fixtures;
use execution_admission::*;
use execution_app::*;
use execution_approval::ProfileApproval;
use execution_capability::*;
use execution_contract::*;
use execution_sqlite::*;

pub fn id(s: &str) -> Id {
    Id::new(s).expect("static or digest ID")
}
pub fn reference(s: &str) -> VersionedRef {
    VersionedRef {
        id: id(s),
        revision: id("r1"),
    }
}
#[derive(Clone)]
pub struct S1Host;
impl S1Host {
    pub fn new() -> Self {
        Self
    }
    pub fn validate(&self, plan: &FrozenExecution) -> Result<(), execution_app::Error> {
        let p = plan.spec();
        if p.request.initiator != fixtures::human()
            && !super::origin::AiBinding::validate(&p.request.initiator)
        {
            return Err(execution_app::Error::Denied);
        }
        let catalog = catalog().map_err(|_| execution_app::Error::Configuration)?;
        let item = catalog
            .snapshot()
            .items
            .iter()
            .find(|i| {
                i.operations
                    .iter()
                    .any(|o| o.resource.reference == p.request.operation.resource)
            })
            .ok_or(execution_app::Error::Denied)?;
        let arguments = p
            .request
            .parameters
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    match value {
                        InputValue::Literal { value } => value.clone(),
                        InputValue::Secret { reference } => serde_json::to_value(reference)
                            .map_err(|_| execution_app::Error::InvalidInput)?,
                    },
                ))
            })
            .collect::<Result<serde_json::Map<String, serde_json::Value>, execution_app::Error>>(
            )?;
        let selection = serde_json::to_vec(&serde_json::json!({"catalog":catalog.reference(),"itemId":item.id,"variantId":"test","arguments":arguments})).map_err(|_| execution_app::Error::InvalidInput)?;
        let selected = catalog
            .select(&selection, &fixtures::CATALOG_LIMITS, &fixtures::PARAMETERS)
            .map_err(|_| execution_app::Error::Denied)?;
        let expected = fixtures::freeze(
            &selected,
            &p.request.request_id,
            p.validity.not_before_unix_ms,
            &p.request.initiator,
            &p.request.actor,
        )
        .map_err(|_| execution_app::Error::Denied)?;
        if expected.digest() != plan.digest() {
            return Err(execution_app::Error::Denied);
        }
        Ok(())
    }
}

impl AuthorityVerifier for S1Host {
    fn verify(
        &self,
        plan: &FrozenExecution,
        _: &AttemptId,
    ) -> Result<AuthorityFacts, VerificationError> {
        self.validate(plan).map_err(|_| VerificationError::Policy)?;
        let p = plan.spec();
        let time = now().map_err(|_| VerificationError::Policy)?;
        Ok(AuthorityFacts {
            verified_origin: p.request.initiator.clone(),
            risk: fixtures::risk(p.request.operation.resource.id.as_str()),
            subject: SubjectFacts {
                authority: p.request.authority.clone(),
                actor: p.request.actor.clone(),
                validity: p.validity,
                budget: p.budget,
            },
            delegation: None,
            policy: p.policy.clone(),
            rules: vec![execution_admission::Rule {
                id: id("s1-catalog-rule"),
                template: plan.clone(),
                effect: match p.request.operation.resource.id.as_str() {
                    "fixture-blocked" | "fixture-withdrawn" | "fixture-unsupported" => {
                        RuleEffect::Deny
                    }
                    _ => RuleEffect::Allow,
                },
            }],
            now_unix_ms: time,
            verification_revision: reference("s1-policy"),
            fresh_until_unix_ms: time + 1000,
        })
    }
}
impl AppHost for S1Host {
    fn service_binding(&self) -> Result<ServiceBinding, execution_app::Error> {
        Ok(ServiceBinding {
            authority: Authority::Test {
                id: id("desktop-fixture"),
            },
            device: DeviceId::new("fixture-device").unwrap(),
        })
    }
    fn authorize(
        &self,
        caller: &RequestContext,
        r: AccessRequest<'_>,
    ) -> Result<(), execution_sqlite::Error> {
        if caller.actor != r.scope.actor {
            return Err(execution_sqlite::Error::Denied);
        }
        self.authorize_service(r)
    }
    fn authorize_service(&self, r: AccessRequest<'_>) -> Result<(), execution_sqlite::Error> {
        let binding = self
            .service_binding()
            .map_err(|_| execution_sqlite::Error::Denied)?;
        if r.scope.authority != binding.authority {
            return Err(execution_sqlite::Error::Denied);
        }
        Ok(())
    }
    fn reliable_now(&self) -> Result<u64, execution_sqlite::Error> {
        now().map_err(|_| execution_sqlite::Error::Clock)
    }
    fn capabilities(
        &self,
        plan: &FrozenExecution,
    ) -> Result<CapabilitySnapshot, execution_app::Error> {
        self.validate(plan)?;
        let time = now()?;
        let p = plan.spec();
        fn inventory<T>(items: Vec<T>) -> Inventory<T> {
            Inventory {
                complete: true,
                entries: items
                    .into_iter()
                    .map(|capability| Entry {
                        capability,
                        availability: Availability::Available,
                    })
                    .collect(),
            }
        }
        Ok(CapabilitySnapshot {
            verified_at_unix_ms: time,
            fresh_until_unix_ms: time + 1000,
            environment: EnvironmentSnapshot {
                authority: p.request.authority.clone(),
                device: p.request.target.device.clone(),
                source: reference("s1-test-capabilities"),
                platform: Some(p.request.target.platform),
                software: execution_capability::Inventory {
                    complete: true,
                    entries: vec![],
                },
                interpreters: inventory(vec![p.launch.interpreter.clone()]),
                launch_io: inventory(vec![
                    LaunchIoCapability::ControlledStdin(TextEncoding::Utf8),
                    LaunchIoCapability::CapturedText(TextEncoding::Utf8),
                ]),
                run_as: inventory(vec![p.run_as.clone()]),
                user_sessions: inventory(match &p.run_as {
                    RunAs::User { account } => vec![account.clone()],
                    _ => vec![],
                }),
                isolation: inventory(vec![
                    Isolation::NetworkDenied,
                    Isolation::NetworkAllowlist,
                    Isolation::ReadPaths,
                    Isolation::WritePaths,
                    Isolation::ChildProcessesDenied,
                    Isolation::Sandbox,
                ]),
            },
        })
    }
    fn trusted_snapshot(
        &self,
        plan: &FrozenExecution,
    ) -> Result<TrustSnapshot, execution_sqlite::Error> {
        self.validate(plan)
            .map_err(|_| execution_sqlite::Error::Denied)?;
        Ok(TrustSnapshot {
            authorization_revision: reference("s1-policy"),
            approval_revision: reference("s1-no-approval"),
            fresh_until_unix_ms: self.reliable_now()? + 1000,
            approvals: vec![],
        })
    }
    fn approval_bindings(
        &self,
        _: &FrozenExecution,
    ) -> Result<Vec<ProfileApproval>, execution_app::Error> {
        Ok(vec![])
    }
    fn configuration_change(&self, _: &ConfigChange) -> Result<(), execution_app::Error> {
        Err(execution_app::Error::Denied)
    }
}

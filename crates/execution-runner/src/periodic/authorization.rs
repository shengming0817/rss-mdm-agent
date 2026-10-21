//! Preserve authenticated collection authority across the physical execution boundary.
use super::*;
use rss_mdm_agent_wire::runtime as w;
use sha2::{Digest as _, Sha256};
pub(super) fn verify(
    c: &Configuration,
    plan: &FrozenExecution,
    artifacts: &Artifacts,
    now: u64,
) -> Result<(), Error> {
    let c = c.payload();
    let p = plan.spec();
    verify_binding(c, p, artifacts, now)?;
    let _lease = crate::staging::verify_retained(&artifacts.content, &p.launch.artifact.sha256)?;
    if std::fs::metadata(&artifacts.content)
        .map_err(|_| Error::Unavailable)?
        .len()
        > 4 * 1024 * 1024
    {
        return Err(Error::Capacity);
    }
    let bytes = std::fs::read(&artifacts.content).map_err(|_| Error::Unavailable)?;
    if format!("{:x}", Sha256::digest(&bytes)) != p.launch.artifact.sha256.as_str() {
        return Err(Error::Denied);
    }
    match &c.collector {
        w::TelemetryCollector::Script {
            content,
            profile,
            arguments,
            environment,
            sql_parameters,
            max_rows,
            ..
        } => {
            if bytes.len() as u64 != content.length
                || Sha256::digest(&bytes).as_slice() != content.sha256
            {
                return Err(Error::Denied);
            }
            let parameters: std::collections::BTreeMap<_, _> = sql_parameters
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|(k, value)| (k, InputValue::Literal { value }))
                .collect();
            if p.request.parameters != parameters
                || !matches!(p.launch.output.format,OutputFormat::Json { max_rows: rows } if rows == *max_rows)
            {
                return Err(Error::Denied);
            }
            let env = environment
                .iter()
                .map(|(k, v)| {
                    Ok((
                        EnvironmentKey::new(k).map_err(|_| Error::Denied)?,
                        InputValue::Literal {
                            value: v.clone().into(),
                        },
                    ))
                })
                .collect::<Result<std::collections::BTreeMap<_, _>, Error>>()?;
            if p.launch.env != env {
                return Err(Error::Denied);
            }
            let expected = if *profile == w::ExecutorProfile::Osquery {
                if !arguments.is_empty()
                    || p.launch.interpreter.profile.id.as_str() != crate::osquery::PROFILE
                    || p.launch.interpreter.profile.revision.as_str() != "1"
                {
                    return Err(Error::Denied);
                }
                crate::osquery::arguments(
                    &bytes,
                    &parameters,
                    *max_rows,
                    p.request.target.platform,
                )?
                .into_iter()
                .map(|value| LaunchArg::Literal { value })
                .collect::<Vec<_>>()
            } else {
                let profile = match profile {
                    w::ExecutorProfile::PowerShell7 => script_plan::ScriptProfile::PowerShell7,
                    w::ExecutorProfile::PosixSh => script_plan::ScriptProfile::PosixSh,
                    w::ExecutorProfile::Bash => script_plan::ScriptProfile::Bash,
                    _ => return Err(Error::Denied),
                };
                if p.launch.interpreter.profile != profile.reference() {
                    return Err(Error::Denied);
                }
                // Use the shared invocation compiler; marker cannot come from caller arguments.
                let marker = "__rss_periodic_artifact__";
                let argv = profile.materialized_file_argv(marker, arguments);
                let prefix = argv.len() - arguments.len() - 1;
                argv.into_iter()
                    .enumerate()
                    .map(|(i, value)| {
                        if i == prefix {
                            LaunchArg::ArtifactPath {}
                        } else {
                            LaunchArg::Literal { value }
                        }
                    })
                    .collect()
            };
            if p.launch.argv != expected {
                return Err(Error::Denied);
            }
        }
        w::TelemetryCollector::Native { plan: read, .. } => {
            let input: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| Error::Denied)?;
            if input.pointer("/read/plan")
                != Some(&serde_json::to_value(read).map_err(|_| Error::Denied)?)
                || input.pointer("/run_as")
                    != Some(&serde_json::to_value(&p.run_as).map_err(|_| Error::Denied)?)
                || !p.launch.env.is_empty()
                || p.launch.interpreter.profile.id.as_str() != "native-collection-worker"
                || p.launch.interpreter.profile.revision.as_str() != "1"
                || p.launch.argv
                    != vec![
                        LaunchArg::Literal {
                            value: "--collection-worker".into(),
                        },
                        LaunchArg::ArtifactPath {},
                    ]
                || std::env::current_exe().map_err(|_| Error::Unavailable)? != artifacts.interpreter
            {
                return Err(Error::Denied);
            }
        }
    }
    Ok(())
}

// Keep signed identity/lease binding independently testable without requiring System material.
fn verify_binding(
    c: &w::TelemetryConfiguration,
    p: &ExecutionInput,
    artifacts: &Artifacts,
    now: u64,
) -> Result<(), Error> {
    if now / 1000 < c.issued_at as u64
        || now / 1000 >= c.expires_at as u64
        || p.validity.not_before_unix_ms < c.issued_at as u64 * 1000
        || p.validity.expires_at_unix_ms > c.expires_at as u64 * 1000
        || p.budget.total_timeout_ms > u64::from(c.execution_millis)
        || p.budget.total_output_bytes > u64::from(c.output_bytes)
        || p.budget.max_attempts != 1
        || !matches!(&p.request.authority, Authority::Enterprise { tenant, .. } if tenant.as_str() == c.tenant_id.to_string())
        || p.request.actor.as_str() != format!("registration:{}", c.registration_id)
        || p.request.target.device.as_str() != c.device_id
        || !matches!(p.request.target.scope, TargetScope::Device {})
        || p.request.delegation.is_some()
        || !matches!(p.session_requirement, SessionRequirement::NotRequired {})
        || p.policy.id.as_str() != c.policy.to_string()
        || p.policy.revision.as_str() != c.policy_version.to_string()
        || !matches!(&p.request.initiator, Initiator::Policy { policy } if policy == &p.policy)
        || p.request.operation.resource.id.as_str() != c.template.id
        || p.request.operation.resource.revision.as_str() != c.template.version
        || !matches!(p.launch.stdin, StandardInput::Closed {})
        || p.launch.artifact_encoding != ArtifactEncoding::Utf8
        || !artifacts.program.is_empty()
        || artifacts.controlled_input.is_some()
    {
        return Err(Error::Denied);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn baseline() -> (w::TelemetryConfiguration, ExecutionInput, Artifacts) {
        let id = "11111111-1111-4111-8111-111111111111";
        let c = w::TelemetryConfiguration {
            tenant_id: id.parse().unwrap(),
            device_id: "device-1".into(),
            registration_id: id.parse().unwrap(),
            generation: 1,
            source_epoch: id.parse().unwrap(),
            issued_at: 100,
            expires_at: 200,
            policy: id.parse().unwrap(),
            policy_version: id.parse().unwrap(),
            source: w::Source::AgentScript,
            binding: "a".repeat(64),
            template: w::TaskResource {
                id: "metric".into(),
                version: "1".into(),
                variant: "default".into(),
            },
            collector: w::TelemetryCollector::Script {
                content: w::TaskContent {
                    length: 2,
                    sha256: [0; 32],
                },
                profile: w::ExecutorProfile::PosixSh,
                arguments: vec![],
                environment: Default::default(),
                sql_parameters: None,
                output_schema: serde_json::json!({"type":"object"}),
                timeout_seconds: 1,
                max_rows: 1,
                mappings: Default::default(),
            },
            fields: vec![],
            sample_millis: 5000,
            normal_sample_millis: 5000,
            boost_expires_at: None,
            upload_millis: 10000,
            max_bytes: 65536,
            max_points: 64,
            max_resources: 64,
            execution_millis: 1000,
            output_bytes: 4096,
            backfill_seconds: 3600,
        };
        let mut p: ExecutionInput = serde_json::from_str(include_str!(
            "../../../execution-contract/tests/fixtures/plan.json"
        ))
        .unwrap();
        p.request.authority = Authority::Enterprise {
            tenant: Id::new(id).unwrap(),
            id: Id::new("enterprise").unwrap(),
        };
        p.request.actor = Id::new(format!("registration:{id}")).unwrap();
        p.request.target.device = DeviceId::new(&c.device_id).unwrap();
        p.request.target.scope = TargetScope::Device {};
        p.request.delegation = None;
        p.policy = VersionedRef {
            id: Id::new(id).unwrap(),
            revision: Id::new(id).unwrap(),
        };
        p.request.initiator = Initiator::Policy {
            policy: p.policy.clone(),
        };
        p.request.operation.resource = VersionedRef {
            id: Id::new("metric").unwrap(),
            revision: Id::new("1").unwrap(),
        };
        p.session_requirement = SessionRequirement::NotRequired {};
        p.launch.stdin = StandardInput::Closed {};
        p.launch.artifact_encoding = ArtifactEncoding::Utf8;
        p.validity = ValidityWindow {
            not_before_unix_ms: 100000,
            expires_at_unix_ms: 200000,
        };
        p.budget = ExecutionBudget {
            total_timeout_ms: 1000,
            total_output_bytes: 4096,
            max_attempts: 1,
        };
        let artifacts = Artifacts {
            program: vec![],
            delegate: None,
            interpreter: "unused".into(),
            content: "unused".into(),
            work_root: "unused".into(),
            controlled_input: None,
            fixture_owned: true,
        };
        (c, p, artifacts)
    }
    #[test]
    fn policy_binding_is_rejected_from_an_otherwise_valid_baseline() {
        let (c, mut p, artifacts) = baseline();
        assert_eq!(verify_binding(&c, &p, &artifacts, 150000), Ok(()));
        p.policy.id = Id::new("22222222-2222-4222-8222-222222222222").unwrap();
        p.request.initiator = Initiator::Policy {
            policy: p.policy.clone(),
        };
        assert_eq!(
            verify_binding(&c, &p, &artifacts, 150000),
            Err(Error::Denied)
        );
    }
    #[test]
    fn lease_time_is_rejected_from_an_otherwise_valid_baseline() {
        let (c, p, artifacts) = baseline();
        assert_eq!(verify_binding(&c, &p, &artifacts, 100000), Ok(()));
        assert_eq!(verify_binding(&c, &p, &artifacts, 199999), Ok(()));
        assert_eq!(
            verify_binding(&c, &p, &artifacts, 99999),
            Err(Error::Denied)
        );
        assert_eq!(
            verify_binding(&c, &p, &artifacts, 200000),
            Err(Error::Denied)
        );
    }
}

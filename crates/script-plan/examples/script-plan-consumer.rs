use execution_contract::*;
use script_plan::*;
use serde_json::json;
use std::collections::BTreeMap;

pub fn limits() -> PlanLimits {
    PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_stdin_bytes: 4096,
        max_attempts: 3,
    }
}
fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
fn reference(value: &str) -> VersionedRef {
    VersionedRef {
        id: id(value),
        revision: id("1"),
    }
}
fn artifact(value: &str) -> ExactArtifactRef {
    ExactArtifactRef {
        resource: reference(value),
        sha256: Digest::new("12".repeat(32)).unwrap(),
    }
}

/// Synthetic test context only; no production authority or installed interpreter is asserted.
pub fn input(profile: ScriptProfile) -> ScriptPlanInput {
    let account = OsAccountRef {
        platform: Platform::Linux,
        subject: id("uid:1000"),
    };
    ScriptPlanInput {
        plan_id: PlanId::new("test-plan").unwrap(),
        request: ExecutionRequest {
            schema_version: V1,
            request_id: RequestId::new("test-request").unwrap(),
            authority: Authority::Test { id: id("fixture") },
            actor: ActorId::new("test-actor").unwrap(),
            initiator: Initiator::Human {
                os_session: OsSessionRef {
                    device: DeviceId::new("test-device").unwrap(),
                    account: account.clone(),
                    session: id("test-session"),
                },
            },
            delegation: None,
            target: Target {
                device: DeviceId::new("test-device").unwrap(),
                platform: Platform::Linux,
                scope: TargetScope::User {
                    account: account.clone(),
                },
            },
            operation: Operation {
                action: id("diagnose"),
                resource: reference("script-resource"),
            },
            parameters: BTreeMap::from([
                (
                    "message".into(),
                    InputValue::Literal {
                        value: json!("-true ; $(touch forbidden) 'quoted'"),
                    },
                ),
                (
                    "credential".into(),
                    InputValue::Secret {
                        reference: reference("secret-handle"),
                    },
                ),
            ]),
        },
        artifact: artifact("native-script"),
        interpreter: artifact("test-interpreter-7"),
        profile,
        artifact_encoding: ArtifactEncoding::Utf8,
        bindings: vec![ParameterBinding {
            parameter: "message".into(),
            target: if profile == ScriptProfile::PowerShell7 {
                ParameterTarget::Named {
                    name: "Message".into(),
                }
            } else {
                ParameterTarget::Positional
            },
        }],
        cwd: "/workspace".into(),
        env: BTreeMap::new(),
        stdin: StdinBinding::Parameter {
            parameter: "credential".into(),
            encoding: TextEncoding::Utf8,
            max_bytes: 128,
        },
        output: OutputSpec {
            stdout: TextEncoding::Utf8,
            stderr: TextEncoding::Utf8,
        },
        run_as: RunAs::User {
            account: account.clone(),
        },
        session_requirement: SessionRequirement::ActiveUser { account },
        constraints: Constraints {
            network: NetworkAccess::Denied {},
            read_paths: vec!["/workspace".into()],
            write_paths: vec![],
            allow_child_processes: false,
            require_sandbox: true,
        },
        budget: ExecutionBudget {
            total_timeout_ms: 1000,
            total_output_bytes: 4096,
            max_attempts: 1,
        },
        validity: ValidityWindow {
            not_before_unix_ms: 1000,
            expires_at_unix_ms: 2000,
        },
        policy: reference("test-policy"),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for profile in [
        ScriptProfile::PowerShell7,
        ScriptProfile::PosixSh,
        ScriptProfile::Bash,
    ] {
        let plan = compile(input(profile), &limits())?;
        assert_eq!(
            plan.spec()
                .launch
                .argv
                .iter()
                .filter(|a| matches!(a, LaunchArg::ArtifactPath {}))
                .count(),
            1
        );
        assert!(matches!(
            plan.spec().launch.stdin,
            StandardInput::Controlled { max_bytes: 128, .. }
        ));
        let bytes = serde_json::to_vec(plan.spec())?;
        assert_eq!(
            FrozenPlan::freeze(decode_plan(&bytes, &limits())?, &limits())?.digest(),
            plan.digest()
        );
    }
    println!(
        "script-plan: synthetic authority; pure planning only, no shell or real platform evidence"
    );
    Ok(())
}

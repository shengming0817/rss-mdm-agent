#[allow(dead_code)]
#[path = "support/mod.rs"]
mod scenario;
use execution_contract::*;
use scenario::{input, limits};
use script_plan::*;
use serde_json::json;

fn literals(p: &FrozenPlan) -> Vec<&str> {
    p.spec()
        .launch
        .argv
        .iter()
        .filter_map(|a| match a {
            LaunchArg::Literal { value } => Some(value.as_str()),
            _ => None,
        })
        .collect()
}
fn assert_context(s: &PlanSpec, i: &ScriptPlanInput) {
    assert_eq!(s.request, i.request);
    assert_eq!(s.plan_id, i.plan_id);
    assert_eq!(s.launch.artifact, i.artifact);
    assert_eq!(s.launch.interpreter.artifact, i.interpreter);
    assert_eq!(s.launch.cwd, i.cwd);
    assert_eq!(s.run_as, i.run_as);
    assert_eq!(s.session_requirement, i.session_requirement);
    assert_eq!(s.constraints, i.constraints);
    assert_eq!(s.budget, i.budget);
    assert_eq!(s.validity, i.validity);
    assert_eq!(s.policy, i.policy);
}
#[test]
fn native_profiles_keep_bytes_arguments_and_entire_execution_context() {
    for profile in [
        ScriptProfile::PowerShell7,
        ScriptProfile::PosixSh,
        ScriptProfile::Bash,
    ] {
        let i = input(profile);
        let p = compile(i.clone(), &limits()).unwrap();
        let bytes = serde_json::to_vec(p.spec()).unwrap();
        assert_eq!(
            FrozenPlan::freeze(decode_plan(&bytes, &limits()).unwrap(), &limits())
                .unwrap()
                .digest(),
            p.digest()
        );
        let s = p.spec();
        assert_context(s, &i);
        assert!(literals(&p).contains(&"-true ; $(touch forbidden) 'quoted'"));
        assert!(!literals(&p).contains(&"secret-handle"));
        assert!(!format!("{i:?}{p:?}{:?}", s.launch.argv).contains("forbidden"));
    }
}
#[test]
fn powershell_pending_parameter_preserves_scalar_types_and_dash_values() {
    for (value, expected) in [
        (json!(""), ""),
        (json!("true"), "true"),
        (json!("-flag"), "-flag"),
        (json!(-42), "-42"),
        (json!("a b;\"x\""), "a b;\"x\""),
    ] {
        let mut i = input(ScriptProfile::PowerShell7);
        i.request
            .parameters
            .insert("message".into(), InputValue::Literal { value });
        let p = compile(i, &limits()).unwrap();
        assert_eq!(
            literals(&p),
            [
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-File",
                "-Message:",
                expected
            ]
        );
    }
    for (value, expected) in [(true, "-Message:$true"), (false, "-Message:$false")] {
        let mut i = input(ScriptProfile::PowerShell7);
        i.request.parameters.insert(
            "message".into(),
            InputValue::Literal {
                value: json!(value),
            },
        );
        assert_eq!(
            literals(&compile(i, &limits()).unwrap()).last(),
            Some(&expected)
        );
    }
}
#[test]
fn parameters_cannot_change_their_destination_or_be_silently_ignored() {
    for case in 0..6 {
        let mut i = input(ScriptProfile::PowerShell7);
        match case {
            0 => i.bindings.clear(),
            1 => i.bindings.push(i.bindings[0].clone()),
            2 => i.bindings[0].parameter = "missing".into(),
            3 => {
                i.bindings[0].target = ParameterTarget::Named {
                    name: "x:evil".into(),
                }
            }
            4 => i.bindings[0].target = ParameterTarget::Positional,
            _ => i.bindings[0].target = ParameterTarget::Named { name: "".into() },
        }
        assert_eq!(
            compile(i, &limits()).unwrap_err(),
            [
                ScriptPlanError::UnusedParameter,
                ScriptPlanError::DuplicateParameter,
                ScriptPlanError::MissingParameter,
                ScriptPlanError::InvalidName,
                ScriptPlanError::UnsupportedTarget,
                ScriptPlanError::InvalidName
            ][case],
            "case {case}"
        );
    }
}
#[test]
fn secrets_only_use_controlled_channels_and_conflicts_are_rejected() {
    let mut i = input(ScriptProfile::Bash);
    i.bindings[0].parameter = "credential".into();
    i.stdin = StdinBinding::Closed;
    assert_eq!(
        compile(i, &limits()).unwrap_err(),
        ScriptPlanError::SecretChannel
    );
    let mut i = input(ScriptProfile::Bash);
    i.stdin = StdinBinding::Parameter {
        parameter: "message".into(),
        encoding: TextEncoding::Utf8,
        max_bytes: 1,
    };
    i.bindings.clear();
    assert_eq!(
        compile(i, &limits()).unwrap_err(),
        ScriptPlanError::SecretChannel
    );
    let mut i = input(ScriptProfile::Bash);
    i.stdin = StdinBinding::Closed;
    let key = EnvironmentKey::new("CREDENTIAL").unwrap();
    i.bindings.push(ParameterBinding {
        parameter: "credential".into(),
        target: ParameterTarget::Environment { key: key.clone() },
    });
    let p = compile(i.clone(), &limits()).unwrap();
    assert!(matches!(
        p.spec().launch.env[&key],
        InputValue::Secret { .. }
    ));
    i.env.insert(
        key,
        InputValue::Literal {
            value: json!("template"),
        },
    );
    assert_eq!(
        compile(i, &limits()).unwrap_err(),
        ScriptPlanError::DestinationConflict
    );
}
#[test]
fn unsupported_values_encodings_platforms_and_startup_environment_fail() {
    for value in [
        json!([]),
        json!({}),
        json!(null),
        json!(1.5),
        json!(9007199254740992_u64),
    ] {
        let mut i = input(ScriptProfile::Bash);
        i.request
            .parameters
            .insert("message".into(), InputValue::Literal { value });
        assert_eq!(
            compile(i, &limits()).unwrap_err(),
            ScriptPlanError::ParameterType
        );
    }
    for key in ["BASH_ENV", "ENV", "SHELLOPTS", "BASHOPTS"] {
        let mut i = input(ScriptProfile::Bash);
        i.env.insert(
            EnvironmentKey::new(key).unwrap(),
            InputValue::Literal {
                value: json!("hidden"),
            },
        );
        assert_eq!(
            compile(i, &limits()).unwrap_err(),
            ScriptPlanError::StartupEnvironment
        );
    }
    let mut i = input(ScriptProfile::Bash);
    i.artifact_encoding = ArtifactEncoding::Utf8Bom;
    assert_eq!(compile(i, &limits()).unwrap_err(), ScriptPlanError::Profile);
    let mut i = input(ScriptProfile::PosixSh);
    i.request.target.platform = Platform::Windows;
    assert_eq!(compile(i, &limits()).unwrap_err(), ScriptPlanError::Profile);
}
#[test]
fn canonical_validation_and_budget_are_not_bypassed() {
    for case in 0..4 {
        let mut i = input(ScriptProfile::PowerShell7);
        match case {
            0 => i.cwd = "relative".into(),
            1 => i.budget.total_output_bytes = 65537,
            2 => {
                i.stdin = StdinBinding::Parameter {
                    parameter: "credential".into(),
                    encoding: TextEncoding::Utf8,
                    max_bytes: 4097,
                }
            }
            _ => {
                i.request.parameters.insert(
                    "message".into(),
                    InputValue::Literal {
                        value: json!("nul\u{0}"),
                    },
                );
            }
        }
        assert!(matches!(
            compile(i, &limits()),
            Err(ScriptPlanError::Contract(_))
        ));
    }
    assert!(compile(
        input(ScriptProfile::Bash),
        &PlanLimits {
            max_collection_items: 1,
            ..limits()
        }
    )
    .is_err());
}

#[test]
fn template_and_parameter_environments_cannot_preload_interpreter_code() {
    for profile in [
        ScriptProfile::PowerShell7,
        ScriptProfile::PosixSh,
        ScriptProfile::Bash,
    ] {
        for key in [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "LD_AUDIT",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_FRAMEWORK_PATH",
            "GCONV_PATH",
            "GLIBC_TUNABLES",
            "DOTNET_STARTUP_HOOKS",
            "DOTNET_ADDITIONAL_DEPS",
            "COMPlus_ReadyToRun",
            "CORECLR_PROFILER_PATH",
            "COR_ENABLE_PROFILING",
            "PSModulePath",
        ] {
            for bound in [false, true] {
                let mut i = input(profile);
                let key = EnvironmentKey::new(key).unwrap();
                if bound {
                    i.bindings[0].target = ParameterTarget::Environment { key };
                } else {
                    i.env.insert(
                        key,
                        InputValue::Literal {
                            value: json!("preload-code"),
                        },
                    );
                }
                assert_eq!(
                    compile(i, &limits()).unwrap_err(),
                    ScriptPlanError::StartupEnvironment,
                    "{profile:?}, parameter={bound}"
                );
            }
        }
    }
    let mut i = input(ScriptProfile::PowerShell7);
    i.env.insert(
        EnvironmentKey::new("DotNet_Startup_Hooks").unwrap(),
        InputValue::Literal {
            value: json!("preload-code"),
        },
    );
    assert_eq!(
        compile(i, &limits()).unwrap_err(),
        ScriptPlanError::StartupEnvironment
    );
}

#[test]
fn duplicate_powershell_names_have_a_static_diagnostic() {
    let mut i = input(ScriptProfile::PowerShell7);
    i.request.parameters.insert(
        "second".into(),
        InputValue::Literal {
            value: json!("private-value"),
        },
    );
    i.bindings.push(ParameterBinding {
        parameter: "second".into(),
        target: ParameterTarget::Named {
            name: "message".into(),
        },
    });
    let error = compile(i, &limits()).unwrap_err();
    assert_eq!(error, ScriptPlanError::DuplicateName);
    assert!(!format!("{error:?}{error}").contains("private-value"));
}

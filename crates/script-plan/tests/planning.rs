#[path = "support/mod.rs"]
mod scenario;
use execution_contract::*;
use scenario::{context, invocation, limits};
use script_plan::*;
use serde_json::json;

const PROFILES: [ScriptProfile; 3] = [
    ScriptProfile::PowerShell7,
    ScriptProfile::PosixSh,
    ScriptProfile::Bash,
];

#[test]
fn replacement_preserves_launch_facts_and_measured_frozen_digests() {
    // Recorded using the original compiler before replacing its unconsumed binding API.
    let digests = [
        "18166a763244b7b3304cf7ba2c2597a008dafe6133dd8f98475e82ba08013545",
        "db8e0330491485bca4d73e37c5e8a4cdfd3621ec7fa87ad240e8c5ebf5d3ccd0",
        "ca6393178869b6b8b5b1d788102ad4d9472b8ad7ef9a1bff65b2a35d4f764dca",
    ];
    for (profile, digest) in PROFILES.into_iter().zip(digests) {
        let input = invocation(profile);
        let launch = compile_invocation(input.clone(), &limits()).unwrap();
        assert_eq!(launch.artifact, input.artifact);
        assert_eq!(launch.interpreter.artifact, input.interpreter);
        assert_eq!(launch.cwd, input.cwd);
        assert_eq!(launch.env, input.env);
        assert_eq!(launch.stdin, input.stdin);
        assert_eq!(launch.output, input.output);
        assert_eq!(launch.artifact_encoding, input.artifact_encoding);
        let plan = FrozenExecution::freeze(context(profile), &limits()).unwrap();
        assert_eq!(plan.digest().as_str(), digest);
        let bytes = serde_json::to_vec(plan.spec()).unwrap();
        let decoded = decode_execution(&bytes, &limits()).unwrap();
        assert_eq!(
            FrozenExecution::freeze(decoded, &limits())
                .unwrap()
                .digest(),
            plan.digest()
        );
    }
}

#[test]
fn file_calling_conventions_keep_signed_arguments_literal_and_in_order() {
    let arguments: Vec<String> = [
        "",
        "true",
        "-flag",
        "-Name:",
        "a b",
        "\"quoted\"",
        r"C:\a\b",
        "$(touch forbidden);x",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for (profile, name, prefix) in [
        (
            ScriptProfile::PowerShell7,
            "native-pwsh7-file",
            vec!["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
        ),
        (ScriptProfile::PosixSh, "native-posix-sh-file", vec![]),
        (
            ScriptProfile::Bash,
            "native-bash-file",
            vec!["--noprofile", "--norc"],
        ),
    ] {
        let mut input = invocation(profile);
        input.arguments = arguments.clone();
        input.env.insert(
            EnvironmentKey::new("VALUE").unwrap(),
            InputValue::Literal {
                value: json!(" true ; $(x) "),
            },
        );
        let expected_env = input.env.clone();
        let launch = compile_invocation(input, &limits()).unwrap();
        assert_eq!(launch.interpreter.profile.id.as_str(), name);
        assert_eq!(launch.interpreter.profile.revision.as_str(), "1");
        let mut expected: Vec<_> = prefix
            .iter()
            .map(|value| LaunchArg::Literal {
                value: (*value).into(),
            })
            .collect();
        expected.push(LaunchArg::ArtifactPath {});
        expected.extend(arguments.iter().map(|value| LaunchArg::Literal {
            value: value.clone(),
        }));
        assert_eq!(launch.argv, expected);
        assert_eq!(launch.env, expected_env);
        let args = profile.materialized_file_argv("/verified/script", &arguments);
        assert!(profile.matches_materialized_file_argv(&args, "/verified/script"));
        assert!(!profile.matches_materialized_file_argv(&args, "/different/script"));
        let mut changed = args.clone();
        changed.insert(0, "-c".into());
        assert!(!profile.matches_materialized_file_argv(&changed, "/verified/script"));
        if !prefix.is_empty() {
            let mut changed = args.clone();
            changed[0] = changed[0].to_ascii_uppercase();
            assert!(!profile.matches_materialized_file_argv(&changed, "/verified/script"));
            let mut changed = args.clone();
            changed.remove(0);
            assert!(!profile.matches_materialized_file_argv(&changed, "/verified/script"));
            let mut changed = args.clone();
            changed.swap(0, 1);
            assert!(!profile.matches_materialized_file_argv(&changed, "/verified/script"));
        }
        assert!(!profile.matches_materialized_file_argv(&[], "/verified/script"));
    }
}

#[test]
fn exact_profile_identity_never_falls_back_or_grants_platform_support() {
    for profile in PROFILES {
        assert_eq!(
            ScriptProfile::from_reference(&profile.reference()).unwrap(),
            profile
        );
        let mut reference = profile.reference();
        reference.revision = Id::new("99").unwrap();
        assert_eq!(
            ScriptProfile::from_reference(&reference),
            Err(ScriptPlanError::Profile)
        );
        reference = profile.reference();
        reference.id = Id::new("unknown").unwrap();
        assert_eq!(
            ScriptProfile::from_reference(&reference),
            Err(ScriptPlanError::Profile)
        );
    }
    for profile in [ScriptProfile::PosixSh, ScriptProfile::Bash] {
        for encoding in [ArtifactEncoding::Utf8Bom, ArtifactEncoding::Utf16LeBom] {
            let mut input = invocation(profile);
            input.artifact_encoding = encoding;
            assert_eq!(
                compile_invocation(input, &limits()).unwrap_err(),
                ScriptPlanError::Profile
            );
        }
        let mut input = invocation(profile);
        input.platform = Platform::Windows;
        assert_eq!(
            compile_invocation(input, &limits()).unwrap_err(),
            ScriptPlanError::Profile
        );
    }
}

#[test]
fn workload_bounds_and_canonical_input_validation_remain_separate() {
    let mut input = invocation(ScriptProfile::PowerShell7);
    input.arguments = vec!["x".into(); limits().max_collection_items];
    assert_eq!(
        compile_invocation(input, &limits()).unwrap_err(),
        ScriptPlanError::Limit
    );
    let mut input = invocation(ScriptProfile::Bash);
    input.arguments = vec!["x".repeat(limits().max_string_bytes + 1)];
    assert_eq!(
        compile_invocation(input, &limits()).unwrap_err(),
        ScriptPlanError::Limit
    );
    for case in 0..4 {
        let mut spec = context(ScriptProfile::Bash);
        match case {
            0 => spec.launch.cwd = "relative".into(),
            1 => spec.budget.total_output_bytes = limits().max_output_bytes + 1,
            2 => spec.launch.argv.push(LaunchArg::Literal {
                value: "nul\0".into(),
            }),
            _ => {
                spec.launch.stdin = StandardInput::Controlled {
                    reference: spec.policy.clone(),
                    encoding: TextEncoding::Utf8,
                    max_bytes: limits().max_stdin_bytes + 1,
                }
            }
        }
        assert!(
            FrozenExecution::freeze(spec, &limits()).is_err(),
            "case {case}"
        );
    }
}

use crate::*;
use execution_contract::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn literal(value: impl Into<String>) -> LaunchArg {
    LaunchArg::Literal {
        value: value.into(),
    }
}
fn scalar(value: &InputValue, limits: &PlanLimits) -> Result<String, ScriptPlanError> {
    let InputValue::Literal { value } = value else {
        return Err(ScriptPlanError::SecretChannel);
    };
    match value {
        Value::String(v) if v.len() <= limits.max_string_bytes => Ok(v.clone()),
        Value::String(_) => Err(ScriptPlanError::Limit),
        Value::Bool(v) => Ok(v.to_string()),
        Value::Number(v) => v
            .as_i64()
            .filter(|v| v.unsigned_abs() <= 9_007_199_254_740_991)
            .map(|v| v.to_string())
            .ok_or(ScriptPlanError::ParameterType),
        _ => Err(ScriptPlanError::ParameterType),
    }
}
fn parameter<'a>(
    name: &str,
    values: &'a BTreeMap<String, InputValue>,
    used: &mut BTreeSet<String>,
    limits: &PlanLimits,
) -> Result<&'a InputValue, ScriptPlanError> {
    if name.len() > limits.max_string_bytes {
        return Err(ScriptPlanError::Limit);
    }
    if !used.insert(name.to_owned()) {
        return Err(ScriptPlanError::Binding);
    }
    values.get(name).ok_or(ScriptPlanError::Binding)
}
fn named(
    name: &str,
    value: &InputValue,
    names: &mut BTreeSet<String>,
    argv: &mut Vec<LaunchArg>,
    limits: &PlanLimits,
) -> Result<(), ScriptPlanError> {
    if name.is_empty()
        || name.len() > 128
        || name.len() > limits.max_string_bytes
        || name.as_bytes()[0].is_ascii_digit()
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        || !names.insert(name.to_ascii_lowercase())
    {
        return Err(ScriptPlanError::Binding);
    }
    let text = scalar(value, limits)?;
    if matches!(
        value,
        InputValue::Literal {
            value: Value::Bool(_)
        }
    ) {
        argv.push(literal(format!("-{name}:${text}")));
    } else {
        // PowerShell's pendingParameter branch precedes dash detection/bool conversion.
        // ref: PowerShell CommandLineParameterParser.cs@411d5fee10110d9881a909804f9d4eb1a06052ea
        argv.push(literal(format!("-{name}:")));
        argv.push(literal(text));
    }
    Ok(())
}
fn input_stream(
    binding: StdinBinding,
    parameters: &BTreeMap<String, InputValue>,
    used: &mut BTreeSet<String>,
    limits: &PlanLimits,
) -> Result<StandardInput, ScriptPlanError> {
    match binding {
        StdinBinding::Closed => Ok(StandardInput::Closed {}),
        StdinBinding::Parameter {
            parameter: key,
            encoding,
            max_bytes,
        } => {
            let InputValue::Secret { reference } = parameter(&key, parameters, used, limits)?
            else {
                return Err(ScriptPlanError::SecretChannel);
            };
            Ok(StandardInput::Controlled {
                reference: reference.clone(),
                encoding,
                max_bytes,
            })
        }
    }
}
fn convention(
    profile: ScriptProfile,
    platform: Platform,
    encoding: ArtifactEncoding,
) -> Result<(VersionedRef, Vec<LaunchArg>), ScriptPlanError> {
    if profile != ScriptProfile::PowerShell7
        && (platform == Platform::Windows || encoding != ArtifactEncoding::Utf8)
    {
        return Err(ScriptPlanError::Profile);
    }
    let (name, prefix): (&str, &[&str]) = match profile {
        ScriptProfile::PowerShell7 => (
            "native-pwsh7-file",
            &["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
        ),
        ScriptProfile::PosixSh => ("native-posix-sh-file", &[]),
        ScriptProfile::Bash => ("native-bash-file", &["--noprofile", "--norc"]),
    };
    let reference = VersionedRef {
        id: Id::new(name)?,
        revision: Id::new("1")?,
    };
    let mut argv: Vec<_> = prefix.iter().map(|v| literal(*v)).collect();
    argv.push(LaunchArg::ArtifactPath {});
    Ok((reference, argv))
}
fn check_environment(
    profile: ScriptProfile,
    env: &BTreeMap<EnvironmentKey, InputValue>,
) -> Result<(), ScriptPlanError> {
    if profile != ScriptProfile::PowerShell7
        && env
            .keys()
            .any(|k| matches!(k.as_str(), "BASH_ENV" | "ENV" | "SHELLOPTS" | "BASHOPTS"))
    {
        return Err(ScriptPlanError::StartupEnvironment);
    }
    // EnvironmentKey already excludes Bash exported-function names.
    Ok(())
}
/// Compile normalized parameters into one native invocation and freeze all execution facts.
/// No script parsing, byte rewriting, artifact/secret resolution, system clock or spawning.
/// C01 is the sole validation/normalization/digest owner; freezing is not authorization.
pub fn compile(input: ScriptPlanInput, limits: &PlanLimits) -> Result<FrozenPlan, ScriptPlanError> {
    let ScriptPlanInput {
        plan_id,
        request,
        artifact,
        interpreter,
        profile,
        artifact_encoding,
        bindings,
        cwd,
        mut env,
        stdin,
        output,
        run_as,
        session_requirement,
        constraints,
        budget,
        validity,
        policy,
    } = input;
    if bindings.len() > limits.max_collection_items
        || request.parameters.len() > limits.max_collection_items
        || env.len() > limits.max_collection_items
    {
        return Err(ScriptPlanError::Limit);
    }
    let (profile_ref, mut argv) = convention(profile, request.target.platform, artifact_encoding)?;
    let mut used = BTreeSet::new();
    let mut names = BTreeSet::new();
    let stdin = input_stream(stdin, &request.parameters, &mut used, limits)?;
    for binding in bindings {
        let value = parameter(&binding.parameter, &request.parameters, &mut used, limits)?;
        match binding.target {
            ParameterTarget::Positional if profile != ScriptProfile::PowerShell7 => {
                argv.push(literal(scalar(value, limits)?))
            }
            ParameterTarget::Named { name } if profile == ScriptProfile::PowerShell7 => {
                named(&name, value, &mut names, &mut argv, limits)?
            }
            ParameterTarget::Environment { key } => {
                if env.contains_key(&key) {
                    return Err(ScriptPlanError::Binding);
                }
                let value = match value {
                    InputValue::Secret { .. } => value.clone(),
                    _ => InputValue::Literal {
                        value: Value::String(scalar(value, limits)?),
                    },
                };
                env.insert(key, value);
            }
            _ => return Err(ScriptPlanError::Binding),
        }
        if argv.len() > limits.max_collection_items || env.len() > limits.max_collection_items {
            return Err(ScriptPlanError::Limit);
        }
    }
    if used.len() != request.parameters.len() {
        return Err(ScriptPlanError::Binding);
    }
    check_environment(profile, &env)?;
    Ok(FrozenPlan::freeze(
        PlanSpec {
            schema_version: V1,
            plan_id,
            request,
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: interpreter,
                    profile: profile_ref,
                },
                argv,
                artifact_encoding,
                stdin,
                output,
                cwd,
                env,
            },
            run_as,
            session_requirement,
            constraints,
            budget,
            validity,
            policy,
        },
        limits,
    )?)
}

use crate::{ScriptInvocationInput, ScriptPlanError, ScriptProfile};
use execution_contract::{ArtifactEncoding, ExecutionLimits, InterpreterRef, LaunchSpec, Platform};

/// Compile literal invocation facts into the canonical launch description.
/// No bindings, shell parsing, defaults, artifact IO, identity checks or freezing occur here.
/// The execution-contract owner validates the complete plan; runners independently enforce
/// material, environment, input, encoding and platform restrictions before execution.
/// ref: Rust library/std/src/process.rs (Command::args keeps each argument independent).
pub fn compile_invocation(
    input: ScriptInvocationInput,
    limits: &ExecutionLimits,
) -> Result<LaunchSpec, ScriptPlanError> {
    if input.profile != ScriptProfile::PowerShell7
        && (input.platform == Platform::Windows
            || input.artifact_encoding != ArtifactEncoding::Utf8)
    {
        return Err(ScriptPlanError::Profile);
    }
    if input
        .arguments
        .len()
        .saturating_add(input.profile.prefix_len())
        > limits.max_collection_items
        || input.env.len() > limits.max_collection_items
        || input.cwd.len() > limits.max_string_bytes
        || input
            .arguments
            .iter()
            .any(|value| value.len() > limits.max_string_bytes)
    {
        return Err(ScriptPlanError::Limit);
    }
    Ok(LaunchSpec {
        artifact: input.artifact,
        interpreter: InterpreterRef {
            artifact: input.interpreter,
            profile: input.profile.reference(),
        },
        argv: input.profile.file_argv(input.arguments),
        artifact_encoding: input.artifact_encoding,
        stdin: input.stdin,
        output: input.output,
        cwd: input.cwd,
        env: input.env,
    })
}

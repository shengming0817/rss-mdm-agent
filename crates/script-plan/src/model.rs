use execution_contract::*;
use std::collections::BTreeMap;

/// Closed file invocation conventions, independent of a runner's platform allowlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptProfile {
    /// PowerShell 7; Windows PowerShell 5.1 is a different interpreter.
    PowerShell7,
    /// Noninteractive POSIX sh.
    PosixSh,
    /// Noninteractive Bash without profile or rc loading.
    Bash,
}
/// Already expanded invocation facts. This input is neither a wire format nor an execution plan.
/// Arguments are literal values; bindings, defaults, identity and budgets belong to callers.
#[derive(Clone)]
pub struct ScriptInvocationInput {
    /// Target platform, not evidence that its runner supports the profile.
    pub platform: Platform,
    /// Closed file invocation convention.
    pub profile: ScriptProfile,
    /// Exact script bytes, verified independently by the runner.
    pub artifact: ExactArtifactRef,
    /// Exact interpreter artifact, selected and verified by the host.
    pub interpreter: ExactArtifactRef,
    /// Ordered literal arguments after the file slot, never shell source.
    pub arguments: Vec<String>,
    /// Required original script encoding, without transcoding.
    pub artifact_encoding: ArtifactEncoding,
    /// Explicit input channel; this compiler never resolves its contents.
    pub stdin: StandardInput,
    /// Capture and decoding requirements.
    pub output: OutputSpec,
    /// Explicit working directory; the runner verifies the actual directory.
    pub cwd: String,
    /// Explicit environment; no inherited values or defaults are added.
    pub env: BTreeMap<EnvironmentKey, InputValue>,
}
impl std::fmt::Debug for ScriptInvocationInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ScriptInvocationInput([redacted])")
    }
}
/// Static diagnostics that never retain arguments, secrets or supplied paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScriptPlanError {
    /// Unknown profile/revision or unsupported profile/platform/encoding combination.
    #[error("unsupported script profile requirement")]
    Profile,
    /// Invocation compilation exceeds the caller's workload limits.
    #[error("script invocation bound exceeded")]
    Limit,
}

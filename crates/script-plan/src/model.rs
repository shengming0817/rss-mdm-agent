use execution_contract::*;
use std::collections::BTreeMap;

/// Statically supported calling conventions; exact interpreter bytes remain host-resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptProfile {
    /// PowerShell 7 file invocation. Windows PowerShell 5.1 is not this profile.
    PowerShell7,
    /// Noninteractive POSIX sh file invocation on macOS/Linux.
    PosixSh,
    /// Noninteractive Bash file invocation on macOS/Linux.
    Bash,
}
/// Parameter delivery, defined by the resource template rather than the parameter value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterTarget {
    /// A positional argument after the script, supported by sh/Bash only.
    Positional,
    /// A PowerShell parameter with a portable ASCII name; values cannot choose the name.
    Named {
        /// Nonempty letters/digits/underscore, beginning with a letter or underscore.
        name: String,
    },
    /// Explicit environment entry; never overwrite an entry supplied by the template.
    Environment {
        /// Exact environment key; Windows collision semantics are checked by C01.
        key: EnvironmentKey,
    },
}
/// One normalized request parameter, used exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterBinding {
    /// Key in ExecutionRequest.parameters. Defaults and catalog constraints belong to C03.
    pub parameter: String,
    /// Explicit process-input destination.
    pub target: ParameterTarget,
}
/// Explicit stdin choice, separate from the required desktop user session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinBinding {
    /// Close stdin and provide no terminal.
    Closed,
    /// Deliver one request secret/input reference through a bounded controlled stream.
    Parameter {
        /// Key referring to InputValue::Secret; no literal bytes are accepted.
        parameter: String,
        /// Materialized byte encoding.
        encoding: TextEncoding,
        /// Positive maximum materialized bytes, bounded by C01 PlanLimits.
        max_bytes: u64,
    },
}
/// Complete compiler input. These are untrusted declarations, never policy or identity proofs.
/// No mutable PlanSpec placeholder or second canonical launch description is used.
#[derive(Clone)]
pub struct ScriptPlanInput {
    /// Exact plan identity, also bound into the canonical digest.
    pub plan_id: PlanId,
    /// Original request and normalized scalar/secret parameters.
    pub request: ExecutionRequest,
    /// Exact original script bytes, verified later without rewriting.
    pub artifact: ExactArtifactRef,
    /// Exact interpreter binary/revision; no PATH or latest fallback.
    pub interpreter: ExactArtifactRef,
    /// Supported calling convention, emitted as an exact profile reference.
    pub profile: ScriptProfile,
    /// Declared byte encoding; sh/Bash require plain UTF-8.
    pub artifact_encoding: ArtifactEncoding,
    /// Ordered bindings; every request parameter is consumed exactly once including stdin.
    pub bindings: Vec<ParameterBinding>,
    /// Absolute target working directory, validated by C01.
    pub cwd: String,
    /// Explicit template environment; the adapter must clear inherited environment.
    pub env: BTreeMap<EnvironmentKey, InputValue>,
    /// Explicit noninteractive input choice.
    pub stdin: StdinBinding,
    /// Required raw capture and strict text decoding.
    pub output: OutputSpec,
    /// Explicit requested execution identity.
    pub run_as: RunAs,
    /// Required target desktop session; it never enables a console or terminal.
    pub session_requirement: SessionRequirement,
    /// Complete restrictions; script declarations cannot weaken them.
    pub constraints: Constraints,
    /// Existing plan-wide time/output/attempt budget.
    pub budget: ExecutionBudget,
    /// Explicit plan validity, not a wall-clock lookup.
    pub validity: ValidityWindow,
    /// Exact policy reference; authenticity is checked by the authorization owner.
    pub policy: VersionedRef,
}
impl std::fmt::Debug for ScriptPlanInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ScriptPlanInput([redacted])")
    }
}
/// Static diagnostics; no parameter value, secret, supplied key or provider error is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScriptPlanError {
    /// Invalid or unsupported profile/platform/encoding combination.
    #[error("unsupported script profile requirement")]
    Profile,
    /// A parameter was consumed more than once.
    #[error("duplicate script parameter binding")]
    DuplicateParameter,
    /// A binding names a parameter absent from the request.
    #[error("missing script parameter")]
    MissingParameter,
    /// A PowerShell parameter name is invalid or exceeds its bound.
    #[error("invalid script parameter name")]
    InvalidName,
    /// PowerShell names repeat after case folding.
    #[error("duplicate script parameter name")]
    DuplicateName,
    /// An environment destination is already populated.
    #[error("conflicting script environment destination")]
    DestinationConflict,
    /// The selected profile cannot use this binding target.
    #[error("unsupported script binding target")]
    UnsupportedTarget,
    /// A request parameter has no binding.
    #[error("unused script parameter")]
    UnusedParameter,

    /// Only normalized string, safe integer and boolean literals are supported.
    #[error("unsupported script parameter type")]
    ParameterType,
    /// A referenced secret cannot be delivered as an argument or literal stdin.
    #[error("invalid controlled input channel")]
    SecretChannel,
    /// A loader/runtime startup-control variable would add hidden code/options.
    #[error("interpreter startup environment is not permitted")]
    StartupEnvironment,
    /// Independent compiler workload bound.
    #[error("script planning bound exceeded")]
    Limit,
    /// Canonical owner validation, including invalid host limits.
    #[error("invalid execution contract: {0}")]
    Contract(#[from] ContractError),
}

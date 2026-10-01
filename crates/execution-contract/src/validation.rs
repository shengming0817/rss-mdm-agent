use crate::{
    ContractError, ErrorKind, ExecutionInput, Field, InputValue, IsolationPolicy, NetworkAccess,
    Platform, Rule, RunAs, TargetScope,
};
use serde::{
    de::{MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Value;
use std::{collections::BTreeSet, fmt};
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
/// Host bounds independent from the untrusted plan; all limits must be positive.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionLimits {
    /// Maximum raw input and canonical plan bytes, checked before parsing/after encoding.
    pub max_input_bytes: usize,
    /// Maximum JSON value depth, root counted as one; supported range is 1..=64.
    pub max_depth: usize,
    /// Maximum total JSON value nodes, including container nodes.
    pub max_nodes: usize,
    /// Maximum UTF-8 bytes in any string or object key.
    pub max_string_bytes: usize,
    /// Maximum members/elements in each object/array.
    pub max_collection_items: usize,
    /// Maximum plan-wide elapsed milliseconds from first attempt start.
    pub max_timeout_ms: u64,
    /// Maximum output bytes summed across all attempts, including discarded bytes.
    pub max_output_bytes: u64,
    /// Maximum controlled stdin bytes per materialization; independent of encoded plan size.
    pub max_stdin_bytes: u64,
    /// Maximum attempts permitted for the complete plan.
    pub max_attempts: u32,
}
impl ExecutionLimits {
    pub(crate) fn validate(&self) -> Result<(), ContractError> {
        for (zero, field) in [
            (self.max_input_bytes == 0, Field::InputBytes),
            (self.max_depth == 0, Field::Depth),
            (self.max_nodes == 0, Field::Nodes),
            (self.max_string_bytes == 0, Field::StringBytes),
            (self.max_collection_items == 0, Field::CollectionItems),
            (self.max_timeout_ms == 0, Field::Timeout),
            (self.max_output_bytes == 0, Field::OutputBytes),
            (self.max_stdin_bytes == 0, Field::StandardInput),
            (self.max_attempts == 0, Field::Attempts),
        ] {
            if zero {
                return Err(ContractError::new(
                    ErrorKind::InvalidConfiguration,
                    field,
                    Rule::NonZero,
                ));
            }
        }
        if self.max_depth > 64 {
            return Err(ContractError::new(
                ErrorKind::InvalidConfiguration,
                Field::Depth,
                Rule::NumericRange,
            ));
        }
        Ok(())
    }
}
pub(crate) fn check_json(value: &Value, limits: &ExecutionLimits) -> Result<(), ContractError> {
    let mut remaining = limits.max_nodes;
    fn walk(
        v: &Value,
        l: &ExecutionLimits,
        depth: usize,
        left: &mut usize,
    ) -> Result<(), ContractError> {
        if depth > l.max_depth {
            return Err(ContractError::new(
                ErrorKind::LimitExceeded,
                Field::Depth,
                Rule::DepthLimit,
            ));
        }
        if *left == 0 {
            return Err(ContractError::new(
                ErrorKind::LimitExceeded,
                Field::Nodes,
                Rule::NodeLimit,
            ));
        }
        *left -= 1;
        match v {
            Value::String(s) if s.len() > l.max_string_bytes => {
                return Err(ContractError::new(
                    ErrorKind::LimitExceeded,
                    Field::StringBytes,
                    Rule::ByteLimit,
                ))
            }
            Value::Number(n) => {
                let safe = if let Some(n) = n.as_u64() {
                    n <= MAX_SAFE_INTEGER
                } else if let Some(n) = n.as_i64() {
                    n.unsigned_abs() <= MAX_SAFE_INTEGER
                } else {
                    n.as_f64().is_some_and(|n| {
                        n.is_finite() && (n.fract() != 0.0 || n.abs() <= MAX_SAFE_INTEGER as f64)
                    })
                };
                if !safe {
                    return Err(ContractError::new(
                        ErrorKind::InvalidValue,
                        Field::Document,
                        Rule::NumericRange,
                    ));
                }
            }
            Value::Array(items) => {
                collection(items.len(), l)?;
                for item in items {
                    walk(item, l, depth + 1, left)?;
                }
            }
            Value::Object(items) => {
                collection(items.len(), l)?;
                for (key, item) in items {
                    if key.len() > l.max_string_bytes {
                        return Err(ContractError::new(
                            ErrorKind::LimitExceeded,
                            Field::StringBytes,
                            Rule::ByteLimit,
                        ));
                    }
                    if key.is_empty() {
                        return Err(ContractError::new(
                            ErrorKind::InvalidValue,
                            Field::Document,
                            Rule::Empty,
                        ));
                    }
                    if key.contains('\0') {
                        return Err(ContractError::new(
                            ErrorKind::InvalidValue,
                            Field::Document,
                            Rule::Nul,
                        ));
                    }
                    walk(item, l, depth + 1, left)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(value, limits, 1, &mut remaining)
}
fn collection(count: usize, limits: &ExecutionLimits) -> Result<(), ContractError> {
    if count > limits.max_collection_items {
        return Err(ContractError::new(
            ErrorKind::LimitExceeded,
            Field::CollectionItems,
            Rule::CollectionLimit,
        ));
    }
    Ok(())
}
// serde_json owns JSON parsing; this visitor additionally rejects repeated names.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("unique JSON data")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| UniqueValue(Value::Number(n)))
                    .ok_or_else(|| {
                        E::custom(
                            ContractError::new(
                                ErrorKind::InvalidValue,
                                Field::Document,
                                Rule::NumericRange,
                            )
                            .for_serde(),
                        )
                    })
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(UniqueValue(v)) = a.next_element()? {
                    out.push(v);
                }
                Ok(UniqueValue(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(key) = a.next_key::<String>()? {
                    if out.contains_key(&key) {
                        return Err(serde::de::Error::custom(
                            ContractError::new(
                                ErrorKind::Encoding,
                                Field::Document,
                                Rule::DuplicateKey,
                            )
                            .for_serde(),
                        ));
                    }
                    let UniqueValue(v) = a.next_value()?;
                    out.insert(key, v);
                }
                Ok(UniqueValue(Value::Object(out)))
            }
        }
        d.deserialize_any(UniqueVisitor)
    }
}
pub(crate) fn decode_value(bytes: &[u8], limits: &ExecutionLimits) -> Result<Value, ContractError> {
    limits.validate()?;
    if bytes.len() > limits.max_input_bytes {
        return Err(ContractError::new(
            ErrorKind::LimitExceeded,
            Field::InputBytes,
            Rule::ByteLimit,
        ));
    }
    let UniqueValue(value) = serde_json::from_slice(bytes).map_err(ContractError::from_serde)?;
    check_json(&value, limits)?;
    Ok(value)
}
pub(crate) fn validate_plan(p: &ExecutionInput, l: &ExecutionLimits) -> Result<(), ContractError> {
    l.validate()?;
    for (value, max, field) in [
        (p.budget.total_timeout_ms, l.max_timeout_ms, Field::Timeout),
        (
            p.budget.total_output_bytes,
            l.max_output_bytes,
            Field::OutputBytes,
        ),
        (
            u64::from(p.budget.max_attempts),
            u64::from(l.max_attempts),
            Field::Attempts,
        ),
    ] {
        if value == 0 {
            return Err(ContractError::new(
                ErrorKind::InvalidBudget,
                field,
                Rule::NonZero,
            ));
        }
        if value > max {
            return Err(ContractError::new(
                ErrorKind::InvalidBudget,
                field,
                Rule::BudgetLimit,
            ));
        }
    }
    if p.validity.not_before_unix_ms >= p.validity.expires_at_unix_ms {
        return Err(ContractError::new(
            ErrorKind::InvalidBudget,
            Field::Validity,
            Rule::TimeOrder,
        ));
    }
    let platform = p.request.target.platform;
    if matches!(&p.session_requirement, crate::SessionRequirement::ActiveUser { account, .. } if account.platform != platform)
    {
        return Err(ContractError::new(
            ErrorKind::InconsistentContext,
            Field::Session,
            Rule::Mismatch,
        ));
    }
    let run_platform = match &p.run_as {
        RunAs::User { account } => account.platform,
        RunAs::System { platform } => *platform,
    };
    if run_platform != platform {
        return Err(ContractError::new(
            ErrorKind::InconsistentContext,
            Field::RunAs,
            Rule::Mismatch,
        ));
    }
    if matches!(&p.request.target.scope,TargetScope::User{account} if account.platform!=platform) {
        return Err(ContractError::new(
            ErrorKind::InconsistentContext,
            Field::Target,
            Rule::Mismatch,
        ));
    }
    path(&p.launch.cwd, platform, Field::WorkingDirectory)?;
    if let IsolationPolicy::Restricted {
        read_paths,
        write_paths,
        ..
    } = &p.constraints
    {
        for value in read_paths {
            path(value, platform, Field::ReadPaths)?;
        }
        for value in write_paths {
            path(value, platform, Field::WritePaths)?;
        }
    }
    if p.launch
        .argv
        .iter()
        .any(|a| matches!(a, crate::LaunchArg::Literal { value } if value.contains('\0')))
    {
        return Err(ContractError::new(
            ErrorKind::InvalidValue,
            Field::Arguments,
            Rule::Nul,
        ));
    }
    if p.launch
        .argv
        .iter()
        .filter(|a| matches!(a, crate::LaunchArg::ArtifactPath {}))
        .count()
        != usize::from(p.launch.interpreter.profile.id.as_str() != "native-osquery-template")
    {
        return Err(ContractError::new(
            ErrorKind::InvalidValue,
            Field::Arguments,
            Rule::ArtifactSlot,
        ));
    }
    if let crate::StandardInput::Controlled { max_bytes, .. } = p.launch.stdin {
        if max_bytes == 0 {
            return Err(ContractError::new(
                ErrorKind::InvalidBudget,
                Field::StandardInput,
                Rule::NonZero,
            ));
        }
        if max_bytes > l.max_stdin_bytes {
            return Err(ContractError::new(
                ErrorKind::InvalidBudget,
                Field::StandardInput,
                Rule::BudgetLimit,
            ));
        }
    }
    if let crate::OutputFormat::Json { max_rows } = p.launch.output.format {
        if max_rows == 0 {
            return Err(ContractError::new(
                ErrorKind::InvalidBudget,
                Field::OutputBytes,
                Rule::BudgetLimit,
            ));
        }
    }
    environment(p)?;
    software(p, l)?;
    if let IsolationPolicy::Restricted {
        network: NetworkAccess::Allowlist { destinations },
        ..
    } = &p.constraints
    {
        if destinations.is_empty() {
            return Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::Network,
                Rule::Empty,
            ));
        }
        let mut unique = BTreeSet::new();
        if destinations.iter().any(|d| !unique.insert(d)) {
            return Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::Network,
                Rule::DuplicateKey,
            ));
        }
    }
    Ok(())
}
fn path(value: &str, platform: Platform, field: Field) -> Result<(), ContractError> {
    let absolute = match platform {
        Platform::Windows => {
            let drive = value.len() >= 3
                && value.as_bytes()[0].is_ascii_alphabetic()
                && value.as_bytes()[1] == b':'
                && b"/\\".contains(&value.as_bytes()[2]);
            let unc = value.strip_prefix("\\\\").is_some_and(|rest| {
                let parts: Vec<_> = rest.split('\\').collect();
                parts.len() >= 2
                    && !parts[0].is_empty()
                    && !parts[1].is_empty()
                    && parts[0] != "?"
                    && parts[0] != "."
            });
            drive || unc
        }
        Platform::Macos | Platform::Linux => value.starts_with('/'),
    };
    if !absolute || value.contains('\0') || value.split(['/', '\\']).any(|p| p == "." || p == "..")
    {
        return Err(ContractError::new(
            ErrorKind::InvalidValue,
            field,
            Rule::AbsolutePath,
        ));
    }
    Ok(())
}
fn environment(p: &ExecutionInput) -> Result<(), ContractError> {
    let mut names = BTreeSet::new();
    for (name, value) in &p.launch.env {
        if !names.insert(name.canonical_for(p.request.target.platform)) {
            return Err(ContractError::new(
                ErrorKind::InconsistentContext,
                Field::Environment,
                Rule::CaseCollision,
            ));
        }
        if let InputValue::Literal { value } = value {
            if value.as_str().is_none_or(|s| s.contains('\0')) {
                return Err(ContractError::new(
                    ErrorKind::InvalidValue,
                    Field::Environment,
                    Rule::EnvironmentValue,
                ));
            }
        }
    }
    Ok(())
}
pub(crate) fn unique_json<'de, D: Deserializer<'de>>(d: D) -> Result<Value, D::Error> {
    UniqueValue::deserialize(d).map(|v| v.0)
}
pub(crate) fn unique_map<'de, D, K, T>(d: D) -> Result<std::collections::BTreeMap<K, T>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    T: Deserialize<'de>,
{
    struct MapVisitor<K, T>(std::marker::PhantomData<(K, T)>);
    impl<'de, K: Deserialize<'de> + Ord, T: Deserialize<'de>> Visitor<'de> for MapVisitor<K, T> {
        type Value = std::collections::BTreeMap<K, T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unique map")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut out = std::collections::BTreeMap::new();
            while let Some(key) = a.next_key::<K>()? {
                if out.contains_key(&key) {
                    return Err(serde::de::Error::custom(
                        ContractError::new(
                            ErrorKind::Encoding,
                            Field::Document,
                            Rule::DuplicateKey,
                        )
                        .for_serde(),
                    ));
                }
                out.insert(key, a.next_value()?);
            }
            Ok(out)
        }
    }
    d.deserialize_map(MapVisitor(std::marker::PhantomData))
}
pub(crate) fn typed_value<T: serde::de::DeserializeOwned>(
    value: Value,
) -> Result<T, ContractError> {
    serde_json::from_value(value).map_err(ContractError::from_serde)
}

fn software(p: &ExecutionInput, limits: &ExecutionLimits) -> Result<(), ContractError> {
    let invalid =
        || ContractError::new(ErrorKind::InconsistentContext, Field::Plan, Rule::Mismatch);
    if let Some(program) = p.execution.software_program() {
        if program.steps.is_empty()
            || program.steps.len() > 32
            || p.launch.interpreter.profile.id.as_str() != "native-software-sequence"
            || p.request.operation.action.as_str()
                != match program.intent {
                    crate::SoftwareOperation::Install => "software.install",
                    crate::SoftwareOperation::Uninstall => "software.uninstall",
                    crate::SoftwareOperation::Detect => "software.detect",
                }
            || !p.request.parameters.is_empty()
        {
            return Err(invalid());
        }
        let invocation = |command: &crate::SoftwareInvocation| -> Result<(), ContractError> {
            if command.timeout_ms == 0
                || command.timeout_ms > 86_400_000
                || command.output_bytes == 0
                || command.output_bytes > 1_048_576
            {
                return Err(invalid());
            }
            let mut input = p.clone();
            input.execution = crate::ExecutionSpec::Process {};
            input.request.operation.action = crate::Id::new("native.step").expect("constant");
            input.launch = command.launch.clone();
            input.run_as = command.run_as.clone();
            input.session_requirement = command.session_requirement.clone();
            input.budget.total_timeout_ms = command.timeout_ms.min(p.budget.total_timeout_ms);
            input.budget.total_output_bytes = command.output_bytes.min(p.budget.total_output_bytes);
            validate_plan(&input, limits)
        };
        for step in &program.steps {
            if step.adapter.platform() != p.request.target.platform
                || !matches!(step.architecture.as_str(), "aarch64" | "x86_64")
                || step.adapter.is_bundle() != step.bundle.is_some()
                || step.adapter.is_bundle() != step.bundle_limits.is_some()
                || matches!(
                    step.adapter,
                    crate::SoftwareKind::Winget | crate::SoftwareKind::Homebrew
                ) != step.export_identity.is_some()
                || (program.intent == crate::SoftwareOperation::Uninstall
                    && step.uninstall.is_none())
            {
                return Err(invalid());
            }
            invocation(&step.install)?;
            if let Some(command) = &step.uninstall {
                invocation(command)?;
            }
            match &step.detection {
                crate::SoftwareDetector::MsiProduct {
                    product_code,
                    version,
                } => {
                    if step.adapter.platform() != Platform::Windows
                        || product_code.len() != 38
                        || !product_code.starts_with('{')
                        || !product_code.ends_with('}')
                        || product_code[1..37].bytes().enumerate().any(|(i, b)| {
                            if [8, 13, 18, 23].contains(&i) {
                                b != b'-'
                            } else {
                                !b.is_ascii_hexdigit()
                            }
                        })
                        || version != &step.version
                    {
                        return Err(invalid());
                    }
                }
                crate::SoftwareDetector::PkgReceipt { receipt, version } => {
                    if step.adapter.platform() != Platform::Macos
                        || receipt.is_empty()
                        || receipt.len() > 1024
                        || receipt.starts_with('-')
                        || receipt.chars().any(char::is_control)
                        || version != &step.version
                    {
                        return Err(invalid());
                    }
                }
                crate::SoftwareDetector::Script {
                    invocation: command,
                } => invocation(command)?,
            }
            if let Some(bundle) = &step.bundle {
                if bundle.platform != p.request.target.platform
                    || bundle.architecture != step.architecture
                    || bundle.entries.is_empty()
                    || bundle.entries.len() > 4096
                {
                    return Err(invalid());
                }
            }
        }
        return Ok(());
    }
    if p.request.operation.action.as_str().starts_with("software.") {
        return Err(invalid());
    }
    Ok(())
}

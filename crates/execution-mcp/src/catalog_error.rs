use schemars::JsonSchema;
use serde::Serialize;
use service_catalog::{ArgumentRule, CatalogError, DefinitionRule, Limit};

// MCP owns this wire projection; C03 remains the validation authority. Exhaustive
// conversions force a deliberate wire decision when C03 adds a diagnostic.
#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum CatalogErrorView {
    InvalidLimits { coordinate: LimitView },
    Malformed,
    DuplicateKey,
    LimitExceeded { coordinate: LimitView },
    UnsupportedVersion,
    InvalidShape,
    InvalidDefinition { rule: DefinitionRuleView },
    ReferenceMismatch,
    NotFound,
    InvalidArguments { rule: ArgumentRuleView },
    Encoding,
}
impl From<CatalogError> for CatalogErrorView {
    fn from(error: CatalogError) -> Self {
        match error {
            CatalogError::InvalidLimits(coordinate) => Self::InvalidLimits {
                coordinate: coordinate.into(),
            },
            CatalogError::Malformed => Self::Malformed,
            CatalogError::DuplicateKey => Self::DuplicateKey,
            CatalogError::LimitExceeded(coordinate) => Self::LimitExceeded {
                coordinate: coordinate.into(),
            },
            CatalogError::UnsupportedVersion => Self::UnsupportedVersion,
            CatalogError::InvalidShape => Self::InvalidShape,
            CatalogError::InvalidDefinition(rule) => Self::InvalidDefinition { rule: rule.into() },
            CatalogError::ReferenceMismatch => Self::ReferenceMismatch,
            CatalogError::NotFound => Self::NotFound,
            CatalogError::InvalidArguments(rule) => Self::InvalidArguments { rule: rule.into() },
            CatalogError::Encoding => Self::Encoding,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LimitView {
    Bytes,
    Depth,
    Nodes,
    StringBytes,
    CollectionItems,
    Parameters,
}
impl From<Limit> for LimitView {
    fn from(limit: Limit) -> Self {
        match limit {
            Limit::Bytes => Self::Bytes,
            Limit::Depth => Self::Depth,
            Limit::Nodes => Self::Nodes,
            Limit::StringBytes => Self::StringBytes,
            Limit::CollectionItems => Self::CollectionItems,
            Limit::Parameters => Self::Parameters,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DefinitionRuleView {
    TimeWindow,
    DuplicateItem,
    Display,
    Operations,
    Requirements,
    ParameterTitle,
    ParameterBounds,
    ParameterChoices,
    ParameterDefault,
}
impl From<DefinitionRule> for DefinitionRuleView {
    fn from(rule: DefinitionRule) -> Self {
        match rule {
            DefinitionRule::TimeWindow => Self::TimeWindow,
            DefinitionRule::DuplicateItem => Self::DuplicateItem,
            DefinitionRule::Display => Self::Display,
            DefinitionRule::Operations => Self::Operations,
            DefinitionRule::Requirements => Self::Requirements,
            DefinitionRule::ParameterTitle => Self::ParameterTitle,
            DefinitionRule::ParameterBounds => Self::ParameterBounds,
            DefinitionRule::ParameterChoices => Self::ParameterChoices,
            DefinitionRule::ParameterDefault => Self::ParameterDefault,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ArgumentRuleView {
    Object,
    UnknownParameter,
    Required,
    Type,
    Range,
    Choice,
    SecretReference,
    RoundedNumber,
}
impl From<ArgumentRule> for ArgumentRuleView {
    fn from(rule: ArgumentRule) -> Self {
        match rule {
            ArgumentRule::Object => Self::Object,
            ArgumentRule::UnknownParameter => Self::UnknownParameter,
            ArgumentRule::Required => Self::Required,
            ArgumentRule::Type => Self::Type,
            ArgumentRule::Range => Self::Range,
            ArgumentRule::Choice => Self::Choice,
            ArgumentRule::SecretReference => Self::SecretReference,
            ArgumentRule::RoundedNumber => Self::RoundedNumber,
        }
    }
}

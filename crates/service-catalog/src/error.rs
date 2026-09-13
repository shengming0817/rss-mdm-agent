use std::fmt;
/// Static host budget coordinate, never a caller-provided field name or value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Raw or compact/canonical JSON bytes, as documented by the entrypoint.
    Bytes,
    /// JSON nesting depth.
    Depth,
    /// Total JSON value count, including containers.
    Nodes,
    /// UTF-8 bytes in an individual string or key.
    StringBytes,
    /// Members of an individual object or array.
    CollectionItems,
    /// Declared or supplied parameter count.
    Parameters,
}
/// Static directory-definition rule; no publisher text is retained in diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionRule {
    /// Invalid UTC expiry/window.
    TimeWindow,
    /// Repeated catalog item ID.
    DuplicateItem,
    /// Blank display name or category.
    Display,
    /// Empty or repeated operation variants.
    Operations,
    /// Empty evidence requirements or repeated capability/evidence requirements.
    Requirements,
    /// Blank parameter title.
    ParameterTitle,
    /// Inverted or unsafe parameter range.
    ParameterBounds,
    /// Empty, repeated or out-of-range choice values.
    ParameterChoices,
    /// Default fails its rule or belongs to a required field.
    ParameterDefault,
}
/// Static argument-validation rule; no parameter names, arguments or secrets are included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentRule {
    /// Arguments must be an object.
    Object,
    /// Parameter is not declared.
    UnknownParameter,
    /// Required parameter is absent.
    Required,
    /// JSON value has the wrong type.
    Type,
    /// String length or integer range exceeded.
    Range,
    /// Value is not in the declared choice set.
    Choice,
    /// Value is not an exact secret reference of the required shape.
    SecretReference,
    /// Original number is nonintegral but floating-point decoding would round it to an integer.
    RoundedNumber,
}
/// Closed structured diagnostics. Neither Display nor Debug includes caller data or serde errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    /// Host supplied an invalid limit at the named coordinate.
    InvalidLimits(Limit),
    /// Input is not one complete valid JSON value.
    Malformed,
    /// A JSON object contains a duplicate key.
    DuplicateKey,
    /// The named host budget was exceeded.
    LimitExceeded(Limit),
    /// Catalog format version is absent or unsupported.
    UnsupportedVersion,
    /// A DTO has unknown/missing fields or unsupported variants.
    InvalidShape,
    /// Directory definition fails the specified static rule.
    InvalidDefinition(DefinitionRule),
    /// A snapshot or external display context does not match exactly.
    ReferenceMismatch,
    /// Explicitly selected item or operation does not exist.
    NotFound,
    /// Arguments fail the specified static rule.
    InvalidArguments(ArgumentRule),
    /// Canonical encoding failed; no partial result may be used.
    Encoding,
}
impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "catalog: {self:?}")
    }
}
impl std::error::Error for CatalogError {}

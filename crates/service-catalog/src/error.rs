use std::fmt;
/// Closed diagnostics. Neither Display nor Debug includes caller data or serde errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    /// Host supplied a zero limit or unsupported depth.
    InvalidLimits,
    /// Input is not one complete valid JSON value.
    Malformed,
    /// A JSON object contains a duplicate key.
    DuplicateKey,
    /// A host byte, depth, node, string or collection budget was exceeded.
    LimitExceeded,
    /// Catalog format version is absent or unsupported.
    UnsupportedVersion,
    /// A DTO has unknown fields, missing fields or unsupported variants.
    InvalidShape,
    /// Catalog definitions conflict or contain invalid constraints/defaults.
    InvalidDefinition,
    /// A requested snapshot or external display context does not match exactly.
    ReferenceMismatch,
    /// The explicitly selected item or operation does not exist.
    NotFound,
    /// Arguments fail the selected parameter declaration.
    InvalidArguments,
    /// Canonical encoding failed; no partial result may be used.
    Encoding,
}
impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "catalog: {self:?}")
    }
}
impl std::error::Error for CatalogError {}

use crate::Parameter;
use execution_contract::{Authority, Digest, Id, Platform, VersionedRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! closed_enum {
    ($name:ident,$doc:literal,{$($variant:ident => $description:literal),+$(,)?})=>{
        #[doc=$doc]
        #[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize,JsonSchema)]
        #[serde(rename_all="camelCase")]
        pub enum $name {$(#[doc=$description] $variant),+}
    }
}
closed_enum!(CatalogKind,"Display classification, independent of backend Resource kinds.",{
    Software=>"Software catalog entry.",Script=>"Published script entry.",Tool=>"User-facing tool; may map to a script resource."
});
closed_enum!(PublicationState,"State recorded in this immutable snapshot, not current authorization.",{
    Listed=>"Was listed in this snapshot.",Withdrawn=>"Was withdrawn in this snapshot."
});
closed_enum!(DistributionHint,"Non-authoritative distribution description.",{
    Required=>"Organization-required description; does not prohibit or authorize an action.",Optional=>"Optional self-service description.",Request=>"Request-first description; does not prove an approval requirement."
});
closed_enum!(RunAsRequirement,"Requested identity class; contains no authenticated account.",{
    TargetUser=>"Requires the explicitly chosen target user.",System=>"Requires system identity; grants no elevation."
});
closed_enum!(InteractionRequirement,"Interaction declaration, not an approval state machine.",{
    None=>"No user interaction declared.",UserSession=>"Requires an interactive user session."
});
/// Exact architecture selector; support remains a capability-owner decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Architecture {
    /// x86-64; backend adapters map their canonical representation explicitly.
    #[serde(rename = "x86_64")]
    X86_64,
    /// ARM64; no implicit architecture fallback.
    #[serde(rename = "aarch64")]
    Aarch64,
}
/// Exact variant coordinates, not a platform support assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSelector {
    /// Target platform namespace.
    pub platform: Platform,
    /// Exact target architecture.
    pub architecture: Architecture,
    /// Opaque resource variant key, separate from catalog operation ID and action.
    pub key: Id,
}
/// Immutable resource definition pin scoped by the containing catalog authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceBinding {
    /// Owner's resource identity and immutable revision; never a moving alias lookup.
    pub reference: VersionedRef,
    /// SHA-256 of the owner's resource version definition, NOT file byte SHA-256.
    pub version_digest: Digest,
    /// Exact variant within that resource version.
    pub selector: ResourceSelector,
}
/// Requirements only. Capability matching, identity binding and enforcement are external.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Requirements {
    /// Unique opaque required capabilities; no device facts or match algorithm.
    pub capabilities: Vec<Id>,
    /// Required execution identity class.
    pub run_as: RunAsRequirement,
    /// Required interactive context.
    pub interaction: InteractionRequirement,
    /// Nonempty unique evidence requirement IDs; not evidence of completion.
    pub evidence: Vec<Id>,
}
/// One independently selectable action and resource variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationVariant {
    /// Stable variant ID within its catalog item.
    pub id: Id,
    /// Action interpreted by the authorization/execution owner.
    pub action: Id,
    /// Complete immutable resource and target selector.
    pub resource: ResourceBinding,
    /// Single parameter declaration source, keyed by bounded parameter name.
    pub parameters: BTreeMap<Id, Parameter>,
    /// Conditions which do not constitute permission.
    pub requirements: Requirements,
}
/// Human and AI share the same item and operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogItem {
    /// Item ID unique within the snapshot.
    pub id: Id,
    /// User-facing classification.
    pub kind: CatalogKind,
    /// Nonempty plain-text name.
    pub name: String,
    /// Plain-text explanation, not trusted HTML or model instructions.
    pub description: String,
    /// Nonempty display category; not a backend group.
    pub category: String,
    /// Discovery hint only; false is not a security boundary.
    pub ai_discoverable: bool,
    /// Informational distribution description.
    pub distribution_hint: DistributionHint,
    /// Publication fact bound into this snapshot's digest.
    pub state: PublicationState,
    /// Nonempty unique operation variants.
    pub operations: Vec<OperationVariant>,
}
/// Directory-owned format discriminator; independent of execution-contract and Agent wire versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogSchemaV1;
impl Serialize for CatalogSchemaV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(1)
    }
}
impl<'de> Deserialize<'de> for CatalogSchemaV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u64::deserialize(deserializer)? {
            1 => Ok(Self),
            _ => Err(serde::de::Error::custom("unsupported catalog version")),
        }
    }
}
impl JsonSchema for CatalogSchemaV1 {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CatalogSchemaV1".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"integer","const":1})
    }
}
/// Untrusted catalog document. Use FrozenCatalog to validate and bind canonical content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogSnapshot {
    /// Only integer 1 is currently supported; independent of execution/Agent wire versions.
    pub schema_version: CatalogSchemaV1,
    /// All entries share this namespace. Deserialization does not authenticate it.
    pub authority: Authority,
    /// Opaque catalog ID and immutable revision.
    pub identity: VersionedRef,
    /// Exclusive UTC Unix millisecond expiry, positive and at most 2^53-1.
    pub expires_at_unix_ms: u64,
    /// Entries; an empty directory is valid.
    pub items: Vec<CatalogItem>,
}
/// Exact catalog content identity. It is neither a signature nor an authorization token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRef {
    /// Complete namespace including enterprise tenant when applicable.
    pub authority: Authority,
    /// Catalog ID/revision, not resource version or format version.
    pub identity: VersionedRef,
    /// Computed canonical catalog SHA-256.
    pub digest: Digest,
}
/// Exact snapshot, operation and normalized argument identity; contains no authority proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionRef {
    /// Exact snapshot and namespace.
    pub catalog: CatalogRef,
    /// Explicit item, never a name search.
    pub item_id: Id,
    /// Explicit operation variant, never a default selector.
    pub variant_id: Id,
    /// Domain-separated digest of normalized arguments including applied defaults and secret references.
    pub arguments_digest: Digest,
}

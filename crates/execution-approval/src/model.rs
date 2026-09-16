use execution_contract::{
    ActorId, Authority, Digest, FrozenPlan, PlanId, ValidityWindow, VersionedRef,
};

/// Caller-supplied references only; neither authenticates an approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileApproval {
    /// Exact profile required by the admission decision.
    pub profile: VersionedRef,
    /// Exact approval record to verify for this profile.
    pub record: VersionedRef,
}
/// Current record status. Unknown never grants approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Verified current active record.
    Active,
    /// Record has been revoked.
    Revoked,
    /// Current status cannot be established.
    Unknown,
}
/// Authenticated adapter output, never a deserializable permission.
/// All plan content is bound by the existing canonical digest.
#[derive(Debug, Clone)]
pub struct ApprovalRecord {
    /// Immutable record identity and revision in the verified authority namespace.
    pub reference: VersionedRef,
    /// Authenticated approver; the verifier checks their right to approve this plan.
    pub approver: ActorId,
    /// Exact approved plan identity.
    pub plan_id: PlanId,
    /// Exact approved canonical digest, not a signature.
    pub plan_digest: Digest,
    /// Exact profiles for which the approver is authorized.
    pub profiles: Vec<VersionedRef>,
    /// Approval window, with an exclusive deadline.
    pub validity: ValidityWindow,
    /// Verified revocation status.
    pub status: ApprovalStatus,
    /// Maximum admitted attempts using this record, not profiles or process exits.
    pub max_uses: u32,
    /// Uses already atomically committed by the storage owner.
    pub used: u32,
    /// CAS revision of the consumption counter.
    pub consumption_revision: u64,
}
/// One coherent trusted snapshot; no public evaluator accepts facts directly.
/// The adapter authenticates issuer/approver, signature or protected source,
/// authority namespace, current policy, revocation and reliable time independently.
/// INVARIANT: APPROVAL-VERIFIER-01 — evaluation obtains facts only through ApprovalVerifier.
/// ~~~compile_fail
/// let _: execution_approval::ApprovalFacts = serde_json::from_str("{}").unwrap();
/// ~~~
#[derive(Debug, Clone)]
pub struct ApprovalFacts {
    /// Namespace in which all records were verified, including the enterprise tenant.
    pub authority: Authority,
    /// Current verified policy revision.
    pub policy: VersionedRef,
    /// Exact trust/revocation snapshot revision to recheck at durable admission.
    pub verification_revision: VersionedRef,
    /// Reliable UTC Unix milliseconds after clock rollback checks.
    pub now_unix_ms: u64,
    /// Exclusive bound on freshness, including offline validity limits.
    pub fresh_until_unix_ms: u64,
    /// Exactly the requested records, from the same verification snapshot.
    pub records: Vec<ApprovalRecord>,
}
/// Product-owned trusted adapter. No production verifier or signing key is provided here.
/// Verify the source and approver rights for the complete plan; reject mixed revisions,
/// unknown trust roots, stale revocation or unreliable time. Decoding is not verification.
/// Like the existing authority port, this does not protect against a malicious host.
pub trait ApprovalVerifier {
    /// Batch-verify the distinct exact record references for this frozen plan.
    fn verify(
        &self,
        plan: &FrozenPlan,
        records: &[VersionedRef],
    ) -> Result<ApprovalFacts, VerificationError>;
}
/// Stable failures without provider text, tokens or user data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationError {
    /// Signature or trusted-source authenticity failed.
    Signature,
    /// Issuer or approver lacks authority for this plan/profile.
    Issuer,
    /// Protected records are unavailable.
    Unavailable,
    /// Reliable time cannot be established.
    Clock,
    /// Revocation state is unavailable or stale.
    Revocation,
    /// Current policy or trust revision cannot be verified.
    Policy,
}
/// Explicit work bounds, with no unlimited/default mode.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalLimits {
    /// Maximum required profiles and profiles in each record.
    pub max_profiles: usize,
    /// Maximum distinct approval records.
    pub max_records: usize,
}
/// Closed rejection vocabulary; never embeds submitted values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Invalid limits or too much work.
    Limit,
    /// Admission decision is bound to a different frozen plan.
    PlanMismatch,
    /// C07 denied the request.
    AdmissionDenied,
    /// Missing, extra or duplicate profile bindings.
    Bindings,
    /// Trusted adapter rejected verification.
    Verification(VerificationError),
    /// Returned facts belong to another authority or policy.
    Context,
    /// Plan validity has not begun.
    PlanNotYetValid,
    /// Plan validity has expired.
    PlanExpired,
    /// Verification snapshot is no longer fresh.
    StaleVerification,
    /// Approval record validity has not begun.
    ApprovalNotYetValid,
    /// Approval record validity has expired.
    ApprovalExpired,
    /// Missing, extra, duplicate or incorrectly bound record/profile.
    Record,
    /// Record was revoked; a new applicable approval is needed.
    Revoked,
    /// Current record status cannot be established.
    StatusUnknown,
    /// No remaining use, or an exhausted revision counter.
    Exhausted,
}
/// Approval applicability only; no variant permits runner dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// C07 allowed without approval. No verifier called and no use charged.
    NotRequired,
    /// All required profiles verified; consumption is still pending.
    Satisfied,
    /// No consumption intent is produced.
    Rejected(Reason),
}

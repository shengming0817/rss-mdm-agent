use crate::*;
use execution_admission::{AdmissionDecision, DecisionOutcome};
use execution_contract::{FrozenPlan, VersionedRef};
use std::collections::{BTreeMap, BTreeSet};

fn key(r: &VersionedRef) -> (&str, &str) {
    (r.id.as_str(), r.revision.as_str())
}
/// Sole applicability entry; performs no writes. Allowed/Denied never call the verifier.
/// Profiles come exclusively from the bound C07 decision, never a caller's subset.
/// ref: jsonwebtoken src/decoding.rs@4c0ae752e9acc108c8e2c4c8ed8128dc66014210
pub fn evaluate(
    plan: &FrozenPlan,
    admission: &AdmissionDecision,
    bindings: &[ProfileApproval],
    verifier: &(impl ApprovalVerifier + ?Sized),
    limits: ApprovalLimits,
) -> ApprovalDecision {
    let result = |outcome, bindings, consumptions| ApprovalDecision {
        plan_id: plan.spec().plan_id.clone(),
        plan_digest: plan.digest().clone(),
        attempt_id: admission.attempt_id().clone(),
        admission_validity: admission.validity().cloned(),
        outcome,
        bindings,
        consumptions,
    };
    let reject = |reason| result(ApprovalOutcome::Rejected(reason), vec![], vec![]);
    if admission.plan_id() != &plan.spec().plan_id
        || admission.plan_digest() != plan.digest()
        || admission.policy() != &plan.spec().policy
        || admission.delegation() != plan.spec().request.delegation.as_ref()
    {
        return reject(Reason::PlanMismatch);
    }
    let profiles = match admission.outcome() {
        DecisionOutcome::Allowed => return result(ApprovalOutcome::NotRequired, vec![], vec![]),
        DecisionOutcome::Denied => return reject(Reason::AdmissionDenied),
        DecisionOutcome::ApprovalRequired { profiles } => profiles,
    };
    if limits.max_profiles == 0
        || limits.max_records == 0
        || profiles.is_empty()
        || profiles.len() > limits.max_profiles
        || bindings.len() > limits.max_profiles
    {
        return reject(Reason::Limit);
    }
    let mut sorted = bindings.to_vec();
    sorted.sort_by(|a, b| key(&a.profile).cmp(&key(&b.profile)));
    if sorted.len() != profiles.len() || sorted.iter().zip(profiles).any(|(b, p)| &b.profile != p) {
        return reject(Reason::Bindings);
    }
    let refs: BTreeMap<_, _> = sorted
        .iter()
        .map(|b| (key(&b.record), b.record.clone()))
        .collect();
    if refs.len() > limits.max_records {
        return reject(Reason::Limit);
    }
    // Different revisions of one record cannot be charged as independent approvals.
    if refs.values().map(|r| &r.id).collect::<BTreeSet<_>>().len() != refs.len() {
        return reject(Reason::Record);
    }
    let requested: Vec<_> = refs.into_values().collect();
    let facts = match verifier.verify(plan, &requested) {
        Ok(facts) => facts,
        Err(error) => return reject(Reason::Verification(error)),
    };
    if facts.authority != plan.spec().request.authority || facts.policy != plan.spec().policy {
        return reject(Reason::Context);
    }
    if facts.now_unix_ms < plan.spec().validity.not_before_unix_ms {
        return reject(Reason::PlanNotYetValid);
    }
    if facts.now_unix_ms >= plan.spec().validity.expires_at_unix_ms {
        return reject(Reason::PlanExpired);
    }
    if facts.now_unix_ms >= facts.fresh_until_unix_ms {
        return reject(Reason::StaleVerification);
    }
    let Some(admission_validity) = admission.validity() else {
        return reject(Reason::AdmissionDenied);
    };
    if !admission_validity.is_current(facts.now_unix_ms, admission_validity.revision()) {
        return reject(Reason::StaleAdmission);
    }
    if facts.records.len() != requested.len() {
        return reject(Reason::Record);
    }
    let mut records = facts.records.iter().collect::<Vec<_>>();
    records.sort_by(|a, b| key(&a.reference).cmp(&key(&b.reference)));
    let mut consumptions = Vec::with_capacity(records.len());
    for (record, expected) in records.iter().zip(&requested) {
        if &record.reference != expected
            || record.plan_id != plan.spec().plan_id
            || &record.plan_digest != plan.digest()
            || record.profiles.is_empty()
            || record.profiles.len() > limits.max_profiles
        {
            return reject(Reason::Record);
        }
        let granted = record.profiles.iter().map(key).collect::<BTreeSet<_>>();
        if granted.len() != record.profiles.len()
            || sorted
                .iter()
                .filter(|b| &b.record == expected)
                .any(|b| !granted.contains(&key(&b.profile)))
        {
            return reject(Reason::Record);
        }
        match record.status {
            ApprovalStatus::Revoked => return reject(Reason::Revoked),
            ApprovalStatus::Unknown => return reject(Reason::StatusUnknown),
            ApprovalStatus::Active => {}
        }
        if facts.now_unix_ms < record.validity.not_before_unix_ms {
            return reject(Reason::ApprovalNotYetValid);
        }
        if facts.now_unix_ms >= record.validity.expires_at_unix_ms {
            return reject(Reason::ApprovalExpired);
        }
        if record.max_uses == 0
            || record.used >= record.max_uses
            || record.consumption_revision == u64::MAX
        {
            return reject(Reason::Exhausted);
        }
        consumptions.push(ConsumptionIntent {
            plan_id: plan.spec().plan_id.clone(),
            plan_digest: plan.digest().clone(),
            attempt_id: admission.attempt_id().clone(),
            approval: record.reference.clone(),
            expected_consumption_revision: record.consumption_revision,
            expected_uses: record.used,
            verification_revision: facts.verification_revision.clone(),
            valid_until_unix_ms: facts
                .fresh_until_unix_ms
                .min(admission_validity.valid_until_unix_ms())
                .min(plan.spec().validity.expires_at_unix_ms)
                .min(record.validity.expires_at_unix_ms),
        });
    }
    result(ApprovalOutcome::Satisfied, sorted, consumptions)
}

//! Device-authorized management projection cache; transport receipts never assert device effects.
use crate::{store::encode, Client, Clock, Error, IdentityProvider};
use rss_mdm_agent_wire::activities::{
    ActivityCoverage, ManagementActivity, ManagementActivityDetail, ManagementActivityPage,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Bounded cached producer facts and the exact snapshot/incremental continuation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementCache {
    /// Current native scope; never reused by another login incarnation.
    pub scope: String,
    /// Source-qualified original facts.
    pub items: BTreeMap<String, ManagementActivity>,
    /// Bounded scrubbed exact details and their explicit association gaps.
    pub details: BTreeMap<String, ManagementActivityDetail>,
    /// Last successful server observation, in UTC seconds.
    pub as_of: Option<i64>,
    /// Original source completeness.
    pub coverage: Vec<ActivityCoverage>,
    /// Producer retention boundary, in UTC seconds.
    pub retained_from: Option<i64>,
    /// Snapshot or changes page continuation.
    pub next: Option<String>,
    /// Fully consumed snapshot's incremental start.
    pub sync: Option<String>,
    /// Pending snapshot anchor, not safe to publish as incremental start yet.
    anchor: Option<String>,
    /// Local retention loss remains explicit across subsequent snapshot/changes pages.
    bounded_out: bool,
    /// True only once the current snapshot is fully consumed within local bounds.
    pub complete: bool,
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Read an already authorized device cache without HTTP or execution.
    pub fn management_cache(&self, scope: &str) -> Result<ManagementCache, Error> {
        Ok(self
            .store
            .get::<ManagementCache>("management-cache")?
            .filter(|v| v.scope == scope)
            .unwrap_or_else(|| ManagementCache {
                scope: scope.into(),
                ..Default::default()
            }))
    }
    /// Clear cached native-subject data on logout/rebinding; no identity or journal reset occurs.
    pub fn clear_management_cache(&self) -> Result<(), Error> {
        self.store
            .conn
            .execute("DELETE FROM state WHERE key='management-cache'", [])?;
        Ok(())
    }
    /// Fetch one bounded authorized page and commit facts plus continuation atomically.
    pub async fn sync_management(&self, scope: &str) -> Result<ManagementCache, Error> {
        self.active()?;
        let binding = self.registration()?;
        let namespace = crate::store::hash(&encode(&(scope, &binding))?);
        let mut cache = self.management_cache(&namespace)?;
        let changes = cache.sync.is_some();
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query
            .append_pair("limit", "64")
            .append_pair("view", "summary");
        if let Some(cursor) = cache.next.as_ref().or(cache.sync.as_ref()) {
            query.append_pair("cursor", cursor);
        }
        let path = format!(
            "management-activities{}?{}",
            if changes { "/changes" } else { "" },
            query.finish()
        );
        let response = self
            .self_service_json::<ManagementActivityPage>(None, reqwest::Method::GET, &path, None)
            .await;
        let page = match response {
            Ok(page) => page,
            Err(Error::Protocol | Error::Conflict) => {
                cache.next = None;
                cache.sync = None;
                cache.anchor = None;
                cache.complete = false;
                cache.coverage.push(ActivityCoverage {
                    source: "continuation".into(),
                    complete: false,
                    reasons: vec!["snapshot-restart-required".into()],
                });
                self.store.put("management-cache", &cache)?;
                return Err(Error::Conflict);
            }
            Err(error) => return Err(error),
        };
        if page.items.len() > 64
            || page.as_of < 0
            || page.coverage.len() > 32
            || page
                .next_cursor
                .as_ref()
                .is_some_and(|v| Some(v) == cache.next.as_ref())
        {
            return Err(Error::Protocol);
        }
        if cache.next.is_some()
            && cache
                .anchor
                .as_ref()
                .is_some_and(|anchor| anchor != &page.sync_cursor)
        {
            return Err(Error::Protocol);
        }
        for activity in page.items {
            if activity.id.len() > 512 || activity.recorded_at < 0 {
                return Err(Error::Protocol);
            }
            cache.items.insert(activity.id.clone(), activity);
        }
        cache.retained_from = page.retained_from;
        if let Some(from) = cache.retained_from {
            cache.items.retain(|_, v| v.recorded_at >= from);
        }
        let limit = self
            .configuration()
            .limits
            .pending_tasks
            .saturating_mul(64)
            .min(4096);
        let mut truncated = false;
        while cache.items.len() > limit {
            let oldest = cache
                .items
                .values()
                .min_by_key(|v| (v.recorded_at, &v.id))
                .map(|v| v.id.clone())
                .ok_or(Error::Storage)?;
            cache.items.remove(&oldest);
            truncated = true;
        }
        cache.as_of = Some(page.as_of);
        cache.coverage = page.coverage;
        cache.bounded_out |= truncated;
        if cache.bounded_out {
            cache.coverage.push(ActivityCoverage {
                source: "local-cache".into(),
                complete: false,
                reasons: vec!["local-retention-limit".into()],
            });
        }
        cache.next = page.next_cursor;
        cache.anchor = Some(page.sync_cursor);
        if cache.next.is_none() {
            cache.sync = cache.anchor.take();
        }
        cache.complete =
            cache.next.is_none() && !cache.bounded_out && cache.coverage.iter().all(|v| v.complete);
        // One state record is the original communication owner's atomic cache/checkpoint commit.
        self.store.put("management-cache", &cache)?;
        cache.scope = scope.into();
        Ok(cache)
    }
    /// Fetch an exact scrubbed producer detail, even when its list page is not locally cached.
    pub async fn management_detail(&self, scope: &str, id: &str) -> Result<ManagementCache, Error> {
        if id.is_empty() || id.len() > 512 {
            return Err(Error::Protocol);
        }
        let mut url = url::Url::parse("https://query.invalid/management-activities/")
            .map_err(|_| Error::Protocol)?;
        url.path_segments_mut()
            .map_err(|_| Error::Protocol)?
            .pop_if_empty()
            .push(id);
        let value: ManagementActivityDetail = self
            .self_service_json(
                None,
                reqwest::Method::GET,
                &format!("{}?view=summary", url.path().trim_start_matches('/')),
                None,
            )
            .await?;
        if value.activity.id != id
            || value.personal
            || value.related.len() > 64
            || value.missing.len() > 64
            || value.as_of < 0
        {
            return Err(Error::Protocol);
        }
        let namespace = crate::store::hash(&encode(&(scope, self.registration()?))?);
        let mut cache = self.management_cache(&namespace)?;
        cache.as_of = Some(cache.as_of.map_or(value.as_of, |old| old.max(value.as_of)));
        cache.items.insert(id.into(), value.activity.clone());
        for related in &value.related {
            cache.items.insert(related.id.clone(), related.clone());
        }
        if cache.details.len() >= 64 && !cache.details.contains_key(id) {
            let old = cache.details.keys().next().cloned().ok_or(Error::Storage)?;
            cache.details.remove(&old);
        }
        cache.details.insert(id.into(), value);
        let limit = self
            .configuration()
            .limits
            .pending_tasks
            .saturating_mul(64)
            .min(4096);
        while cache.items.len() > limit {
            let old = cache
                .items
                .values()
                .filter(|v| v.id != id)
                .min_by_key(|v| v.recorded_at)
                .map(|v| v.id.clone())
                .ok_or(Error::Capacity)?;
            cache.items.remove(&old);
        }
        self.store.put("management-cache", &cache)?;
        cache.scope = scope.into();
        Ok(cache)
    }
    /// Authorized current-registration cache, scoped to the authenticated native incarnation.
    pub fn current_management_cache(&self, scope: &str) -> Result<ManagementCache, Error> {
        let namespace = crate::store::hash(&encode(&(scope, self.registration()?))?);
        let mut cache = self.management_cache(&namespace)?;
        cache.scope = scope.into();
        Ok(cache)
    }
}

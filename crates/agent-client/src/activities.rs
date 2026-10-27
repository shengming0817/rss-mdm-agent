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
    /// Read a bounded page through the same management activity owner. This does not
    /// advance the independent full-history synchronization cursor.
    pub async fn management_page(
        &self,
        batch: Option<&str>,
        related: Option<&str>,
        after: Option<&str>,
        limit: u16,
    ) -> Result<ManagementActivityPage, Error> {
        if !(1..=64).contains(&limit)
            || batch.is_some_and(|v| uuid::Uuid::parse_str(v).is_err())
            || related.is_some_and(|v| v.is_empty() || v.len() > 512)
            || after.is_some_and(|v| v.is_empty() || v.len() > 4096)
        {
            return Err(Error::Protocol);
        }
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("limit", &limit.to_string());
        if batch.is_none() && related.is_none() {
            query.append_pair("kind", "batch");
        }
        for (key, value) in [("batch", batch), ("related", related), ("cursor", after)] {
            if let Some(value) = value {
                query.append_pair(key, value);
            }
        }
        let value: ManagementActivityPage = self
            .self_service_json(
                None,
                reqwest::Method::GET,
                &format!("management-activities?{}", query.finish()),
                None,
            )
            .await?;
        if value.items.len() > usize::from(limit)
            || value.as_of < 0
            || value
                .next_cursor
                .as_deref()
                .is_some_and(|v| Some(v) == after || v.len() > 4096)
            || value.items.iter().any(|v| {
                batch.is_some_and(|id| v.links.batch.map(|v| v.to_string()).as_deref() != Some(id))
            })
        {
            return Err(Error::Untrusted);
        }
        Ok(value)
    }
    /// Recover an explicit event's accepted batch through the existing activity projection.
    /// Missing or incomplete visibility never proves nonacceptance; only an exact idempotent
    /// replay may follow a successful read with no match.
    pub async fn event_batch(&self, request: uuid::Uuid) -> Result<Option<uuid::Uuid>, Error> {
        let page: ManagementActivityPage = self
            .self_service_json(
                None,
                reqwest::Method::GET,
                &format!("management-activities?request={request}&kind=batch&limit=64"),
                None,
            )
            .await?;
        if page.items.len() > 64 || page.as_of < 0 {
            return Err(Error::Untrusted);
        }
        let mut found = None;
        for item in page.items {
            if item.kind != rss_mdm_agent_wire::activities::ActivityKind::Batch
                || item.links.request != Some(request)
            {
                return Err(Error::Untrusted);
            }
            if let Some(batch) = item.links.batch {
                if found.is_some_and(|old| old != batch) {
                    return Err(Error::Untrusted);
                }
                found = Some(batch);
            }
        }
        Ok(found)
    }
    fn save_management_cache(&self, mut cache: ManagementCache) -> Result<ManagementCache, Error> {
        let limit = self.configuration().limits;
        let count = limit.pending_tasks.saturating_mul(64).min(4096);
        loop {
            if cache.bounded_out {
                cache.complete = false;
                cache.coverage.retain(|v| v.source != "local-cache");
                cache.coverage.push(ActivityCoverage {
                    source: "local-cache".into(),
                    complete: false,
                    reasons: vec!["local-retention-budget".into()],
                });
            }
            if cache.items.len() <= count
                && cache.details.len() <= 64
                && encode(&cache)?.len() <= limit.response_bytes
            {
                break;
            }
            cache.bounded_out = true;
            if let Some(key) = cache.details.keys().next().cloned() {
                cache.details.remove(&key);
                continue;
            }
            if let Some(key) = cache
                .items
                .values()
                .min_by_key(|v| (v.recorded_at, &v.id))
                .map(|v| v.id.clone())
            {
                cache.items.remove(&key);
                continue;
            }
            if cache.coverage.len() > 1 {
                cache.coverage.clear();
                continue;
            }
            return Err(Error::Capacity);
        }
        self.store.put("management-cache", &cache)?;
        Ok(cache)
    }

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
                cache.coverage.retain(|v| v.source != "continuation");
                cache.coverage.push(ActivityCoverage {
                    source: "continuation".into(),
                    complete: false,
                    reasons: vec!["snapshot-restart-required".into()],
                });
                self.save_management_cache(cache)?;
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
        cache.as_of = Some(page.as_of);
        cache.coverage = page.coverage;
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
        cache = self.save_management_cache(cache)?;
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
        cache = self.save_management_cache(cache)?;
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

//! Local projections of trusted collection evidence; upload acknowledgement is independent.
use crate::collection_worker::ReadPlan;
use agent_client::{CollectedStore, CredentialRedactor, Error};
use execution_contract::*;
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

mod catalog;
mod projected;
pub(crate) use projected::LocalProjection;
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Definition {
    pub descriptor: QueryDescriptor,
    pub read: ReadPlan,
    pub seen_at: u64,
    pub enterprise: Option<Digest>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Association {
    pub query: QueryRef,
    pub subject: String,
    pub session: Id,
    pub parameters: Digest,
    pub kind: QueryKind,
    pub created_at: u64,
    pub origin: Digest,
}
#[derive(Serialize, Deserialize)]
struct Published {
    enterprise: Option<Digest>,
    locator: PolicyLocator,
    generated_uid: Option<String>,
    descriptor: QueryDescriptor,
    item: w::SelfServiceDirectoryItem,
    subject: String,
    session: Id,
    context: NativeContextRef,
    seen_at: u64,
}
#[derive(Serialize, Deserialize)]
struct PolicyRead {
    enterprise: Option<Digest>,
    descriptor: QueryDescriptor,
    generated_uid: Option<String>,
    seen_at: u64,
}
#[derive(Serialize, Deserialize)]
struct Observed {
    source: w::TelemetryConfiguration,
    enterprise: Option<Digest>,
    descriptor: QueryDescriptor,
}
fn account_uid(subject: &str) -> Result<Option<String>, Error> {
    if subject == "0" {
        Ok(None)
    } else {
        crate::account_identity::generated_uid(subject)
    }
}
fn policy_revision(item: &w::SelfServiceDirectoryItem, subject: &str) -> Result<Digest, Error> {
    digest(&(item, subject, account_uid(subject)?))
}
pub(crate) struct Data {
    store: CollectedStore,
    pub(crate) service_id: uuid::Uuid,
    redactor: CredentialRedactor,
    pinned: std::collections::BTreeSet<Id>,
    local_reads: BTreeMap<Id, ReadPlan>,
    enterprise: Option<Digest>,
}
pub(crate) fn digest(value: &impl Serialize) -> Result<Digest, Error> {
    Digest::new(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|_| Error::Protocol)?)
    ))
    .map_err(|_| Error::Protocol)
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, Error> {
    serde_json::from_value(v).map_err(|_| Error::Protocol)
}
fn encode(v: &impl Serialize) -> Result<Value, Error> {
    serde_json::to_value(v).map_err(|_| Error::Protocol)
}
/// Mapping is based on closed recipes, never a Policy name or a producer-selected query ID.
fn domain(q: &w::NativeQueryDefinition) -> Option<&'static str> {
    use w::NativeQueryDefinition::*;
    Some(match q {
        WindowsCpu => "hardware.cpu",
        WindowsNetwork => "hardware.network",
        WindowsBattery => "hardware.battery",
        MacosHardware { section } => match section {
            w::MacosHardwareSection::Identity => "hardware.identity",
            w::MacosHardwareSection::Os => "hardware.os",
            w::MacosHardwareSection::Cpu => "hardware.cpu",
            w::MacosHardwareSection::Memory => "hardware.memory",
            w::MacosHardwareSection::Firmware => "hardware.firmware",
            w::MacosHardwareSection::Disks => "hardware.disks",
            w::MacosHardwareSection::ApfsContainers => "hardware.containers",
            w::MacosHardwareSection::Volumes => "hardware.volumes",
            w::MacosHardwareSection::Network => "hardware.network",
            w::MacosHardwareSection::Battery => "hardware.battery",
        },
        System {
            query:
                w::NativeSystemQuery::MacosSecurity {
                    section: w::MacosSecuritySection::Filevault,
                }
                | w::NativeSystemQuery::WindowsBitlocker,
        } => "security.encryption",
        System {
            query:
                w::NativeSystemQuery::MacosSecurity {
                    section: w::MacosSecuritySection::Firewall,
                }
                | w::NativeSystemQuery::WindowsFirewall,
        } => "security.firewall",
        System {
            query: w::NativeSystemQuery::WindowsDefender,
        } => "security.protection",
        WindowsPerformance { .. } | MacosPerformance { .. } => "performance",
        WindowsSoftware | MacosApplications { .. } => "software.installations",
        MacosPkgReceipts {} => "software.receipts",
        MacosBrew { .. } => "software.packages",
        WindowsWmi {
            namespace,
            class,
            properties,
            filter,
        } if namespace == r"root\cimv2" && filter.is_none() => {
            let (kind, allowed): (&str, &[&str]) = match class.as_str() {
                "Win32_ComputerSystem" => (
                    "hardware.system",
                    &["Model", "Manufacturer", "TotalPhysicalMemory"],
                ),
                "Win32_ComputerSystemProduct" => ("hardware.identity", &["UUID"]),
                "Win32_BIOS" => ("hardware.firmware", &["SerialNumber", "SMBIOSBIOSVersion"]),
                "Win32_DiskDrive" => ("hardware.disks", &["DeviceID", "Model", "Size"]),
                "Win32_LogicalDisk" => (
                    "hardware.volumes",
                    &["DeviceID", "Size", "FreeSpace", "DriveType"],
                ),
                "Win32_OperatingSystem" => (
                    "hardware.os",
                    &["Caption", "Version", "BuildNumber", "SystemDrive"],
                ),
                _ => return None,
            };
            if properties.keys().any(|k| !allowed.contains(&k.as_str())) {
                return None;
            }
            kind
        }
        WindowsRegistry {
            hive: w::RegistryHive::LocalMachine,
            view: w::RegistryView::Registry64,
            path,
            enumerate_subkeys: false,
            values,
        } if values.len() == 1
            && values.values().all(|v| {
                v.value_type == w::RegistryType::Sz
                    && match path.as_str() {
                        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion" => v.name == "EditionID",
                        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment" => {
                            v.name == "PROCESSOR_ARCHITECTURE"
                        }
                        _ => false,
                    }
            }) =>
        {
            "hardware.os"
        }
        // Generic registry/WMI and all other security recipes have no default local read grant.
        // A future mapping must name the exact typed properties and their access boundary.
        _ => return None,
    })
}
fn read_granted(read: &ReadPlan) -> bool {
    read.plan
        .spec()
        .queries
        .values()
        .any(|q| domain(&q.definition).is_some())
}
fn subject(read: &ReadPlan) -> Option<&str> {
    match &read.execution.target {
        w::SoftwareExecutionTarget::User { identity, .. } => Some(identity),
        _ => None,
    }
}
fn mapped_field(q: &w::NativeQueryDefinition, key: &str) -> Option<String> {
    use w::NativeQueryDefinition::*;
    let exact = match q {
        WindowsWmi { class, .. } => match (class.as_str(), key) {
            ("Win32_ComputerSystem", "Model") => "hardware.identity.model",
            ("Win32_ComputerSystem", "Manufacturer") => "hardware.identity.manufacturer",
            ("Win32_ComputerSystem", "TotalPhysicalMemory") => "hardware.memory.total_bytes",
            ("Win32_ComputerSystemProduct", "UUID") => "hardware.identity.hardware_uuid",
            ("Win32_BIOS", "SerialNumber") => "hardware.identity.serial",
            ("Win32_BIOS", "SMBIOSBIOSVersion") => "hardware.firmware.version",
            ("Win32_OperatingSystem", "Caption") => "hardware.os.name",
            ("Win32_OperatingSystem", "Version") => "hardware.os.version",
            ("Win32_OperatingSystem", "BuildNumber") => "hardware.os.build",
            ("Win32_OperatingSystem", "SystemDrive") => "hardware.os.system_drive",
            _ => return None,
        },
        WindowsRegistry { values, .. } => match values.get(key)?.name.as_str() {
            "EditionID" => "hardware.os.edition",
            "PROCESSOR_ARCHITECTURE" => "hardware.os.architecture",
            _ => return None,
        },
        WindowsPerformance { section } | MacosPerformance { section } => {
            return Some(format!(
                "performance.{}.{}",
                serde_json::to_value(section).ok()?.as_str()?,
                key
            ))
        }
        _ => return Some(format!("{}.{}", domain(q)?, key)),
    };
    Some(exact.into())
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
fn visible(read: &ReadPlan, caller: &str) -> bool {
    match subject(read) {
        None => true,
        Some(owner) => {
            owner == caller
                && crate::account_identity::generated_uid(caller)
                    .is_ok_and(|current| current == read.execution.generated_uid)
        }
    }
}
fn title(q: &w::NativeQueryDefinition) -> String {
    use w::NativeQueryDefinition::*;
    let title = match q {
        MacosHardware { section } => format!("macOS hardware: {section:?}"),
        MacosApplications { root, .. } => format!("macOS applications: {root:?}"),
        MacosBrew {
            prefix, package, ..
        } => format!("Homebrew {package:?}: {prefix}"),
        MacosPkgReceipts {} => "macOS package receipts".into(),
        WindowsWmi { class, .. } => format!("Windows {class}"),
        WindowsRegistry { values, .. } => format!(
            "Windows registry: {}",
            values
                .values()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => format!("{q:?}"),
    };
    title.chars().take(256).collect()
}
fn single(read: &ReadPlan, query: &w::NativeQuery) -> Result<ReadPlan, Error> {
    let mut result = read.clone();
    let source = read
        .plan
        .spec()
        .queries
        .iter()
        .find(|(_, candidate)| *candidate == query)
        .map(|(id, _)| id)
        .ok_or(Error::Protocol)?;
    if let crate::collection_worker::ReadSource::Local { projection } = &read.source {
        result.source = crate::collection_worker::ReadSource::Local {
            projection: Box::new(projection.single(source)?),
        };
    }
    result.plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
        version: 1,
        scope: read.plan.spec().scope.clone(),
        queries: [("read".into(), query.clone())].into(),
        budget: read.plan.spec().budget,
    })?;
    Ok(result)
}
impl Data {
    pub fn new(
        store: CollectedStore,
        redactor: CredentialRedactor,
        service_id: uuid::Uuid,
    ) -> Self {
        Self {
            store,
            service_id,
            redactor,
            pinned: Default::default(),
            local_reads: Default::default(),
            enterprise: None,
        }
    }
    pub fn bind_enterprise(
        &mut self,
        binding: Option<&execution_app::ServiceBinding>,
    ) -> Result<(), Error> {
        self.enterprise = binding
            .map(|b| digest(&(&b.authority, &b.device)))
            .transpose()?;
        Ok(())
    }
    pub fn protect(
        &mut self,
        value: &std::sync::Arc<zeroize::Zeroizing<String>>,
    ) -> Result<(), Error> {
        self.redactor.protect_user_session(value)
    }
    pub fn settings_read(
        &self,
        subject: &str,
        session: &Id,
        section: SettingsSection,
    ) -> Result<Option<SettingsSnapshot>, Error> {
        self.store
            .read(&format!(
                "settings/{}/{section:?}",
                digest(&(subject, session, account_uid(subject)?))?.as_str()
            ))?
            .map(decode)
            .transpose()
    }
    pub fn settings_write(
        &mut self,
        subject: &str,
        session: &Id,
        snapshot: &SettingsSnapshot,
    ) -> Result<(), Error> {
        // Reconciliation revisits retained tasks in UUID order, not observation order.
        if self
            .settings_read(subject, session, snapshot.section)?
            .is_some_and(|current| current.collected_at_unix_ms >= snapshot.collected_at_unix_ms)
        {
            return Ok(());
        }
        self.store.write(
            &format!(
                "settings/{}/{:?}",
                digest(&(subject, session, account_uid(subject)?))?.as_str(),
                snapshot.section
            ),
            &encode(snapshot)?,
        )
    }
    pub fn history_limit(&self) -> usize {
        (self.store.limits().0 / (128 * 1024))
            .min(self.store.limits().1.saturating_sub(self.pinned.len()) / 4)
            .clamp(1, 32)
    }
    pub fn pin(&mut self, query: &QueryRef, read: &ReadPlan) {
        self.pinned.insert(query.id.clone());
        self.local_reads.insert(query.id.clone(), read.clone());
    }
    fn scan(&self, prefix: &str) -> Result<Vec<(String, Value)>, Error> {
        let mut values = Vec::new();
        let mut after = None;
        loop {
            let page = self.store.page(prefix, after.as_deref(), 128)?;
            let done = page.len() < 128;
            after = page.last().map(|(key, _)| key.clone());
            values.extend(page);
            if done {
                return Ok(values);
            }
        }
    }
    pub fn input_requests(&self) -> Result<Vec<RequestId>, Error> {
        self.scan("input/")?
            .into_iter()
            .map(|(key, _)| {
                RequestId::new(key.strip_prefix("input/").ok_or(Error::Protocol)?)
                    .map_err(|_| Error::Protocol)
            })
            .collect()
    }
    pub fn forget_input(&mut self, request: &RequestId) -> Result<(), Error> {
        self.store.remove(&format!("input/{}", request.as_str()))
    }
    /// Keep bounded metadata independently of value eviction; never evict active request identity.
    pub fn maintain(
        &mut self,
        contexts: &[(String, Id, NativeContextRef)],
        active_inputs: &std::collections::BTreeSet<RequestId>,
        active_policies: &std::collections::BTreeSet<String>,
    ) -> Result<(), Error> {
        let current_settings = contexts
            .iter()
            .map(|(subject, session, _)| {
                digest(&(subject, session, account_uid(subject)?))
                    .map(|id| format!("settings/{}/", id.as_str()))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        for (key, _) in self.scan("settings/")? {
            if !current_settings
                .iter()
                .any(|prefix| key.starts_with(prefix))
            {
                self.store.remove(&key)?;
            }
        }
        let protected = self
            .scan("request/")?
            .into_iter()
            .map(|(_, v)| decode::<Association>(v).map(|a| a.query.id))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let mut published = Vec::new();
        for (key, value) in self.scan("published/")? {
            let p: Published = decode(value)?;
            if contexts.iter().any(|(subject, session, context)| {
                *subject == p.subject && *session == p.session && *context == p.context
            }) {
                published.push((key, p.seen_at));
            } else {
                self.store.remove(&key)?;
            }
        }
        published.sort_by_key(|(_, at)| *at);
        let mut excess = published.len().saturating_sub(128);
        for (key, _) in published {
            if excess == 0 {
                break;
            }
            if !protected
                .iter()
                .any(|id| key == format!("published/{}", id.as_str()))
            {
                self.store.remove(&key)?;
                excess -= 1;
            }
        }
        let mut definitions = Vec::new();
        for (key, value) in self.scan("definition/")? {
            let d: Definition = decode(value)?;
            if subject(&d.read).is_some_and(|owner| !visible(&d.read, owner)) {
                self.pinned.remove(&d.descriptor.reference.id);
                self.local_reads.remove(&d.descriptor.reference.id);
                if !protected.contains(&d.descriptor.reference.id) {
                    self.store.remove(&key)?;
                    self.store
                        .remove(&format!("latest/{}", d.descriptor.reference.id.as_str()))?;
                }
                continue;
            }
            if !self.pinned.contains(&d.descriptor.reference.id)
                && !protected.contains(&d.descriptor.reference.id)
            {
                definitions.push((key, d.seen_at, d.descriptor.reference.id));
            }
        }
        definitions.sort_by_key(|(_, at, _)| *at);
        let excess = definitions.len().saturating_sub(64);
        for (key, _, id) in definitions.into_iter().take(excess) {
            self.store.remove(&key)?;
            self.store.remove(&format!("latest/{}", id.as_str()))?;
        }
        let mut observed = Vec::new();
        for (key, value) in self.scan("observed/")? {
            let metadata: Observed = decode(value)?;
            let d = metadata.descriptor;
            if !active_policies.contains(d.reference.id.as_str()) {
                let latest: Option<CollectedData> = self
                    .store
                    .read(&format!("latest/{}", d.reference.id.as_str()))?
                    .map(decode)
                    .transpose()?;
                observed.push((
                    key,
                    latest.and_then(|v| v.collected_at).unwrap_or(0),
                    d.reference.id,
                ));
            }
        }
        observed.sort_by_key(|(_, at, _)| *at);
        let excess = observed.len().saturating_sub(32);
        for (key, _, id) in observed.into_iter().take(excess) {
            self.store.remove(&key)?;
            self.store.remove(&format!("latest/{}", id.as_str()))?;
        }
        for request in self.input_requests()? {
            if !active_inputs.contains(&request) {
                self.forget_input(&request)?;
            }
        }
        // Counts alone do not bound schemas of different sizes. Reclaim unreferenced
        // metadata by bytes too, while preserving current recipes and request identity.
        let mut candidates = Vec::new();
        let mut total = 0usize;
        for prefix in ["definition/", "published/", "observed/", "policy-read/"] {
            for (key, value) in self.scan(prefix)? {
                let bytes = serde_json::to_vec(&value)
                    .map_err(|_| Error::Protocol)?
                    .len();
                total = total.saturating_add(bytes);
                let id = key.strip_prefix(prefix).ok_or(Error::Protocol)?.to_owned();
                let protected = protected.iter().any(|q| q.as_str() == id.as_str())
                    || self.pinned.iter().any(|q| q.as_str() == id.as_str())
                    || active_policies.contains(&id);
                if !protected {
                    candidates.push((
                        key,
                        value.get("seen_at").and_then(Value::as_u64).unwrap_or(0),
                        bytes,
                        id,
                    ));
                }
            }
        }
        candidates.sort_by_key(|(_, at, _, _)| *at);
        let records = self.store.limits().1;
        // Reserve room for request identity, recovery input and results even with the
        // smallest supported namespace. Current recipes and active work stay protected.
        let target = records.saturating_sub((records / 4).max(8));
        let mut count = self.store.records()?;
        for (key, _, bytes, id) in candidates {
            if total <= self.store.limits().0 / 2 && count <= target {
                break;
            }
            self.store.remove(&key)?;
            self.store.remove(&format!("latest/{id}"))?;
            total = total.saturating_sub(bytes);
            count = self.store.records()?;
        }
        Ok(())
    }
    fn policy_metadata(&self, a: &Association) -> Result<PolicyRead, Error> {
        if a.kind != QueryKind::Policy {
            return Err(Error::Denied);
        }
        let published: Published = decode(
            self.store
                .read(&format!("published/{}", a.query.id.as_str()))?
                .ok_or(Error::NotFound)?,
        )?;
        if published.enterprise != self.enterprise
            || self.enterprise.is_none()
            || published.subject != a.subject
            || published.descriptor.reference != a.query
            || published.generated_uid != account_uid(&a.subject)?
        {
            return Err(Error::Denied);
        }
        Ok(PolicyRead {
            enterprise: self.enterprise.clone(),
            descriptor: published.descriptor,
            generated_uid: published.generated_uid,
            seen_at: now(),
        })
    }
    pub fn policy_result(
        &mut self,
        a: &Association,
        status: &w::SelfServiceStatus,
    ) -> Result<(), Error> {
        let request = RequestId::new(status.request_id.to_string()).map_err(|_| Error::Protocol)?;
        if self.captured(&request)? {
            return Ok(());
        }
        let Some(w::SelfServiceResult::Script { evidence }) = &status.result else {
            return Ok(());
        };
        let mut value = empty(
            a.query.clone(),
            Some(RequestId::new(status.request_id.to_string()).map_err(|_| Error::Protocol)?),
            DataQuality::Failed,
        );
        value.scope = DataScope::User {
            subject: crate::backend::plan::id(&a.subject)?,
        };
        value.collected_at = Some(u64::try_from(evidence.collected_at).map_err(|_| Error::Clock)?);
        value.quality = match evidence.quality {
            w::OutputQuality::Complete
                if evidence.schema_valid
                    && evidence.budget_valid
                    && evidence.exit_code == Some(0)
                    && evidence.output_reference.is_none() =>
            {
                DataQuality::Complete
            }
            w::OutputQuality::Truncated => DataQuality::Truncated,
            w::OutputQuality::Partial => DataQuality::Partial,
            _ => DataQuality::Failed,
        };
        value.enumeration_complete = value.quality == DataQuality::Complete;
        if evidence.schema_valid && evidence.budget_valid && evidence.output_reference.is_none() {
            value.rows = match &evidence.output {
                Value::Array(rows) => rows.clone(),
                Value::Null => vec![],
                other => vec![other.clone()],
            };
        }
        let metadata = self.policy_metadata(a)?;
        self.snapshots(
            &[value],
            &[(
                format!("policy-read/{}", a.query.id.as_str()),
                encode(&metadata)?,
            )],
        )
    }
    pub fn periodic(
        &mut self,
        c: &w::TelemetryConfiguration,
        e: &ProcessEvidence,
        at: i64,
        normalized: Option<BTreeMap<w::FieldKey, w::TelemetryReading>>,
    ) -> Result<(), Error> {
        if let w::TelemetryCollector::Native { .. } = &c.collector {
            return self.capture_projected_read(
                ReadPlan::periodic(c, c.expires_at)?,
                None,
                e,
                u64::try_from(at / 1000).map_err(|_| Error::Clock)?,
                normalized.as_ref(),
            );
        }
        // Script/osquery periodic leases permit observation only. They never become a
        // manually executable script catalog or expose process diagnostics to readers.
        let revision = digest(c)?;
        let reference = QueryRef {
            id: crate::backend::plan::id(format!("telemetry:{}", revision.as_str()))?,
            revision,
        };
        let descriptor = QueryDescriptor {
            reference: reference.clone(),
            title: c.policy.to_string(),
            kind: QueryKind::Telemetry,
            origin: QueryOrigin::Enterprise,
            domains: Vec::new(),
            input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
            policy: None,
            scope: DataScope::Device,
            available: false,
            reason: Some(DataQuality::Denied),
        };
        self.store.write(
            &format!("observed/{}", reference.id.as_str()),
            &encode(&Observed {
                enterprise: {
                    let b = crate::backend::plan::enterprise_binding(
                        self.service_id,
                        c.tenant_id,
                        c.registration_id,
                        c.generation,
                        &c.device_id,
                    )?;
                    Some(digest(&(&b.authority, &b.device))?)
                },
                source: c.clone(),
                descriptor,
            })?,
        )?;
        let mut value = empty(reference.clone(), None, DataQuality::Failed);
        value.collected_at = Some(u64::try_from(at / 1_000_000).map_err(|_| Error::Clock)?);
        // Remote scripts cannot acquire a local field grant from their own metadata.
        value.quality = DataQuality::Denied;
        self.snapshots(
            &[value],
            &[(
                format!("latest/projected:{}", reference.id.as_str()),
                encode(&(c, normalized))?,
            )],
        )
    }
    pub fn remember(&mut self, request: &RequestId, read: ReadPlan) -> Result<(), Error> {
        read.validate()?;
        if !matches!(
            read.source,
            crate::collection_worker::ReadSource::Local { .. }
        ) {
            self.register_projected(read.clone())?;
        } else {
            for query in read.plan.spec().queries.values() {
                self.register(single(&read, query)?)?;
            }
        }
        if self.captured(request)? {
            return Ok(());
        }
        self.store
            .write(&format!("input/{}", request.as_str()), &encode(&read)?)
    }
    pub fn captured(&self, request: &RequestId) -> Result<bool, Error> {
        Ok(self
            .store
            .read(&format!("result/{}", request.as_str()))?
            .is_some())
    }
    pub fn remembered(&self, request: &RequestId) -> Result<Option<ReadPlan>, Error> {
        self.store
            .read(&format!("input/{}", request.as_str()))?
            .map(decode)
            .transpose()
    }
    pub fn publish(
        &mut self,
        item: w::SelfServiceDirectoryItem,
        locator: PolicyLocator,
        subject: &str,
        session: &Id,
        context: &NativeContextRef,
    ) -> Result<QueryDescriptor, Error> {
        let revision = digest(&(
            self.enterprise.as_ref().ok_or(Error::Identity)?,
            policy_revision(&item, subject)?,
        ))?;
        let reference = QueryRef {
            id: crate::backend::plan::id(format!("policy:{}", revision.as_str()))?,
            revision,
        };
        let title = item
            .self_service
            .presentation
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or("Published query")
            .chars()
            .take(256)
            .collect();
        let input_schema = query_input_schema(&item).map_err(|_| Error::Unsupported)?;
        let policy = Some(PublishedQuery {
            policy_id: crate::backend::plan::id(item.policy_id.to_string())?,
            policy_revision: item.policy_revision,
            policy_version: crate::backend::plan::id(item.policy_version.to_string())?,
            description: item
                .self_service
                .presentation
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(1024)
                .collect(),
        });
        let descriptor = QueryDescriptor {
            reference,
            title,
            kind: QueryKind::Policy,
            origin: QueryOrigin::Enterprise,
            domains: Vec::new(),
            input_schema,
            policy,
            scope: DataScope::User {
                subject: crate::backend::plan::id(subject)?,
            },
            available: true,
            reason: None,
        };
        self.store.write(
            &format!("published/{}", descriptor.reference.id.as_str()),
            &encode(&Published {
                enterprise: self.enterprise.clone(),
                locator,
                generated_uid: account_uid(subject)?,
                descriptor: descriptor.clone(),
                item,
                subject: subject.into(),
                session: session.clone(),
                context: context.clone(),
                seen_at: now(),
            })?,
        )?;
        Ok(descriptor)
    }

    pub fn published(
        &self,
        query: &QueryRef,
        subject: &str,
        session: &Id,
        context: &NativeContextRef,
    ) -> Result<Option<(w::SelfServiceDirectoryItem, PolicyLocator)>, Error> {
        let Some(value) = self
            .store
            .read(&format!("published/{}", query.id.as_str()))?
        else {
            return Ok(None);
        };
        let p: Published = decode(value)?;
        if p.enterprise != self.enterprise
            || self.enterprise.is_none()
            || p.subject != subject
            || &p.session != session
            || &p.context != context
        {
            return Err(Error::Denied);
        }
        if digest(&(
            self.enterprise.as_ref().ok_or(Error::Identity)?,
            policy_revision(&p.item, subject)?,
        ))? != query.revision
        {
            return Err(Error::Conflict);
        }
        Ok(Some((p.item, p.locator)))
    }
    pub fn authorize_read(&self, read: &DataRead, caller: &str, session: &Id) -> Result<(), Error> {
        let query = match &read.selection {
            DataSelection::Query { query } => query.clone(),
            DataSelection::Request { request } => {
                self.read_association(request, caller, session)?.query
            }
        };
        if !["policy:", "native:", "telemetry:"]
            .iter()
            .any(|p| query.id.as_str().starts_with(p))
        {
            return Err(Error::NotFound);
        }
        if query.id.as_str().starts_with("policy:") {
            self.policy_read(&query, caller)?;
        }
        if query.id.as_str().starts_with("telemetry:") {
            let observed: Observed = decode(
                self.store
                    .read(&format!("observed/{}", query.id.as_str()))?
                    .ok_or(Error::Unavailable)?,
            )?;
            if observed.enterprise != self.enterprise
                || self.enterprise.is_none()
                || observed.descriptor.domains.is_empty()
            {
                return Err(Error::Denied);
            }
            let d = observed.descriptor;
            if d.reference != query {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    }
    pub fn register(&mut self, mut read: ReadPlan) -> Result<Definition, Error> {
        if !matches!(
            read.source,
            crate::collection_worker::ReadSource::Local { .. }
        ) {
            return self.register_projected(read);
        }
        read.validate()?;
        if read.plan.spec().queries.len() != 1 {
            return Err(Error::Protocol);
        }
        let physical = read
            .plan
            .spec()
            .queries
            .values()
            .next()
            .cloned()
            .ok_or(Error::Protocol)?;
        read = single(&read, &physical)?;
        let query = read
            .plan
            .spec()
            .queries
            .values()
            .next()
            .ok_or(Error::Protocol)?;
        // Physical semantics and scope form the identity. A periodic budget, source
        // label, deadline or login incarnation must not split the same local facts.
        let cardinality = query.cardinality;
        let revision = digest(&(
            query.version,
            &query.definition,
            query.cardinality,
            &query.instance_keys,
            &read.plan.spec().scope,
            subject(&read),
            &read.execution.generated_uid,
            &read.source,
        ))?;
        let mut spec: w::NativePlanSpec = read.plan.clone().into();
        spec.budget.timeout_seconds = spec.budget.timeout_seconds.min(60);
        spec.budget.output_bytes = spec.budget.output_bytes.min(2 * 1024 * 1024);
        spec.budget.max_rows = spec.budget.max_rows.min(4096);
        for q in spec.queries.values_mut() {
            q.budget = spec.budget;
        }
        read.plan = w::NativeCollectionPlan::new(spec)?;
        let reference = QueryRef {
            id: crate::backend::plan::id(format!("native:{}", revision.as_str()))?,
            revision,
        };
        let supported = read
            .local_projection()
            .queries
            .values()
            .all(|q| q.domain.is_some());
        let definition = Definition {
            descriptor: QueryDescriptor {
                reference,
                title: title(
                    &read
                        .plan
                        .spec()
                        .queries
                        .values()
                        .next()
                        .ok_or(Error::Protocol)?
                        .definition,
                ),
                kind: QueryKind::Native,
                origin: QueryOrigin::Local,
                domains: read
                    .local_projection()
                    .queries
                    .values()
                    .next()
                    .ok_or(Error::Protocol)?
                    .domains(cardinality),
                input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
                policy: None,
                scope: match subject(&read) {
                    Some(s) => DataScope::User {
                        subject: crate::backend::plan::id(s)?,
                    },
                    None => DataScope::Device,
                },
                available: supported
                    && read.plan.supported_by(&crate::collection_worker::engines()),
                reason: (!supported).then_some(DataQuality::Unsupported),
            },
            read,
            seen_at: now(),
            enterprise: None,
        };
        self.store.write(
            &format!("definition/{}", definition.descriptor.reference.id.as_str()),
            &encode(&definition)?,
        )?;
        Ok(definition)
    }
    pub fn local_definition(&self, query: &QueryRef, caller: &str) -> Result<Definition, Error> {
        let mut definition = self.definition(query, caller)?;
        definition.read = self
            .local_reads
            .get(&query.id)
            .cloned()
            .ok_or(Error::Denied)?;
        Ok(definition)
    }
    pub fn definition(&self, query: &QueryRef, caller: &str) -> Result<Definition, Error> {
        let definition: Definition = decode(
            self.store
                .read(&format!("definition/{}", query.id.as_str()))?
                .ok_or(Error::Unavailable)?,
        )?;
        if definition.descriptor.reference != *query {
            return Err(Error::Conflict);
        }
        if !visible(&definition.read, caller)
            || (!definition.descriptor.domains.is_empty() && !read_granted(&definition.read))
            || (definition.enterprise.is_some() && definition.enterprise != self.enterprise)
        {
            return Err(Error::Denied);
        }
        Ok(definition)
    }
    pub fn directory(&self, caller: &str) -> Result<Vec<QueryDescriptor>, Error> {
        let mut result = Vec::new();
        let mut after = None;
        // The backing namespace itself is capped at 1024 records.
        loop {
            let page = self.store.page("definition/", after.as_deref(), 128)?;
            for (_, v) in &page {
                let mut d: Definition = decode(v.clone())?;
                if visible(&d.read, caller)
                    && (d.enterprise.is_none() || d.enterprise == self.enterprise)
                    && !d.descriptor.domains.is_empty()
                    && read_granted(&d.read)
                {
                    if !self.pinned.contains(&d.descriptor.reference.id) {
                        d.descriptor.available = false;
                        d.descriptor.reason = Some(DataQuality::Denied);
                    }
                    result.push(d.descriptor);
                }
            }
            if page.len() < 128 {
                break;
            }
            after = page.last().map(|(k, _)| k.clone());
        }
        for (_, v) in self.store.page("observed/", None, 128)? {
            let observed: Observed = decode(v)?;
            if observed.enterprise == self.enterprise
                && self.enterprise.is_some()
                && !observed.descriptor.domains.is_empty()
            {
                result.push(observed.descriptor);
            }
        }
        for (_, value) in self.scan("policy-read/")? {
            let meta: PolicyRead = decode(value)?;
            if self.policy_read(&meta.descriptor.reference, caller).is_ok() {
                let mut descriptor = meta.descriptor;
                descriptor.available = false;
                descriptor.reason = Some(DataQuality::Expired);
                result.push(descriptor);
            }
        }
        Ok(result)
    }
    pub fn history(&self) -> Result<Vec<(RequestId, u64)>, Error> {
        let mut result = Vec::new();
        let mut after = None;
        loop {
            let page = self.store.page("request/", after.as_deref(), 128)?;
            for (key, value) in &page {
                let a: Association = decode(value.clone())?;
                result.push((
                    RequestId::new(key.strip_prefix("request/").ok_or(Error::Protocol)?)
                        .map_err(|_| Error::Protocol)?,
                    a.created_at,
                ));
            }
            if page.len() < 128 {
                break;
            }
            after = page.last().map(|(key, _)| key.clone());
        }
        result.sort_by_key(|(_, at)| *at);
        Ok(result)
    }
    pub fn retire(&mut self, request: &RequestId) -> Result<(), Error> {
        for prefix in ["result", "input", "request"] {
            self.store
                .remove(&format!("{prefix}/{}", request.as_str()))?;
        }
        Ok(())
    }
    pub fn related_requests(
        &self,
        query: &QueryRef,
        subject: &str,
        session: &Id,
    ) -> Result<Vec<RequestId>, Error> {
        let mut result = Vec::new();
        let mut after = None;
        loop {
            let page = self.store.page("request/", after.as_deref(), 128)?;
            for (key, value) in &page {
                let a: Association = decode(value.clone())?;
                if &a.query == query && a.subject == subject && &a.session == session {
                    result.push(
                        RequestId::new(key.strip_prefix("request/").ok_or(Error::Protocol)?)
                            .map_err(|_| Error::Protocol)?,
                    );
                }
            }
            if page.len() < 128 {
                break;
            }
            after = page.last().map(|(key, _)| key.clone());
        }
        Ok(result)
    }
    pub fn associate(&mut self, request: &RequestId, value: &Association) -> Result<(), Error> {
        let key = format!("request/{}", request.as_str());
        if let Some(previous) = self.store.read(&key)? {
            let mut previous: Association = decode(previous)?;
            previous.created_at = value.created_at;
            if encode(&previous)? != encode(value)? {
                return Err(Error::Conflict);
            }
        } else {
            self.store.write(&key, &encode(value)?)?;
        }
        Ok(())
    }
    pub fn association(
        &self,
        request: &RequestId,
        caller: &str,
        session: &Id,
    ) -> Result<Association, Error> {
        let value: Association = decode(
            self.store
                .read(&format!("request/{}", request.as_str()))?
                .ok_or(Error::NotFound)?,
        )?;
        if value.subject != caller || &value.session != session {
            return Err(Error::Denied);
        }
        if value.kind == QueryKind::Native {
            self.definition(&value.query, caller)?;
        }
        Ok(value)
    }
    fn policy_read(&self, query: &QueryRef, caller: &str) -> Result<PolicyRead, Error> {
        let metadata: PolicyRead = decode(
            self.store
                .read(&format!("policy-read/{}", query.id.as_str()))?
                .ok_or(Error::Denied)?,
        )?;
        if metadata.enterprise != self.enterprise || self.enterprise.is_none() {
            return Err(Error::Denied);
        }
        if metadata.descriptor.reference != *query {
            return Err(Error::Conflict);
        }
        if metadata.descriptor.scope
            != (DataScope::User {
                subject: crate::backend::plan::id(caller)?,
            })
            || metadata.generated_uid != account_uid(caller)?
        {
            return Err(Error::Denied);
        }
        Ok(metadata)
    }
    fn read_association(
        &self,
        request: &RequestId,
        caller: &str,
        session: &Id,
    ) -> Result<Association, Error> {
        let association: Association = decode(
            self.store
                .read(&format!("request/{}", request.as_str()))?
                .ok_or(Error::NotFound)?,
        )?;
        if association.kind == QueryKind::Policy {
            self.policy_read(&association.query, caller)?;
            Ok(association)
        } else {
            self.association(request, caller, session)
        }
    }
    pub fn native(
        &mut self,
        read: ReadPlan,
        request: Option<RequestId>,
        evidence: &ProcessEvidence,
        at: u64,
    ) -> Result<(), Error> {
        read.validate()?;
        if !matches!(
            read.source,
            crate::collection_worker::ReadSource::Local { .. }
        ) {
            return self.capture_projected(read, request, evidence, at);
        }
        let now = i64::try_from(at / 1000).map_err(|_| Error::Clock)?;
        let result = if evidence.finished
            && evidence.quiescent
            && evidence.exit_code == Some(0)
            && evidence.quality == OutputQuality::Complete
            && evidence.total_output_bytes <= u64::from(read.plan.spec().budget.output_bytes)
        {
            let result: w::NativeCollectionResult =
                serde_json::from_slice(&evidence.stdout).map_err(|_| Error::Protocol)?;
            let completion: CollectionCompletion =
                serde_json::from_slice(&evidence.stderr).map_err(|_| Error::Protocol)?;
            if !completion.closed
                || completion.output_sha256 != format!("{:x}", Sha256::digest(&evidence.stdout))
            {
                return Err(Error::Untrusted);
            }
            result.validate_for(&read.plan, now)?;
            Some(result)
        } else {
            None
        };
        let mut values = Vec::new();
        let mut material = Vec::new();
        for (source, q) in &read.plan.spec().queries {
            let definition = self.register(single(&read, q)?)?;
            let baseline =
                if let (Some(result), crate::collection_worker::ReadSource::Local { projection }) =
                    (&result, &read.source)
                {
                    projection
                        .queries
                        .get(source)
                        .ok_or(Error::Protocol)?
                        .baseline_fields(&read.plan, result, now)?
                } else {
                    BTreeMap::new()
                };
            material.push((
                format!(
                    "latest/projected:{}",
                    definition.descriptor.reference.id.as_str()
                ),
                encode(&baseline)?,
            ));
            let mut value = empty(
                definition.descriptor.reference,
                request.clone(),
                DataQuality::Failed,
            );
            value.scope = definition.descriptor.scope;
            value.collected_at = Some(at / 1000);
            if let Some(result) = result.as_ref().and_then(|r| r.queries.get(source)) {
                value.collected_at =
                    Some(u64::try_from(result.collected_at).map_err(|_| Error::Clock)?);
                value.enumeration_complete = result.complete_for(q, now);
                value.quality = if value.enumeration_complete {
                    DataQuality::Complete
                } else {
                    match result.status {
                        w::NativeQueryStatus::Truncated => DataQuality::Truncated,
                        w::NativeQueryStatus::Denied => DataQuality::Denied,
                        w::NativeQueryStatus::Missing => DataQuality::Missing,
                        w::NativeQueryStatus::Unsupported => DataQuality::Unsupported,
                        w::NativeQueryStatus::Failed
                        | w::NativeQueryStatus::Invalid
                        | w::NativeQueryStatus::Timeout
                        | w::NativeQueryStatus::Complete => DataQuality::Failed,
                        _ => DataQuality::Partial,
                    }
                };
                if let Some(kind) = read
                    .local_projection()
                    .queries
                    .get(source)
                    .ok_or(Error::Protocol)?
                    .domain
                    .as_deref()
                {
                    if self.safe(&encode(&result.rows)?)? {
                        if value.enumeration_complete
                            && result.rows.len() == 1
                            && q.cardinality == w::NativeCardinality::ExactlyOne
                        {
                            let crate::collection_worker::ReadSource::Local { projection } =
                                &read.source
                            else {
                                return Err(Error::Protocol);
                            };
                            value.fields = projection
                                .queries
                                .get(source)
                                .ok_or(Error::Protocol)?
                                .fields(&result.rows[0])?;
                        }
                        let schema = q.definition.properties();
                        let crate::collection_worker::ReadSource::Local { projection } =
                            &read.source
                        else {
                            return Err(Error::Protocol);
                        };
                        let mapping = projection.queries.get(source).ok_or(Error::Protocol)?;
                        value.rows = result.rows.iter().filter(|row| row.keys().eq(schema.keys()) && schema.iter().all(|(key,kind)| row.get(key).is_some_and(|value| kind.accepts(value))) && q.definition.accepts_row(row))
                            .map(|row| Ok(json!({"source":source,"domain":kind,"definition":q.definition,"values":mapping.row(row)?}))).collect::<Result<Vec<_>, Error>>()?;
                        if value.rows.len() != result.rows.len() {
                            value.enumeration_complete = false;
                            value.quality = if value.rows.is_empty() {
                                DataQuality::Failed
                            } else {
                                DataQuality::Partial
                            };
                        }
                    } else {
                        value.quality = DataQuality::Denied;
                        value.enumeration_complete = false;
                    }
                } else {
                    value.quality = DataQuality::Unsupported;
                    value.enumeration_complete = false;
                }
            } else if evidence.quality == OutputQuality::Truncated {
                value.quality = DataQuality::Truncated;
            }
            for (key, observed) in baseline {
                if observed.quality == DataQuality::Complete {
                    if let Some(projected) = observed.value {
                        if !projected.is_array() {
                            value.fields.entry(key).or_insert(projected);
                        }
                    }
                }
            }
            values.push(value);
        }
        self.snapshots(&values, &material)
    }
    #[cfg(test)]
    pub fn snapshot(&mut self, value: &CollectedData) -> Result<(), Error> {
        self.snapshots(std::slice::from_ref(value), &[])
    }
    fn snapshots(
        &mut self,
        values: &[CollectedData],
        extra: &[(String, Value)],
    ) -> Result<(), Error> {
        let mut writes = std::collections::BTreeMap::new();
        let mut remove = Vec::new();
        for value in values {
            let value = self.bounded(value)?;
            let key = format!("latest/{}", value.query.id.as_str());
            let previous: Option<CollectedData> = self.store.read(&key)?.map(decode).transpose()?;
            if previous.is_none_or(|p| p.collected_at <= value.collected_at) {
                writes.insert(key, encode(&value)?);
            }
            if let Some(request) = &value.request {
                // The request result is a completion receipt only after the entire batch commits.
                writes.insert(format!("result/{}", request.as_str()), encode(&value)?);
                remove.push(format!("input/{}", request.as_str()));
            }
        }
        for (key, material) in extra {
            if let Some(id) = key.strip_prefix("latest/projected:") {
                if values.iter().any(|v| v.query.id.as_str() == id)
                    && !writes.contains_key(&format!("latest/{id}"))
                {
                    continue;
                }
            }
            writes.insert(key.clone(), material.clone());
        }
        self.store
            .commit(&writes.into_iter().collect::<Vec<_>>(), &remove)
    }
    fn bounded(&self, value: &CollectedData) -> Result<CollectedData, Error> {
        let value_rows = value.rows.len();
        let mut value = value.clone();
        let mut remaining = 256 * 1024usize;
        if serde_json::to_vec(&value.fields)
            .map_err(|_| Error::Protocol)?
            .len()
            > 16 * 1024
        {
            value.fields.clear();
            value.quality = DataQuality::Truncated;
            value.enumeration_complete = false;
        }
        value.rows.retain(|row| {
            let bytes = serde_json::to_vec(row)
                .map(|v| v.len())
                .unwrap_or(usize::MAX);
            if bytes <= remaining {
                remaining -= bytes;
                true
            } else {
                false
            }
        });
        if value.rows.len() != value_rows {
            value.quality = DataQuality::Truncated;
            value.enumeration_complete = false;
        }
        if !self.safe(&encode(&value.rows)?)? || !self.safe(&encode(&value.fields)?)? {
            return Err(Error::Untrusted);
        }
        Ok(value)
    }
    fn safe(&self, value: &Value) -> Result<bool, Error> {
        let text = serde_json::to_string(value).map_err(|_| Error::Protocol)?;
        Ok(self.redactor.redact(&text)? == text)
    }
    pub fn read(
        &self,
        input: &DataRead,
        caller: &str,
        session: &Id,
        now: u64,
    ) -> Result<CollectedData, Error> {
        self.read_page(input, caller, session, now)
            .map(|(value, _)| value)
    }
    pub(crate) fn read_page(
        &self,
        input: &DataRead,
        caller: &str,
        session: &Id,
        now: u64,
    ) -> Result<(CollectedData, Digest), Error> {
        if !(1..=128).contains(&input.limit) {
            return Err(Error::Capacity);
        }
        let (query, key) = match &input.selection {
            DataSelection::Query { query } => {
                if query.id.as_str().starts_with("native:") {
                    self.definition(query, caller)?;
                } else if query.id.as_str().starts_with("policy:") {
                    self.policy_read(query, caller)?;
                }
                (query.clone(), format!("latest/{}", query.id.as_str()))
            }
            DataSelection::Request { request } => {
                let a = self.read_association(request, caller, session)?;
                (a.query, format!("result/{}", request.as_str()))
            }
        };
        let mut value: CollectedData = self
            .store
            .read(&key)?
            .map(decode)
            .transpose()?
            .unwrap_or_else(|| empty(query, None, DataQuality::NotCollected));
        if !self.safe(&encode(&value.rows)?)? || !self.safe(&encode(&value.fields)?)? {
            return Err(Error::Denied);
        }
        let snapshot = digest(&value)?;
        let offset = if let Some(cursor) = &input.after {
            if cursor.snapshot != snapshot {
                return Err(Error::Conflict);
            }
            cursor.offset as usize
        } else {
            0
        };
        if offset > value.rows.len() {
            return Err(Error::Protocol);
        }
        let all = std::mem::take(&mut value.rows);
        if serde_json::to_vec(&value)
            .map_err(|_| Error::Protocol)?
            .len()
            > 48 * 1024
        {
            return Err(Error::Capacity);
        }
        // Bound actual JSON bytes as well as row count, including the MCP/IPC wrappers.
        for row in all.iter().skip(offset).take(input.limit as usize) {
            value.rows.push(row.clone());
            if serde_json::to_vec(&value)
                .map_err(|_| Error::Protocol)?
                .len()
                > 48 * 1024
            {
                value.rows.pop();
                break;
            }
        }
        if offset < all.len() && value.rows.is_empty() {
            return Err(Error::Capacity);
        }
        let end = offset + value.rows.len();
        value.next = (end < all.len()).then_some(DataCursor {
            snapshot: snapshot.clone(),
            offset: end as u32,
        });
        value.stale = input.max_age_seconds.is_some_and(|age| {
            value
                .collected_at
                .is_none_or(|at| now.saturating_sub(at) > u64::from(age))
        });
        Ok((value, snapshot))
    }
}
fn empty(query: QueryRef, request: Option<RequestId>, quality: DataQuality) -> CollectedData {
    CollectedData {
        query,
        scope: DataScope::Device,
        request,
        collected_at: None,
        quality,
        enumeration_complete: false,
        fields: BTreeMap::new(),
        rows: vec![],
        stale: false,
        next: None,
    }
}

#[cfg(test)]
mod tests;

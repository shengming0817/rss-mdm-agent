//! Frozen local views plus shared enterprise projections in the existing collected-data owner.
use super::*;
use crate::collection_projection::{self as projection, FieldResult};
use crate::collection_worker::ReadSource;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SharedProjection {
    id: String,
    version: String,
    digest: Digest,
    fields: Vec<w::FieldDefinition>,
    mappings: BTreeMap<String, w::AgentCollectionMapping>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LocalQueryProjection {
    pub domain: Option<String>,
    fields: BTreeMap<String, w::ValueMapping>,
    types: BTreeMap<String, w::ValueType>,
    rows: w::ValueMapping,
    row_type: w::ValueType,
    baselines: Vec<SharedProjection>,
}

/// Product-owned local field names and additions, frozen with the physical read.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LocalProjection {
    pub queries: BTreeMap<String, LocalQueryProjection>,
}

fn local_type(value: &w::NativeType) -> w::ValueType {
    match value {
        w::NativeType::String | w::NativeType::UInt64 | w::NativeType::CimTime => {
            w::ValueType::String {
                max_length: 65536,
                allow_empty: true,
            }
        }
        w::NativeType::Integer => w::ValueType::Integer,
        w::NativeType::Number => w::ValueType::Number,
        w::NativeType::Boolean => w::ValueType::Boolean,
        w::NativeType::Array { items } => w::ValueType::Array {
            items: Box::new(local_type(items)),
            max_items: 65535,
        },
    }
}

impl LocalProjection {
    pub(crate) fn freeze(plan: &w::NativeCollectionPlan) -> Result<Self, Error> {
        let packaged = rss_mdm_agent_wire::packaged_collections::definitions()?;
        let mut queries = BTreeMap::new();
        for (source, query) in &plan.spec().queries {
            let mut fields = BTreeMap::new();
            let mut types = BTreeMap::new();
            let mut columns = BTreeMap::new();
            let mut row_types = BTreeMap::new();
            for (key, ty) in query.definition.properties() {
                if let Some(name) = mapped_field(&query.definition, &key) {
                    fields.insert(name.clone(), w::ValueMapping::identity(format!("/{key}")));
                    types.insert(name, local_type(&ty));
                }
                let name = if matches!(&query.definition, w::NativeQueryDefinition::WindowsWmi { class, .. } if class == "Win32_DiskDrive")
                {
                    match key.as_str() {
                        "DeviceID" => "id",
                        "Model" => "model",
                        "Size" => "size_bytes",
                        _ => continue,
                    }
                    .to_owned()
                } else {
                    key.clone()
                };
                columns.insert(name.clone(), w::ValueMapping::identity(format!("/{key}")));
                row_types.insert(name, local_type(&ty));
            }
            let mut baselines = Vec::new();
            if domain(&query.definition).is_some() {
                for definition in &packaged {
                    for (original, candidate) in &definition.plan.spec().queries {
                        if candidate.definition != query.definition
                            || candidate.cardinality != query.cardinality
                        {
                            continue;
                        }
                        let mut mappings = BTreeMap::new();
                        let selected = definition
                            .fields
                            .iter()
                            .filter(|f| {
                                definition
                                    .mappings
                                    .get(f.key.as_str())
                                    .is_some_and(|m| &m.query == original)
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        for field in &selected {
                            let mut binding = definition.mappings[field.key.as_str()].clone();
                            binding.query = source.clone();
                            mappings.insert(field.key.as_str().to_owned(), binding);
                        }
                        if !selected.is_empty() {
                            baselines.push(SharedProjection {
                                id: definition.id.clone(),
                                version: definition.version.clone(),
                                digest: digest(definition)?,
                                fields: selected,
                                mappings,
                            });
                        }
                    }
                }
            }
            queries.insert(
                source.clone(),
                LocalQueryProjection {
                    domain: domain(&query.definition).map(str::to_owned),
                    fields,
                    types,
                    rows: w::ValueMapping {
                        pointer: String::new(),
                        conversion: w::Conversion::Identity,
                        columns,
                    },
                    row_type: w::ValueType::Object {
                        properties: row_types,
                    },
                    baselines,
                },
            );
        }
        Ok(Self { queries })
    }
    pub(crate) fn validate(&self, plan: &w::NativeCollectionPlan) -> Result<(), Error> {
        if !self.queries.keys().eq(plan.spec().queries.keys()) {
            return Err(Error::Untrusted);
        }
        for query in self.queries.values() {
            if !query.fields.keys().eq(query.types.keys()) {
                return Err(Error::Protocol);
            }
            for (key, mapping) in &query.fields {
                mapping.validate_type(&query.types[key])?;
            }
            query.rows.validate_type(&query.row_type)?;
            for baseline in &query.baselines {
                if baseline.fields.len() != baseline.mappings.len() {
                    return Err(Error::Protocol);
                }
                for field in &baseline.fields {
                    baseline
                        .mappings
                        .get(field.key.as_str())
                        .ok_or(Error::Protocol)?
                        .validate_field(plan, field, false)?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn single(&self, source: &str) -> Result<Self, Error> {
        let mut query = self.queries.get(source).cloned().ok_or(Error::Protocol)?;
        for baseline in &mut query.baselines {
            for binding in baseline.mappings.values_mut() {
                binding.query = "read".into();
            }
        }
        Ok(Self {
            queries: [("read".into(), query)].into(),
        })
    }
}

impl LocalQueryProjection {
    pub(super) fn domains(&self, cardinality: w::NativeCardinality) -> Vec<String> {
        if cardinality == w::NativeCardinality::List {
            return self.domain.iter().cloned().collect();
        }
        self.fields
            .keys()
            .filter_map(|k| k.rsplit_once('.').map(|(d, _)| d.to_owned()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub(super) fn fields(
        &self,
        row: &BTreeMap<String, Value>,
    ) -> Result<BTreeMap<String, Value>, Error> {
        let input = encode(row)?;
        self.fields
            .iter()
            .map(|(key, mapping)| {
                Ok((
                    key.clone(),
                    projection::project(
                        &input,
                        mapping,
                        self.types.get(key).ok_or(Error::Protocol)?,
                    )?,
                ))
            })
            .collect()
    }
    pub(super) fn row(&self, row: &BTreeMap<String, Value>) -> Result<Value, Error> {
        projection::project(&encode(row)?, &self.rows, &self.row_type)
    }
    pub(super) fn baseline_fields(
        &self,
        plan: &w::NativeCollectionPlan,
        result: &w::NativeCollectionResult,
        now: i64,
    ) -> Result<BTreeMap<String, FieldResult>, Error> {
        let mut values = BTreeMap::new();
        for baseline in &self.baselines {
            for (key, value) in
                projection::native(&baseline.fields, &baseline.mappings, plan, result, now)?
            {
                if let Some(previous) = values.insert(key, value.clone()) {
                    if encode(&previous)? != encode(&value)? {
                        return Err(Error::Conflict);
                    }
                }
            }
        }
        Ok(values)
    }
}

impl Data {
    fn projection_enterprise(&self, read: &ReadPlan) -> Result<Option<Digest>, Error> {
        let binding = match &read.source {
            ReadSource::Task { task, .. } => crate::backend::plan::enterprise_binding(
                self.service_id,
                task.tenant_id,
                task.registration_id,
                task.generation,
                &task.device_id,
            )?,
            ReadSource::Periodic {
                configuration: c, ..
            } => crate::backend::plan::enterprise_binding(
                self.service_id,
                c.tenant_id,
                c.registration_id,
                c.generation,
                &c.device_id,
            )?,
            _ => return Ok(None),
        };
        Ok(Some(digest(&(&binding.authority, &binding.device))?))
    }
    fn projection_material(read: &ReadPlan) -> Result<Value, Error> {
        match &read.source {
            ReadSource::Task { task: t, .. } => encode(&(
                &t.resource,
                t.resource_digest,
                &t.fields,
                &t.mappings,
                &t.authorization,
            )),
            ReadSource::Periodic {
                configuration: c, ..
            } => encode(&(
                &c.template,
                c.resource_digest,
                c.policy,
                c.policy_version,
                c.source_epoch,
                &c.collector,
                &c.fields,
                &c.binding,
            )),
            ReadSource::Local { .. } => Err(Error::Protocol),
        }
    }
    pub(super) fn register_projected(&mut self, read: ReadPlan) -> Result<Definition, Error> {
        read.validate()?;
        let enterprise = self.projection_enterprise(&read)?;
        let revision = digest(&(
            &read.plan,
            &read.execution,
            &enterprise,
            Self::projection_material(&read)?,
            read.local_projection(),
        ))?;
        let reference = QueryRef {
            id: crate::backend::plan::id(format!("native:{}", revision.as_str()))?,
            revision,
        };
        let domains = read
            .plan
            .spec()
            .queries
            .iter()
            .flat_map(|(source, q)| read.local_projection().queries[source].domains(q.cardinality))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let definition = Definition {
            descriptor: QueryDescriptor {
                reference,
                title: "Enterprise collection".into(),
                kind: QueryKind::Native,
                origin: QueryOrigin::Enterprise,
                domains,
                input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
                policy: None,
                scope: match subject(&read) {
                    Some(owner) => DataScope::User {
                        subject: crate::backend::plan::id(owner)?,
                    },
                    None => DataScope::Device,
                },
                available: false,
                reason: Some(DataQuality::Denied),
            },
            read,
            seen_at: now(),
            enterprise,
        };
        self.store.write(
            &format!("definition/{}", definition.descriptor.reference.id.as_str()),
            &encode(&definition)?,
        )?;
        Ok(definition)
    }
    fn projection_fields<'a>(
        read: &'a ReadPlan,
    ) -> Result<
        (
            &'a [w::FieldDefinition],
            &'a BTreeMap<String, w::AgentCollectionMapping>,
        ),
        Error,
    > {
        match &read.source {
            ReadSource::Task { task, .. } => Ok((&task.fields, &task.mappings)),
            ReadSource::Periodic { configuration, .. } => match &configuration.collector {
                w::TelemetryCollector::Native { mappings, .. } => {
                    Ok((&configuration.fields, mappings))
                }
                _ => Err(Error::Protocol),
            },
            ReadSource::Local { .. } => Err(Error::Protocol),
        }
    }
    fn admitted_fields(read: &ReadPlan) -> Result<std::collections::BTreeSet<String>, Error> {
        let (definitions, mappings) = Self::projection_fields(read)?;
        let local = read.local_projection();
        Ok(local
            .queries
            .values()
            .flat_map(|q| q.baselines.iter())
            .flat_map(|b| b.fields.iter().map(move |f| (f, &b.mappings)))
            .filter_map(|(f, m)| {
                let key = f.key.as_str();
                (definitions.iter().any(|d| d == f) && mappings.get(key) == m.get(key))
                    .then_some(key.to_owned())
            })
            .collect())
    }
    pub(super) fn capture_projected(
        &mut self,
        read: ReadPlan,
        request: Option<RequestId>,
        evidence: &ProcessEvidence,
        at: u64,
    ) -> Result<(), Error> {
        self.capture_projected_read(read, request, evidence, at, None)
    }
    pub(super) fn capture_projected_read(
        &mut self,
        read: ReadPlan,
        request: Option<RequestId>,
        evidence: &ProcessEvidence,
        at: u64,
        normalized: Option<&BTreeMap<w::FieldKey, w::TelemetryReading>>,
    ) -> Result<(), Error> {
        let definition = self.register_projected(read.clone())?;
        let reference = definition.descriptor.reference;
        let mut value = empty(reference.clone(), request, DataQuality::Unknown);
        value.scope = definition.descriptor.scope;
        value.collected_at = Some(at / 1000);
        let mut fields = BTreeMap::new();
        if evidence.finished
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
            let now = i64::try_from(at / 1000).map_err(|_| Error::Clock)?;
            result.validate_for(&read.plan, now)?;
            let (definitions, mappings) = Self::projection_fields(&read)?;
            if let Some(normalized) = normalized {
                for field in definitions {
                    let reading = normalized.get(&field.key).ok_or(Error::Protocol)?;
                    let (value, quality) = match reading {
                        w::TelemetryReading::Valid { value, .. } => {
                            (Some(projection::json_scalar(value)?), DataQuality::Complete)
                        }
                        w::TelemetryReading::Missing { .. } => (None, DataQuality::Missing),
                        w::TelemetryReading::Failed { .. } => (None, DataQuality::Failed),
                    };
                    fields.insert(
                        field.key.as_str().to_owned(),
                        FieldResult {
                            value,
                            quality,
                            collected_at: now,
                            deleted: false,
                        },
                    );
                }
            } else if matches!(read.source, ReadSource::Periodic { .. }) {
                fields = definitions
                    .iter()
                    .map(|f| {
                        (
                            f.key.as_str().to_owned(),
                            FieldResult {
                                value: None,
                                quality: DataQuality::Failed,
                                collected_at: now,
                                deleted: false,
                            },
                        )
                    })
                    .collect();
            } else {
                fields = projection::native(definitions, mappings, &read.plan, &result, now)?;
            }
            // Enterprise metadata is not a local grant. Only a compiled baseline with the
            // same source, exact mapping and field definition can expose an enterprise field.
            let admitted = Self::admitted_fields(&read)?;
            let visible = fields
                .iter()
                .filter(|(key, _)| admitted.contains(*key))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>();
            let mut qualities = visible
                .values()
                .map(|v| v.quality.clone())
                .collect::<Vec<_>>();
            // Local aliases are product-owned views of an already authorized physical
            // observation. Their frozen declarations never come from the enterprise fields.
            for (source, query) in &read.plan.spec().queries {
                let view = read
                    .local_projection()
                    .queries
                    .get(source)
                    .ok_or(Error::Protocol)?;
                let Some(domain) = view.domain.as_deref() else {
                    continue;
                };
                let Some(observation) = result.queries.get(source) else {
                    qualities.push(DataQuality::Unknown);
                    continue;
                };
                let complete = observation.complete_for(query, now);
                let quality = if observation.status == w::NativeQueryStatus::Complete && !complete {
                    DataQuality::Failed
                } else {
                    projection::status(observation.status)
                };
                qualities.push(quality);
                let schema = query.definition.properties();
                for row in &observation.rows {
                    if !row.keys().eq(schema.keys())
                        || !schema
                            .iter()
                            .all(|(k, t)| row.get(k).is_some_and(|v| t.accepts(v)))
                        || !query.definition.accepts_row(row)
                    {
                        return Err(Error::Protocol);
                    }
                    if complete && query.cardinality == w::NativeCardinality::ExactlyOne {
                        for (key, v) in view.fields(row)? {
                            if value
                                .fields
                                .get(&key)
                                .is_some_and(|previous| previous != &v)
                            {
                                return Err(Error::Conflict);
                            }
                            value.fields.insert(key, v);
                        }
                    }
                    value.rows.push(json!({"source":source,"domain":domain,"definition":query.definition,"values":view.row(row)?}));
                }
            }
            value.quality = match qualities.first() {
                None => DataQuality::Denied,
                Some(first) if qualities.iter().all(|v| v == first) => first.clone(),
                _ => DataQuality::Partial,
            };
            value.enumeration_complete = value.quality == DataQuality::Complete;
            for (key, observed) in &visible {
                if let Some(projected) = &observed.value {
                    if let Some(rows) = projected.as_array() {
                        value
                            .rows
                            .extend(rows.iter().map(|row| json!({"field":key,"value":row})));
                    } else {
                        value.fields.insert(key.clone(), projected.clone());
                    }
                }
            }
        } else if evidence.quality == OutputQuality::Truncated {
            value.quality = DataQuality::Truncated;
        } else if evidence.finished && evidence.quiescent {
            value.quality = DataQuality::Failed;
        }
        let retained = format!("latest/projected:{}", reference.id.as_str());
        self.snapshots(&[value], &[(retained, encode(&fields)?)])
    }
    pub(crate) fn capture_script(
        &mut self,
        task: &w::TaskSpec,
        request: &RequestId,
        evidence: &ProcessEvidence,
        at: u64,
    ) -> Result<(), Error> {
        task.validate()?;
        let w::ScriptProjection::Collection {
            fields,
            mappings,
            output_schema,
            ..
        } = &task.projection
        else {
            return Err(Error::Protocol);
        };
        let binding = crate::backend::plan::enterprise_binding(
            self.service_id,
            task.tenant_id,
            task.registration_id,
            task.generation,
            &task.device_id,
        )?;
        let enterprise = Some(digest(&(&binding.authority, &binding.device))?);
        let association: Option<Association> = self
            .store
            .read(&format!("request/{}", request.as_str()))?
            .map(decode)
            .transpose()?;
        let metadata = association
            .as_ref()
            .map(|a| self.policy_metadata(a))
            .transpose()?;
        if let Some(a) = &association {
            let published: Published = decode(
                self.store
                    .read(&format!("published/{}", a.query.id.as_str()))?
                    .ok_or(Error::Denied)?,
            )?;
            let matches = match (&task.authorization, &published.item.task) {
                (
                    w::TaskAuthorization::SelfService {
                        request_id,
                        subject,
                        policy_id,
                        policy_revision,
                        policy_version,
                        ..
                    },
                    w::SelfServiceTask::Script {
                        resource, run_as, ..
                    },
                ) => {
                    request_id.to_string() == request.as_str()
                        && subject == &published.item.subject
                        && policy_id == &published.item.policy_id
                        && policy_revision == &published.item.policy_revision
                        && policy_version == &published.item.policy_version
                        && resource == &task.resource
                        && run_as == &task.run_as
                }
                _ => false,
            };
            if !matches {
                return Err(Error::Denied);
            }
        }
        if metadata
            .as_ref()
            .is_some_and(|m| m.enterprise != enterprise)
        {
            return Err(Error::Denied);
        }
        let reference = metadata
            .as_ref()
            .map(|m| m.descriptor.reference.clone())
            .unwrap_or(QueryRef {
                id: crate::backend::plan::id(format!("telemetry:{}", digest(task)?.as_str()))?,
                revision: digest(task)?,
            });
        let mut value = empty(
            reference.clone(),
            Some(request.clone()),
            DataQuality::Unknown,
        );
        if let Some(m) = &metadata {
            value.scope = m.descriptor.scope.clone();
        }
        value.collected_at = Some(at / 1000);
        let mut projected = BTreeMap::new();
        if evidence.finished
            && evidence.quiescent
            && evidence.exit_code == Some(0)
            && evidence.quality == OutputQuality::Complete
            && evidence.total_output_bytes <= u64::from(task.output_bytes)
        {
            let output: Value =
                serde_json::from_slice(&evidence.stdout).map_err(|_| Error::Protocol)?;
            crate::telemetry::mapping::validate_script_output(
                output_schema,
                &output,
                evidence.stdout.len(),
                task.max_rows,
            )?;
            for field in fields {
                let result = projection::project(
                    &output,
                    mappings.get(field.key.as_str()).ok_or(Error::Protocol)?,
                    &field.value_type,
                )
                .and_then(|v| projection::field(field, v));
                projected.insert(
                    field.key.as_str().to_owned(),
                    FieldResult {
                        value: result.as_ref().ok().cloned(),
                        quality: if result.is_ok() {
                            DataQuality::Complete
                        } else {
                            DataQuality::Failed
                        },
                        collected_at: i64::try_from(at / 1000).map_err(|_| Error::Clock)?,
                        deleted: false,
                    },
                );
            }
            // Only a previously authenticated self-service request grants this result access.
            value.quality = if metadata.is_some() {
                projection::quality(&projected)
            } else {
                DataQuality::Denied
            };
            value.enumeration_complete = value.quality == DataQuality::Complete;
            if metadata.is_some() {
                for (key, field) in &projected {
                    if let Some(v) = &field.value {
                        if let Some(rows) = v.as_array() {
                            value
                                .rows
                                .extend(rows.iter().map(|v| json!({"field":key,"value":v})));
                        } else {
                            value.fields.insert(key.clone(), v.clone());
                        }
                    }
                }
            }
        } else if evidence.quality == OutputQuality::Truncated {
            value.quality = DataQuality::Truncated;
        } else if evidence.finished && evidence.quiescent {
            value.quality = DataQuality::Failed;
        }
        let mut extra = vec![(
            format!("latest/projected:{}", reference.id.as_str()),
            encode(&(task, projected))?,
        )];
        if let Some(metadata) = metadata {
            extra.push((
                format!("policy-read/{}", reference.id.as_str()),
                encode(&metadata)?,
            ));
        }
        self.snapshots(&[value], &extra)
    }
}

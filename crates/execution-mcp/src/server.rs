use crate::{
    catalog_error::CatalogErrorView,
    model::{CatalogInput, Empty, ErrorView, PreviewInput, ProposeInput, ToolOutput},
    transport::OriginalArguments,
    *,
};
use execution_contract::Id;
use rmcp::{model::*, service::RequestContext, ErrorData, RoleServer, ServerHandler};
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use service_catalog::{CatalogError, CatalogItem, CatalogRef};
use std::{borrow::Cow, sync::Arc};
use tokio_util::sync::CancellationToken;

pub(crate) struct Handler<S> {
    pub service: Arc<S>,
    pub limits: McpLimits,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CatalogView {
    catalog: CatalogRef,
    items: Vec<ItemView>,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ItemView {
    item: CatalogItem,
    parameters: Vec<ParameterView>,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ParameterView {
    variant_id: Id,
    input_schema: Value,
}

pub(crate) struct Failure {
    code: ServiceError,
    catalog: Option<CatalogErrorView>,
}
impl From<ServiceError> for Failure {
    fn from(code: ServiceError) -> Self {
        Self {
            code,
            catalog: None,
        }
    }
}
impl From<CatalogError> for Failure {
    fn from(error: CatalogError) -> Self {
        Self {
            code: ServiceError::InvalidInput,
            catalog: Some(error.into()),
        }
    }
}
fn decode<T: DeserializeOwned>(raw: &str) -> Result<T, Failure> {
    serde_json::from_str(raw).map_err(|_| ServiceError::InvalidInput.into())
}
fn schema<T: JsonSchema>() -> Arc<serde_json::Map<String, Value>> {
    let value = schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value();
    let mut object = value.as_object().expect("object schema").clone();
    // Every tool DTO is object-shaped, including tagged enum branches. Schemars
    // omits the root type for those unions; MCP 2025-11-25 requires it explicitly.
    object.insert("type".into(), Value::String("object".into()));
    Arc::new(object)
}
// Tool identity and effect metadata have one typed owner. Exhaustive matches
// bind schemas and dispatch to that owner without repeating wire-name lists.
#[derive(Clone, Copy)]
pub(crate) enum ToolKind {
    Catalog,
    Capabilities,
    Preview,
    Propose,
    Submit,
    Status,
    Cancel,
}
#[derive(Clone, Copy)]
enum Effect {
    ReadOnly,
    MayPersist,
}
impl ToolKind {
    const ALL: [Self; 7] = [
        Self::Catalog,
        Self::Capabilities,
        Self::Preview,
        Self::Propose,
        Self::Submit,
        Self::Status,
        Self::Cancel,
    ];

    fn descriptor(self) -> (&'static str, &'static str, Effect) {
        match self {
            Self::Catalog => ("execution_catalog", "Authorized directory and shared parameter schemas. Visibility does not authorize execution.", Effect::ReadOnly),
            Self::Capabilities => ("execution_capabilities", "Current bound-context capabilities. Unknown is not supported.", Effect::ReadOnly),
            Self::Preview => ("execution_preview", "Freeze an exact plan without approval or execution.", Effect::MayPersist),
            Self::Propose => ("execution_propose", "Record a bounded immutable candidate; never approve or execute it.", Effect::MayPersist),
            Self::Submit => ("execution_submit", "Accept an exact plan idempotently. On timeout query or retry the SAME operationRequestId. Accepted is not success.", Effect::MayPersist),
            Self::Status => ("execution_status", "Authorized lookup by the original operationRequestId.", Effect::ReadOnly),
            Self::Cancel => ("execution_cancel", "Request business cancellation; receipt does not prove termination or rollback.", Effect::MayPersist),
        }
    }
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.descriptor().0 == name)
    }
    pub(crate) fn timeout_error(self) -> ServiceError {
        match self.descriptor().2 {
            Effect::ReadOnly => ServiceError::Unavailable,
            Effect::MayPersist => ServiceError::OutcomeUnknown,
        }
    }
    fn definition(self) -> Tool {
        match self {
            Self::Catalog => tool::<Empty, CatalogView>(self),
            Self::Capabilities => tool::<Empty, CapabilityView>(self),
            Self::Preview => tool::<PreviewInput, PlanPreview>(self),
            Self::Propose => tool::<ProposeInput, CandidateReceipt>(self),
            Self::Submit => tool::<SubmitRequest, OperationStatus>(self),
            Self::Status => tool::<OperationRequest, OperationStatus>(self),
            Self::Cancel => tool::<OperationRequest, CancelResult>(self),
        }
    }
}
fn tool<I: JsonSchema, O: JsonSchema>(kind: ToolKind) -> Tool {
    let (name, description, effect) = kind.descriptor();
    let mut t = Tool::default();
    t.name = name.into();
    t.description = Some(description.into());
    t.input_schema = schema::<I>();
    t.output_schema = Some(schema::<ToolOutput<O>>());
    let mut hints = ToolAnnotations::default();
    hints.read_only_hint = Some(matches!(effect, Effect::ReadOnly));
    hints.open_world_hint = Some(false);
    t.annotations = Some(hints);
    t
}
pub(crate) fn tools() -> Vec<Tool> {
    ToolKind::ALL
        .into_iter()
        .map(ToolKind::definition)
        .collect()
}
pub(crate) fn failure(code: ServiceError) -> CallToolResult {
    result::<()>(Err(code.into()), 1024).expect("static error fits minimum limit")
}
fn result<T: Serialize>(r: Result<T, Failure>, max: usize) -> Result<CallToolResult, ErrorData> {
    let error = r.is_err();
    let output = match r {
        Ok(result) => ToolOutput::Ok { result },
        Err(f) => ToolOutput::Error {
            error: ErrorView {
                code: f.code,
                catalog_reason: f.catalog,
            },
        },
    };
    let bytes = crate::output::encode(&output, max)
        .map_err(|_| ErrorData::internal_error("output budget exceeded", None))?;
    let mut r = CallToolResult::success(vec![]);
    r.result_type = None;
    r.structured_content = Some(serde_json::from_slice(&bytes).expect("encoded JSON"));
    r.is_error = Some(error);
    Ok(r)
}
impl<S: ExecutionServicePort> Handler<S> {
    async fn selection(
        &self,
        input: CatalogInput,
        wait: CancellationToken,
    ) -> Result<CatalogCandidate, Failure> {
        let c = self
            .service
            .catalog(Some(input.catalog.clone()), wait)
            .await?;
        // Serialization of RawValue preserves numeric tokens. C03 remains the only
        // owner of catalog numeric/secret/default normalization and validation.
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Selection<'a> {
            catalog: &'a CatalogRef,
            item_id: &'a Id,
            variant_id: &'a Id,
            arguments: &'a serde_json::value::RawValue,
        }
        let bytes = crate::output::encode(
            &Selection {
                catalog: &input.catalog,
                item_id: &input.item_id,
                variant_id: &input.variant_id,
                arguments: &input.arguments,
            },
            self.limits.catalog.max_bytes,
        )?;
        let selection = c.select(&bytes, &self.limits.catalog, &self.limits.parameters)?;
        Ok(CatalogCandidate {
            operation_request_id: input.operation_request_id,
            selection,
        })
    }
    async fn catalog(&self, raw: &str, wait: CancellationToken) -> Result<CatalogView, Failure> {
        let _: Empty = decode(raw)?;
        let c = self.service.catalog(None, wait).await?;
        if c.snapshot().items.len() > self.limits.catalog.max_collection_items {
            return Err(ServiceError::Limit.into());
        }
        let mut items = Vec::new();
        for item in c
            .snapshot()
            .items
            .iter()
            .filter(|item| item.ai_discoverable)
        {
            let mut parameters = Vec::new();
            for op in &item.operations {
                let p = c.projection(&item.id, &op.id, &self.limits.parameters)?;
                parameters.push(ParameterView {
                    variant_id: op.id.clone(),
                    input_schema: p.input_schema().clone(),
                });
            }
            items.push(ItemView {
                item: item.clone(),
                parameters,
            });
        }
        Ok(CatalogView {
            catalog: c.reference(),
            items,
        })
    }
    async fn preview(&self, raw: &str, wait: CancellationToken) -> Result<PlanPreview, Failure> {
        let request = match decode::<PreviewInput>(raw)? {
            PreviewInput::Catalog { selection } => {
                PreviewRequest::Catalog(Box::new(self.selection(selection, wait.clone()).await?))
            }
            PreviewInput::Candidate {
                operation_request_id,
                candidate,
            } => PreviewRequest::Candidate {
                operation_request_id,
                candidate,
            },
        };
        Ok(self.service.preview(request, wait).await?)
    }
    async fn propose(
        &self,
        raw: &str,
        wait: CancellationToken,
    ) -> Result<CandidateReceipt, Failure> {
        let request = match decode::<ProposeInput>(raw)? {
            ProposeInput::Catalog { selection } => {
                CandidateRequest::Catalog(Box::new(self.selection(selection, wait.clone()).await?))
            }
            ProposeInput::Script {
                operation_request_id,
                source_utf8,
                interpreter,
            } => {
                if source_utf8.is_empty()
                    || source_utf8.contains('\0')
                    || source_utf8.len() > self.limits.parameters.max_bytes
                {
                    return Err(ServiceError::InvalidInput.into());
                }
                CandidateRequest::Script(ScriptDraft {
                    operation_request_id,
                    source_utf8,
                    interpreter,
                })
            }
        };
        Ok(self.service.propose(request, wait).await?)
    }
    pub(crate) async fn call(
        &self,
        name: &str,
        raw: &str,
        wait: CancellationToken,
    ) -> Result<CallToolResult, ErrorData> {
        self.service
            .check_binding()
            .map_err(|_| ErrorData::invalid_request("service binding unavailable", None))?;
        let max = self.limits.response_bytes;
        let kind = ToolKind::from_name(name)
            .ok_or_else(|| ErrorData::invalid_params("unknown tool", None))?;
        match kind {
            ToolKind::Catalog => result(self.catalog(raw, wait).await, max),
            ToolKind::Capabilities => {
                let r = async {
                    let _: Empty = decode(raw)?;
                    Ok(self.service.capabilities(wait).await?)
                }
                .await;
                result(r, max)
            }
            ToolKind::Preview => result(self.preview(raw, wait).await, max),
            ToolKind::Propose => result(self.propose(raw, wait).await, max),
            ToolKind::Submit => result(
                async { Ok(self.service.submit(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::Status => result(
                async { Ok(self.service.status(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::Cancel => result(
                async { Ok(self.service.cancel(decode(raw)?, wait).await?) }.await,
                max,
            ),
        }
    }
}
impl<S: ExecutionServicePort> ServerHandler for Handler<S> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "rss-execution",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_protocol_version(ProtocolVersion::V_2025_11_25)
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![ProtocolVersion::V_2025_11_25])
    }
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.service
            .check_binding()
            .map_err(|_| ErrorData::invalid_request("service binding unavailable", None))?;
        if request.and_then(|r| r.cursor).is_some() {
            return Err(ErrorData::invalid_params("invalid cursor", None));
        }
        Ok(ListToolsResult {
            tools: tools(),
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let raw = context
            .extensions
            .get::<OriginalArguments>()
            .ok_or_else(|| ErrorData::invalid_request("bounded ingress required", None))?;
        Ok(self.call(&request.name, &raw.0, context.ct).await?.into())
    }
}

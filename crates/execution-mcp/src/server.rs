use crate::{
    model::{Empty, ErrorView, ToolOutput},
    transport::OriginalArguments,
    *,
};
use execution_contract::{BackendSelection, BackendTask, TaskSubmission};
use rmcp::{model::*, service::RequestContext, ErrorData, RoleServer, ServerHandler};
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{borrow::Cow, sync::Arc};
use tokio_util::sync::CancellationToken;

pub(crate) struct Handler<S> {
    pub service: Arc<S>,
    pub limits: McpLimits,
}

pub(crate) struct Failure {
    code: ServiceError,
}
impl From<ServiceError> for Failure {
    fn from(code: ServiceError) -> Self {
        Self { code }
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
    Tasks,
    Capabilities,
    Execute,
    Status,
    Cancel,
}
#[derive(Clone, Copy)]
enum Effect {
    ReadOnly,
    MayPersist,
}
impl ToolKind {
    const ALL: [Self; 5] = [
        Self::Tasks,
        Self::Capabilities,
        Self::Execute,
        Self::Status,
        Self::Cancel,
    ];

    fn descriptor(self) -> (&'static str, &'static str, Effect) {
        match self {
            Self::Tasks => ("execution_tasks", "Verified backend tasks. Select the exact task, attempt and revision; presence never authorizes execution.", Effect::ReadOnly),
            Self::Capabilities => ("execution_capabilities", "Current bound-context capabilities. Unknown is not supported.", Effect::ReadOnly),
            Self::Execute => ("execution_execute", "Propose an exact backend task. The desktop user must confirm installation. Query the returned original request; never choose another attempt after response loss.", Effect::MayPersist),
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
            Self::Tasks => tool::<Empty, Vec<BackendTask>>(self),
            Self::Capabilities => tool::<Empty, CapabilityView>(self),
            Self::Execute => tool::<BackendSelection, TaskSubmission>(self),
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
            error: ErrorView { code: f.code },
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
    async fn tasks(&self, raw: &str, wait: CancellationToken) -> Result<Vec<BackendTask>, Failure> {
        let _: Empty = decode(raw)?;
        let tasks = self.service.tasks(wait).await?;
        if tasks.len() > 128 {
            return Err(ServiceError::Limit.into());
        }
        Ok(tasks)
    }
    async fn execute(&self, raw: &str, wait: CancellationToken) -> Result<TaskSubmission, Failure> {
        Ok(self
            .service
            .execute(decode::<BackendSelection>(raw)?, wait)
            .await?)
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
            ToolKind::Tasks => result(self.tasks(raw, wait).await, max),
            ToolKind::Capabilities => {
                let r = async {
                    let _: Empty = decode(raw)?;
                    Ok(self.service.capabilities(wait).await?)
                }
                .await;
                result(r, max)
            }
            ToolKind::Execute => result(self.execute(raw, wait).await, max),
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
        let metadata = context
            .extensions
            .get::<crate::transport::OriginalMetadata>()
            .ok_or_else(|| ErrorData::invalid_request("bounded ingress required", None))?;
        let service = match self.service.bind_call(&metadata.0) {
            Ok(service) => service,
            Err(error) => {
                return Ok(result::<Value>(Err(error.into()), self.limits.response_bytes)?.into())
            }
        };
        let handler = Handler {
            service,
            limits: self.limits.clone(),
        };
        Ok(handler
            .call(&request.name, &raw.0, context.ct)
            .await?
            .into())
    }
}

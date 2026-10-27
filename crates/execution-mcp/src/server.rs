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

// MCP model interfaces require an object with named properties at the root.
// Keep the canonical TaskRef as a nested value, including all owner variants.
#[derive(serde::Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskDetailQuery {
    reference: execution_app::tasks::TaskRef,
}

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
    let mut value = schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value();
    constrain_schema_integers(&mut value);
    let mut object = value.as_object().expect("object schema").clone();
    // Every tool DTO is object-shaped, including tagged enum branches. Schemars
    // omits the root type for those unions; MCP 2025-11-25 requires it explicitly.
    object.insert("type".into(), Value::String("object".into()));
    Arc::new(object)
}
// The MCP/SDK boundary accepts JSON safe integers. Rust-owned static DTO schemas
// must advertise that same range; Resource parameter schemas are passed unchanged.
fn constrain_schema_integers(value: &mut Value) {
    const MAX: i64 = 9_007_199_254_740_991;
    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("integer") {
                for (name, bound) in [("maximum", MAX), ("minimum", -MAX)] {
                    if object.get(name).and_then(Value::as_f64).is_some_and(|n| {
                        if name == "maximum" {
                            n > MAX as f64
                        } else {
                            n < -MAX as f64
                        }
                    }) {
                        object.insert(name.into(), bound.into());
                    }
                }
            }
            for child in object.values_mut() {
                constrain_schema_integers(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                constrain_schema_integers(child);
            }
        }
        _ => {}
    }
}
// Tool identity and effect metadata have one typed owner. Exhaustive matches
// bind schemas and dispatch to that owner without repeating wire-name lists.
#[derive(Clone, Copy)]
pub(crate) enum ToolKind {
    LocalSoftwareRead,
    LocalSoftwarePrepare,
    LocalSoftwareStatus,
    QueryDirectory,
    DataRead,
    QuerySubmit,
    QueryStatus,
    Offers,
    PolicyTools,
    InvokePolicy,
    PrepareOperation,
    SoftwareRead,
    Capabilities,
    Execute,
    TaskList,
    TaskDetail,
    TaskControl,
    TaskControlStatus,
}
#[derive(Clone, Copy)]
enum Effect {
    ReadOnly,
    MayPersist,
}
impl ToolKind {
    const ALL: [Self; 18] = [
        Self::LocalSoftwareRead,
        Self::LocalSoftwarePrepare,
        Self::LocalSoftwareStatus,
        Self::QueryDirectory,
        Self::DataRead,
        Self::QuerySubmit,
        Self::QueryStatus,
        Self::Offers,
        Self::PolicyTools,
        Self::InvokePolicy,
        Self::PrepareOperation,
        Self::SoftwareRead,
        Self::Capabilities,
        Self::Execute,
        Self::TaskList,
        Self::TaskDetail,
        Self::TaskControl,
        Self::TaskControlStatus,
    ];

    fn descriptor(self) -> (&'static str, &'static str, Effect) {
        match self {
            Self::LocalSoftwareRead => ("execution_software_read", "Read retained installed-software facts, qualified fixed updates and local action availability. No collector or mutation runs.", Effect::ReadOnly),
            Self::LocalSoftwarePrepare => ("execution_software_prepare", "Prepare a fixed local software action for the desktop user to review and submit. Never executes. Return the original request for recovery; do not prepare again to poll.", Effect::MayPersist),
            Self::LocalSoftwareStatus => ("execution_software_status", "Query the original local software request and observed result. Unknown does not permit resubmission.", Effect::ReadOnly),
            Self::QueryDirectory => ("execution_query_directory", "Discover exact local query definitions. Descriptors are not execution grants.", Effect::ReadOnly),
            Self::DataRead => ("execution_data_read", "Read retained local data by query or original request, with snapshot pagination. This never starts collection.", Effect::ReadOnly),
            Self::QuerySubmit => ("execution_query_submit", "Explicitly submit one query. Reuse the original request after response loss. Published scripts retain their Policy risk and confirmation flow.", Effect::MayPersist),
            Self::QueryStatus => ("execution_query_status", "Read the original query state and existing confirmation reference. Never resubmit to poll.", Effect::ReadOnly),
            Self::SoftwareRead => ("execution_software_observe", "Collect a fresh bounded native software fact from one current signed definition step. No installation or script execution.", Effect::ReadOnly),
            Self::PrepareOperation => ("execution_operation_prepare", "Show an editable operation card for a current policy tool. Use for requests to display or edit settings. Suggestions do not execute: the user saves the final parameters in the desktop. Only registered scenes are accepted.", Effect::MayPersist),
            Self::InvokePolicy => ("execution_policy_invoke", "Invoke one authorized Policy directly by formal ID, tool name or alias. Supply Resource inputs. Current configuration and entry permissions are resolved by the product owner.", Effect::MayPersist),
            Self::PolicyTools => ("execution_policy_tools", "Currently authorized self-service software, scripts and enrollment entries. Publication, allowAi and risk are verified for this AI origin.", Effect::ReadOnly),
            Self::Offers => ("execution_offers", "Verified backend tasks. Select the exact task, attempt and revision; presence never authorizes execution.", Effect::ReadOnly),
            Self::Capabilities => ("execution_capabilities", "Current bound-context capabilities. Unknown is not supported.", Effect::ReadOnly),
            Self::Execute => ("execution_execute", "Propose an exact backend task. The desktop user must confirm installation. Query the returned original request; never choose another attempt after response loss.", Effect::MayPersist),
            Self::TaskList => ("execution_task_list", "List authorized original tasks with bounded filters and coverage; never starts collection or execution.", Effect::ReadOnly),
            Self::TaskDetail => ("execution_task_detail", "Read original task, steps, independent effect/cancellation/upload facts and allowedActions.", Effect::ReadOnly),
            Self::TaskControl => ("execution_task_control", "Explicit stage control with original task ref, current scope, expectedRevision and unique control ID. After response loss query the same control ID. Human confirmation is not a control action.", Effect::MayPersist),
            Self::TaskControlStatus => ("execution_task_control_status", "Read the original owner's control receipt; Unknown never authorizes a new request.", Effect::ReadOnly),
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
            Self::LocalSoftwareRead => {
                tool::<execution_contract::SoftwareRead, execution_contract::SoftwarePage>(self)
            }
            Self::LocalSoftwarePrepare => {
                tool::<LocalSoftwareProposal, execution_contract::LocalSoftwarePrepared>(self)
            }
            Self::LocalSoftwareStatus => {
                tool::<execution_contract::QueryRequest, LocalSoftwareResult>(self)
            }
            Self::QueryDirectory => {
                tool::<execution_contract::QueryDirectory, execution_contract::DataResponse>(self)
            }
            Self::DataRead => {
                tool::<execution_contract::DataRead, execution_contract::DataResponse>(self)
            }
            Self::QuerySubmit => {
                tool::<execution_contract::QuerySubmit, execution_contract::DataResponse>(self)
            }
            Self::QueryStatus => {
                tool::<execution_contract::QueryRequest, execution_contract::DataResponse>(self)
            }
            Self::SoftwareRead => tool::<
                execution_contract::SoftwareReadSelection,
                execution_contract::BackendRequest,
            >(self),
            Self::PrepareOperation => {
                tool::<OperationProposal, execution_contract::BackendRequest>(self)
            }
            Self::InvokePolicy => tool::<PolicyToolCall, OperationRequest>(self),
            Self::PolicyTools => tool::<PolicyToolsQuery, PolicyToolPage>(self),
            Self::Offers => tool::<Empty, Vec<BackendTask>>(self),
            Self::Capabilities => tool::<Empty, CapabilityView>(self),
            Self::Execute => tool::<BackendSelection, TaskSubmission>(self),
            Self::TaskList => {
                tool::<execution_app::tasks::TaskQuery, execution_app::tasks::TaskPage>(self)
            }
            Self::TaskDetail => tool::<TaskDetailQuery, execution_app::tasks::TaskDetail>(self),
            Self::TaskControl => tool::<
                execution_app::tasks::TaskControl,
                execution_app::tasks::TaskControlReceipt,
            >(self),
            Self::TaskControlStatus => tool::<
                execution_app::tasks::TaskControlQuery,
                execution_app::tasks::TaskControlReceipt,
            >(self),
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
    static DEFINITIONS: std::sync::OnceLock<Vec<Tool>> = std::sync::OnceLock::new();
    DEFINITIONS
        .get_or_init(|| {
            ToolKind::ALL
                .into_iter()
                .map(ToolKind::definition)
                .collect()
        })
        .clone()
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
    async fn offers(
        &self,
        raw: &str,
        wait: CancellationToken,
    ) -> Result<Vec<BackendTask>, Failure> {
        let _: Empty = decode(raw)?;
        let tasks = self.service.offers(wait).await?;
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
        let Some(kind) = ToolKind::from_name(name) else {
            if !name.starts_with("policy_") {
                return Err(ErrorData::invalid_params("unknown tool", None));
            }
            return result(
                async {
                    Ok(self
                        .service
                        .invoke_policy(
                            rss_mdm_agent_wire::runtime::CapabilitySelector::ToolName {
                                name: name.to_owned(),
                            },
                            decode(raw)?,
                            wait,
                        )
                        .await?)
                }
                .await,
                max,
            );
        };
        match kind {
            ToolKind::LocalSoftwareRead => result(
                async { Ok(self.service.software_read(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::LocalSoftwarePrepare => result(
                async { Ok(self.service.software_prepare(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::LocalSoftwareStatus => result(
                async { Ok(self.service.software_status(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::QueryDirectory
            | ToolKind::DataRead
            | ToolKind::QuerySubmit
            | ToolKind::QueryStatus => result(
                async {
                    use execution_contract::DataOperation as D;
                    let operation = match kind {
                        ToolKind::QueryDirectory => D::Directory(decode(raw)?),
                        ToolKind::DataRead => D::Read(decode(raw)?),
                        ToolKind::QuerySubmit => D::Submit(decode(raw)?),
                        ToolKind::QueryStatus => D::Status(decode(raw)?),
                        _ => unreachable!("query tools"),
                    };
                    Ok(self.service.collected_data(operation, wait).await?)
                }
                .await,
                max,
            ),
            ToolKind::SoftwareRead => result(
                async { Ok(self.service.observe_software(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::PrepareOperation => result(
                async { Ok(self.service.prepare_operation(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::InvokePolicy => result(
                async {
                    let input: PolicyToolCall = decode(raw)?;
                    let value = self
                        .service
                        .invoke_policy(
                            input.selector,
                            PolicyToolInput {
                                parameters: input.parameters,
                            },
                            wait,
                        )
                        .await?;
                    Ok(OperationRequest {
                        operation_request_id: value.action.request().clone(),
                    })
                }
                .await,
                max,
            ),
            ToolKind::PolicyTools => result(
                async { Ok(self.service.policy_tools(decode(raw)?, wait).await?) }.await,
                max,
            ),
            ToolKind::Offers => result(self.offers(raw, wait).await, max),
            ToolKind::Capabilities => {
                let r = async {
                    let _: Empty = decode(raw)?;
                    Ok(self.service.capabilities(wait).await?)
                }
                .await;
                result(r, max)
            }
            ToolKind::Execute => result(self.execute(raw, wait).await, max),
            ToolKind::TaskList => result(
                async {
                    match self
                        .service
                        .task_operation(
                            execution_app::tasks::TaskOperation::List(decode(raw)?),
                            wait,
                        )
                        .await?
                    {
                        execution_app::tasks::TaskResponse::Page(value) => Ok(value),
                        _ => Err(ServiceError::Unavailable.into()),
                    }
                }
                .await,
                max,
            ),
            ToolKind::TaskDetail => result(
                async {
                    match self
                        .service
                        .task_operation(
                            execution_app::tasks::TaskOperation::Detail(
                                decode::<TaskDetailQuery>(raw)?.reference,
                            ),
                            wait,
                        )
                        .await?
                    {
                        execution_app::tasks::TaskResponse::Detail(value) => Ok(value),
                        _ => Err(ServiceError::Unavailable.into()),
                    }
                }
                .await,
                max,
            ),
            ToolKind::TaskControl => result(
                async {
                    match self
                        .service
                        .task_operation(
                            execution_app::tasks::TaskOperation::Control(decode(raw)?),
                            wait,
                        )
                        .await?
                    {
                        execution_app::tasks::TaskResponse::Control(value) => Ok(value),
                        _ => Err(ServiceError::OutcomeUnknown.into()),
                    }
                }
                .await,
                max,
            ),
            ToolKind::TaskControlStatus => result(
                async {
                    match self
                        .service
                        .task_operation(
                            execution_app::tasks::TaskOperation::ControlStatus(decode(raw)?),
                            wait,
                        )
                        .await?
                    {
                        execution_app::tasks::TaskResponse::Control(value) => Ok(value),
                        _ => Err(ServiceError::Unavailable.into()),
                    }
                }
                .await,
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
        // ref: rmcp v3.4.0 src/model.rs::PaginatedRequestParams; standard metadata pagination preserves the existing frame budget.
        let all = tools();
        let cursor = request.and_then(|r| r.cursor);
        let start = cursor
            .as_ref()
            .map(|v| {
                v.parse::<usize>()
                    .ok()
                    .filter(|i| i.to_string() == v.as_str() && *i < all.len())
            })
            .unwrap_or(Some(0))
            .ok_or_else(|| ErrorData::invalid_params("invalid cursor", None))?;
        let budget = self.limits.response_bytes.saturating_sub(4096);
        let mut used = 0;
        let mut page = Vec::new();
        for tool in all.into_iter().skip(start) {
            let bytes = serde_json::to_vec(&tool)
                .map_err(|_| ErrorData::internal_error("tool metadata unavailable", None))?
                .len();
            if used + bytes > budget {
                break;
            }
            used += bytes;
            page.push(tool);
        }
        if page.is_empty() {
            return Err(ErrorData::internal_error("output budget exceeded", None));
        }
        let after = start + page.len();
        Ok(ListToolsResult {
            tools: page,
            next_cursor: (after < ToolKind::ALL.len()).then(|| after.to_string()),
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

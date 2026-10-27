//! Installed CLI: a stateless consumer of the authenticated production service.
// ref: clap-rs/clap clap_builder/src/builder/command.rs@v4.5.60
use clap::{Args, Parser, Subcommand};
use execution_app::tasks::*;
use execution_contract::*;
use execution_ipc::{
    entry::{EntryContext, EntryOperation},
    host::{ClientOrigin, Reply, Request, ServiceClient},
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(
    name = "rssmdm",
    version,
    about = "RSS MDM 受控命令行。名称只定位对象；执行仍由受信服务授权。"
)]
struct Cli {
    /// Machine-readable response; diagnostics always use stderr.
    #[arg(long, global = true)]
    json: bool,
    /// Explicit protected deployment pins; never an arbitrary endpoint.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Status,
    #[command(subcommand)]
    Capabilities(Discovery),
    #[command(subcommand)]
    Nodes(Discovery),
    #[command(subcommand)]
    Tools(Discovery),
    /// Exact typed selector: node:settings.account, id:UUID, tool:policy_HEX, alias:NAME.
    Resolve(Select),
    Inspect(Select),
    Schema(Select),
    Preview(Select),
    /// Open the current user's desktop; no business effect is submitted.
    Open(Select),
    Invoke(Invocation),
    #[command(subcommand)]
    Data(DataCommand),
    #[command(subcommand)]
    Query(QueryCommand),
    #[command(subcommand)]
    Tasks(TaskCommand),
    #[command(subcommand)]
    Batches(BatchCommand),
    #[command(subcommand)]
    Assets(AssetCommand),
    Checkin,
    Sync,
}
#[derive(Subcommand)]
enum Discovery {
    List {
        #[arg(long)]
        after: Option<String>,
    },
}
#[derive(Args)]
struct Select {
    /// Exact kind:value selector or current rssmdm URI.
    #[arg(value_parser=selector_arg)]
    target: String,
}
fn selector_arg(value: &str) -> std::result::Result<String, &'static str> {
    let valid = if value.starts_with("rssmdm:") {
        EntryIntent::parse(value).is_ok()
    } else {
        value
            .split_once(':')
            .is_some_and(|(kind, value)| EntrySelector::parse(kind, value).is_ok())
    };
    if valid {
        Ok(value.into())
    } else {
        Err("invalid typed selector")
    }
}
#[derive(Args)]
struct Invocation {
    #[command(flatten)]
    select: Select,
    /// Reuse this ID after a lost response. Different content under the same ID is rejected.
    #[arg(long)]
    request_id: Option<String>,
    /// Read schema-allowed parameters from bounded stdin; values never enter argv.
    #[arg(long)]
    input_stdin: bool,
}
#[derive(Args)]
struct QueryChoice {
    query: String,
    #[arg(long)]
    revision: String,
}
#[derive(Subcommand)]
enum DataCommand {
    Read {
        /// Original request; reading never triggers collection.
        request: String,
        #[arg(long, default_value_t = 64)]
        limit: u16,
        /// Snapshot digest returned by the preceding page.
        #[arg(long, requires = "offset")]
        snapshot: Option<String>,
        #[arg(long, requires = "snapshot")]
        offset: Option<u32>,
    },
}
#[derive(Subcommand)]
enum QueryCommand {
    List {
        #[arg(long)]
        after: Option<String>,
        /// Independent remote Policy directory cursor.
        #[arg(long)]
        policy_after: Option<String>,
    },
    Submit {
        #[command(flatten)]
        choice: QueryChoice,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        input_stdin: bool,
    },
    Status {
        request: String,
    },
    Result {
        request: String,
    },
    Cancel {
        request: String,
        #[arg(long)]
        control_id: Option<String>,
    },
}
#[derive(Subcommand)]
enum AssetCommand {
    Refresh {
        #[command(flatten)]
        choice: QueryChoice,
        #[arg(long)]
        request_id: Option<String>,
    },
}
#[derive(Subcommand)]
enum TaskCommand {
    /// Current verified offers, independently from retained task history.
    Offers,
    List {
        #[arg(long, default_value = "history")]
        view: String,
        #[arg(long)]
        after: Option<String>,
    },
    Show {
        target: String,
    },
    Wait {
        target: String,
        #[arg(long, default_value_t = 60)]
        timeout: u64,
    },
    /// NDJSON observations; Ctrl-C stops observation and does not cancel the task.
    Watch {
        target: String,
        #[arg(long, default_value_t = 60)]
        timeout: u64,
    },
    ControlStatus {
        target: String,
        control_id: String,
        /// Original scope returned by task detail.
        #[arg(long)]
        scope: String,
    },
    Control {
        target: String,
        #[arg(value_parser=["cancel","retry","pause","resume"])]
        action: String,
        #[arg(long)]
        control_id: Option<String>,
    },
}
#[derive(Subcommand)]
enum BatchCommand {
    List {
        #[arg(long)]
        after: Option<String>,
    },
    Show {
        id: String,
        #[arg(long)]
        after: Option<String>,
    },
    Related {
        activity: String,
        #[arg(long)]
        after: Option<String>,
    },
}
#[derive(Debug)]
struct Failure {
    code: u8,
    reason: &'static str,
}
type Result<T> = std::result::Result<T, Failure>;
fn invalid() -> Failure {
    Failure {
        code: 2,
        reason: "invalid_input",
    }
}
fn ipc(error: execution_app::Error) -> Failure {
    use execution_app::Error as E;
    Failure {
        code: match error {
            E::Denied | E::Unbound => 3,
            E::NotFound => 4,
            E::Conflict => 5,
            E::OutcomeUnknown => 8,
            E::Unsupported => 6,
            _ => 7,
        },
        reason: match error {
            E::Denied | E::Unbound => "denied",
            E::NotFound => "not_found",
            E::Conflict => "conflict_or_expired",
            E::OutcomeUnknown => "outcome_unknown",
            E::Unsupported => "unsupported",
            _ => "service_unavailable",
        },
    }
}
fn send(client: &ServiceClient, request: Request) -> Result<Reply> {
    let submitting = matches!(
        &request,
        Request::Entry {
            input: EntryOperation::Invoke { .. }
                | EntryOperation::Refresh { .. }
                | EntryOperation::Data {
                    input: DataOperation::Submit(_)
                }
        } | Request::Task {
            input: TaskOperation::Control(_),
            ..
        }
    );
    let reply = client.request(request).map_err(|error| {
        let mut failure = ipc(error);
        if submitting && failure.code == 7 {
            failure.code = 8;
            failure.reason = "outcome_unknown";
        }
        failure
    })?;
    match &reply {
        Reply::Rejected => Err(Failure {
            code: 3,
            reason: "denied",
        }),
        Reply::Unavailable => Err(Failure {
            code: if submitting { 8 } else { 7 },
            reason: if submitting {
                "outcome_unknown"
            } else {
                "service_unavailable"
            },
        }),
        Reply::DataFailure { reason } => Err(Failure {
            code: match reason {
                DataError::InvalidInput => 2,
                DataError::Denied => 3,
                DataError::NotFound => 4,
                DataError::Conflict | DataError::Expired | DataError::InProgress => 5,
                DataError::Unsupported => 6,
                _ => {
                    if submitting {
                        8
                    } else {
                        7
                    }
                }
            },
            reason: "operation_rejected",
        }),
        Reply::OperationRejected { .. } => Err(Failure {
            code: 5,
            reason: "operation_rejected",
        }),
        _ => Ok(reply),
    }
}
fn entry(client: &ServiceClient, input: EntryOperation) -> Result<Reply> {
    send(client, Request::Entry { input })
}
fn attach(client: &ServiceClient) -> Result<EntryContext> {
    match entry(client, EntryOperation::Attach)? {
        Reply::EntryContext { value } => Ok(value),
        _ => Err(Failure {
            code: 7,
            reason: "protocol_mismatch",
        }),
    }
}
fn request_id(value: Option<String>) -> Result<RequestId> {
    let value = value.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let id = uuid::Uuid::parse_str(&value).map_err(|_| invalid())?;
    if id.is_nil() || id.to_string() != value {
        return Err(invalid());
    }
    // Emitted before IPC. Automation may supply the same ID explicitly and query it after any interruption.
    eprintln!("request_id={value}");
    RequestId::new(value).map_err(|_| invalid())
}
fn parameters(read: bool) -> Result<BTreeMap<String, serde_json::Value>> {
    if !read {
        return Ok(Default::default());
    }
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    std::io::stdin()
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > 16384 {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}
fn intent(client: &ServiceClient, target: &str, action: EntryAction) -> Result<EntryIntent> {
    if target.starts_with("rssmdm:") {
        return EntryIntent::parse(target).map_err(|_| invalid());
    }
    let (kind, value) = target.split_once(':').ok_or_else(invalid)?;
    let selector = EntrySelector::parse(kind, value).map_err(|_| invalid())?;
    let namespace = if matches!(&selector, EntrySelector::LocalTool { .. })
        || matches!(&selector, EntrySelector::Node { id } if !id.ends_with(".enterprise") && id != "tasks.management")
    {
        None
    } else {
        attach(client)?.namespace
    };
    Ok(EntryIntent {
        action,
        selector,
        namespace,
        version: None,
        revision: None,
        step: None,
        parameters: Default::default(),
    })
}
fn task_ref(target: &str) -> Result<TaskRef> {
    let (kind, id) = target.split_once(':').unwrap_or(("request", target));
    match kind {
        "request" => Ok(TaskRef::Request {
            request: RequestId::new(id).map_err(|_| invalid())?,
        }),
        "activity" => Ok(TaskRef::Activity { id: id.into() }),
        _ => Err(invalid()),
    }
}
fn task(client: &ServiceClient, input: TaskOperation) -> Result<Reply> {
    send(
        client,
        Request::Task {
            input,
            origin: ClientOrigin::Desktop {},
        },
    )
}
fn control(
    client: &ServiceClient,
    target: &str,
    action: &str,
    id: Option<String>,
) -> Result<Reply> {
    let reference = task_ref(target)?;
    let Reply::Task {
        value: TaskResponse::Detail(detail),
    } = task(client, TaskOperation::Detail(reference.clone()))?
    else {
        return Err(invalid());
    };
    let action = match action {
        "cancel" => TaskAction::Cancel,
        "retry" => TaskAction::Retry,
        "pause" => TaskAction::Pause,
        "resume" => TaskAction::Resume,
        _ => return Err(invalid()),
    };
    if !detail.allowed_actions.contains(&action) {
        return Err(Failure {
            code: 3,
            reason: "action_not_allowed",
        });
    }
    let id = Id::new(request_id(id)?.as_str()).map_err(|_| invalid())?;
    task(
        client,
        TaskOperation::Control(TaskControl {
            reference,
            id,
            scope: detail.scope,
            expected_revision: detail.revision,
            action,
        }),
    )
}
fn data(client: &ServiceClient, input: DataOperation) -> Result<Reply> {
    entry(client, EntryOperation::Data { input })
}
fn read(
    client: &ServiceClient,
    request: String,
    limit: u16,
    after: Option<DataCursor>,
) -> Result<Reply> {
    data(
        client,
        DataOperation::Read(DataRead {
            selection: DataSelection::Request {
                request: RequestId::new(request).map_err(|_| invalid())?,
            },
            after,
            limit,
            max_age_seconds: None,
        }),
    )
}
fn query(choice: QueryChoice) -> Result<QueryRef> {
    Ok(QueryRef {
        id: Id::new(choice.query).map_err(|_| invalid())?,
        revision: Digest::new(choice.revision).map_err(|_| invalid())?,
    })
}
fn open(client: &ServiceClient, select: Select) -> Result<serde_json::Value> {
    let target = intent(client, &select.target, EntryAction::View)?;
    let uri = target.uri().map_err(|_| invalid())?;
    execution_ipc::navigation::open(&uri).map_err(ipc)?;
    Ok(serde_json::json!({"opened":true,"effect":"navigation_requested"}))
}
fn value(reply: Reply) -> Result<serde_json::Value> {
    serde_json::to_value(reply).map_err(|_| invalid())
}
fn dispatch(command: Command, client: &ServiceClient) -> Result<serde_json::Value> {
    let reply = match command {
        Command::Status => send(client, Request::ServiceStatus {})?,
        Command::Nodes(Discovery::List { after }) => {
            if after.is_some() {
                return Err(invalid());
            }
            return Ok(serde_json::json!({"nodes":PAGE_NODES}));
        }
        Command::Capabilities(Discovery::List { after })
        | Command::Tools(Discovery::List { after }) => {
            entry(client, EntryOperation::Discover { after })?
        }
        Command::Resolve(select)
        | Command::Inspect(select)
        | Command::Schema(select)
        | Command::Preview(select) => {
            let target = intent(client, &select.target, EntryAction::View)?;
            if matches!(target.selector, EntrySelector::Capability { .. }) {
                entry(client, EntryOperation::Preview { intent: target })?
            } else {
                target.validate().map_err(|_| invalid())?;
                if target.namespace.is_some() && target.namespace != attach(client)?.namespace {
                    return Err(Failure {
                        code: 3,
                        reason: "namespace_mismatch",
                    });
                }
                entry(
                    client,
                    EntryOperation::Resolve {
                        selector: target.selector,
                    },
                )?
            }
        }
        Command::Open(select) => return open(client, select),
        Command::Invoke(invocation) => {
            let mut target = intent(client, &invocation.select.target, EntryAction::Invoke)?;
            target.action = EntryAction::Invoke;
            if invocation.input_stdin {
                target.parameters = parameters(true)?;
            }
            let original = if matches!(target.selector, EntrySelector::LocalTool { .. }) {
                let kind = target.parameters.get("kind").and_then(|v| v.as_str());
                if matches!(kind, Some("status" | "submit" | "keepDisplay")) {
                    target
                        .parameters
                        .get("request")
                        .or_else(|| {
                            target
                                .parameters
                                .get("input")
                                .and_then(|v| v.get("request"))
                        })
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                } else {
                    None
                }
            } else {
                None
            };
            if original
                .as_ref()
                .zip(invocation.request_id.as_ref())
                .is_some_and(|(a, b)| a != b)
            {
                return Err(invalid());
            }
            entry(
                client,
                EntryOperation::Invoke {
                    intent: target,
                    request: request_id(invocation.request_id.or(original))?,
                },
            )?
        }
        Command::Data(DataCommand::Read {
            request,
            limit,
            snapshot,
            offset,
        }) => {
            let after = snapshot
                .map(|v| {
                    Ok(DataCursor {
                        snapshot: Digest::new(v).map_err(|_| invalid())?,
                        offset: offset.ok_or_else(invalid)?,
                    })
                })
                .transpose()?;
            read(client, request, limit, after)?
        }
        Command::Query(QueryCommand::List {
            after,
            policy_after,
        }) => data(
            client,
            DataOperation::Directory(QueryDirectory {
                refresh_policies: true,
                policy_cursor: policy_after,
                after: after.map(Id::new).transpose().map_err(|_| invalid())?,
                limit: 64,
            }),
        )?,
        Command::Query(QueryCommand::Submit {
            choice,
            request_id: id,
            input_stdin,
        }) => data(
            client,
            DataOperation::Submit(QuerySubmit {
                request: request_id(id)?,
                query: query(choice)?,
                parameters: parameters(input_stdin)?,
            }),
        )?,
        Command::Query(QueryCommand::Status { request }) => data(
            client,
            DataOperation::Status(QueryRequest {
                request: RequestId::new(request).map_err(|_| invalid())?,
            }),
        )?,
        Command::Query(QueryCommand::Result { request }) => read(client, request, 64, None)?,
        Command::Query(QueryCommand::Cancel {
            request,
            control_id,
        }) => control(client, &request, "cancel", control_id)?,
        Command::Tasks(TaskCommand::Offers) => send(client, Request::Offers {})?,
        Command::Tasks(TaskCommand::List { view, after }) => {
            attach(client)?;
            task(
                client,
                TaskOperation::List(TaskQuery {
                    view: match view.as_str() {
                        "active" => TaskView::Active,
                        "history" => TaskView::History,
                        "management" => TaskView::Management,
                        _ => return Err(invalid()),
                    },
                    cursor: after,
                    limit: 64,
                    ..Default::default()
                }),
            )?
        }
        Command::Tasks(TaskCommand::Show { target }) => {
            attach(client)?;
            task(client, TaskOperation::Detail(task_ref(&target)?))?
        }
        Command::Tasks(TaskCommand::ControlStatus {
            target,
            control_id,
            scope,
        }) => {
            attach(client)?;
            task(
                client,
                TaskOperation::ControlStatus(TaskControlQuery {
                    reference: task_ref(&target)?,
                    id: Id::new(control_id).map_err(|_| invalid())?,
                    scope,
                }),
            )?
        }
        Command::Tasks(TaskCommand::Control {
            target,
            action,
            control_id,
        }) => {
            attach(client)?;
            control(client, &target, &action, control_id)?
        }
        Command::Tasks(TaskCommand::Wait { target, timeout }) => {
            return observe(client, &target, timeout, false)
        }
        Command::Tasks(TaskCommand::Watch { target, timeout }) => {
            return observe(client, &target, timeout, true)
        }
        Command::Batches(BatchCommand::List { after }) => entry(
            client,
            EntryOperation::Activities {
                batch: None,
                related: None,
                after,
                limit: 64,
            },
        )?,
        Command::Batches(BatchCommand::Show { id, after }) => entry(
            client,
            EntryOperation::Activities {
                batch: Some(id),
                related: None,
                after,
                limit: 64,
            },
        )?,
        Command::Batches(BatchCommand::Related { activity, after }) => entry(
            client,
            EntryOperation::Activities {
                batch: None,
                related: Some(activity),
                after,
                limit: 50,
            },
        )?,
        Command::Assets(AssetCommand::Refresh {
            choice,
            request_id: id,
        }) => entry(
            client,
            EntryOperation::Refresh {
                query: query(choice)?,
                request: request_id(id)?,
            },
        )?,
        Command::Checkin => entry(client, EntryOperation::Checkin)?,
        Command::Sync => entry(client, EntryOperation::Sync)?,
    };
    value(reply)
}
static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(unix)]
extern "C" fn interrupted(_: i32) {
    INTERRUPTED.store(true, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(windows)]
unsafe extern "system" fn console_interrupt(event: u32) -> i32 {
    if event == windows_sys::Win32::System::Console::CTRL_C_EVENT {
        INTERRUPTED.store(true, std::sync::atomic::Ordering::Relaxed);
        1
    } else {
        0
    }
}
fn observe(
    client: &ServiceClient,
    target: &str,
    timeout: u64,
    watch: bool,
) -> Result<serde_json::Value> {
    if timeout == 0 || timeout > 86400 {
        return Err(invalid());
    }
    #[cfg(unix)]
    // SAFETY: the static handler only sets an atomic flag.
    unsafe {
        libc::signal(libc::SIGINT, interrupted as *const () as usize);
    }
    #[cfg(windows)]
    // SAFETY: static handler only sets an atomic flag and retains no borrowed state.
    if unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console_interrupt), 1)
    } == 0
    {
        return Err(Failure {
            code: 7,
            reason: "observation_unavailable",
        });
    }
    attach(client)?;
    let reference = task_ref(target)?;
    let start = Instant::now();
    loop {
        if INTERRUPTED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Failure {
                code: 130,
                reason: "observation_interrupted",
            });
        }
        let reply = task(client, TaskOperation::Detail(reference.clone()))?;
        let ended = matches!(&reply,Reply::Task {value:TaskResponse::Detail(v)} if !v.active || v.stage == "outcomeUnknown");
        let value = value(reply)?;
        if watch {
            println!(
                "{}",
                serde_json::json!({"version":1,"ok":result_exit(&value)==0,"exitCode":result_exit(&value),"result":value})
            );
            let _ = std::io::stdout().flush();
        }
        if ended {
            return Ok(value);
        }
        if start.elapsed() >= Duration::from_secs(timeout) {
            return Err(Failure {
                code: 9,
                reason: "wait_timeout",
            });
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
fn main() -> std::process::ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code() as u8;
            if code != 0 && std::env::args_os().any(|a| a == "--json") {
                println!(
                    "{}",
                    serde_json::json!({"version":1,"ok":false,"error":{"code":"invalid_input","exitCode":2}})
                );
                eprintln!("rssmdm: invalid_input");
            } else {
                let _ = error.print();
            }
            return std::process::ExitCode::from(code);
        }
    };
    let watching = matches!(&cli.command, Command::Tasks(TaskCommand::Watch { .. }));
    let json = cli.json || watching;
    let result = (|| {
        let client = match cli.config {
            Some(path) => ServiceClient::installed_at(&path),
            None => ServiceClient::installed(),
        }
        .map_err(ipc)?;
        dispatch(cli.command, &client)
    })();
    match result {
        Ok(result) => {
            let exit = result_exit(&result);
            if !watching {
                println!(
                    "{}",
                    if json {
                        serde_json::json!({"version":1,"ok":exit==0,"exitCode":exit,"result":result})
                        .to_string()
                    } else {
                        serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into())
                    }
                );
            }
            std::process::ExitCode::from(exit)
        }
        Err(failure) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"version":1,"ok":false,"error":{"code":failure.reason,"exitCode":failure.code}})
                );
            }
            eprintln!("rssmdm: {}", failure.reason);
            std::process::ExitCode::from(failure.code)
        }
    }
}
fn result_exit(result: &serde_json::Value) -> u8 {
    let status = if result["kind"] == "task" && result["value"]["kind"] == "detail" {
        &result["value"]["value"]
    } else {
        return 0;
    };
    if status["facts"]["kind"] == "execution" {
        let execution = &status["facts"]["value"]["status"];
        if execution["phase"] == "outcomeUnknown" || execution["assessment"] == "unknown" {
            return 8;
        }
        if execution["phase"] == "admissionDenied"
            || execution["phase"] == "failedBeforeDispatch"
            || execution["assessment"] == "notSatisfied"
        {
            return 10;
        }
    }
    if status["facts"]["kind"] == "preparation" && status["facts"]["value"]["state"] == "failed" {
        return 10;
    }
    0
}

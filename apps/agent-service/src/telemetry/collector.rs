//! Signed periodic configurations compile into the existing materializer and process owner.
use crate::backend::plan::{self, id, reference};
use agent_client::{telemetry::Configuration, Error};
use execution_contract::*;
use execution_runner::{Artifacts, PeriodicRunner};
use rss_mdm_agent_wire::runtime as w;
use std::{collections::BTreeMap, path::PathBuf};

pub(super) struct Collector {
    runner: PeriodicRunner,
    active: Option<Active>,
    completed: Option<ProcessEvidence>,
    preparing: Option<Preparing>,
}
struct NativeMaterials {
    image: SoftwareMaterial,
    tools: BTreeMap<String, SoftwareMaterial>,
}
struct Preparing {
    configuration: Configuration,
    observed: i64,
    cancelled: std::sync::atomic::AtomicBool,
    result: std::sync::mpsc::Receiver<Result<NativeMaterials, Error>>,
}
pub(super) struct Active {
    pub configuration: Configuration,
    pub observed: i64,
    input_digest: Digest,
    _leases: Vec<execution_runner::staging::RetainedMaterialLease>,
}
impl Collector {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            runner: PeriodicRunner::new().map_err(crate::error::app_error)?,
            active: None,
            completed: None,
            preparing: None,
        })
    }
    pub fn active(&self) -> bool {
        self.preparing.is_some() || self.runner.active()
    }
    pub fn configuration(&self) -> Option<&Configuration> {
        self.preparing
            .as_ref()
            .map(|v| &v.configuration)
            .or_else(|| self.active.as_ref().map(|v| &v.configuration))
    }
    pub fn cancel(&self) -> Result<(), Error> {
        if let Some(preparing) = &self.preparing {
            preparing
                .cancelled
                .store(true, std::sync::atomic::Ordering::Release);
        }
        self.runner.cancel().map_err(crate::error::app_error)
    }
    pub fn shutdown(&mut self) -> Result<(), Error> {
        self.cancel()?;
        // Preparation performs bounded read-only validation and cannot launch a process.
        // Wait for its single worker before releasing this owner.
        if let Some(preparing) = &self.preparing {
            let _ = preparing
                .result
                .recv_timeout(std::time::Duration::from_secs(1))
                .map_err(|_| Error::Unavailable)?;
            self.preparing = None;
        }
        self.runner.shutdown().map_err(crate::error::app_error)?;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while self.runner.active() {
            if let Some(evidence) = self.runner.take().map_err(crate::error::app_error)? {
                self.completed = Some(evidence);
                break;
            }
            if std::time::Instant::now() >= until {
                return Err(Error::Unavailable);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }
    pub fn take(&mut self) -> Result<Option<(Active, ProcessEvidence)>, Error> {
        let evidence = match self.completed.take() {
            Some(value) => Some(value),
            None => self.runner.take().map_err(crate::error::app_error)?,
        };
        let Some(evidence) = evidence else {
            return Ok(None);
        };
        let active = self.active.take().ok_or(Error::Storage)?;
        let mut evidence = evidence;
        if matches!(
            active.configuration.payload().collector,
            w::TelemetryCollector::Native { .. }
        ) {
            let completion = serde_json::from_slice::<CollectionCompletion>(&evidence.stderr);
            if completion.map_or(true, |v| v.input_sha256 != active.input_digest.as_str()) {
                evidence.quality = OutputQuality::Failed;
            }
        }
        Ok(Some((active, evidence)))
    }
    pub fn start(
        &mut self,
        configuration: Configuration,
        bytes: Option<Vec<u8>>,
        core: &mut crate::service::Core,
        observed: i64,
    ) -> Result<(), Error> {
        if self.active() {
            return Err(Error::Capacity);
        }
        if let w::TelemetryCollector::Native { plan: read, .. } = &configuration.payload().collector
        {
            let read = read.clone();
            let config = core.config.clone();
            let (sender, result) = std::sync::mpsc::sync_channel(1);
            // Keep hashing and OS prerequisite reads off the SQLite/IPC owner. There is
            // exactly one preparation or physical collection; no task or receipt is created.
            std::thread::Builder::new()
                .name("rss-collection-prepare".into())
                .spawn(move || {
                    let prepared = (|| {
                        crate::collection_worker::preflight(&read)?;
                        let image = plan::worker_image(&config)?;
                        let tools = crate::collection_worker::tools(&config, &read)?;
                        Ok(NativeMaterials { image, tools })
                    })();
                    let _ = sender.send(prepared);
                })?;
            self.preparing = Some(Preparing {
                configuration,
                observed,
                result,
                cancelled: std::sync::atomic::AtomicBool::new(false),
            });
            return Ok(());
        }
        self.start_ready(configuration, bytes, core, observed, None)
    }
    /// Finish preparation on the original owner, after its current revocation checks.
    pub fn advance(&mut self, core: &mut crate::service::Core) -> Result<(), Error> {
        let Some(preparing) = &self.preparing else {
            return Ok(());
        };
        let result = match preparing.result.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(()),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(Error::Unavailable),
        };
        let preparing = self.preparing.take().ok_or(Error::Storage)?;
        if preparing
            .cancelled
            .load(std::sync::atomic::Ordering::Acquire)
            || !core.host.identity_ready()
            || core.stopping
        {
            return Err(Error::Denied);
        }
        self.start_ready(
            preparing.configuration,
            None,
            core,
            preparing.observed,
            Some(result?),
        )
    }
    fn start_ready(
        &mut self,
        configuration: Configuration,
        bytes: Option<Vec<u8>>,
        core: &mut crate::service::Core,
        observed: i64,
        native: Option<NativeMaterials>,
    ) -> Result<(), Error> {
        let p = configuration.payload();
        let now_ms = crate::wall()?;
        let deadline =
            (p.expires_at as u64 * 1000).min(now_ms.saturating_add(u64::from(p.execution_millis)));
        if deadline <= now_ms {
            return Err(Error::Expired);
        }
        let platform = plan::platform()?;
        let run_as = RunAs::System { platform };
        let session = SessionRequirement::NotRequired {};
        let budget = ExecutionBudget {
            total_timeout_ms: deadline - now_ms,
            total_output_bytes: u64::from(p.output_bytes),
            max_attempts: 1,
        };
        let cwd = core
            .config
            .work_root
            .to_str()
            .ok_or(Error::Configuration)?
            .to_owned();
        let mut parameters = BTreeMap::new();
        let (launch, artifacts) = match &p.collector {
            w::TelemetryCollector::Script {
                content,
                profile,
                arguments,
                environment,
                sql_parameters,
                max_rows,
                ..
            } => {
                let interpreter = core
                    .config
                    .interpreters
                    .iter()
                    .find(|i| i.profile == *profile)
                    .ok_or(Error::Unsupported)?;
                interpreter
                    .image
                    .verify(&interpreter.image.path)
                    .map_err(|_| Error::Untrusted)?;
                let bytes = bytes.ok_or(Error::Unavailable)?;
                use sha2::{Digest as _, Sha256};
                if bytes.len() as u64 != content.length
                    || Sha256::digest(&bytes).as_slice() != content.sha256
                {
                    return Err(Error::Untrusted);
                }
                let artifact = ExactArtifactRef {
                    resource: reference("telemetry-content", plan::hex(&content.sha256))?,
                    sha256: Digest::new(plan::hex(&content.sha256)).map_err(|_| Error::Protocol)?,
                };
                let path = core
                    .material_root
                    .publish(
                        None,
                        std::io::Cursor::new(&bytes),
                        &artifact.sha256,
                        bytes.len() as u64,
                    )
                    .map_err(crate::error::app_error)?;
                parameters = sql_parameters
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(k, value)| (k, InputValue::Literal { value }))
                    .collect();
                let env = environment
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            EnvironmentKey::new(k).map_err(|_| Error::Protocol)?,
                            InputValue::Literal {
                                value: v.clone().into(),
                            },
                        ))
                    })
                    .collect::<Result<_, Error>>()?;
                let output = OutputSpec {
                    format: OutputFormat::Json {
                        max_rows: *max_rows,
                    },
                    stdout: TextEncoding::Utf8,
                    stderr: TextEncoding::Utf8,
                };
                let launch = if *profile == w::ExecutorProfile::Osquery {
                    if !arguments.is_empty() {
                        return Err(Error::Untrusted);
                    }
                    let argv = execution_runner::osquery::arguments(
                        &bytes,
                        &parameters,
                        *max_rows,
                        platform,
                    )
                    .map_err(crate::error::app_error)?;
                    LaunchSpec {
                        artifact,
                        interpreter: InterpreterRef {
                            artifact: ExactArtifactRef {
                                resource: reference(
                                    execution_runner::osquery::PROFILE,
                                    &interpreter.image.sha256,
                                )?,
                                sha256: Digest::new(&interpreter.image.sha256)
                                    .map_err(|_| Error::Configuration)?,
                            },
                            profile: reference(execution_runner::osquery::PROFILE, "1")?,
                        },
                        argv: argv
                            .into_iter()
                            .map(|value| LaunchArg::Literal { value })
                            .collect(),
                        artifact_encoding: ArtifactEncoding::Utf8,
                        stdin: StandardInput::Closed {},
                        output,
                        cwd,
                        env,
                    }
                } else {
                    script_plan::compile_invocation(
                        script_plan::ScriptInvocationInput {
                            platform,
                            profile: plan::script_profile(*profile)?,
                            artifact,
                            interpreter: plan::script_interpreter(interpreter)?,
                            arguments: arguments.clone(),
                            artifact_encoding: ArtifactEncoding::Utf8,
                            stdin: StandardInput::Closed {},
                            output,
                            cwd,
                            env,
                        },
                        &plan::storage_limits().input,
                    )
                    .map_err(|_| Error::Unsupported)?
                };
                let artifacts = Artifacts {
                    program: vec![],
                    delegate: None,
                    interpreter: interpreter.image.path.clone(),
                    content: path,
                    work_root: core.config.work_root.clone(),
                    controlled_input: None,
                };
                (launch, artifacts)
            }
            w::TelemetryCollector::Native { plan: read, .. } => {
                if read.execution_identity() != w::ExecutionIdentity::System
                    || !read.supported_by(&crate::collection_worker::engines())
                {
                    return Err(Error::Unsupported);
                }
                let NativeMaterials { image, tools } = native.ok_or(Error::Unavailable)?;
                let input = crate::collection_worker::Input {
                    read: crate::collection_worker::ReadPlan::periodic(
                        p,
                        (deadline / 1000) as i64,
                    )?,

                    image_sha256: image.artifact.sha256.clone(),
                    run_as: run_as.clone(),
                    session: session.clone(),
                    tools: tools.clone(),
                    observed: Default::default(),
                };
                let bytes = serde_json::to_vec(&input).map_err(|_| Error::Protocol)?;
                if bytes.len() as u64 > crate::collection_worker::INPUT_BYTES {
                    return Err(Error::Capacity);
                }
                use sha2::{Digest as _, Sha256};
                let hash = format!("{:x}", Sha256::digest(&bytes));
                let artifact = ExactArtifactRef {
                    resource: reference("periodic-native-input", &hash)?,
                    sha256: Digest::new(hash).map_err(|_| Error::Protocol)?,
                };
                let content = core
                    .material_root
                    .publish(
                        None,
                        std::io::Cursor::new(&bytes),
                        &artifact.sha256,
                        bytes.len() as u64,
                    )
                    .map_err(crate::error::app_error)?;
                parameters.insert(
                    "collection-tools".into(),
                    InputValue::Literal {
                        value: serde_json::to_value(tools).map_err(|_| Error::Protocol)?,
                    },
                );
                let launch = LaunchSpec {
                    artifact,
                    interpreter: InterpreterRef {
                        artifact: image.artifact,
                        profile: reference(crate::collection_worker::PROFILE, "1")?,
                    },
                    argv: vec![
                        LaunchArg::Literal {
                            value: "--collection-worker".into(),
                        },
                        LaunchArg::ArtifactPath {},
                    ],
                    artifact_encoding: ArtifactEncoding::Utf8,
                    stdin: StandardInput::Closed {},
                    output: OutputSpec {
                        format: OutputFormat::Json { max_rows: 1 },
                        stdout: TextEncoding::Utf8,
                        stderr: TextEncoding::Utf8,
                    },
                    cwd,
                    env: Default::default(),
                };
                (
                    launch,
                    Artifacts {
                        program: vec![],
                        delegate: None,
                        interpreter: PathBuf::from(image.path),
                        content,
                        work_root: core.config.work_root.clone(),
                        controlled_input: None,
                    },
                )
            }
        };
        parameters.insert(
            "rss-periodic-collection".into(),
            InputValue::Literal {
                value: serde_json::to_value(p).map_err(|_| Error::Protocol)?,
            },
        );
        let policy = reference(p.policy.to_string(), p.policy_version.to_string())?;
        let frozen = FrozenExecution::freeze(
            ExecutionInput {
                schema_version: V1,
                execution: ExecutionSpec::Process {},
                request: ExecutionRequest {
                    schema_version: V1,
                    request_id: RequestId::new(uuid::Uuid::new_v4().to_string())
                        .map_err(|_| Error::Protocol)?,
                    authority: core.host.binding.authority.clone(),
                    actor: core.host.actor.clone(),
                    initiator: Initiator::Policy {
                        policy: policy.clone(),
                    },
                    delegation: None,
                    target: Target {
                        device: core.host.binding.device.clone(),
                        platform,
                        scope: TargetScope::Device {},
                    },
                    operation: Operation {
                        action: id("telemetry-sample")?,
                        resource: reference(&p.template.id, &p.template.version)?,
                    },
                    parameters,
                },
                launch,
                run_as,
                session_requirement: session,
                constraints: IsolationPolicy::OsIdentity {},
                budget,
                validity: ValidityWindow {
                    not_before_unix_ms: now_ms,
                    expires_at_unix_ms: deadline,
                },
                policy,
            },
            &plan::storage_limits().input,
        )
        .map_err(|_| Error::Protocol)?;
        let leases = vec![execution_runner::staging::verify_retained(
            &artifacts.content,
            &frozen.spec().launch.artifact.sha256,
        )
        .map_err(crate::error::app_error)?];
        let input_digest = frozen.spec().launch.artifact.sha256.clone();
        self.runner
            .start(&configuration, frozen, artifacts, now_ms)
            .map_err(crate::error::app_error)?;
        self.active = Some(Active {
            configuration,
            observed,
            input_digest,
            _leases: leases,
        });
        Ok(())
    }
}

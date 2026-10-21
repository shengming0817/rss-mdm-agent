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
        })
    }
    pub fn active(&self) -> bool {
        self.runner.active()
    }
    pub fn configuration(&self) -> Option<&Configuration> {
        self.active.as_ref().map(|v| &v.configuration)
    }
    pub fn cancel(&self) -> Result<(), Error> {
        self.runner.cancel().map_err(crate::error::app_error)
    }
    pub fn shutdown(&mut self) -> Result<(), Error> {
        self.runner.shutdown().map_err(crate::error::app_error)
    }
    pub fn take(&mut self) -> Result<Option<(Active, ProcessEvidence)>, Error> {
        let Some(evidence) = self.runner.take().map_err(crate::error::app_error)? else {
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
        let p = configuration.payload();
        let now_ms = u64::try_from(observed / 1000).map_err(|_| Error::Clock)?;
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
                crate::collection_worker::preflight(read)?;
                let image = plan::pin(&std::env::current_exe()?)?;
                let tools = crate::collection_worker::tools(&core.config, read)?;
                let input = crate::collection_worker::Input {
                    read: crate::collection_worker::ReadPlan {
                        plan: read.clone(),
                        platform: read.platform(),
                        deadline: (deadline / 1000) as i64,
                        execution: w::NativeExecutionBinding {
                            identity: w::ExecutionIdentity::System,
                            context_revision: 1,
                            target: w::SoftwareExecutionTarget::Device,
                            generated_uid: None,
                        },
                    },
                    finish_by_unix_ms: deadline,
                    image_sha256: image.artifact.sha256.clone(),
                    run_as: run_as.clone(),
                    session: session.clone(),
                    tools: tools.clone(),
                    observed: Default::default(),
                };
                let bytes = serde_json::to_vec(&input).map_err(|_| Error::Protocol)?;
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
        let policy = reference(p.policy.to_string(), p.policy_version.to_string())?;
        let frozen = FrozenExecution::freeze(
            ExecutionInput {
                schema_version: V8,
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

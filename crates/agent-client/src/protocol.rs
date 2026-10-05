//! Client-side validation, encoding and evidence interpretation for generated API values.
//! ref: rss-mdm Agent V1 API at 149c34d5 (encoding and coordinate semantics).
use crate::Error;
use agent_api::runtime::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Canonical structured output budget.
pub const OUTPUT_MAX_BYTES: usize = 16 * 1024 * 1024;
/// Fixed raw bytes in one nonfinal output chunk.
pub const OUTPUT_CHUNK_BYTES: usize = 256 * 1024;
/// Maximum task event wire budget.
pub const MAX_TASK_REQUEST_BYTES: usize = 1_114_112;
/// Maximum independent cancellation page.
pub const MAX_TASK_CANCELLATIONS: usize = 128;
/// Maximum stdout/stderr diagnostic bytes.
pub const MAX_TASK_DIAGNOSTIC_BYTES: usize = 16 * 1024;
/// Maximum recorded execution duration.
pub const MAX_TASK_DURATION_MS: u64 = 604_800_000;
/// Validate generated protocol shape at the client boundary.
pub trait ValidateApi: serde::Serialize {
    /// Validate schema and client invariants.
    fn validate(&self) -> Result<(), Error>
    where
        Self: Sized,
    {
        agent_api::validate(self).map_err(|_| Error::Protocol)
    }
}
impl ValidateApi for TaskSpec {
    fn validate(&self) -> Result<(), Error> {
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        self.authorization.validate(self.tenant_id)?;
        if self.deadline < self.expires_at
            || self.parameters.len() > 32
            || serde_json::to_vec(&self.parameters)
                .map_err(|_| Error::Protocol)?
                .len()
                > 65_536
            || self.parameters.values().any(|v| {
                !(v.is_string() || v.is_boolean() || v.as_i64().is_some())
                    || v.as_str().is_some_and(|s| s.contains('\0'))
            })
            || [
                &self.resource.id,
                &self.resource.version,
                &self.resource.variant,
            ]
            .iter()
            .any(|s| {
                s.is_empty()
                    || s.len() > 128
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
                    || s.split('/').any(|p| p.is_empty() || p == "." || p == "..")
            })
        {
            return Err(Error::Protocol);
        }
        if self.wire_version != WIRE_VERSION
            || self.tenant_id.is_nil()
            || self.registration_id.is_nil()
            || self.task_id.is_nil()
            || self.attempt_id.is_nil()
            || self.generation == 0
            || self.generation > i64::MAX as u64
            || matches!(
                (self.profile, self.platform),
                (ExecutorProfile::PowerShell7, TaskPlatform::Macos)
                    | (
                        ExecutorProfile::PosixSh | ExecutorProfile::Bash,
                        TaskPlatform::Windows
                    )
            )
            || self.device_id.is_empty()
            || self.device_id.len() > 1024
            || self.device_id.chars().any(char::is_control)
            || self.expires_at < 0
            || self.content.length == 0
            || self.content.length > 16_777_216
            || !(1..=3600).contains(&self.timeout_seconds)
            || !(1..=16_777_216).contains(&self.output_bytes)
            || self.max_rows == 0
            || self.arguments.len() > 64
            || self.environment.len() > 32
            || self
                .arguments
                .iter()
                .chain(self.environment.values())
                .any(|v| v.len() > 65_536 || v.contains('\0'))
            || self.environment.keys().any(|key| {
                !key.starts_with("RSS_PARAM_")
                    || key.len() <= 10
                    || key.len() > 64
                    || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
            || (self.profile == ExecutorProfile::Osquery
                && (!self.arguments.is_empty()
                    || !self.environment.is_empty()
                    || self.sql_parameters.as_ref().is_none_or(|p| {
                        p.len() > 32
                            || !serde_json::to_vec(p).is_ok_and(|b| b.len() <= 65536)
                            || p.iter().any(|(k, v)| {
                                k.is_empty()
                                    || k.len() > 64
                                    || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                                    || match v {
                                        serde_json::Value::String(s) => {
                                            s.len() > 4096 || s.contains('\0')
                                        }
                                        serde_json::Value::Bool(_) => false,
                                        serde_json::Value::Number(n) => n.as_i64().is_none(),
                                        _ => true,
                                    }
                            })
                    })
                    || self.run_as != ExecutionIdentity::System))
            || (self.profile != ExecutorProfile::Osquery && self.sql_parameters.is_some())
        {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}
impl ValidateApi for SoftwareTaskSpec {
    fn validate(&self) -> Result<(), Error> {
        self.authorization.validate(self.tenant_id)?;
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        let valid_id = |s: &str, max: usize| {
            !s.is_empty()
                && s.len() <= max
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
                && s.split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
        };
        if self.wire_version != WIRE_VERSION
            || self.tenant_id.is_nil()
            || self.registration_id.is_nil()
            || self.task_id.is_nil()
            || self.attempt_id.is_nil()
            || self.generation == 0
            || self.generation > i64::MAX as u64
            || self.expires_at < 0
            || self.device_id.is_empty()
            || self.device_id.len() > 1024
            || self.device_id.chars().any(char::is_control)
            || self.steps.is_empty()
            || self.steps.len() > 32
        {
            return Err(Error::Protocol);
        }
        self.execution_context.validate_for(self.platform)?;
        let encoded = serde_json::to_vec(&self.steps).map_err(|_| Error::Protocol)?;
        if encoded.len() > 4_194_304
            || ring::digest::digest(&ring::digest::SHA256, &encoded).as_ref()
                != self.definition_digest
        {
            return Err(Error::Protocol);
        }
        let mut keys = std::collections::BTreeSet::new();
        for (index, step) in self.steps.iter().enumerate() {
            step.action.validate()?;
            if step
                .action
                .execution_target(self.platform, &self.execution_context)?
                != step.target
                || matches ! (& step . action . behavior , SoftwareTaskBehavior :: Msix (n) if matches ! (n . deployment , SoftwareTaskMsixDeployment::TargetUserRegistration(SoftwareTaskUser::ActiveInteractive)))
            {
                return Err(Error::Protocol);
            }
            crate::software_validation::validate_step(
                step,
                index,
                self.platform,
                self.architecture,
            )?;
            step.export
                .validate_for(step, self.tenant_id, &self.execution_context)?;
            if step.artifacts.is_empty() || keys.len() + step.artifacts.len() > 64 {
                return Err(Error::Protocol);
            }
            for artifact in &step.artifacts {
                if !valid_id(&artifact.key, 132)
                    || !artifact.key.starts_with(&format!("{index}/"))
                    || !keys.insert(&artifact.key)
                    || artifact.length == 0
                    || artifact.length > 1_099_511_627_776
                {
                    return Err(Error::Protocol);
                }
            }
        }
        Ok(())
    }
}
impl ValidateApi for TaskPayload {}
impl ValidateApi for SoftwareTaskAction {
    fn validate(&self) -> Result<(), Error> {
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        crate::software_validation::validate_action(self)
    }
}
impl ValidateApi for TaskDiagnostics {
    fn validate(&self) -> Result<(), Error> {
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        if self.stdout.len() > MAX_TASK_DIAGNOSTIC_BYTES
            || self.stderr.len() > MAX_TASK_DIAGNOSTIC_BYTES
            || self.stdout.contains('\0')
            || self.stderr.contains('\0')
        {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}
impl ValidateApi for TaskResult {
    fn validate(&self) -> Result<(), Error> {
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        self.diagnostics.validate()?;
        validate_result(
            self.exit_code,
            self.quality,
            &self.output,
            &self.diagnostics,
            self.collected_at,
        )
    }
}
impl ValidateApi for TaskEventRequest {}
impl ValidateApi for TaskClaimRequest {}
impl ValidateApi for SoftwareExecutionContext {
    fn validate(&self) -> Result<(), Error> {
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        if self.revision == 0
            || self.revision > i64::MAX as u64
            || self.os_version == [0; 4]
            || self.source_credentials.len() > 16
        {
            return Err(Error::Protocol);
        }
        let valid = |s: &str, max: usize| {
            !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)
        };
        if self
            .interactive_user
            .as_ref()
            .is_some_and(|u| !valid(&u.identity, 184) || u.session_id.is_nil())
        {
            return Err(Error::Protocol);
        }
        let mut seen = std::collections::BTreeSet::new();
        for c in &self.source_credentials {
            if !valid(&c.source, 128)
                || !valid(&c.reference, 128)
                || !seen.insert((&c.source, &c.reference))
            {
                return Err(Error::Protocol);
            }
        }
        Ok(())
    }
}
impl ValidateApi for SoftwareDetectionObservation {}
impl ValidateApi for SoftwareTaskResult {
    fn validate(&self) -> Result<(), Error> {
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        if self.definition_digest == [0; 32] || self.steps.is_empty() || self.steps.len() > 32 {
            return Err(Error::Protocol);
        }
        for (index, result) in self.steps.iter().enumerate() {
            if result.index as usize != index
                || result.step_digest == [0; 32]
                || result.package.is_empty()
                || result.package.len() > 1024
                || result.package.chars().any(char::is_control)
            {
                return Err(Error::Protocol);
            }
            if serde_json::to_vec(&result.identity)
                .map_err(|_| Error::Protocol)?
                .len()
                > 8192
            {
                return Err(Error::Protocol);
            }
            result.before.validate()?;
            result.after.validate()?;
            if matches!(&result.target, SoftwareExecutionTarget::User {identity, session_id} if (identity . is_empty () || identity . len () > 184 || identity . chars () . any (char :: is_control) || session_id . is_nil ()) )
            {
                return Err(Error::Protocol);
            }
            if matches ! (result . process , SoftwareProcessObservation :: Failed { failure } if result . diagnostics . failure () != Some (failure))
            {
                return Err(Error::Protocol);
            }
        }
        Ok(())
    }
}
impl ValidateApi for SoftwareTaskExport {}
impl ValidateApi for EnrollmentTaskSpec {}

/// Client operations on the generated OutputManifest value.
pub trait OutputManifestExt: Sized {
    /// new for the current API value.
    fn new(bytes: u32, sha256: [u8; 32]) -> Result<Self, Error>;
    /// bytes for the current API value.
    fn bytes(&self) -> u32;
    /// sha256 for the current API value.
    fn sha256(&self) -> [u8; 32];
    /// count for the current API value.
    fn count(&self) -> usize;
}
impl OutputManifestExt for OutputManifest {
    fn new(bytes: u32, sha256: [u8; 32]) -> Result<Self, Error> {
        if bytes == 0 || bytes as usize > OUTPUT_MAX_BYTES {
            return Err(Error::Protocol);
        }
        Ok(Self { bytes, sha256 })
    }
    fn bytes(&self) -> u32 {
        self.bytes
    }
    fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    fn count(&self) -> usize {
        (self.bytes as usize).div_ceil(OUTPUT_CHUNK_BYTES)
    }
}

/// Client operations on the generated OutputChunk value.
pub trait OutputChunkExt: Sized {
    /// new for the current API value.
    fn new(manifest: OutputManifest, index: u16, bytes: &[u8]) -> Result<Self, Error>;
    /// manifest for the current API value.
    fn manifest(&self) -> &OutputManifest;
    /// index for the current API value.
    fn index(&self) -> u16;
    /// bytes for the current API value.
    fn bytes(&self) -> Result<Vec<u8>, Error>;
}
impl OutputChunkExt for OutputChunk {
    fn new(manifest: OutputManifest, index: u16, bytes: &[u8]) -> Result<Self, Error> {
        let offset = usize::from(index) * OUTPUT_CHUNK_BYTES;
        if offset >= manifest.bytes() as usize
            || bytes.len() != (manifest.bytes() as usize - offset).min(OUTPUT_CHUNK_BYTES)
        {
            return Err(Error::Protocol);
        }
        Ok(Self {
            manifest,
            index,
            data: URL_SAFE_NO_PAD.encode(bytes),
        })
    }
    fn manifest(&self) -> &OutputManifest {
        &self.manifest
    }
    fn index(&self) -> u16 {
        self.index
    }
    fn bytes(&self) -> Result<Vec<u8>, Error> {
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.data)
            .map_err(|_| Error::Protocol)?;
        OutputManifest::new(self.manifest.bytes, self.manifest.sha256)?;
        let offset = usize::from(self.index) * OUTPUT_CHUNK_BYTES;
        if offset >= self.manifest.bytes as usize
            || bytes.len() != (self.manifest.bytes as usize - offset).min(OUTPUT_CHUNK_BYTES)
            || URL_SAFE_NO_PAD.encode(&bytes) != self.data
        {
            return Err(Error::Protocol);
        }
        Ok(bytes)
    }
}

/// Client operations on the generated ChunkedTaskResult value.
pub trait ChunkedTaskResultExt: Sized {
    /// split for the current API value.
    fn split(result: TaskResult) -> Result<(Self, Vec<OutputChunk>), Error>;
    /// manifest for the current API value.
    fn manifest(&self) -> &OutputManifest;
    /// assemble for the current API value.
    fn assemble(&self, chunks: Vec<OutputChunk>) -> Result<TaskResult, Error>;
}
impl ChunkedTaskResultExt for ChunkedTaskResult {
    fn split(result: TaskResult) -> Result<(Self, Vec<OutputChunk>), Error> {
        result.validate()?;
        let bytes = serde_json::to_vec(result.output()).map_err(|_| Error::Protocol)?;
        let manifest = OutputManifest::new(
            bytes.len().try_into().map_err(|_| Error::Protocol)?,
            Sha256::digest(&bytes).into(),
        )?;
        let chunks = bytes
            .chunks(OUTPUT_CHUNK_BYTES)
            .enumerate()
            .map(|(index, bytes)| OutputChunk::new(manifest.clone(), index as u16, bytes))
            .collect::<Result<_, _>>()?;
        Ok((
            Self {
                exit_code: result.exit_code(),
                quality: result.quality(),
                diagnostics: result.diagnostics().clone(),
                collected_at: result.collected_at(),
                manifest,
            },
            chunks,
        ))
    }
    fn manifest(&self) -> &OutputManifest {
        &self.manifest
    }
    fn assemble(&self, mut chunks: Vec<OutputChunk>) -> Result<TaskResult, Error> {
        if chunks.len() != self.manifest.count() {
            return Err(Error::Protocol);
        }
        chunks.sort_by_key(OutputChunk::index);
        let mut bytes = Vec::with_capacity(self.manifest.bytes() as usize);
        for (index, chunk) in chunks.into_iter().enumerate() {
            if usize::from(chunk.index()) != index || chunk.manifest() != &self.manifest {
                return Err(Error::Protocol);
            }
            bytes.extend(chunk.bytes()?);
        }
        if bytes.len() != self.manifest.bytes() as usize
            || <[u8; 32]>::from(Sha256::digest(&bytes)) != self.manifest.sha256()
        {
            return Err(Error::Protocol);
        }
        let output: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
        if serde_json::to_vec(&output).map_err(|_| Error::Protocol)? != bytes {
            return Err(Error::Protocol);
        }
        TaskResult::new(
            self.exit_code,
            self.quality,
            output,
            self.diagnostics.clone(),
            self.collected_at,
        )
    }
}

/// Client operations on the generated Capability value.
pub trait CapabilityExt: Sized {
    /// is software for the current API value.
    fn is_software(&self) -> bool;
    /// as str for the current API value.
    fn as_str(&self) -> &'static str;
}
impl CapabilityExt for Capability {
    fn is_software(&self) -> bool {
        !matches!(
            self,
            Self::InventoryCollectionV1 | Self::TaskExecuteV1 | Self::MdmEnrollmentV1
        )
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::InventoryCollectionV1 => "inventory.collect.v1",
            Self::TaskExecuteV1 => "task.execute.v1",
            Self::SoftwareMsiSystemV1 => "software.msi.system.v1",
            Self::SoftwareMsiUserV1 => "software.msi.user.v1",
            Self::SoftwarePkgSystemV1 => "software.pkg.system.v1",
            Self::SoftwareBundleWindowsSystemV1 => "software.bundle.windows.system.v1",
            Self::SoftwareBundleWindowsUserV1 => "software.bundle.windows.user.v1",
            Self::SoftwareBundleMacosSystemV1 => "software.bundle.macos.system.v1",
            Self::SoftwareBundleMacosUserV1 => "software.bundle.macos.user.v1",
            Self::SoftwareWingetSystemV1 => "software.winget.system.v1",
            Self::SoftwareWingetUserV1 => "software.winget.user.v1",
            Self::SoftwareBrewBottleUserV1 => "software.brew.bottle.user.v1",
            Self::SoftwareExeSystemV1 => "software.exe.system.v1",
            Self::SoftwareExeUserV1 => "software.exe.user.v1",
            Self::SoftwareDmgAppSystemV1 => "software.dmg.app.system.v1",
            Self::SoftwareDmgAppUserV1 => "software.dmg.app.user.v1",
            Self::SoftwareDmgPkgSystemV1 => "software.dmg.pkg.system.v1",
            Self::SoftwareMsixRegistrationUserV1 => "software.msix.registration.user.v1",
            Self::SoftwareMsixProvisioningSystemV1 => "software.msix.provisioning.system.v1",
            Self::MdmEnrollmentV1 => "mdm.enrollment.v1",
        }
    }
}

/// Client operations on the generated TaskCancellation value.
pub trait TaskCancellationExt: Sized {
    /// new for the current API value.
    fn new(task_id: Uuid, attempt_id: Uuid) -> Result<Self, Error>;
    /// task id for the current API value.
    fn task_id(&self) -> Uuid;
    /// attempt id for the current API value.
    fn attempt_id(&self) -> Uuid;
}
impl TaskCancellationExt for TaskCancellation {
    fn new(task_id: Uuid, attempt_id: Uuid) -> Result<Self, Error> {
        if task_id.is_nil() || attempt_id.is_nil() {
            return Err(Error::Protocol);
        }
        Ok(Self {
            task_id,
            attempt_id,
        })
    }
    fn task_id(&self) -> Uuid {
        self.task_id
    }
    fn attempt_id(&self) -> Uuid {
        self.attempt_id
    }
}

/// Client operations on the generated TaskClaimResponse value.
pub trait TaskClaimResponseExt: Sized {
    /// new for the current API value.
    fn new(task: Option<SignedTask>, cancellations: Vec<TaskCancellation>) -> Result<Self, Error>;
    /// into task for the current API value.
    fn into_task(self) -> Option<SignedTask>;
    /// cancellations for the current API value.
    fn cancellations(&self) -> &[TaskCancellation];
}
impl TaskClaimResponseExt for TaskClaimResponse {
    fn new(task: Option<SignedTask>, cancellations: Vec<TaskCancellation>) -> Result<Self, Error> {
        if cancellations.len() > MAX_TASK_CANCELLATIONS {
            return Err(Error::Protocol);
        }
        Ok(Self {
            wire_version: WIRE_VERSION,
            task,
            cancellations,
        })
    }
    fn into_task(self) -> Option<SignedTask> {
        self.task
    }
    fn cancellations(&self) -> &[TaskCancellation] {
        &self.cancellations
    }
}

/// Client operations on the generated TaskEventAck value.
pub trait TaskEventAckExt: Sized {
    /// new for the current API value.
    fn new(permit: Option<SignedTask>, cancel_requested: bool) -> Self;
    /// accepted for the current API value.
    fn accepted(&self) -> bool;
    /// permit for the current API value.
    fn permit(&self) -> Option<&SignedTask>;
    /// into permit for the current API value.
    fn into_permit(self) -> Option<SignedTask>;
    /// cancel requested for the current API value.
    fn cancel_requested(&self) -> bool;
}
impl TaskEventAckExt for TaskEventAck {
    fn new(permit: Option<SignedTask>, cancel_requested: bool) -> Self {
        Self {
            wire_version: WIRE_VERSION,
            accepted: true,
            permit,
            cancel_requested,
        }
    }
    fn accepted(&self) -> bool {
        self.accepted
    }
    fn permit(&self) -> Option<&SignedTask> {
        self.permit.as_ref()
    }
    fn into_permit(self) -> Option<SignedTask> {
        self.permit
    }
    fn cancel_requested(&self) -> bool {
        self.cancel_requested
    }
}

/// Client operations on the generated TaskDiagnostics value.
pub trait TaskDiagnosticsExt: Sized {
    /// new for the current API value.
    fn new(
        stdout: String,
        stderr: String,
        duration_ms: u64,
        executed_at: i64,
        failure: Option<TaskFailure>,
    ) -> Result<Self, Error>;
    /// stdout for the current API value.
    fn stdout(&self) -> &str;
    /// stderr for the current API value.
    fn stderr(&self) -> &str;
    /// duration ms for the current API value.
    fn duration_ms(&self) -> u64;
    /// executed at for the current API value.
    fn executed_at(&self) -> i64;
    /// failure for the current API value.
    fn failure(&self) -> Option<TaskFailure>;
}
impl TaskDiagnosticsExt for TaskDiagnostics {
    fn new(
        stdout: String,
        stderr: String,
        duration_ms: u64,
        executed_at: i64,
        failure: Option<TaskFailure>,
    ) -> Result<Self, Error> {
        if stdout.len() > MAX_TASK_DIAGNOSTIC_BYTES
            || stderr.len() > MAX_TASK_DIAGNOSTIC_BYTES
            || stdout.contains('\0')
            || stderr.contains('\0')
            || duration_ms > MAX_TASK_DURATION_MS
            || executed_at < 1
        {
            return Err(Error::Protocol);
        }
        Ok(Self {
            stdout,
            stderr,
            duration_ms,
            executed_at,
            failure,
        })
    }
    fn stdout(&self) -> &str {
        &self.stdout
    }
    fn stderr(&self) -> &str {
        &self.stderr
    }
    fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
    fn executed_at(&self) -> i64 {
        self.executed_at
    }
    fn failure(&self) -> Option<TaskFailure> {
        self.failure
    }
}

/// Client operations on the generated TaskResult value.
pub trait TaskResultExt: Sized {
    /// new for the current API value.
    fn new(
        exit_code: Option<i32>,
        quality: OutputQuality,
        output: Value,
        diagnostics: TaskDiagnostics,
        collected_at: i64,
    ) -> Result<Self, Error>;
    /// collected at for the current API value.
    fn collected_at(&self) -> i64;
    /// exit code for the current API value.
    fn exit_code(&self) -> Option<i32>;
    /// quality for the current API value.
    fn quality(&self) -> OutputQuality;
    /// output for the current API value.
    fn output(&self) -> &Value;
    /// diagnostics for the current API value.
    fn diagnostics(&self) -> &TaskDiagnostics;
}
impl TaskResultExt for TaskResult {
    fn new(
        exit_code: Option<i32>,
        quality: OutputQuality,
        output: Value,
        diagnostics: TaskDiagnostics,
        collected_at: i64,
    ) -> Result<Self, Error> {
        diagnostics.validate()?;
        validate_result(exit_code, quality, &output, &diagnostics, collected_at)?;
        Ok(Self {
            exit_code,
            quality,
            output,
            diagnostics,
            collected_at,
        })
    }
    fn collected_at(&self) -> i64 {
        self.collected_at
    }
    fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
    fn quality(&self) -> OutputQuality {
        self.quality
    }
    fn output(&self) -> &Value {
        &self.output
    }
    fn diagnostics(&self) -> &TaskDiagnostics {
        &self.diagnostics
    }
}

/// Client operations on the generated TaskEventRequest value.
pub trait TaskEventRequestExt: Sized {
    /// new for the current API value.
    fn new(
        operation_id: Uuid,
        attempt_id: Uuid,
        event: TaskEvent,
        execution_context: SoftwareExecutionContext,
    ) -> Result<Self, Error>;
    /// execution context for the current API value.
    fn execution_context(&self) -> &SoftwareExecutionContext;
    /// operation id for the current API value.
    fn operation_id(&self) -> Uuid;
    /// attempt id for the current API value.
    fn attempt_id(&self) -> Uuid;
    /// event for the current API value.
    fn event(&self) -> &TaskEvent;
}
impl TaskEventRequestExt for TaskEventRequest {
    fn new(
        operation_id: Uuid,
        attempt_id: Uuid,
        event: TaskEvent,
        execution_context: SoftwareExecutionContext,
    ) -> Result<Self, Error> {
        execution_context.validate()?;
        if operation_id.is_nil() || attempt_id.is_nil() {
            return Err(Error::Protocol);
        }
        match &event {
            TaskEvent::SoftwareResult(result) => result.validate()?,
            TaskEvent::Result(result) => result.validate()?,
            TaskEvent::OutputChunk(chunk) => {
                chunk.bytes()?;
            }
            TaskEvent::ChunkedResult(result) => {
                OutputManifest::new(result.manifest.bytes, result.manifest.sha256)?;
                result.diagnostics.validate()?;
            }
            _ => {}
        }
        let input = Self {
            wire_version: WIRE_VERSION,
            operation_id,
            attempt_id,
            event,
            execution_context,
        };
        if serde_json::to_vec(&input)
            .map_err(|_| Error::Protocol)?
            .len()
            > MAX_TASK_REQUEST_BYTES
        {
            return Err(Error::Protocol);
        }
        Ok(input)
    }
    fn execution_context(&self) -> &SoftwareExecutionContext {
        &self.execution_context
    }
    fn operation_id(&self) -> Uuid {
        self.operation_id
    }
    fn attempt_id(&self) -> Uuid {
        self.attempt_id
    }
    fn event(&self) -> &TaskEvent {
        &self.event
    }
}

/// Client operations on the generated TaskClaimRequest value.
pub trait TaskClaimRequestExt: Sized {
    /// new for the current API value.
    fn new(
        operation_id: Uuid,
        profiles: Vec<ExecutorProfile>,
        execution_context: SoftwareExecutionContext,
    ) -> Result<Self, Error>;
    /// profiles for the current API value.
    fn profiles(&self) -> &[ExecutorProfile];
    /// execution context for the current API value.
    fn execution_context(&self) -> &SoftwareExecutionContext;
    /// operation id for the current API value.
    fn operation_id(&self) -> Uuid;
}
impl TaskClaimRequestExt for TaskClaimRequest {
    fn new(
        operation_id: Uuid,
        profiles: Vec<ExecutorProfile>,
        execution_context: SoftwareExecutionContext,
    ) -> Result<Self, Error> {
        execution_context.validate()?;
        if operation_id.is_nil()
            || profiles.len() > 4
            || profiles
                .iter()
                .enumerate()
                .any(|(i, p)| profiles[..i].contains(p))
        {
            return Err(Error::Protocol);
        }
        Ok(Self {
            wire_version: WIRE_VERSION,
            operation_id,
            profiles,
            execution_context,
        })
    }
    fn profiles(&self) -> &[ExecutorProfile] {
        &self.profiles
    }
    fn execution_context(&self) -> &SoftwareExecutionContext {
        &self.execution_context
    }
    fn operation_id(&self) -> Uuid {
        self.operation_id
    }
}

/// Client operations on the generated SoftwareTaskAction value.
pub trait SoftwareTaskActionExt: Sized {
    /// required profile for the current API value.
    fn required_profile(&self, platform: TaskPlatform) -> Option<Capability>;
    /// execution target for the current API value.
    fn execution_target(
        &self,
        platform: TaskPlatform,
        context: &SoftwareExecutionContext,
    ) -> Result<SoftwareExecutionTarget, Error>;
    /// bind execution target for the current API value.
    fn bind_execution_target(
        &mut self,
        platform: TaskPlatform,
        context: &SoftwareExecutionContext,
    ) -> Result<SoftwareExecutionTarget, Error>;
    /// observed identity for the current API value.
    fn observed_identity(&self) -> SoftwareObservedIdentity;
    /// detected version for the current API value.
    fn detected_version(&self) -> String;
}
impl SoftwareTaskActionExt for SoftwareTaskAction {
    fn required_profile(&self, platform: TaskPlatform) -> Option<Capability> {
        use Capability as C;
        use SoftwareTaskBehavior as B;
        use SoftwareTaskScope as S;
        if matches!(
            &self.behavior,
            B::Msi(_) | B::Winget(_) | B::Exe(_) | B::Msix(_)
        ) && platform != TaskPlatform::Windows
            || matches!(&self.behavior, B::Pkg(_) | B::Brew(_) | B::Dmg(_))
                && platform != TaskPlatform::Macos
        {
            return None;
        }
        let native = |scope: S, system: C, user: C| {
            Some(match scope {
                S::System => system,
                S::User => user,
            })
        };
        match &self.behavior {
            B::Msi(n) => native(n.scope, C::SoftwareMsiSystemV1, C::SoftwareMsiUserV1),
            B::Pkg(n) if n.scope == S::System => Some(C::SoftwarePkgSystemV1),
            B::Pkg(_) => None,
            B::Winget(n) => native(n.scope, C::SoftwareWingetSystemV1, C::SoftwareWingetUserV1),
            B::Brew(n) if n.scope == S::User => Some(C::SoftwareBrewBottleUserV1),
            B::Brew(_) => None,
            B::Exe(n) => native(n.scope, C::SoftwareExeSystemV1, C::SoftwareExeUserV1),
            B::Bundle(n) => Some(match (platform, n.install.invocation.run_as) {
                (TaskPlatform::Windows, ExecutionIdentity::System) => {
                    C::SoftwareBundleWindowsSystemV1
                }
                (TaskPlatform::Windows, ExecutionIdentity::LoggedInUser) => {
                    C::SoftwareBundleWindowsUserV1
                }
                (TaskPlatform::Macos, ExecutionIdentity::System) => C::SoftwareBundleMacosSystemV1,
                (TaskPlatform::Macos, ExecutionIdentity::LoggedInUser) => {
                    C::SoftwareBundleMacosUserV1
                }
            }),
            B::Dmg(n) => match (&n.payload, n.scope) {
                (SoftwareTaskDmgPayload::AppCopy { .. }, S::System) => {
                    Some(C::SoftwareDmgAppSystemV1)
                }
                (SoftwareTaskDmgPayload::AppCopy { .. }, S::User) => Some(C::SoftwareDmgAppUserV1),
                (SoftwareTaskDmgPayload::ContainedPkg { .. }, S::System) => {
                    Some(C::SoftwareDmgPkgSystemV1)
                }
                _ => None,
            },
            B::Msix(n) => Some(match n.deployment {
                SoftwareTaskMsixDeployment::TargetUserRegistration(_) => {
                    C::SoftwareMsixRegistrationUserV1
                }
                SoftwareTaskMsixDeployment::DeviceProvisioning => {
                    C::SoftwareMsixProvisioningSystemV1
                }
            }),
        }
    }
    fn execution_target(
        &self,
        platform: TaskPlatform,
        context: &SoftwareExecutionContext,
    ) -> Result<SoftwareExecutionTarget, Error> {
        context.validate_for(platform)?;
        if self.required_profile(platform).is_none() {
            return Err(Error::Protocol);
        }
        use SoftwareTaskBehavior as B;
        let invocation = match &self.behavior {
            B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => &n.install,
            B::Exe(n) => &n.install,
            B::Bundle(n) => &n.install.invocation,
            B::Dmg(n) => &n.invocation,
            B::Msix(n) => &n.invocation,
        };
        if matches!(invocation.run_as, ExecutionIdentity::System) && !context.system_broker {
            return Err(Error::Protocol);
        }
        if matches!(&self.behavior, B::Brew(_))
            && context
                .interactive_user
                .as_ref()
                .is_none_or(|u| u.identity.parse::<u32>().ok().is_none_or(|uid| uid == 0))
        {
            return Err(Error::Protocol);
        }
        let needs_user = invocation.run_as == ExecutionIdentity::LoggedInUser;
        let target = if needs_user {
            let user = context.interactive_user.as_ref().ok_or(Error::Protocol)?;
            SoftwareExecutionTarget::User {
                identity: user.identity.clone(),
                session_id: user.session_id,
            }
        } else {
            SoftwareExecutionTarget::Device
        };
        if matches ! (& self . behavior , B :: Winget (n) if n . install . run_as == ExecutionIdentity :: System)
            && !context
                .interactive_user
                .as_ref()
                .is_some_and(|u| u.administrator)
        {
            return Err(Error::Protocol);
        }
        let detector = match &self.behavior {
            B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => Some(&n.detect),
            B::Exe(n) => Some(&n.detect),
            B::Bundle(n) => Some(&n.detect),
            _ => None,
        };
        if let Some(SoftwareTaskDetection::Script { command }) = detector {
            match command.invocation.run_as {
                ExecutionIdentity::System if !context.system_broker => {
                    return Err(Error::Protocol);
                }
                ExecutionIdentity::LoggedInUser if context.interactive_user.is_none() => {
                    return Err(Error::Protocol);
                }
                _ => (),
            }
        }
        if let B::Msix(n) = &self.behavior {
            if context.os_version < n.minimum_os
                || (n.require_sideload && !context.msix_sideload)
                || (n.allow_unsigned && !context.msix_unsigned)
            {
                return Err(Error::Protocol);
            }
            match &n.deployment {
                SoftwareTaskMsixDeployment::TargetUserRegistration(selector) => {
                    let user = context.interactive_user.as_ref().ok_or(Error::Protocol)?;
                    if matches ! (selector , SoftwareTaskUser::Exact(identity) if identity !=& user . identity)
                        || !needs_user
                    {
                        return Err(Error::Protocol);
                    }
                }
                SoftwareTaskMsixDeployment::DeviceProvisioning if needs_user => {
                    return Err(Error::Protocol);
                }
                _ => (),
            }
        }
        Ok(target)
    }
    fn bind_execution_target(
        &mut self,
        platform: TaskPlatform,
        context: &SoftwareExecutionContext,
    ) -> Result<SoftwareExecutionTarget, Error> {
        let target = self.execution_target(platform, context)?;
        if let (SoftwareTaskBehavior::Msix(n), SoftwareExecutionTarget::User { identity, .. }) =
            (&mut self.behavior, &target)
        {
            if let SoftwareTaskMsixDeployment::TargetUserRegistration(target) = &mut n.deployment {
                *target = SoftwareTaskUser::Exact(identity.clone());
            }
        }
        Ok(target)
    }
    fn observed_identity(&self) -> SoftwareObservedIdentity {
        use SoftwareTaskBehavior as B;
        let detection = |v: &SoftwareTaskDetection| match v {
            SoftwareTaskDetection::MsiProduct { product_code, .. } => {
                SoftwareObservedIdentity::MsiProduct {
                    product_code: product_code.clone(),
                }
            }
            SoftwareTaskDetection::PkgReceipt { receipt, .. } => {
                SoftwareObservedIdentity::PkgReceipt {
                    receipt: receipt.clone(),
                }
            }
            SoftwareTaskDetection::Registry {
                scope, key, value, ..
            } => SoftwareObservedIdentity::Registry {
                scope: *scope,
                key: key.clone(),
                value: value.clone(),
            },
            SoftwareTaskDetection::File { scope, path, .. } => SoftwareObservedIdentity::File {
                scope: *scope,
                path: path.clone(),
            },
            SoftwareTaskDetection::Script { command } => {
                SoftwareObservedIdentity::ControlledDetector {
                    entry: command.entry.clone(),
                }
            }
        };
        match &self.behavior {
            B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => detection(&n.detect),
            B::Exe(n) => detection(&n.detect),
            B::Bundle(n) => detection(&n.detect),
            B::Dmg(n) => match &n.payload {
                SoftwareTaskDmgPayload::AppCopy { application, .. } => {
                    SoftwareObservedIdentity::MacApplication {
                        bundle_id: application.bundle_id.clone(),
                        target_name: application.target_name.clone(),
                    }
                }
                SoftwareTaskDmgPayload::ContainedPkg { receipt, .. } => {
                    SoftwareObservedIdentity::PkgReceipt {
                        receipt: receipt.clone(),
                    }
                }
            },
            B::Msix(n) => match n.deployment {
                SoftwareTaskMsixDeployment::TargetUserRegistration(_) => {
                    SoftwareObservedIdentity::MsixRegistration {
                        identity: n.identity.clone(),
                    }
                }
                SoftwareTaskMsixDeployment::DeviceProvisioning => {
                    SoftwareObservedIdentity::MsixProvisioning {
                        identity: n.identity.clone(),
                    }
                }
            },
        }
    }
    fn detected_version(&self) -> String {
        use SoftwareTaskBehavior as B;
        let detection = |d: &SoftwareTaskDetection| match d {
            SoftwareTaskDetection::MsiProduct { version, .. }
            | SoftwareTaskDetection::PkgReceipt { version, .. }
            | SoftwareTaskDetection::Registry { version, .. }
            | SoftwareTaskDetection::File { version, .. } => version.clone(),
            SoftwareTaskDetection::Script { .. } => self.version.clone(),
        };
        match &self.behavior {
            B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => detection(&n.detect),
            B::Exe(n) => detection(&n.detect),
            B::Bundle(n) => detection(&n.detect),
            B::Dmg(n) => match &n.payload {
                SoftwareTaskDmgPayload::AppCopy { application, .. } => application.version.clone(),
                _ => self.version.clone(),
            },
            B::Msix(n) => n
                .identity
                .version
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join("."),
        }
    }
}

/// Client operations on the generated TaskPayload value.
pub trait TaskPayloadExt: Sized {
    /// task id for the current API value.
    fn task_id(&self) -> Uuid;
    /// attempt id for the current API value.
    fn attempt_id(&self) -> Uuid;
    /// expires at for the current API value.
    fn expires_at(&self) -> i64;
    /// permit for the current API value.
    fn permit(&self) -> TaskPermit;
    /// platform for the current API value.
    fn platform(&self) -> TaskPlatform;
    /// architecture for the current API value.
    fn architecture(&self) -> TaskArchitecture;
    /// signing bytes for the current API value.
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error>;
}
impl TaskPayloadExt for TaskPayload {
    fn task_id(&self) -> Uuid {
        match self {
            Self::TaskSpec(v) => v.task_id,
            Self::SoftwareTaskSpec(v) => v.task_id,
            Self::EnrollmentTaskSpec(v) => v.task_id,
        }
    }
    fn attempt_id(&self) -> Uuid {
        match self {
            Self::TaskSpec(v) => v.attempt_id,
            Self::SoftwareTaskSpec(v) => v.attempt_id,
            Self::EnrollmentTaskSpec(v) => v.attempt_id,
        }
    }
    fn expires_at(&self) -> i64 {
        match self {
            Self::TaskSpec(v) => v.expires_at,
            Self::SoftwareTaskSpec(v) => v.expires_at,
            Self::EnrollmentTaskSpec(v) => v.expires_at,
        }
    }
    fn permit(&self) -> TaskPermit {
        match self {
            Self::TaskSpec(v) => v.permit,
            Self::SoftwareTaskSpec(v) => v.permit,
            Self::EnrollmentTaskSpec(v) => v.permit,
        }
    }
    fn platform(&self) -> TaskPlatform {
        match self {
            Self::TaskSpec(v) => v.platform,
            Self::SoftwareTaskSpec(v) => v.platform,
            Self::EnrollmentTaskSpec(v) => v.platform,
        }
    }
    fn architecture(&self) -> TaskArchitecture {
        match self {
            Self::TaskSpec(v) => v.architecture,
            Self::SoftwareTaskSpec(v) => v.architecture,
            Self::EnrollmentTaskSpec(v) => v.architecture,
        }
    }
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error> {
        match self {
            Self::TaskSpec(v) => v.signing_bytes(key_id),
            Self::SoftwareTaskSpec(v) => v.signing_bytes(key_id),
            Self::EnrollmentTaskSpec(v) => v.signing_bytes(key_id),
        }
    }
}

/// Client operations on the generated SoftwareDetectionObservation value.
pub trait SoftwareDetectionObservationExt: Sized {
    /// satisfies for the current API value.
    fn satisfies(&self, intent: SoftwareTaskIntent, version: &str) -> bool;
    /// is unknown for the current API value.
    fn is_unknown(&self) -> bool;
}
impl SoftwareDetectionObservationExt for SoftwareDetectionObservation {
    fn satisfies(&self, intent: SoftwareTaskIntent, version: &str) -> bool {
        match (self, intent) {
            (
                Self::Present {
                    version: observed, ..
                },
                SoftwareTaskIntent::Install | SoftwareTaskIntent::Detect,
            ) => observed == version,
            (Self::Absent { .. }, SoftwareTaskIntent::Uninstall) => true,
            _ => false,
        }
    }
    fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
}

/// Client operations on the generated SoftwareTaskResult value.
pub trait SoftwareTaskResultExt: Sized {
    /// assess for for the current API value.
    fn assess_for(&self, task: &SoftwareTaskSpec) -> Result<SoftwareEvidenceAssessment, Error>;
    /// validate association for the current API value.
    fn validate_association(&self, task: &SoftwareTaskSpec) -> Result<(), Error>;
}
impl SoftwareTaskResultExt for SoftwareTaskResult {
    fn assess_for(&self, task: &SoftwareTaskSpec) -> Result<SoftwareEvidenceAssessment, Error> {
        use SoftwareEvidenceAssessment as A;
        self.validate_association(task)?;
        if self.steps.iter().any(|step| step.after.is_unknown()) {
            return Ok(A::Unknown);
        }
        let reboot = self.steps.iter().any(|step| step.reboot_required);
        let reboot_allowed = self.steps.iter().zip(&task.steps).all(|(result, step)| {
            !result.reboot_required || step.action.reboot == SoftwareTaskReboot::Report
        });
        if reboot {
            return Ok(if reboot_allowed {
                A::RebootPending
            } else {
                A::Unsatisfied
            });
        }
        let matches =
            self.steps
                .iter()
                .zip(&task.steps)
                .enumerate()
                .all(|(index, (result, step))| {
                    let intent = if index + 1 == task.steps.len() {
                        task.intent
                    } else {
                        SoftwareTaskIntent::Install
                    };
                    result
                        .after
                        .satisfies(intent, &step.action.detected_version())
                });
        Ok(if matches {
            A::Satisfied
        } else {
            A::Unsatisfied
        })
    }
    fn validate_association(&self, task: &SoftwareTaskSpec) -> Result<(), Error> {
        self.validate()?;
        if self.definition_digest != task.definition_digest
            || self.intent != task.intent
            || self.steps.len() != task.steps.len()
        {
            return Err(Error::Protocol);
        }
        for (result, step) in self.steps.iter().zip(&task.steps) {
            if result.step_digest != step.digest()?
                || result.target != step.target
                || result.package != step.action.package
                || result.identity != step.action.observed_identity()
            {
                return Err(Error::Protocol);
            }
        }
        Ok(())
    }
}

/// Client operations on the generated SoftwareTaskStep value.
pub trait SoftwareTaskStepExt: Sized {
    /// digest for the current API value.
    fn digest(&self) -> Result<[u8; 32], Error>;
}
impl SoftwareTaskStepExt for SoftwareTaskStep {
    fn digest(&self) -> Result<[u8; 32], Error> {
        Ok(Sha256::digest(serde_json::to_vec(self).map_err(|_| Error::Protocol)?).into())
    }
}

/// Client operations on the generated SoftwareTaskMsixArchitecture value.
pub trait SoftwareTaskMsixArchitectureExt: Sized {
    /// matches for the current API value.
    fn matches(self, architecture: TaskArchitecture) -> bool;
}
impl SoftwareTaskMsixArchitectureExt for SoftwareTaskMsixArchitecture {
    fn matches(self, architecture: TaskArchitecture) -> bool {
        self == Self::Neutral
            || matches!(
                (self, architecture),
                (Self::X86_64, TaskArchitecture::X86_64)
                    | (Self::Aarch64, TaskArchitecture::Aarch64)
            )
    }
}

/// Client operations on the generated SoftwareTaskBehavior value.
pub trait SoftwareTaskBehaviorExt: Sized {
    /// kind for the current API value.
    fn kind(&self) -> &'static str;
    /// installer for the current API value.
    fn installer(&self) -> &str;
    /// bundle for the current API value.
    fn bundle(&self) -> Option<&SoftwareTaskBundle>;
    /// invocation for the current API value.
    fn invocation(&self) -> &SoftwareTaskInvocation;
    /// supports removal for the current API value.
    fn supports_removal(&self) -> bool;
}
impl SoftwareTaskBehaviorExt for SoftwareTaskBehavior {
    fn kind(&self) -> &'static str {
        match self {
            Self::Msi(_) => "msi",
            Self::Pkg(_) => "pkg",
            Self::Bundle(_) => "bundle",
            Self::Winget(_) => "winget",
            Self::Brew(_) => "brew",
            Self::Exe(_) => "exe",
            Self::Dmg(_) => "dmg",
            Self::Msix(_) => "msix",
        }
    }
    fn installer(&self) -> &str {
        match self {
            Self::Msi(n) | Self::Pkg(n) | Self::Winget(n) | Self::Brew(n) => &n.installer,
            Self::Exe(n) => &n.installer,
            Self::Bundle(b) => &b.archive,
            Self::Dmg(d) => &d.image,
            Self::Msix(m) => match &m.container {
                SoftwareTaskMsixContainer::Package { installer }
                | SoftwareTaskMsixContainer::Bundle { installer, .. } => installer,
            },
        }
    }
    fn bundle(&self) -> Option<&SoftwareTaskBundle> {
        if let Self::Bundle(b) = self {
            Some(&b.manifest)
        } else {
            None
        }
    }
    fn invocation(&self) -> &SoftwareTaskInvocation {
        match self {
            Self::Msi(n) | Self::Pkg(n) | Self::Winget(n) | Self::Brew(n) => &n.install,
            Self::Exe(n) => &n.install,
            Self::Bundle(b) => &b.install.invocation,
            Self::Dmg(d) => &d.invocation,
            Self::Msix(m) => &m.invocation,
        }
    }
    fn supports_removal(&self) -> bool {
        match self {
            Self::Msi(n) | Self::Pkg(n) | Self::Winget(n) | Self::Brew(n) => n.uninstall.is_some(),
            Self::Exe(n) => n.uninstall.is_some(),
            Self::Bundle(b) => b.uninstall.is_some(),
            Self::Dmg(d) => match &d.payload {
                SoftwareTaskDmgPayload::AppCopy { uninstall, .. } => *uninstall,
                SoftwareTaskDmgPayload::ContainedPkg { uninstall, .. } => uninstall.is_some(),
            },
            Self::Msix(m) => m.uninstall,
        }
    }
}

/// Client checks for generated TaskSpec values.
pub trait TaskSpecExt: Sized {
    /// signing bytes.
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error>;
}
impl TaskSpecExt for TaskSpec {
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error> {
        self.validate()?;
        if key_id.is_empty()
            || key_id.len() > 128
            || !key_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Error::Protocol);
        }
        let mut bytes = b"rss-mdm-agent-script-task-v1-ed25519\0".to_vec();
        bytes.extend((key_id.len() as u32).to_be_bytes());
        bytes.extend(key_id.as_bytes());
        bytes.extend(serde_json::to_vec(self).map_err(|_| Error::Protocol)?);
        if bytes.len() > 131_072 {
            return Err(Error::Protocol);
        }
        Ok(bytes)
    }
}

/// Client checks for generated SoftwareTaskSpec values.
pub trait SoftwareTaskSpecExt: Sized {
    /// signing bytes.
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error>;
}
impl SoftwareTaskSpecExt for SoftwareTaskSpec {
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error> {
        self.validate()?;
        if key_id.is_empty()
            || key_id.len() > 128
            || !key_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Error::Protocol);
        }
        let mut bytes = b"rss-mdm-agent-software-task-v1-ed25519\0".to_vec();
        bytes.extend((key_id.len() as u32).to_be_bytes());
        bytes.extend(key_id.as_bytes());
        bytes.extend(serde_json::to_vec(self).map_err(|_| Error::Protocol)?);
        Ok(bytes)
    }
}

/// Client checks for generated EnrollmentTaskSpec values.
pub trait EnrollmentTaskSpecExt: Sized {
    /// signing bytes.
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error>;
}
impl EnrollmentTaskSpecExt for EnrollmentTaskSpec {
    fn signing_bytes(&self, key_id: &str) -> Result<Vec<u8>, Error> {
        self.entry.validate(self.platform)?;
        if self.wire_version != WIRE_VERSION
            || self.tenant_id.is_nil()
            || self.organization != self.tenant_id
            || self.registration_id.is_nil()
            || self.task_id.is_nil()
            || self.attempt_id.is_nil()
            || self.generation == 0
            || self.generation > i64::MAX as u64
            || self.expires_at <= 0
            || self.device_id.is_empty()
            || self.device_id.chars().count() > 256
            || self.device_id.chars().any(char::is_control)
            || key_id.is_empty()
            || key_id.len() > 128
            || key_id.chars().any(char::is_control)
        {
            return Err(Error::Protocol);
        }
        let mut bytes = b"rss-mdm-agent-mdm-enrollment-v1-ed25519\0".to_vec();
        bytes.extend(serde_json::to_vec(&(key_id, self)).map_err(|_| Error::Protocol)?);
        Ok(bytes)
    }
}

/// Client checks for generated TaskAuthorization values.
pub trait TaskAuthorizationExt: Sized {
    /// validate.
    fn validate(&self, tenant: Uuid) -> Result<(), Error>;
}
impl TaskAuthorizationExt for TaskAuthorization {
    fn validate(&self, tenant: Uuid) -> Result<(), Error> {
        let valid = match self {
            Self::Policy {
                policy_id,
                policy_version,
            } => !policy_id.is_nil() && !policy_version.is_nil(),
            Self::RemoteOperation { operation_id } => !operation_id.is_nil(),
            Self::SelfService {
                request_id,
                subject,
                source,
                policy_id,
                policy_revision,
                policy_version,
                allow_ai,
                risk_level,
                confirmed,
            } => {
                !request_id.is_nil()
                    && match subject {
                        SelfServiceSubject::Device => true,
                        SelfServiceSubject::User(user) => {
                            user.tenant_id == tenant
                                && !user.instance_id.is_nil()
                                && !user.principal_id.is_nil()
                        }
                    }
                    && !policy_id.is_nil()
                    && *policy_revision > 0
                    && !policy_version.is_nil()
                    && matches!(risk_level, 1 | 2)
                    && (*source != SelfServiceCaller::Ai
                        || (*allow_ai && (*risk_level == 1 || *confirmed)))
            }
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Protocol)
        }
    }
}

/// Client checks for generated SoftwareExecutionContext values.
pub trait SoftwareExecutionContextExt: Sized {
    /// validate for.
    fn validate_for(&self, platform: TaskPlatform) -> Result<(), Error>;
    /// validate update.
    fn validate_update(&self, previous: &Self) -> Result<(), Error>;
}
impl SoftwareExecutionContextExt for SoftwareExecutionContext {
    fn validate_for(&self, platform: TaskPlatform) -> Result<(), Error> {
        self.validate()?;
        if let Some(u) = &self.interactive_user {
            let valid = match platform {
                TaskPlatform::Windows => u.identity.strip_prefix("S-1-").is_some_and(|tail| {
                    tail.split('-').count() >= 2
                        && tail
                            .split('-')
                            .all(|n| !n.is_empty() && n.parse::<u32>().is_ok())
                }),
                TaskPlatform::Macos => {
                    u.identity.parse::<u32>().is_ok()
                        && u.identity.bytes().all(|b| b.is_ascii_digit())
                }
            };
            if !valid {
                return Err(Error::Protocol);
            }
        }
        if platform == TaskPlatform::Macos && (self.msix_sideload || self.msix_unsigned) {
            return Err(Error::Protocol);
        }
        Ok(())
    }
    fn validate_update(&self, previous: &Self) -> Result<(), Error> {
        self.validate()?;
        if self.revision < previous.revision
            || (self.revision == previous.revision && self != previous)
        {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}

/// Client checks for generated SoftwareTaskExport values.
pub trait SoftwareTaskExportExt: Sized {
    /// validate for.
    fn validate_for(
        &self,
        step: &SoftwareTaskStep,
        tenant: Uuid,
        context: &SoftwareExecutionContext,
    ) -> Result<(), Error>;
}
impl SoftwareTaskExportExt for SoftwareTaskExport {
    fn validate_for(
        &self,
        step: &SoftwareTaskStep,
        tenant: Uuid,
        context: &SoftwareExecutionContext,
    ) -> Result<(), Error> {
        use SoftwareTaskBehavior as B;
        let (binding, uri) = match (self, &step.action.behavior) {
            (Self::Direct, B::Winget(_) | B::Brew(_)) => return Err(Error::Protocol),
            (Self::Direct, _) => return Ok(()),
            (
                Self::Winget {
                    binding,
                    uri,
                    identifier,
                },
                B::Winget(_),
            ) => {
                text(identifier, 128)?;
                if !uri.ends_with('/') {
                    return Err(Error::Protocol);
                }
                (binding, uri)
            }
            (
                Self::Brew {
                    binding,
                    uri,
                    commit,
                    tap,
                    credential_reference,
                },
                B::Brew(_),
            ) => {
                if commit.len() != 40
                    || !commit
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    || !uri.ends_with(".git")
                {
                    return Err(Error::Protocol);
                }
                text(tap, 128)?;
                text(credential_reference, 128)?;
                if !context
                    .source_credentials
                    .iter()
                    .any(|c| c.source == binding.source && c.reference == *credential_reference)
                {
                    return Err(Error::Protocol);
                }
                (binding, uri)
            }
            _ => return Err(Error::Protocol),
        };
        https(uri)?;
        text(&binding.source, 128)?;
        text(&binding.resource, 128)?;
        text(&binding.resource_version, 128)?;
        if binding.tenant_id != tenant
            || binding.dependencies.len() > 255
            || binding.artifacts.is_empty()
            || binding.artifacts.len() > 256
            || [
                binding.publication,
                binding.source_digest,
                binding.resource_digest,
                binding.definition_digest,
                binding.document_sha256,
            ]
            .contains(&[0; 32])
        {
            return Err(Error::Protocol);
        }
        let mut coordinates = std::collections::BTreeSet::new();
        for dep in &binding.dependencies {
            text(&dep.resource, 128)?;
            text(&dep.version, 128)?;
            if dep.sha256 == [0; 32] || !coordinates.insert((&dep.resource, &dep.version)) {
                return Err(Error::Protocol);
            }
        }
        let mut urls = std::collections::BTreeSet::new();
        for artifact in &binding.artifacts {
            https(&artifact.url)?;
            if artifact.length == 0
                || artifact.length > 1_099_511_627_776
                || artifact.sha256 == [0; 32]
                || !urls.insert(&artifact.url)
            {
                return Err(Error::Protocol);
            }
        }
        if step.artifacts.iter().any(|a| {
            !binding
                .artifacts
                .iter()
                .any(|export| export.length == a.length && export.sha256 == a.sha256)
        }) {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}

/// Client checks for generated EnrollmentEntry values.
pub trait EnrollmentEntryExt: Sized {
    /// validate.
    fn validate(&self, platform: TaskPlatform) -> Result<(), Error>;
}
impl EnrollmentEntryExt for EnrollmentEntry {
    fn validate(&self, platform: TaskPlatform) -> Result<(), Error> {
        let value = match (self, platform) {
            (Self::Windows { server }, TaskPlatform::Windows) => server,
            (Self::Macos { url }, TaskPlatform::Macos) => url,
            _ => return Err(Error::Protocol),
        };
        let url = url::Url::parse(value).map_err(|_| Error::Protocol)?;
        if value.len() > 2048
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || value.chars().any(char::is_control)
        {
            return Err(Error::Protocol);
        }
        if matches!(self, Self::Windows { .. }) && (url.path() != "/" || url.query().is_some()) {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}

/// Trusted task verification context.
pub struct TaskVerification<'a> {
    /// Expected key identifier from the configured trust set.
    pub key_id: &'a str,
    /// Trusted Ed25519 public key.
    pub public_key: &'a [u8],
    /// Locally enrolled tenant.
    pub tenant_id: Uuid,
    /// Locally enrolled device identity.
    pub device_id: &'a str,
    /// Actual local operating system.
    pub platform: TaskPlatform,
    /// Actual local processor architecture.
    pub architecture: TaskArchitecture,
    /// Locally enrolled current registration.
    pub registration_id: Uuid,
    /// Locally enrolled generation.
    pub generation: u64,
    /// Task identity being accepted.
    pub task_id: Uuid,
    /// Attempt identity being accepted.
    pub attempt_id: Uuid,
    /// Required permission; an offer cannot authorize start.
    pub permit: TaskPermit,
    /// Trusted current Unix time.
    pub now: i64,
}
/// Authenticated payload for the exact local context. Only verification constructs this value.
pub struct VerifiedTask<'a>(&'a TaskPayload);
impl VerifiedTask<'_> {
    /// Borrow the verified immutable executor inputs.
    pub fn payload(&self) -> &TaskPayload {
        self.0
    }
}

/// Authenticate an immutable task against local identity and time.
pub trait SignedTaskExt {
    /// Verify the signature and frozen coordinates.
    fn verify(&self, context: &TaskVerification<'_>) -> Result<VerifiedTask<'_>, Error>;
}
impl SignedTaskExt for SignedTask {
    /// Authenticate exact bytes, signing key identity, expiry and all local authority coordinates.
    fn verify(&self, context: &TaskVerification<'_>) -> Result<VerifiedTask<'_>, Error> {
        let p = &self.payload;
        let (tenant, device, registration, generation) = match p {
            TaskPayload::TaskSpec(v) => (
                v.tenant_id,
                v.device_id.as_str(),
                v.registration_id,
                v.generation,
            ),
            TaskPayload::EnrollmentTaskSpec(v) => (
                v.tenant_id,
                v.device_id.as_str(),
                v.registration_id,
                v.generation,
            ),
            TaskPayload::SoftwareTaskSpec(v) => (
                v.tenant_id,
                v.device_id.as_str(),
                v.registration_id,
                v.generation,
            ),
        };
        if self.key_id != context.key_id
            || context.now < 0
            || context.now >= p.expires_at()
            || p.permit() != context.permit
            || tenant != context.tenant_id
            || device != context.device_id
            || p.platform() != context.platform
            || p.architecture() != context.architecture
            || registration != context.registration_id
            || generation != context.generation
            || p.task_id() != context.task_id
            || p.attempt_id() != context.attempt_id
            || self.signature.len() != 86
        {
            return Err(Error::Protocol);
        }
        let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&self.signature)
            .map_err(|_| Error::Protocol)?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, context.public_key)
            .verify(&p.signing_bytes(&self.key_id)?, &signature)
            .map_err(|_| Error::Protocol)?;
        Ok(VerifiedTask(p))
    }
}

/// Local interpretation of task evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareEvidenceAssessment {
    /// Signed postconditions satisfied.
    Satisfied,
    /// Detection contradicts the signed task.
    Unsatisfied,
    /// Independent observation unresolved.
    Unknown,
    /// Reboot requires later detection.
    RebootPending,
}
fn text(value: &str, max: usize) -> Result<(), Error> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(Error::Protocol);
    }
    Ok(())
}
fn https(value: &str) -> Result<(), Error> {
    text(value, 2048)?;
    let u = url::Url::parse(value).map_err(|_| Error::Protocol)?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err(Error::Protocol);
    }
    Ok(())
}

/// Runtime profile structural and local platform checks.
impl ValidateApi for agent_api::enroll::RuntimeProfile {
    fn validate(&self) -> Result<(), Error> {
        agent_api::validate(self).map_err(|_| Error::Protocol)?;
        if self.capabilities.first() != Some(&Capability::InventoryCollectionV1)
            || self.capabilities.len() > 20
            || !self.capabilities.windows(2).all(|v| v[0] < v[1])
        {
            return Err(Error::Protocol);
        }
        self.execution_context.validate_for(self.platform)
    }
}
/// Create and freeze client reports.
pub trait ReportRequestExt: Sized {
    /// Construct a validated, sorted report.
    fn new(
        report_id: Uuid,
        sequence: u64,
        observed_at: i64,
        collection: CollectionDefinition,
        body: ReportBody,
    ) -> Result<Self, Error>;
    /// Freeze report bytes for retries.
    fn canonical(&self) -> Result<Vec<u8>, Error>;
}
impl ReportRequestExt for ReportRequest {
    fn new(
        report_id: Uuid,
        sequence: u64,
        observed_at: i64,
        collection: CollectionDefinition,
        mut body: ReportBody,
    ) -> Result<Self, Error> {
        if let ReportBody::Snapshot { values } | ReportBody::Partial { values } = &mut body {
            values.sort_by(|a, b| a.field.cmp(&b.field));
            if values.len() > 128 || values.windows(2).any(|v| v[0].field == v[1].field) {
                return Err(Error::Protocol);
            }
            for value in values {
                let field = collection
                    .fields
                    .iter()
                    .find(|f| f.key == value.field)
                    .ok_or(Error::Protocol)?;
                match &value.value {
                    CollectedValue::Null if !field.nullable => return Err(Error::Protocol),
                    CollectedValue::Value(v) => validate_scalar(v, &field.value_type)?,
                    _ => {}
                }
            }
        }
        let request = Self {
            wire_version: WIRE_VERSION,
            report_id,
            sequence,
            observed_at,
            collection,
            body,
        };
        agent_api::validate(&request).map_err(|_| Error::Protocol)?;
        Ok(request)
    }
    fn canonical(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Protocol)?;
        if bytes.len() > 1_048_576 {
            return Err(Error::Protocol);
        }
        Ok(bytes)
    }
}
fn validate_scalar(value: &Scalar, shape: &ValueType) -> Result<(), Error> {
    if matches!(value, Scalar::String(v) if v.contains('\0')) {
        return Err(Error::Protocol);
    }
    match (value, shape) {
        (Scalar::Integer(_), ValueType::Integer)
        | (Scalar::Number(_), ValueType::Number)
        | (Scalar::Boolean(_), ValueType::Boolean)
        | (Scalar::Time(_), ValueType::Time) => Ok(()),
        (
            Scalar::String(v),
            ValueType::String {
                max_length,
                allow_empty,
            },
        ) if v.chars().count() <= *max_length as usize && (*allow_empty || !v.is_empty()) => Ok(()),
        (Scalar::String(v), ValueType::Enum { values }) if values.contains(v) => Ok(()),
        (Scalar::Array(v), ValueType::Array { items, max_items })
            if v.len() <= *max_items as usize =>
        {
            for value in v {
                validate_scalar(value, items)?;
            }
            Ok(())
        }
        (Scalar::Object(v), ValueType::Object { properties }) if v.len() == properties.len() => {
            for (key, value) in v {
                validate_scalar(value, properties.get(key).ok_or(Error::Protocol)?)?;
            }
            Ok(())
        }
        _ => Err(Error::Protocol),
    }
}

fn validate_result(
    exit_code: Option<i32>,
    quality: OutputQuality,
    output: &Value,
    diagnostics: &TaskDiagnostics,
    collected_at: i64,
) -> Result<(), Error> {
    let quality_is_coherent = match quality {
        OutputQuality::Complete => exit_code == Some(0) && diagnostics.failure().is_none(),
        OutputQuality::Failed => diagnostics.failure().is_some(),
        OutputQuality::Truncated => diagnostics.failure() == Some(TaskFailure::OutputLimit),
        OutputQuality::Partial => true,
    };
    let failure_is_coherent = match diagnostics.failure() {
        Some(TaskFailure::NonZeroExit) => exit_code.is_some_and(|code| code != 0),
        Some(TaskFailure::LaunchFailed) => exit_code.is_none(),
        _ => true,
    };
    if collected_at < diagnostics.executed_at()
        || !quality_is_coherent
        || !failure_is_coherent
        || serde_json::to_vec(output)
            .map_err(|_| Error::Protocol)?
            .len()
            > OUTPUT_MAX_BYTES
    {
        return Err(Error::Protocol);
    }
    Ok(())
}

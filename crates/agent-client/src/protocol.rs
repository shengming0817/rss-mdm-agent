//! Agent-owned authentication, local execution admission and evidence interpretation.
//! Protocol shape, encoding and digests belong to rss-mdm-agent-wire.
use crate::Error;
use base64::Engine;
use rss_mdm_agent_wire::runtime::*;
use uuid::Uuid;
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

/// Authenticate the exact payload against local identity, authority and time.
pub fn verify_task<'a>(
    signed: &'a SignedTask,
    context: &TaskVerification<'_>,
) -> Result<VerifiedTask<'a>, Error> {
    let p = &signed.payload;
    let (tenant, device, registration, generation) = match p {
        TaskPayload::Script(v) => (
            v.tenant_id,
            v.device_id.as_str(),
            v.registration_id,
            v.generation,
        ),
        TaskPayload::Enrollment(v) => (
            v.tenant_id,
            v.device_id.as_str(),
            v.registration_id,
            v.generation,
        ),
        TaskPayload::Software(v) => (
            v.tenant_id,
            v.device_id.as_str(),
            v.registration_id,
            v.generation,
        ),
    };
    if signed.key_id != context.key_id
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
        || signed.signature.len() != 86
    {
        return Err(Error::Protocol);
    }
    if let TaskPayload::Software(spec) = p {
        validate_software_admission(spec)?;
    }
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&signed.signature)
        .map_err(|_| Error::Protocol)?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, context.public_key)
        .verify(&p.signing_bytes(&signed.key_id)?, &signature)
        .map_err(|_| Error::Protocol)?;
    Ok(VerifiedTask(p))
}
/// Check the signed plan against its frozen local execution and credential context.
fn validate_software_admission(spec: &SoftwareTaskSpec) -> Result<(), Error> {
    spec.validate()?;
    for step in &spec.steps {
        if execution_target(&step.action, spec.platform, &spec.execution_context)? != step.target {
            return Err(Error::Protocol);
        }
        if let SoftwareTaskExport::Brew {
            binding,
            credential_reference,
            ..
        } = &step.export
        {
            if !spec
                .execution_context
                .source_credentials
                .iter()
                .any(|c| c.source == binding.source && c.reference == *credential_reference)
            {
                return Err(Error::Protocol);
            }
        }
    }
    Ok(())
}

/// Derive the local execution target for the frozen software action.
fn execution_target(
    action: &SoftwareTaskAction,
    platform: TaskPlatform,
    context: &SoftwareExecutionContext,
) -> Result<SoftwareExecutionTarget, Error> {
    context.validate_for(platform)?;
    if action.required_profile(platform).is_none() {
        return Err(Error::Protocol);
    }
    use SoftwareTaskBehavior as B;
    let invocation = match &action.behavior {
        B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => &n.install,
        B::Exe(n) => &n.install,
        B::Bundle(n) => &n.install.invocation,
        B::Dmg(n) => &n.invocation,
        B::Msix(n) => &n.invocation,
    };
    if matches!(invocation.run_as, ExecutionIdentity::System) && !context.system_broker {
        return Err(Error::Protocol);
    }
    if matches!(&action.behavior, B::Brew(_))
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
    if matches ! (& action . behavior , B :: Winget (n) if n . install . run_as == ExecutionIdentity :: System)
        && !context
            .interactive_user
            .as_ref()
            .is_some_and(|u| u.administrator)
    {
        return Err(Error::Protocol);
    }
    let detector = match &action.behavior {
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
    if let B::Msix(n) = &action.behavior {
        if context.os_version < n.minimum_os
            || (n.require_sideload && !context.msix_sideload)
            || (n.allow_unsigned && !context.msix_unsigned)
        {
            return Err(Error::Protocol);
        }
        match &n.deployment {
            SoftwareTaskMsixDeployment::TargetUserRegistration { target: selector } => {
                let user = context.interactive_user.as_ref().ok_or(Error::Protocol)?;
                if matches ! (selector , SoftwareTaskUser::Exact { identity } if identity !=& user . identity)
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
/// Interpret the detector identity required by the local action.
pub fn observed_identity(action: &SoftwareTaskAction) -> SoftwareObservedIdentity {
    use SoftwareTaskBehavior as B;
    let detection = |v: &SoftwareTaskDetection| match v {
        SoftwareTaskDetection::MsiProduct { product_code, .. } => {
            SoftwareObservedIdentity::MsiProduct {
                product_code: product_code.clone(),
            }
        }
        SoftwareTaskDetection::PkgReceipt { receipt, .. } => SoftwareObservedIdentity::PkgReceipt {
            receipt: receipt.clone(),
        },
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
        SoftwareTaskDetection::Script { command } => SoftwareObservedIdentity::ControlledDetector {
            entry: command.entry.clone(),
        },
    };
    match &action.behavior {
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
            SoftwareTaskMsixDeployment::TargetUserRegistration { .. } => {
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
/// Interpret the version required by the local detector.
fn detected_version(action: &SoftwareTaskAction) -> String {
    use SoftwareTaskBehavior as B;
    let detection = |d: &SoftwareTaskDetection| match d {
        SoftwareTaskDetection::MsiProduct { version, .. }
        | SoftwareTaskDetection::PkgReceipt { version, .. }
        | SoftwareTaskDetection::Registry { version, .. }
        | SoftwareTaskDetection::File { version, .. } => version.clone(),
        SoftwareTaskDetection::Script { .. } => action.version.clone(),
    };
    match &action.behavior {
        B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => detection(&n.detect),
        B::Exe(n) => detection(&n.detect),
        B::Bundle(n) => detection(&n.detect),
        B::Dmg(n) => match &n.payload {
            SoftwareTaskDmgPayload::AppCopy { application, .. } => application.version.clone(),
            _ => action.version.clone(),
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
/// Assess local software evidence against the signed plan.
pub fn assess_software_result(
    result: &SoftwareTaskResult,
    task: &SoftwareTaskSpec,
) -> Result<SoftwareEvidenceAssessment, Error> {
    use SoftwareEvidenceAssessment as A;
    validate_software_result(result, task)?;
    if result
        .steps
        .iter()
        .any(|step| matches!(step.after, SoftwareDetectionObservation::Unknown { .. }))
    {
        return Ok(A::Unknown);
    }
    let reboot = result.steps.iter().any(|step| step.reboot_required);
    let reboot_allowed = result.steps.iter().zip(&task.steps).all(|(result, step)| {
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
        result
            .steps
            .iter()
            .zip(&task.steps)
            .enumerate()
            .all(|(index, (result, step))| {
                let intent = if index + 1 == task.steps.len() {
                    task.intent
                } else {
                    SoftwareTaskIntent::Install
                };
                satisfies(&result.after, intent, &detected_version(&step.action))
            });
    Ok(if matches {
        A::Satisfied
    } else {
        A::Unsatisfied
    })
}
fn validate_software_result(
    result: &SoftwareTaskResult,
    task: &SoftwareTaskSpec,
) -> Result<(), Error> {
    result.validate()?;
    if result.definition_digest != task.definition_digest
        || result.intent != task.intent
        || result.steps.len() != task.steps.len()
    {
        return Err(Error::Protocol);
    }
    for (result, step) in result.steps.iter().zip(&task.steps) {
        if result.step_digest != step.digest()?
            || result.target != step.target
            || result.package != step.action.package
            || result.identity != observed_identity(&step.action)
        {
            return Err(Error::Protocol);
        }
    }
    Ok(())
}
fn satisfies(
    observation: &SoftwareDetectionObservation,
    intent: SoftwareTaskIntent,
    version: &str,
) -> bool {
    match (observation, intent) {
        (
            SoftwareDetectionObservation::Present {
                version: observed, ..
            },
            SoftwareTaskIntent::Install | SoftwareTaskIntent::Detect,
        ) => observed == version,
        (SoftwareDetectionObservation::Absent { .. }, SoftwareTaskIntent::Uninstall) => true,
        _ => false,
    }
}

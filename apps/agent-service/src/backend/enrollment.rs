//! Standard OS enrollment runs through the existing journal, runner and authenticated user helper.
use super::plan::{self, hex, id, reference};
use agent_client::{Error, Materials, NativeEnrollmentMaterial, Offer};
use execution_contract::*;
use execution_runner::{staging::MaterialRoot, Artifacts};
use rss_mdm_agent_wire::runtime as w;
use sha2::{Digest as _, Sha256};
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct NativeMaterials {
    pub material: Option<NativeEnrollmentMaterial>,
    pub profile_bytes: Option<Vec<u8>>,
}
pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    payload: &w::EnrollmentTaskSpec,
    host: (&execution_app::ServiceBinding, &ActorId, &mut MaterialRoot),
    delegate: Option<Arc<execution_ipc::helper::Connection>>,
    native: NativeMaterials,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let context = delegate
        .as_ref()
        .ok_or(Error::Unavailable)?
        .context()
        .clone();
    compile_bound(
        offer,
        materials,
        payload,
        host,
        (delegate, &context),
        plan::pin(&std::env::current_exe()?)?,
        native,
    )
}
fn compile_bound(
    offer: &Offer,
    materials: &Materials,
    payload: &w::EnrollmentTaskSpec,
    host: (&execution_app::ServiceBinding, &ActorId, &mut MaterialRoot),
    helper: (
        Option<Arc<execution_ipc::helper::Connection>>,
        &execution_ipc::helper::UserContext,
    ),
    image: SoftwareMaterial,
    native: NativeMaterials,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let (binding, actor, root) = host;
    let (delegate, context) = helper;
    let NativeMaterials {
        material,
        profile_bytes,
    } = native;
    let w::TaskPayload::Enrollment(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if compared != **original
        || payload.device_id != binding.device.as_str()
        || !materials.files().is_empty()
    {
        return Err(Error::Untrusted);
    }
    materials.validate_offer(offer)?;
    let platform = plan::platform()?;
    let account = OsAccountRef {
        platform,
        subject: id(&context.subject)?,
    };
    let run_as = RunAs::User {
        account: account.clone(),
    };
    let session = SessionRequirement::ActiveUser {
        account,
        session: context.binding.clone(),
    };
    let work_root = context.work_root.clone();
    let profile = profile_bytes
        .map(|bytes| {
            let digest = Digest::new(hex(&Sha256::digest(&bytes))).map_err(|_| Error::Protocol)?;
            let source = root
                .publish(
                    Some(&context.subject),
                    std::io::Cursor::new(&bytes),
                    &digest,
                    bytes.len() as u64,
                )
                .map_err(crate::error::app_error)?;
            let path = root
                .native_export(
                    &source,
                    &digest,
                    "enrollment.mobileconfig",
                    Some(&context.subject),
                )
                .map_err(crate::error::app_error)?;
            Ok::<_, Error>((path, digest))
        })
        .transpose()?;
    let mut parameters = std::collections::BTreeMap::new();
    if let Some((_, digest)) = &profile {
        parameters.insert(
            "nativeProfile".into(),
            InputValue::Literal {
                value: digest.as_str().into(),
            },
        );
    }
    let bytes = serde_json::to_vec(&crate::enrollment_worker::Input {
        task: payload.clone(),
        material,
        profile,
        image_sha256: image.artifact.sha256.clone(),
        run_as: run_as.clone(),
        session: session.clone(),
    })
    .map_err(|_| Error::Protocol)?;
    let hash = hex(&Sha256::digest(&bytes));
    let artifact = ExactArtifactRef {
        resource: reference("native-enrollment-input", &hash)?,
        sha256: Digest::new(hash).map_err(|_| Error::Protocol)?,
    };
    let content = root
        .publish(
            Some(&context.subject),
            std::io::Cursor::new(&bytes),
            &artifact.sha256,
            bytes.len() as u64,
        )
        .map_err(crate::error::app_error)?;
    let interpreter = image.path.clone();
    let frozen = freeze(
        offer,
        payload,
        (binding, actor),
        NativeLaunch {
            run_as,
            session,
            work_root: work_root.clone(),
            image,
            artifact,
            parameters,
        },
    )?;
    Ok((
        frozen,
        Artifacts {
            program: vec![],
            delegate,
            interpreter: interpreter.into(),
            content,
            work_root,
            controlled_input: None,
        },
    ))
}

struct NativeLaunch {
    run_as: RunAs,
    session: SessionRequirement,
    work_root: std::path::PathBuf,
    image: SoftwareMaterial,
    artifact: ExactArtifactRef,
    parameters: std::collections::BTreeMap<String, InputValue>,
}
fn freeze(
    offer: &Offer,
    payload: &w::EnrollmentTaskSpec,
    authority: (&execution_app::ServiceBinding, &ActorId),
    launch: NativeLaunch,
) -> Result<FrozenExecution, Error> {
    let (binding, actor) = authority;
    let platform = plan::platform()?;
    let NativeLaunch {
        run_as,
        session,
        work_root,
        image,
        artifact,
        parameters,
    } = launch;
    let now = crate::wall()?;
    let task_hash = hex(&Sha256::digest(
        serde_json::to_vec(payload).map_err(|_| Error::Protocol)?,
    ));
    FrozenExecution::freeze(
        ExecutionInput {
            schema_version: V1,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: super::request_id(offer)?,
                authority: binding.authority.clone(),
                actor: actor.clone(),
                initiator: Initiator::Backend {
                    task: id(payload.task_id.to_string())?,
                    attempt: id(payload.attempt_id.to_string())?,
                    trigger: BackendTrigger::Automatic {},
                },
                delegation: None,
                target: Target {
                    device: binding.device.clone(),
                    platform,
                    scope: TargetScope::Device {},
                },
                operation: Operation {
                    action: id("backend-enrollment")?,
                    resource: reference("backend-enrollment", task_hash)?,
                },
                parameters,
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: image.artifact,
                    profile: reference(crate::enrollment_worker::PROFILE, "1")?,
                },
                argv: vec![
                    LaunchArg::Literal {
                        value: "--enrollment-worker".into(),
                    },
                    LaunchArg::ArtifactPath {},
                ],
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output: OutputSpec {
                    // The worker returns one closed JSON enum scalar, not collection rows.
                    // The bridge decodes EnrollmentEntryOutcome after complete UTF-8 capture.
                    format: OutputFormat::Text {},
                    stdout: TextEncoding::Utf8,
                    stderr: TextEncoding::Utf8,
                },
                cwd: work_root.to_str().ok_or(Error::Configuration)?.into(),
                env: Default::default(),
            },
            run_as,
            session_requirement: session,
            constraints: IsolationPolicy::OsIdentity {},
            budget: ExecutionBudget {
                total_timeout_ms: crate::enrollment_worker::TIMEOUT_MS,
                total_output_bytes: crate::enrollment_worker::OUTPUT_BYTES,
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: now,
                expires_at_unix_ms: u64::try_from(payload.expires_at)
                    .map_err(|_| Error::Clock)?
                    .checked_mul(1000)
                    .ok_or(Error::Clock)?,
            },
            policy: reference(
                format!("backend:{}", payload.task_id),
                payload.attempt_id.to_string(),
            )?,
        },
        &plan::storage_limits().input,
    )
    .map_err(|_| Error::Untrusted)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::backend::{
        bridge::ExecutionBridge,
        test_support::{local, FixtureOutput},
    };
    use crate::protocol_test_support::{Root, Server};
    use execution_app::*;

    #[tokio::test]
    async fn compiled_native_plan_passes_bridge_and_rejects_excess_budget() {
        let _budget = crate::scenario_budget::enter();
        let server = Server::new().await;
        let transport = Root::new();
        let mut client = server.client(&transport, agent_client::OpenMode::Create);
        server.register(&mut client).await;
        server.data.lock().unwrap().native_enrollment();
        let offer = client.claim().await.unwrap().offer.unwrap();
        let materials = client.prepare(&offer).await.unwrap();
        let w::TaskPayload::Enrollment(payload) = offer.payload() else {
            panic!("enrollment")
        };
        let mut payload = (**payload).clone();
        payload.expires_at = (crate::wall().unwrap() / 1000 + 60) as i64;
        let mut host = local::TestHost::new();
        let binding = ServiceBinding {
            authority: host.template.spec().request.authority.clone(),
            device: DeviceId::new("device-1").unwrap(),
        };
        let actor = host.template.spec().request.actor.clone();
        let account = OsAccountRef {
            platform: Platform::Macos,
            subject: id("501").unwrap(),
        };
        let image = plan::pin(std::path::Path::new("/usr/bin/true")).unwrap();
        // Exercise the production plan builder after staging; no native helper/ACL/OS launch is claimed.
        let plan = freeze(
            &offer,
            &payload,
            (&binding, &actor),
            NativeLaunch {
                run_as: RunAs::User {
                    account: account.clone(),
                },
                session: SessionRequirement::ActiveUser {
                    account,
                    session: id("fixture-session").unwrap(),
                },
                work_root: transport.path.clone(),
                artifact: image.artifact.clone(),
                image,
                parameters: Default::default(),
            },
        )
        .unwrap();
        assert_eq!(plan.spec().launch.output.format, OutputFormat::Text {});
        host.template = plan.clone();
        let db = local::Database::new();
        let runner =
            DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Wait, 16).unwrap();
        let store = execution_sqlite::Store::initialize_test(
            &db.path,
            binding.authority,
            plan::storage_limits(),
        )
        .unwrap();
        let mut config = AppConfig::test_defaults();
        config.max_timeout_ms = plan::storage_limits().input.max_timeout_ms;
        let app = ExecutionApp::new(store, host, runner, config).unwrap();
        let caller = RequestContext { actor };
        let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
        bridge
            .prepare(&offer, &materials, &app, &caller, &plan)
            .unwrap();
        let mut oversized = plan.spec().clone();
        oversized.budget.total_timeout_ms += 1;
        let oversized = FrozenExecution::freeze(oversized, &plan::storage_limits().input).unwrap();
        assert!(matches!(
            bridge.prepare(&offer, &materials, &app, &caller, &oversized),
            Err(agent_client::Error::Untrusted)
        ));
    }
}

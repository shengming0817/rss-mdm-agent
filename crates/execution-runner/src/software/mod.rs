//! Native read-only software detection and bounded preparation.
pub(crate) mod activity;
pub(crate) mod native_detection;
pub use activity::NativeActivity;
use execution_app::Error;
pub(crate) use native_detection::detect as native_detection;
use std::sync::Arc;
pub(crate) struct PreparationControl {
    pub deadline: std::time::Instant,
    pub cancelled: Arc<std::sync::atomic::AtomicBool>,
}
impl PreparationControl {
    pub(crate) fn check(&self) -> Result<(), Error> {
        if self.cancelled.load(std::sync::atomic::Ordering::Acquire)
            || std::time::Instant::now() >= self.deadline
        {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn test() -> Self {
        Self {
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

/// Read an exact native macOS package receipt without creating a process or guessing removal.
pub fn receipt_state(
    receipt: &str,
) -> Result<execution_contract::SoftwareState, execution_app::Error> {
    let version = execution_contract::PackageValue::new("observation")
        .map_err(|_| execution_app::Error::InvalidInput)?;
    Ok(native_detection::detect(
        &execution_contract::SoftwareDetector::PkgReceipt {
            receipt: receipt.into(),
            version,
            run_as: execution_contract::RunAs::System {
                platform: execution_contract::Platform::Macos,
            },
            session_requirement: execution_contract::SessionRequirement::NotRequired {},
        },
        &execution_contract::RunAs::System {
            platform: execution_contract::Platform::Macos,
        },
    ))
}

/// Read only closed platform-native identities. Script and other detectors are denied.
/// The caller must authorize the exact signed definition and live OS subject before using this port.
pub fn observe_native(
    detector: &execution_contract::SoftwareDetector,
    run_as: &execution_contract::RunAs,
) -> Result<execution_contract::SoftwareState, execution_app::Error> {
    if !matches!(
        detector,
        execution_contract::SoftwareDetector::PkgReceipt { .. }
            | execution_contract::SoftwareDetector::MsiProduct { .. }
    ) {
        return Err(execution_app::Error::Denied);
    }
    Ok(native_detection::detect(detector, run_as))
}

#[cfg(all(test, target_os = "macos"))]
mod observation_tests {
    use super::*;
    use execution_contract::*;
    #[test]
    fn native_observation_reads_receipts_and_preserves_unavailable_platform_evidence() {
        let run_as = RunAs::System {
            platform: Platform::Macos,
        };
        let expected = PackageValue::new("1").unwrap();
        let receipt = format!("com.rss-mdm.uninstalled-test-{}", std::process::id());
        let input: ExecutionInput = serde_json::from_str(include_str!(
            "../../../execution-contract/tests/fixtures/software.json"
        ))
        .unwrap();
        let invocation = input.execution.software_program().unwrap().steps[0]
            .invocations[&SoftwarePhase::Mutation]
            .clone();
        assert_eq!(
            observe_native(
                &SoftwareDetector::Script {
                    invocation: Box::new(invocation)
                },
                &run_as
            ),
            Err(execution_app::Error::Denied)
        );
        assert!(matches!(
            observe_native(
                &SoftwareDetector::PkgReceipt {
                    run_as: run_as.clone(),
                    session_requirement: SessionRequirement::NotRequired {},
                    receipt,
                    version: expected.clone()
                },
                &run_as
            )
            .unwrap(),
            SoftwareState::Absent {}
        ));
        assert!(matches!(
            observe_native(
                &SoftwareDetector::PkgReceipt {
                    run_as: run_as.clone(),
                    session_requirement: SessionRequirement::NotRequired {},
                    receipt: "../../etc".into(),
                    version: expected.clone()
                },
                &run_as
            )
            .unwrap(),
            SoftwareState::Unknown { .. }
        ));
        assert!(matches!(
            observe_native(
                &SoftwareDetector::MsiProduct {
                    run_as: RunAs::System { platform: Platform::Windows },
                    session_requirement: SessionRequirement::NotRequired {},
                    product_code: "{00000000-0000-0000-0000-000000000001}".into(),
                    version: expected
                },
                &run_as
            )
            .unwrap(),
            SoftwareState::Unknown { .. }
        ));
    }
}

/// Whether a fixed candidate has native forward-version semantics already supported by this runner.
/// Source managers still repeat their own guard during Prepare/Upgrade; this is not dispatch authority.
/// Unknown ecosystem ordering and explicit downgrade permissions are never advertised as updates.
pub fn forward_update(step: &execution_contract::SoftwareProgramStep, installed: &str) -> bool {
    !step.allow_downgrade
        && installed != step.version.as_str()
        && execution_contract::PackageValue::new(installed).is_ok_and(|version| {
            native_detection::not_downgrade(
                step,
                &execution_contract::SoftwareState::Present { version },
            )
        })
}

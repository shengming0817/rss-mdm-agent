use crate::Error;
use execution_contract::software_commands;
use rss_mdm_agent_wire::runtime as wire;

/// One source-authorized attempt budget, including the longest possible approved mutation path.
/// Before/after detectors share a single observation budget; DMG phases share its invocation budget.
pub fn software_budget(spec: &wire::SoftwareTaskSpec) -> Result<(u64, u64), Error> {
    let mut timeout = 0u64;
    let mut output = 0u64;
    for step in &spec.steps {
        let c = software_commands(&step.action);
        let mut mutation = Vec::new();
        match spec.intent {
            wire::SoftwareTaskIntent::Install => {
                mutation.push(c.install);
                if c.upgrade_policy == wire::SoftwareTaskUpgrade::UninstallThenInstall {
                    mutation.push(c.uninstall.ok_or(Error::Unsupported)?);
                } else if let Some(upgrade) = c
                    .upgrade
                    .filter(|_| c.upgrade_policy == wire::SoftwareTaskUpgrade::InPlace)
                {
                    timeout = timeout
                        .checked_add(
                            u64::from(c.install.timeout_seconds.max(upgrade.timeout_seconds))
                                * 1000,
                        )
                        .ok_or(Error::Capacity)?;
                    output = output
                        .checked_add(u64::from(c.install.output_bytes.max(upgrade.output_bytes)))
                        .ok_or(Error::Capacity)?;
                    mutation.clear();
                }
            }
            wire::SoftwareTaskIntent::Uninstall => {
                mutation.push(c.uninstall.ok_or(Error::Unsupported)?)
            }
            wire::SoftwareTaskIntent::Detect => (),
        }
        for invocation in mutation {
            timeout = timeout
                .checked_add(u64::from(invocation.timeout_seconds) * 1000)
                .ok_or(Error::Capacity)?;
            output = output
                .checked_add(u64::from(invocation.output_bytes))
                .ok_or(Error::Capacity)?;
        }
        match c.detection {
            Some(wire::SoftwareTaskDetection::Script { command }) => {
                timeout = timeout
                    .checked_add(u64::from(command.invocation.timeout_seconds) * 1000)
                    .ok_or(Error::Capacity)?;
                output = output
                    .checked_add(u64::from(command.invocation.output_bytes))
                    .ok_or(Error::Capacity)?;
            }
            // Fixed native observations use this same invocation budget, not a new grant.
            _ if spec.intent == wire::SoftwareTaskIntent::Detect => {
                timeout = timeout
                    .checked_add(u64::from(c.install.timeout_seconds) * 1000)
                    .ok_or(Error::Capacity)?;
                output = output
                    .checked_add(u64::from(c.install.output_bytes))
                    .ok_or(Error::Capacity)?;
            }
            _ => (),
        }
    }
    let bounds = super::plan::storage_limits().input;
    if timeout == 0
        || output == 0
        || timeout > bounds.max_timeout_ms
        || output > bounds.max_output_bytes
    {
        return Err(Error::Capacity);
    }
    Ok((timeout, output))
}

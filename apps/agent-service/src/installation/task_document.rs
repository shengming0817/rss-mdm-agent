//! Validate semantic Task Scheduler ownership, independent of XML formatting.
use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Settings {
    disallow_start_if_on_batteries: bool,
    stop_if_going_on_batteries: bool,
    enabled: bool,
    execution_time_limit: String,
    multiple_instances_policy: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Principals {
    principal: Vec<Principal>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Principal {
    #[serde(rename = "@id")]
    _id: Option<String>,
    user_id: String,
    logon_type: Option<String>,
    run_level: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Actions {
    #[serde(rename = "@Context")]
    context: String,
    exec: Vec<Exec>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Exec {
    command: String,
    arguments: String,
    working_directory: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RetirementTask {
    principals: Principals,
    actions: Actions,
    triggers: BootTriggers,
    settings: Settings,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct BootTriggers {
    boot_trigger: Vec<Boot>,
    registration_trigger: Vec<Boot>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Boot {
    enabled: bool,
    delay: String,
    repetition: Repetition,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Repetition {
    interval: String,
    duration: Option<String>,
    stop_at_duration_end: bool,
}
pub(super) fn retirement_owned(xml: &str, binary: &str, arguments: &str) -> Result<(), Error> {
    if xml.len() > 65536 || xml.contains("<!DOCTYPE") {
        return Err(Error::Identity);
    }
    let task: RetirementTask = quick_xml::de::from_str(xml).map_err(|_| Error::Identity)?;
    let [principal] = task.principals.principal.as_slice() else {
        return Err(Error::Identity);
    };
    let [action] = task.actions.exec.as_slice() else {
        return Err(Error::Identity);
    };
    let [trigger] = task.triggers.boot_trigger.as_slice() else {
        return Err(Error::Identity);
    };
    let [registration] = task.triggers.registration_trigger.as_slice() else {
        return Err(Error::Identity);
    };
    let recurring = |trigger: &Boot| {
        trigger.enabled
            && trigger.delay == "PT30S"
            && trigger.repetition.interval == "PT1M"
            && trigger.repetition.duration.is_none()
            && !trigger.repetition.stop_at_duration_end
    };
    if principal.user_id != "S-1-5-18"
        || principal.logon_type.is_some()
        || principal.run_level != "HighestAvailable"
        || task.actions.context != "System"
        || action.command != binary
        || action.arguments != arguments
        || action
            .working_directory
            .as_ref()
            .is_some_and(|v| !v.is_empty())
        || !recurring(trigger)
        || !recurring(registration)
        || task.settings.disallow_start_if_on_batteries
        || task.settings.stop_if_going_on_batteries
        || !task.settings.enabled
        || task.settings.multiple_instances_policy != "IgnoreNew"
        || task.settings.execution_time_limit != "PT2M"
    {
        return Err(Error::Identity);
    }
    Ok(())
}

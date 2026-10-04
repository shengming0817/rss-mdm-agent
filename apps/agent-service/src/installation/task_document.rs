//! Validate semantic Task Scheduler ownership, independent of XML formatting.
use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Task {
    principals: Principals,
    actions: Actions,
    triggers: Triggers,
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
    logon_type: String,
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
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Triggers {
    logon_trigger: Vec<Logon>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Logon {
    user_id: String,
}
pub(super) fn owned(xml: &str, subject: &str, binary: &str, arguments: &str) -> Result<(), Error> {
    if xml.len() > 65536 || xml.contains("<!DOCTYPE") {
        return Err(Error::Identity);
    }
    let task: Task = quick_xml::de::from_str(xml).map_err(|_| Error::Identity)?;
    let [principal] = task.principals.principal.as_slice() else {
        return Err(Error::Identity);
    };
    let [action] = task.actions.exec.as_slice() else {
        return Err(Error::Identity);
    };
    let [trigger] = task.triggers.logon_trigger.as_slice() else {
        return Err(Error::Identity);
    };
    if principal.user_id != subject
        || principal.logon_type != "InteractiveToken"
        || principal.run_level != "LeastPrivilege"
        || task.actions.context != "User"
        || action.command != binary
        || action.arguments != arguments
        || action
            .working_directory
            .as_ref()
            .is_some_and(|s| !s.is_empty())
        || trigger.user_id != subject
    {
        return Err(Error::Identity);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduler_ownership_rejects_extra_actions_and_elevation() {
        let xml = r#"<Task><Principals><Principal id="User"><UserId>S-1-5-21-7</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Triggers><LogonTrigger><UserId>S-1-5-21-7</UserId></LogonTrigger></Triggers><Actions Context="User"><Exec><Command>C:\Program Files\RSS\service.exe</Command><Arguments>--config &quot;C:\RSS\execution.json&quot; --user-helper</Arguments></Exec></Actions></Task>"#;
        let program = r"C:\Program Files\RSS\service.exe";
        let args = r#"--config "C:\RSS\execution.json" --user-helper"#;
        owned(xml, "S-1-5-21-7", program, args).unwrap();
        for bad in [
            xml.replace("LeastPrivilege", "HighestAvailable"),
            xml.replace("</Actions>", "<ComHandler/></Actions>"),
            xml.replace("<Exec>", "<Exec><Command>C:\\evil.exe</Command>"),
            xml.replace("S-1-5-21-7", "S-1-5-21-8"),
        ] {
            assert!(owned(&bad, "S-1-5-21-7", program, args).is_err());
        }
    }
}

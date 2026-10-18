//! Fixed Windows carrier finalization, generated and validated before OS registration.
//! ref: Microsoft Task Scheduler BootTrigger / RegistrationTrigger / RepetitionPattern; DeleteService remarks.
use super::*;
pub(super) struct Definition {
    pub name: String,
    pub command: String,
    pub arguments: String,
    pub script: String,
    pub xml: String,
}
pub(super) fn definition(
    task: uuid::Uuid,
    binary: &str,
    script: &str,
    system: &str,
) -> Result<Definition, Error> {
    // cmd expands these characters even inside some quoted batch constructs. The
    // installation owner rejects such host paths instead of inventing escaping.
    for path in [binary, script, system] {
        if !path.is_ascii()
            || path.is_empty()
            || path.bytes().any(|c| c < 32 || b"\"%!^&|<>".contains(&c))
        {
            return Err(Error::Configuration);
        }
    }
    if task.is_nil() {
        return Err(Error::Identity);
    }
    let name = format!("RssExecution-Retire-{task}");
    let service = format!("RssExecution-Install-{task}");
    let command = format!(r"{system}\cmd.exe");
    let arguments = format!(r#"/d /s /c ""{script}"""#);
    // A successful sc delete is only a mark. Require a later query to prove the
    // registration absent before touching the EXE. Every failure keeps this task.
    // The fixed .cmd definition remains as protected audit, like the JSON handoff.
    let script = format!(
        r#"@echo off
setlocal DisableDelayedExpansion
"{system}\sc.exe" query "{service}" >nul 2>&1
if "%errorlevel%"=="1060" goto files
if errorlevel 1 exit /b 1
"{system}\sc.exe" delete "{service}" >nul 2>&1
exit /b 1
:files
del /f /q "{binary}" >nul 2>&1
if exist "{binary}" exit /b 1
"{system}\schtasks.exe" /delete /tn "\{name}" /f >nul 2>&1
if errorlevel 1 exit /b 1
exit /b 0
"#
    )
    .replace('\n', "\r\n");
    let xml = format!(
        r#"<?xml version="1.0"?><Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><Triggers><BootTrigger><Repetition><Interval>PT1M</Interval><StopAtDurationEnd>false</StopAtDurationEnd></Repetition><Enabled>true</Enabled><Delay>PT30S</Delay></BootTrigger><RegistrationTrigger><Repetition><Interval>PT1M</Interval><StopAtDurationEnd>false</StopAtDurationEnd></Repetition><Enabled>true</Enabled><Delay>PT30S</Delay></RegistrationTrigger></Triggers><Principals><Principal id="System"><UserId>S-1-5-18</UserId><LogonType>ServiceAccount</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals><Settings><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><Enabled>true</Enabled><StartWhenAvailable>true</StartWhenAvailable><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><ExecutionTimeLimit>PT2M</ExecutionTimeLimit></Settings><Actions Context="System"><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions></Task>"#,
        quick_xml::escape::escape(&command),
        quick_xml::escape::escape(&arguments)
    );
    Ok(Definition {
        name,
        command,
        arguments,
        script,
        xml,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retirement_system_task_binds_exact_owner_image_and_arguments() {
        let task = uuid::Uuid::new_v4();
        let plan = definition(
            task,
            r"C:\RSS\task.exe",
            r"C:\RSS\task.cmd",
            r"C:\Windows\System32",
        )
        .unwrap();
        super::super::task_document::retirement_owned(&plan.xml, &plan.command, &plan.arguments)
            .unwrap();
        assert!(plan.name.ends_with(&task.to_string()));
        assert!(!plan.script.contains("MoveFileEx"));
        for bad in [
            plan.xml.replace("S-1-5-18", "S-1-5-19"),
            plan.xml.replace("<Interval>PT1M", "<Interval>PT5M"),
            plan.xml.replace("IgnoreNew", "Parallel"),
            plan.xml.replace("</Actions>", "<ComHandler/></Actions>"),
            plan.xml.replace("task.cmd", "other.cmd"),
        ] {
            assert!(super::super::task_document::retirement_owned(
                &bad,
                &plan.command,
                &plan.arguments
            )
            .is_err());
        }
        for bad in [
            r"C:\%TEMP%\task.exe",
            r"C:\a!b\task.exe",
            "C:\\x\nshutdown",
            r"C:\a&b\task.exe",
        ] {
            assert!(definition(task, bad, r"C:\RSS\task.cmd", r"C:\Windows\System32").is_err());
        }
    }
}

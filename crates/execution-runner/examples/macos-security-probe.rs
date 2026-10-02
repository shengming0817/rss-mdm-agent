//! Explicit macOS acceptance executable; never a production service mode.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use installation_security::Artifact;
    use std::ffi::CString;
    extern "C" {
        fn rss_security_probe_main(requirement: *const std::ffi::c_char, fake: i32) -> i32;
        fn rss_execution_gui_active() -> i32;
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let fake = args == ["--fake-service"];
    let requirement = if fake {
        if unsafe { libc::geteuid() } != 0 {
            return Err("fake service requires the isolated administrator installation".into());
        }
        None
    } else {
        let [flag, path] = args.as_slice() else {
            return Err("expected --config <protected deployment>".into());
        };
        if flag != "--config" {
            return Err("expected --config".into());
        }
        let config: serde_json::Value = serde_json::from_slice(
            &installation_security::read_protected(std::path::Path::new(path))?,
        )?;
        if config["ipc_version"] != execution_runner::host::IPC_VERSION {
            return Err("deployment protocol mismatch".into());
        }
        let service: Artifact = serde_json::from_value(config["service"].clone())?;
        service.verify(&service.path)?;
        Some(service.requirement()?)
    };
    // Healthy wire comes from the production owner, not a second probe schema.
    let baseline =
        execution_runner::host::encode(execution_runner::host::Request::ServiceStatus {})?;
    println!(
        "{}",
        serde_json::json!({
            "ready": true, "uid": unsafe { libc::geteuid() },
            "pid": std::process::id(), "session": execution_runner::host::current_session()?,
            "binding": execution_runner::host::current_session_binding()?,
            "guiActive": unsafe { rss_execution_gui_active() } == 1,
            "baseline": serde_json::from_slice::<serde_json::Value>(&baseline)?,
        })
    );
    let empty = CString::new("")?;
    let code = unsafe {
        rss_security_probe_main(
            requirement.as_ref().unwrap_or(&empty).as_ptr(),
            i32::from(fake),
        )
    };
    if code != 0 {
        return Err("native probe control failed".into());
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macOS acceptance target required");
    std::process::exit(1);
}

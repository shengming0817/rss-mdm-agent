//! The sole production execution service. Initialization is explicit and never a startup fallback.
mod error;
use agent_client::Error;
use agent_service::deployment::Deployment;
use execution_ipc::host::Request;
use std::{path::PathBuf, sync::atomic::AtomicBool};
static STOP: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
extern "C" fn stopped(_: i32) {
    STOP.store(true, std::sync::atomic::Ordering::Release);
}
fn run() -> Result<(), Error> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    #[cfg(windows)]
    if let [mode, name, path] = args.as_slice() {
        if mode == "--installation-worker" {
            return agent_service::installation::worker(
                name.to_str().ok_or(Error::Configuration)?.into(),
                PathBuf::from(path),
            );
        }
    }
    if args.first().is_some_and(|a| a == "--installation") {
        return agent_service::installation::run(&args[1..]);
    }
    if let [mode, path] = args.as_slice() {
        if mode == "--software-worker" {
            let code = agent_service::software_worker::run(std::path::Path::new(path))?;
            std::process::exit(code);
        }
    }
    let path = if args.first().is_some_and(|a| a == "--config") {
        if args.len() < 2 {
            return Err(Error::Configuration);
        }
        let path = PathBuf::from(args.remove(1));
        args.remove(0);
        path
    } else {
        Deployment::default_path()?
    };
    let deployment = Deployment::load(&path)?;
    if args.len() == 1 && args[0] == "--validate-installation" {
        deployment
            .service
            .verify(&std::env::current_exe()?)
            .map_err(|_| Error::Identity)?;
        println!(
            "{}",
            serde_json::to_string(&deployment).map_err(|_| Error::Protocol)?
        );
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--validate-persistent" {
        deployment.validate_persistent()?;
        println!("persistent identity and storage verified");
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--user-helper" {
        return serve(Box::new(deployment.user_helper()?), false);
    }
    if args.len() == 2 && args[0] == "--initialize" {
        let secret = agent_service::installation::stdin_secret()?;
        return agent_service::installation::initialize(
            &deployment,
            std::path::Path::new(&args[1]),
            &secret,
        );
    }
    if args.len() == 1 && args[0] == "--renew-certificate" {
        return tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?
            .block_on(deployment.renew());
    }
    let request = match args.as_slice() {
        [arg] if arg == "--query" => Some(Request::Tasks { after: None }),
        [arg] if arg == "--service-status" => Some(Request::ServiceStatus {}),
        [] => None,
        _ => return Err(Error::Configuration),
    };
    if let Some(request) = request {
        let reply = execution_ipc::host::ServiceClient::new(deployment.server_policy())
            .map_err(error::app_error)?
            .request(request)
            .map_err(error::app_error)?;
        println!(
            "{}",
            std::str::from_utf8(&execution_ipc::host::encode_reply(reply))
                .map_err(|_| Error::Protocol)?
        );
        return Ok(());
    }
    let handler = deployment.assemble()?;
    serve(handler, true)
}
fn serve(handler: Box<dyn execution_ipc::host::Handler>, system: bool) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let _ = system;
        unsafe {
            libc::signal(libc::SIGTERM, stopped as *const () as usize);
            libc::signal(libc::SIGINT, stopped as *const () as usize);
        }
        execution_ipc::host::run(handler, &STOP).map_err(error::app_error)?;
    }
    #[cfg(windows)]
    execution_ipc::windows_service::run(handler, &STOP, system).map_err(error::app_error)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("agent_service: {error}");
        std::process::exit(1);
    }
}

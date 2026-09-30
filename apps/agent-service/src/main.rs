//! The sole production execution service. Initialization is explicit and never a startup fallback.
use agent_client::{wire, Error};
use agent_service::deployment::Deployment;
use execution_runner::host::{ClientOrigin, Request};
use std::{io::Read, path::PathBuf, sync::atomic::AtomicBool};
static STOP: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
extern "C" fn stopped(_: i32) {
    STOP.store(true, std::sync::atomic::Ordering::Release);
}
fn run() -> Result<(), Error> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
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
    if args.len() == 1 && args[0] == "--user-helper" {
        return serve(Box::new(deployment.user_helper()?), false);
    }
    if args.len() == 1 && args[0] == "--initialize" {
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        std::io::stdin().take(65).read_to_end(&mut bytes)?;
        let password = wire::Secret::parse(
            std::str::from_utf8(&bytes)
                .map_err(|_| Error::Identity)?
                .trim(),
        )
        .map_err(|_| Error::Identity)?;
        return tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?
            .block_on(deployment.initialize(&password));
    }
    let request = match args.as_slice() {
        [arg] if arg == "--query" => Some(Request::Tasks { after: None }),
        [arg, request, task, attempt, revision] if arg == "--start-task" => {
            Some(Request::StartTask {
                request: execution_contract::RequestId::new(
                    request.to_str().ok_or(Error::Protocol)?,
                )
                .map_err(|_| Error::Protocol)?,
                task: execution_contract::Id::new(task.to_str().ok_or(Error::Protocol)?)
                    .map_err(|_| Error::Protocol)?,
                attempt: execution_contract::Id::new(attempt.to_str().ok_or(Error::Protocol)?)
                    .map_err(|_| Error::Protocol)?,
                revision: execution_contract::Digest::new(
                    revision.to_str().ok_or(Error::Protocol)?,
                )
                .map_err(|_| Error::Protocol)?,
                origin: ClientOrigin::Desktop {},
            })
        }
        [] => None,
        _ => return Err(Error::Configuration),
    };
    if let Some(request) = request {
        let reply = execution_runner::host::ServiceClient::new(deployment.server_policy())?
            .request(request)?;
        println!(
            "{}",
            serde_json::to_string(&reply).map_err(|_| Error::Protocol)?
        );
        return Ok(());
    }
    let service = deployment.open()?;
    let helper_policy = if deployment.helper_work_roots.is_empty() {
        None
    } else {
        Some(execution_runner::host::PeerPolicy {
            images: vec![deployment.service.clone()],
            subjects: deployment.helper_work_roots.keys().cloned().collect(),
            interactive: true,
        })
    };
    let handler = service.spawn(deployment.clients, helper_policy)?;
    serve(Box::new(handler), true)
}
fn serve(handler: Box<dyn execution_runner::host::Handler>, system: bool) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let _ = system;
        unsafe {
            libc::signal(libc::SIGTERM, stopped as *const () as usize);
            libc::signal(libc::SIGINT, stopped as *const () as usize);
        }
        execution_runner::host::run(handler, &STOP)?;
    }
    #[cfg(windows)]
    execution_runner::windows_service::run(handler, &STOP, system)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("agent_service: {error}");
        std::process::exit(1);
    }
}

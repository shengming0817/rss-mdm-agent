//! System daemon or per-user helper. No production authority or test configuration switches.
use std::sync::atomic::AtomicBool;
static STOP: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
extern "C" fn stopped(_: i32) {
    STOP.store(true, std::sync::atomic::Ordering::Release);
}
fn main() {
    #[cfg(target_os = "macos")]
    {
        if let Some(argument) = std::env::args().nth(1) {
            let system = match argument.as_str() {
                "--probe-user" => false,
                "--probe-system" => true,
                _ => {
                    eprintln!("usage: rss-execution-service [--probe-user|--probe-system]");
                    std::process::exit(2)
                }
            };
            let response = execution_runner::macos_service::query(
                br#"{"version":2,"request":{"method":"status","request":"mechanism-probe"}}"#,
                system,
            );
            match response {
                Ok(bytes) => {
                    println!("{}", String::from_utf8_lossy(&bytes));
                    return;
                }
                Err(error) => {
                    eprintln!("execution probe: {error}");
                    std::process::exit(1)
                }
            }
        }
        unsafe {
            libc::signal(libc::SIGTERM, stopped as *const () as usize);
            libc::signal(libc::SIGINT, stopped as *const () as usize);
        }
        if let Err(error) =
            execution_runner::host::run(Box::new(execution_runner::host::Unbound), &STOP)
        {
            eprintln!("execution service: {error}");
            std::process::exit(1)
        }
    }
    #[cfg(windows)]
    {
        let argument = std::env::args().nth(1);
        if matches!(argument.as_deref(), Some("--probe-user" | "--probe-system")) {
            match execution_runner::windows_service::query(
                br#"{"version":2,"request":{"method":"status","request":"mechanism-probe"}}"#,
                argument.as_deref() == Some("--probe-system"),
            ) {
                Ok(bytes) => println!("{}", String::from_utf8_lossy(&bytes)),
                Err(error) => {
                    eprintln!("execution probe: {error}");
                    std::process::exit(1)
                }
            }
            return;
        }
        let system = match argument.as_deref() {
            None => true,
            Some("--user") => false,
            _ => {
                eprintln!("usage: rss-execution-service [--user|--probe-user|--probe-system]");
                std::process::exit(2)
            }
        };
        if let Err(error) = execution_runner::windows_service::run(
            Box::new(execution_runner::host::Unbound),
            &STOP,
            system,
        ) {
            eprintln!("execution service: {error}");
            std::process::exit(1)
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        std::process::exit(1)
    }
}

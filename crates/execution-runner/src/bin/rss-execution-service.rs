//! System daemon or per-user helper. No production authority or test configuration switches.
use std::sync::atomic::{AtomicBool, Ordering};
static STOP: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
extern "C" fn stopped(_: i32) {
    STOP.store(true, Ordering::Release);
}
fn main() {
    #[cfg(target_os = "macos")]
    {
        if let Some(argument) = std::env::args().nth(1) {
            let system = match argument.as_str() {
                "--probe-user" => false,
                "--probe-system" => true,
                _ => std::process::exit(2),
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
                Err(_) => std::process::exit(1),
            }
        }
        unsafe {
            libc::signal(libc::SIGTERM, stopped as *const () as usize);
            libc::signal(libc::SIGINT, stopped as *const () as usize);
        }
        if execution_runner::host::run(Box::new(execution_runner::host::Unbound), &STOP).is_err() {
            std::process::exit(1)
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::process::exit(1)
    }
}

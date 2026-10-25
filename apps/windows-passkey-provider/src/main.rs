//! Thin current-user host. All authenticator and maintenance behavior lives in the module.
#[cfg(windows)]
fn main() {
    use std::sync::Arc;
    use windows_passkey_provider::{registration, NativeUi, ProviderRuntime};
    let result = (|| {
        let args: Vec<_> = std::env::args().skip(1).collect();
        match args.as_slice() {
            [] => {
                println!(
                    "{}",
                    serde_json::to_string(&registration::inspect()?)
                        .map_err(|_| windows_passkey_provider::Error::Protocol)?
                );
                Ok(())
            }
            [arg] if arg.eq_ignore_ascii_case("-Embedding") => {
                ProviderRuntime::new(Arc::new(NativeUi))?.run()
            }
            [arg] if arg == "stop" => registration::stop(),
            [arg] if arg == "register" => registration::register(),
            [arg] if arg == "unregister" => registration::unregister(),
            [arg] if arg == "repair" => registration::repair(),
            [arg] if arg == "enable" => registration::open_settings(),
            _ => Err(windows_passkey_provider::Error::Input),
        }
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows Passkey Provider requires Windows 11 24H2 or later");
    std::process::exit(1);
}

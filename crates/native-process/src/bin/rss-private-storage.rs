use std::io::Write;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [verb, path] if verb == "read" => {
            native_process::private_storage::read(std::path::Path::new(path), 65536)
                .and_then(|bytes| std::io::stdout().write_all(&bytes))
        }
        [verb, path] if verb == "validate" => {
            native_process::private_storage::validate(std::path::Path::new(path))
        }
        [verb, path] if verb == "directory" => {
            native_process::private_storage::directory(std::path::Path::new(path))
        }
        _ => Err(std::io::Error::other("invalid private storage invocation")),
    };
    if result.is_err() {
        eprintln!("private_storage_unavailable");
        std::process::exit(1);
    }
}

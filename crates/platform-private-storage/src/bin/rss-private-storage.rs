use std::{
    io::{self, Write},
    path::Path,
};
fn run(args: &[std::ffi::OsString]) -> io::Result<()> {
    match args {
        [verb, path, limit] if verb == "read" => {
            let limit: usize = limit
                .to_str()
                .and_then(|s| s.parse().ok())
                .filter(|n| (1..=65536).contains(n))
                .ok_or(io::ErrorKind::InvalidInput)?;
            let bytes = platform_private_storage::read(Path::new(path), limit)?;
            io::stdout().write_all(&bytes)
        }
        [verb, path] if verb == "directory" => platform_private_storage::directory(Path::new(path)),
        [verb, path] if verb == "validate-directory" => {
            platform_private_storage::PrivateDirectory::open(Path::new(path)).map(|_| ())
        }
        [verb, path] if verb == "validate-single-link" => {
            platform_private_storage::validate_single_link(Path::new(path))
        }
        [verb, path] if verb == "validate-file" => {
            platform_private_storage::open_existing(Path::new(path)).map(|_| ())
        }
        [verb, path] if verb == "create-new" => {
            platform_private_storage::create_new(Path::new(path))
                .and_then(|file| file.sync_all())
                .and_then(|_| platform_private_storage::sync_parent(Path::new(path)))
        }
        [verb, path] if verb == "validate-if-present" => {
            match platform_private_storage::open_existing(Path::new(path)) {
                Ok(_) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }
        _ => Err(io::ErrorKind::InvalidInput.into()),
    }
}
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if run(&args).is_err() {
        eprintln!("private_storage_unavailable");
        std::process::exit(1);
    }
}

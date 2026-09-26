fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [] => local_service::run(),
        [arg] if arg == "--verification-candidate" => {
            local_service::write_verification_candidate(std::io::stdout().lock())
        }
        _ => Err(local_service::Rejected),
    };
    if result.is_err() {
        eprintln!("local_service_unavailable");
        std::process::exit(1);
    }
}

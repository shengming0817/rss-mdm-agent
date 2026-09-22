fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [verb, id] if verb == "launch" => native_process::launch(id),
        [verb, reference] if verb == "absent" => {
            match serde_json::from_str::<native_process::Scope>(reference) {
                Ok(scope) if native_process::absent(&scope) => Ok(()),
                _ => Err(std::io::Error::other("scope unconfirmed")),
            }
        }
        _ => Err(std::io::Error::other("invalid launcher invocation")),
    };
    if result.is_err() {
        eprintln!("worker_scope_unavailable");
        std::process::exit(1);
    }
}

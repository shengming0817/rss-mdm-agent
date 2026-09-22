fn main() {
    if std::env::args_os().len() != 1 || local_service::run().is_err() {
        eprintln!("local_service_unavailable");
        std::process::exit(1);
    }
}

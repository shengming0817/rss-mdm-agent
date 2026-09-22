//! Negative platform fixture: never add this executable to the installed policy.
fn main() {
    match local_service::query() {
        Ok(_) => {
            eprintln!("unexpected_service_admission");
            std::process::exit(1);
        }
        Err(_) => println!("untrusted_process_rejected"),
    }
}

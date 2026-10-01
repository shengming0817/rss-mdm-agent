use execution_runner::host::*;
fn main() {
    let mut rows = serde_json::Map::new();
    for (name, view) in [
        ("notInstalled", ServiceView::NotInstalled),
        ("configurationRequired", ServiceView::ConfigurationRequired),
        ("serviceRejected", ServiceView::Rejected),
        ("disconnected", ServiceView::Unavailable),
        ("mismatch", ServiceView::Mismatch),
        (
            "registrationRequired",
            ServiceView::Connected {
                status: ServiceStatus::new(Readiness::RegistrationRequired),
            },
        ),
        (
            "notReady",
            ServiceView::Connected {
                status: ServiceStatus::new(Readiness::NotReady),
            },
        ),
        (
            "ready",
            ServiceView::Connected {
                status: ServiceStatus::new(Readiness::Ready),
            },
        ),
    ] {
        rows.insert(name.into(), serde_json::to_value(view).unwrap());
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}

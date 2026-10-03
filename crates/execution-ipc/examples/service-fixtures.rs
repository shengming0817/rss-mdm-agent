use execution_ipc::host::*;
fn sample(readiness: Readiness) -> ServiceStatus {
    ServiceStatus {
        build: env!("CARGO_PKG_VERSION").into(),
        protocol: IPC_VERSION,
        platform: "macos".into(),
        readiness,
    }
}
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
                status: sample(Readiness::RegistrationRequired),
            },
        ),
        (
            "notReady",
            ServiceView::Connected {
                status: sample(Readiness::NotReady),
            },
        ),
        (
            "ready",
            ServiceView::Connected {
                status: sample(Readiness::Ready),
            },
        ),
    ] {
        rows.insert(name.into(), serde_json::to_value(view).unwrap());
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}

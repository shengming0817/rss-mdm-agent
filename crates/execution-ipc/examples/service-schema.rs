fn main() {
    let mut schema =
        serde_json::to_value(schemars::schema_for!(execution_ipc::host::ServiceView)).unwrap();
    schema["x-rss-ipc-version"] = execution_ipc::host::IPC_VERSION.into();
    schema["x-rss-registration-schema"] = serde_json::to_value(schemars::schema_for!(
        execution_ipc::host::RegistrationStatus
    ))
    .unwrap();
    schema["x-rss-deployment-version"] = execution_ipc::host::DEPLOYMENT_VERSION.into();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}

// ref: schemars 1.2.2 src/generate.rs (serialization schema, not an input/approval contract)
fn main() {
    let schema = schemars::generate::SchemaSettings::draft07()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<execution_app::tasks::TaskDetail>();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}

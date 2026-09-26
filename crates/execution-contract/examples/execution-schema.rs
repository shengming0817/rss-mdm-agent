fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = serde_json::json!({"execution": execution_contract::execution_schema(),"audit": execution_contract::audit_schema()});
    println!("{}", serde_json::to_string_pretty(&schemas)?);
    Ok(())
}

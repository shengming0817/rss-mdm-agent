fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = serde_json::json!({"plan": execution_contract::plan_schema(),"audit": execution_contract::audit_schema()});
    println!("{}", serde_json::to_string_pretty(&schemas)?);
    Ok(())
}

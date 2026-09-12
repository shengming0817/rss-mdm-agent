fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = serde_json::json!({"event": ai_session_contract::event_schema(),"command": ai_session_contract::command_schema()});
    println!("{}", serde_json::to_string_pretty(&schemas)?);
    Ok(())
}

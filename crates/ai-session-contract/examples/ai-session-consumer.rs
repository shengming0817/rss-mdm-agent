//! Isolated V2 consumer: no Tauri, Node, provider, database or code generation.
use ai_session_contract::{decode, encode, fingerprint, Event, Limits};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("fixture path required")?;
    let fixtures: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let limits = Limits {
        max_bytes: 262144,
        max_text_bytes: 131072,
        max_depth: 32,
        max_nodes: 16384,
    };
    for value in fixtures["valid"]
        .as_array()
        .ok_or("valid fixtures required")?
    {
        let decoded = decode(&serde_json::to_vec(value)?, &limits)?;
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&encode(&decoded, &limits)?)?,
            *value
        );
    }
    for value in fixtures["valid"]
        .as_array()
        .ok_or("fixtures")?
        .iter()
        .filter(|v| v["kind"] == "event")
    {
        let event: Event = serde_json::from_value(value.clone())?;
        match value["body"]["type"].as_str() {
            Some("error") => assert!(matches!(event, Event::Error { .. })),
            Some("invalidated") => assert!(matches!(event, Event::Invalidated { .. })),
            Some("surface") => match value["body"]["operation"].as_str() {
                Some("create") => assert!(matches!(event, Event::SurfaceCreate { .. })),
                Some("update") => assert!(matches!(event, Event::SurfaceUpdate { .. })),
                Some("delete") => assert!(matches!(event, Event::SurfaceDelete { .. })),
                _ => panic!("unknown operation"),
            },
            _ => {}
        }
    }
    let command: ai_session_contract::Command =
        serde_json::from_value(fixtures["valid"][0].clone())?;
    assert_eq!(fingerprint(&command, &limits)?, fixtures["commandHash"]);
    println!("V2 independent consumer: shared wire golden and command identity passed; no engine invoked");
    Ok(())
}

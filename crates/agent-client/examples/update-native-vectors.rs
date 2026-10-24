//! Refresh the existing fixed-seed wire fixtures after an intentional protocol pin change.
//! No backend checkout or private fixture is read.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rss_mdm_agent_wire::runtime::{NativeCollectionPlan, TaskPayload};
use sha2::{Digest, Sha256};
#[path = "../../../tests/agent-protocol/projection.rs"]
mod projection;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native-vectors.json");
    let mut vectors: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let key = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[42; 32])
        .map_err(|_| "invalid fixture seed")?;
    for vector in vectors.as_array_mut().ok_or("fixture array")? {
        let plan: NativeCollectionPlan = serde_json::from_value(vector["plan"].clone())?;
        let (fields, mappings) = projection::native(&plan);
        for task in vector["tasks"].as_array_mut().ok_or("tasks")? {
            let mut payload: serde_json::Value =
                serde_json::from_str(task["payloadJson"].as_str().ok_or("payload")?)?;
            payload["task"]["fields"] = serde_json::to_value(&fields)?;
            payload["task"]["mappings"] = serde_json::to_value(&mappings)?;
            let payload: TaskPayload = serde_json::from_value(payload)?;
            let bytes = payload.signing_bytes("vector-key")?;
            task["payloadJson"] = serde_json::to_string(&payload)?.into();
            task["signature"] = URL_SAFE_NO_PAD.encode(key.sign(&bytes).as_ref()).into();
            task["signingBytesLength"] = bytes.len().into();
            task["signingBytesSha256"] = format!("{:x}", Sha256::digest(bytes)).into();
        }
    }
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&vectors)?),
    )?;
    Ok(())
}

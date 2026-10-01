//! Controlled signed protocol participant for the installed-service macOS acceptance harness.
//! Only test targets include this fixture; it is never a production transport mode.
#[path = "../../../crates/agent-client/tests/support/mod.rs"]
mod protocol;
use agent_client::wire;
use base64::Engine;
use ring::signature::KeyPair;
use std::io::{BufRead, Write};

#[tokio::main]
async fn main() {
    let server = protocol::Server::new().await;
    {
        let data = server.data.lock().unwrap();
        println!(
            "{}",
            serde_json::json!({"origin": server.url.as_str(), "tenant":data.tenant,"key":base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data.signer.public_key().as_ref())})
        );
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if tx.blocking_send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    loop {
        server.time.set(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        );
        tokio::select! {
            line = rx.recv() => {
                let Some(line) = line else { break };
                let command: serde_json::Value = serde_json::from_str(&line).unwrap();
                let mut data = server.data.lock().unwrap();
                match command["kind"].as_str().unwrap() {
                    "script" => {
                        data.task = uuid::Uuid::new_v4();
                        data.bytes = command["body"].as_str().unwrap().as_bytes().to_vec();
                        data.script();
                        {
                            let wire::TaskPayload::Script(mut payload) = data.offer.as_ref().unwrap().payload.clone() else { unreachable!() };
                            payload.expires_at += 3600;
                            if let Some(timeout) = command["timeout"].as_u64() { payload.timeout_seconds = timeout.clamp(1, 300) as u32; }
                            if command["user"].as_bool() == Some(true) { payload.run_as = wire::ExecutionIdentity::LoggedInUser; }
                            data.offer = Some(data.signed(wire::TaskPayload::Script(payload)));
                        }
                    }
                    "package" => {
                        data.task = uuid::Uuid::new_v4(); data.attempt = uuid::Uuid::new_v4();
                        data.bytes = std::fs::read(command["path"].as_str().unwrap()).unwrap();
                        data.software(1, false);
                        let wire::TaskPayload::Software(mut payload) = data.offer.as_ref().unwrap().payload.clone() else { unreachable!() };
                        if let wire::SoftwareTaskBehavior::Pkg(native) = &mut payload.steps[0].action.behavior {
                            if let wire::SoftwareTaskDetection::PkgReceipt { receipt, .. } = &mut native.detect { *receipt = command["receipt"].as_str().unwrap().into(); }
                        }
                        use sha2::{Digest, Sha256};
                        payload.definition_digest = Sha256::digest(serde_json::to_vec(&payload.steps).unwrap()).into();
                        data.offer = Some(data.signed(wire::TaskPayload::Software(payload)));
                    }
                    "cancel" => { let cancellation = wire::TaskCancellation::new(data.task, data.attempt).unwrap(); data.cancellations.push(cancellation); },
                    "result_failure" => data.result_failure = true,
                    "revoke" => data.denied = true,
                    "status" => (),
                    "stop" => break,
                    _ => panic!("unknown harness command"),
                }
                println!("{}", serde_json::json!({"task": data.task, "attempt":data.attempt,"startRequests":data.start_ops.len(),"resultCalls":data.result_calls,"acknowledged":data.acknowledged,"results":data.results}));
                std::io::stdout().flush().unwrap();
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => (),
        }
    }
}

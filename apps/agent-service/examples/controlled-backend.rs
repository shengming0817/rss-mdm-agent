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
        let mut data = server.data.lock().unwrap();
        data.explicit_offers = true;
        // Real signed-binary hashing and image staging must fit inside the issued Start grant.
        // Keep the short protocol-test default; this native participant explicitly issues 120s.
        data.start_validity_seconds = 120;
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
                        data.software(1, command["user"].as_bool() == Some(true));
                        let wire::TaskPayload::Software(mut payload) = data.offer.as_ref().unwrap().payload.clone() else { unreachable!() };
                        payload.expires_at +=
                            command["validitySeconds"].as_i64().unwrap_or(3660).clamp(2, 3660) - 60;
                        if let wire::SoftwareTaskBehavior::Pkg(native) = &mut payload.steps[0].action.behavior {
                            if let wire::SoftwareTaskDetection::PkgReceipt { receipt, .. } = &mut native.detect { *receipt = command["receipt"].as_str().unwrap().into(); }
                        }
                        use sha2::{Digest, Sha256};
                        payload.definition_digest = Sha256::digest(serde_json::to_vec(&payload.steps).unwrap()).into();
                        data.offer = Some(data.signed(wire::TaskPayload::Software(payload)));
                    }
                    "bundle" => {
                        data.task = uuid::Uuid::new_v4(); data.attempt = uuid::Uuid::new_v4();
                        data.bytes = std::fs::read(command["path"].as_str().unwrap()).unwrap();
                        data.software(1, true);
                        let wire::TaskPayload::Software(mut payload) = data.offer.as_ref().unwrap().payload.clone() else { unreachable!() };
                        payload.expires_at += 3600;
                        let wire::SoftwareTaskBehavior::Pkg(native) = &payload.steps[0].action.behavior else { unreachable!() };
                        let invocation = native.install.clone();
                        let script = |entry: &str| wire::SoftwareTaskScript {
                            interpreter: wire::SoftwareTaskInterpreter::PosixSh,
                            entry: entry.into(), invocation: invocation.clone(),
                        };
                        payload.steps[0].action.package = "native-security-replay".into();
                        payload.steps[0].action.behavior = wire::SoftwareTaskBehavior::Bundle(wire::SoftwareTaskBundleBehavior {
                            archive: "package".into(), manifest: serde_json::from_value(command["manifest"].clone()).unwrap(),
                            install: script("install.sh"), uninstall: None,
                            detect: wire::SoftwareTaskDetection::Script { command: script("detect.sh") },
                        });
                        use sha2::{Digest, Sha256};
                        payload.definition_digest = Sha256::digest(serde_json::to_vec(&payload.steps).unwrap()).into();
                        data.offer = Some(data.signed(wire::TaskPayload::Software(payload)));
                    }
                    "dmg" => {
                        data.task = uuid::Uuid::new_v4(); data.attempt = uuid::Uuid::new_v4();
                        data.bytes = std::fs::read(command["path"].as_str().unwrap()).unwrap();
                        data.software(1,false);
                        let wire::TaskPayload::Software(mut payload) = data.offer.as_ref().unwrap().payload.clone() else { unreachable!() };
                        payload.expires_at += 3600;
                        payload.intent = if command["uninstall"].as_bool() == Some(true) { wire::SoftwareTaskIntent::Uninstall } else { wire::SoftwareTaskIntent::Install };
                        let action = &mut payload.steps[0].action;
                        action.package = command["bundle"].as_str().unwrap().into();
                        action.version = command["version"].as_str().unwrap().into();
                        let wire::SoftwareTaskBehavior::Pkg(native) = &action.behavior else { unreachable!() };
                        let mut invocation = native.install.clone(); invocation.timeout_seconds = 120; invocation.output_bytes = 1_048_576;
                        action.behavior = wire::SoftwareTaskBehavior::Dmg(wire::SoftwareTaskDmg {
                            image: "package".into(),volume: command["volume"].as_str().unwrap().into(),scope: wire::SoftwareTaskScope::System,
                            invocation,upgrade: wire::SoftwareTaskUpgrade::InPlace,
                            payload: if let Some(receipt) = command["receipt"].as_str() {
                                wire::SoftwareTaskDmgPayload::ContainedPkg { path: command["contained"].as_str().unwrap().into(),length: command["length"].as_u64().unwrap(),sha256: serde_json::from_value(command["sha256"].clone()).unwrap(),receipt: receipt.into(),uninstall: None }
                            } else { wire::SoftwareTaskDmgPayload::AppCopy {
                                application: wire::SoftwareTaskMacApplication {
                                    path: command["application"].as_str().unwrap().into(),bundle_id: action.package.clone(),version: action.version.clone(),
                                    material_sha256: [0;32],target_name: command["target"].as_str().unwrap().into(),
                                },uninstall: true,
                            } },
                        });
                        use sha2::{Digest,Sha256};
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
                println!("{}", serde_json::json!({"task": data.task, "attempt":data.attempt,"received":data.received,"startRequests":data.start_ops.len(),"resultCalls":data.result_calls,"acknowledged":data.acknowledged,"results":data.results}));
                std::io::stdout().flush().unwrap();
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => (),
        }
    }
}

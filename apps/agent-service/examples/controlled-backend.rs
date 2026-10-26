//! Controlled signed protocol participant for the installed-service macOS acceptance harness.
//! Only test targets include this fixture; it is never a production transport mode.
#[path = "../../../tests/agent-protocol/mod.rs"]
mod protocol;
use base64::Engine;
use ring::signature::KeyPair;
use rss_mdm_agent_wire::runtime as wire;
use std::io::{BufRead, Write};

/// Protocol fixtures consumed by the real installed owner's native task views.
fn task_history(server: &protocol::Server) -> serde_json::Value {
    use rss_mdm_agent_wire::activities::*;
    let binding = server
        .data
        .lock()
        .unwrap()
        .binding
        .clone()
        .expect("registered native owner");
    let signed = protocol::telemetry::native_performance_configuration(server, &binding);
    let policy = signed.configuration.policy;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let activity: ManagementActivity = serde_json::from_value(serde_json::json!({
        "id":"execution:native-background", "source":"execution", "kind":"execution",
        "phase":"received", "recordedAt":now, "observedAt":now, "origin":"policy",
        "target":{"device":binding.device_id,"registration":binding.registration_id,"generation":binding.generation},
        "links":{"steps":[],"operations":[]}, "delivery":"received", "execution":"running",
        "effect":"unknown", "reporting":null, "decision":null, "auditOutcome":"recorded"
    })).unwrap();
    let mut data = server.data.lock().unwrap();
    data.telemetry.configuration = Some(signed);
    data.management_page = Some(ManagementActivityPage {
        items: vec![activity.clone()],
        next_cursor: None,
        sync_cursor: "native-history-anchor".into(),
        as_of: now,
        retained_from: Some(now - 60),
        coverage: vec![ActivityCoverage {
            source: "execution".into(),
            complete: false,
            reasons: vec!["fixture-partial-source".into()],
        }],
    });
    data.management_detail = Some(ManagementActivityDetail {
        activity: activity.clone(),
        personal: false,
        related: vec![],
        related_cursor: None,
        missing: vec!["effect-not-observed".into()],
        as_of: now,
    });
    serde_json::json!({"policy":policy,"activity":activity.id})
}

// The producer renews the same bounded grant, preserving the original definition
// identity while the real UI journey runs. No consumer expiry bypass is involved.
fn renew_task_history(server: &protocol::Server, now: i64) {
    let mut data = server.data.lock().unwrap();
    if let Some(signed) = &data.telemetry.configuration {
        let mut configuration = signed.configuration.clone();
        configuration.issued_at = now;
        configuration.expires_at = now + 300;
        data.telemetry.configuration = Some(protocol::telemetry::sign(&data, configuration));
    }
}

#[tokio::main]
async fn main() {
    let server = protocol::Server::tls().await;
    {
        let mut data = server.data.lock().unwrap();
        data.explicit_offers = true;
        println!(
            "{}",
            serde_json::json!({"origin": server.url.as_str(), "tenant":data.tenant,"serviceId":protocol::SERVICE_ID,"connections":data.connection_configuration,"key":base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data.signer.public_key().as_ref()), "caPem":String::from_utf8(protocol::pki::ca()).unwrap()})
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
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        server.time.set(now);
        renew_task_history(&server, now);
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
            line = rx.recv() => {
                let Some(line) = line else { break };
                let command: serde_json::Value = serde_json::from_str(&line).unwrap();
                if command["kind"] == "configuration" {
                    println!("{}", serde_json::to_string(&server.configuration()).unwrap());
                    std::io::stdout().flush().unwrap();
                    continue;
                }
                if command["kind"] == "task-history" {
                    println!("{}", task_history(&server));
                    std::io::stdout().flush().unwrap();
                    continue;
                }
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
                    "software-catalog" => {
                        let item = protocol::self_service_software::publish(
                            &mut data, command["path"].as_str().unwrap(), command["receipt"].as_str().unwrap(),
                            command["user"].as_bool().unwrap_or(false),
                            command["validitySeconds"].as_i64().unwrap_or(3600),
                        );
                        match item {
                            Ok(item) => println!("{}", serde_json::json!({"policyId":item.policy_id,"policyRevision":item.policy_revision,"policyVersion":item.policy_version,"item":item})),
                            Err(reason) => println!("{}", serde_json::json!({"error":reason})),
                        }
                        std::io::stdout().flush().unwrap();
                        continue;
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
                        data.offer = Some(data.signed(wire::TaskPayload::try_from(*payload).unwrap()));
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
                                    target_name: command["target"].as_str().unwrap().into(),
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

#[cfg(test)]
mod task_history_tests {
    use super::*;
    #[tokio::test]
    async fn signed_history_renews_time_without_splitting_the_original_policy() {
        let server = protocol::Server::new().await;
        let root = protocol::Root::new();
        let mut client = server.client(&root, agent_client::OpenMode::Create);
        server.register(&mut client).await;
        task_history(&server);
        let before = server
            .data
            .lock()
            .unwrap()
            .telemetry
            .configuration
            .clone()
            .unwrap()
            .configuration;
        assert!(before.signing_bytes("test").is_ok());
        renew_task_history(&server, before.issued_at + 250);
        let data = server.data.lock().unwrap();
        let after = &data.telemetry.configuration.as_ref().unwrap().configuration;
        assert_eq!(after.policy, before.policy);
        assert_eq!(after.policy_version, before.policy_version);
        assert_eq!(after.resource_digest, before.resource_digest);
        assert_eq!(after.expires_at - after.issued_at, 300);
        assert!(after.signing_bytes("test").is_ok());
        assert!(data.management_page.as_ref().unwrap().items[0]
            .execution
            .is_some());
    }
}

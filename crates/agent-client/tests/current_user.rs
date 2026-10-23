use agent_client::{
    current_user::{Owner, Phase},
    Error, OpenMode,
};
use execution_contract::{Id, NativeContextRef};
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use support::*;
fn owner() -> Owner {
    Owner {
        peer: "authenticated-os-user:session".into(),
        context: NativeContextRef {
            id: Id::new("context").unwrap(),
            generation: Id::new("generation").unwrap(),
        },
    }
}
#[tokio::test]
async fn mtls_login_lost_reply_reopen_original_replay_and_logout_preserve_owner() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let user = current_user::user(server.data.lock().unwrap().tenant);
    let proof = "a".repeat(64);
    let owned = owner();
    server.data.lock().unwrap().current_user.lose_reply = true;
    assert_eq!(
        client
            .bind_current_user(&owned, &user, &proof)
            .await
            .unwrap_err(),
        Error::Unavailable
    );
    assert_eq!(
        client.current_user_status().unwrap().phase,
        Phase::BindingUnknown
    );
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    assert_eq!(
        client
            .bind_current_user(&owned, &user, &"b".repeat(64))
            .await
            .unwrap_err(),
        Error::Conflict
    );
    let bound = client
        .bind_current_user(&owned, &user, &proof)
        .await
        .unwrap();
    assert_eq!(bound.revision, 1);
    let mut impostor = user.clone();
    impostor.principal_id = uuid::Uuid::new_v4();
    assert_eq!(
        client
            .bind_current_user(&owned, &impostor, &proof)
            .await
            .unwrap_err(),
        Error::Identity
    );

    {
        let d = server.data.lock().unwrap();
        assert_eq!(d.current_user.inputs.len(), 2);
        assert_eq!(d.current_user.inputs[0], d.current_user.inputs[1]);
    }
    assert!(!std::fs::read(root.path.join("communication.sqlite"))
        .unwrap()
        .windows(64)
        .any(|w| w == proof.as_bytes()));
    server.data.lock().unwrap().current_user.lose_reply = true;
    assert_eq!(
        client
            .retire_current_user(&owned, None, false)
            .await
            .unwrap_err(),
        Error::Unavailable
    );
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    client
        .retire_current_user(&owned, None, false)
        .await
        .unwrap();
    assert_eq!(client.current_user_status().unwrap().phase, Phase::Unbound);
    let d = server.data.lock().unwrap();
    assert_eq!(d.current_user.inputs[2], d.current_user.inputs[3]);
    assert!(d.current_user.user.is_none());
    assert!(d.current_user.owner.is_some());
}
#[tokio::test]
async fn lost_proof_requires_explicit_cleanup_and_conflict_never_blindly_overwrites() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let user = current_user::user(server.data.lock().unwrap().tenant);
    let owned = owner();
    let proof = "a".repeat(64);
    server.data.lock().unwrap().current_user.lose_reply = true;
    assert!(client
        .bind_current_user(&owned, &user, &proof)
        .await
        .is_err());
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    assert_eq!(
        client
            .retire_current_user(&owned, None, false)
            .await
            .unwrap_err(),
        Error::Unavailable
    );
    client
        .retire_current_user(&owned, None, true)
        .await
        .unwrap();
    assert_eq!(client.current_user().await.unwrap().revision, 2);
    client
        .bind_current_user(&owned, &user, &proof)
        .await
        .unwrap();
    server.data.lock().unwrap().current_user.revision += 1;
    assert_eq!(
        client
            .retire_current_user(&owned, None, false)
            .await
            .unwrap_err(),
        Error::Conflict
    );
    let calls = server.data.lock().unwrap().current_user.inputs.len();
    assert_eq!(
        client
            .bind_current_user(&owned, &user, &proof)
            .await
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(server.data.lock().unwrap().current_user.inputs.len(), calls);
    client
        .retire_current_user(&owned, None, true)
        .await
        .unwrap();
}
#[tokio::test]
async fn another_local_owner_logout_cannot_clear_the_later_login() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let user = current_user::user(server.data.lock().unwrap().tenant);
    let first = owner();
    let mut second = first.clone();
    second.peer = "other-os-peer".into();
    second.context.generation = Id::new("other-generation").unwrap();
    client
        .bind_current_user(&first, &user, &"a".repeat(64))
        .await
        .unwrap();
    client
        .bind_current_user(&second, &user, &"b".repeat(64))
        .await
        .unwrap();
    client
        .retire_current_user(&first, None, false)
        .await
        .unwrap();
    assert_eq!(client.current_user().await.unwrap().revision, 2);
    let mut foreign = user.clone();
    foreign.tenant_id = uuid::Uuid::new_v4();
    assert_eq!(
        client
            .bind_current_user(&first, &foreign, &"a".repeat(64))
            .await
            .unwrap_err(),
        Error::Identity
    );
}

#[tokio::test]
async fn renewed_device_certificate_invalidates_the_retained_receipt() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let registration = server.register(&mut client).await;
    let user = current_user::user(server.data.lock().unwrap().tenant);
    client
        .bind_current_user(&owner(), &user, &"a".repeat(64))
        .await
        .unwrap();
    server.time.set(4_070_908_700);
    server.data.lock().unwrap().candidate_not_after = 4_071_000_000;
    client.drive_pki().await.unwrap();
    assert_eq!(client.registration().unwrap(), registration);
    let status = client.current_user_status().unwrap();
    assert_eq!(status.phase, Phase::Invalidated);
    assert!(status.current.is_none());
    client
        .bind_current_user(&owner(), &user, &"b".repeat(64))
        .await
        .unwrap();
    assert_eq!(client.current_user_status().unwrap().phase, Phase::Bound);
}

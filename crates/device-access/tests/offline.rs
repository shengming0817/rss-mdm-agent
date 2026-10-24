//! Actual SQLite boundaries. Test protector is deliberately not an OS protection claim.
use device_access::*;
use rss_mdm_agent_wire::runtime::{AccessWindowsFacts, CredentialReceipt, CredentialSubject};
use std::path::PathBuf;
use uuid::Uuid;
use zeroize::Zeroizing;
struct TestProtector;
impl Protector for TestProtector {
    fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(bytes.to_vec())
    }
    fn open(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        Ok(Zeroizing::new(bytes.to_vec()))
    }
}
struct Fixture {
    root: PathBuf,
    id: Uuid,
    authority: Authority,
    clock: Clock,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("rss-offline-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        Self {
            root,
            id: Uuid::new_v4(),
            authority: Authority {
                instance: Uuid::new_v4(),
                principal: Uuid::new_v4(),
                join: "local-machine".into(),
                source: rss_mdm_agent_wire::runtime::AccessWindowsAuthority::Local,
                login: "LOCAL\\test".into(),
                tenant: Uuid::new_v4(),
                device: "device".into(),
                registration: Uuid::new_v4(),
                registration_generation: 1,
                binding: Uuid::new_v4(),
                binding_generation: 1,
                factor_generation: 1,
                sid: "S-1-5-21-1-2-3-1001".into(),
            },
            clock: Clock {
                utc: 1800000000,
                monotonic: 1000,
                boot: Uuid::new_v4(),
            },
        }
    }
    fn setup(&self) -> (Store, String, Vec<u8>, Uuid, AccessWindowsFacts) {
        let mut store = Store::create(&self.root, self.id).unwrap();
        store
            .bind_registration(self.authority.registration())
            .unwrap();
        let mut candidate = Candidate::generate().unwrap();
        let secret = candidate.totp_secret().to_vec();
        assert!(candidate
            .confirm(&code(&secret, self.clock.utc), self.clock)
            .unwrap());
        let recovery = candidate.recovery_secret().to_owned();
        let operation = Uuid::new_v4();
        store.stage(operation, &candidate, &TestProtector).unwrap();
        assert!(store.stage(operation, &candidate, &TestProtector).is_err());
        let facts = AccessWindowsFacts {
            factor_generation: 1,
            recovery: CredentialReceipt {
                subject: CredentialSubject::RssLogin {
                    binding: self.authority.binding,
                    binding_generation: 1,
                    sid: self.authority.sid.clone(),
                },
                material: Uuid::new_v4(),
                version: 1,
            },
        };
        store
            .activate(
                self.authority.clone(),
                1,
                true,
                operation,
                facts.clone(),
                self.clock,
                &TestProtector,
            )
            .unwrap();
        (store, recovery, secret, operation, facts)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn code(secret: &[u8], utc: i64) -> String {
    use ring::hmac;
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret),
        &((utc / 30) as u64).to_be_bytes(),
    );
    let bytes = tag.as_ref();
    let offset = (bytes[19] & 15) as usize;
    format!(
        "{:06}",
        (u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff)
            % 1_000_000
    )
}
#[test]
fn consumption_survives_reopen_and_activation_replay() {
    let f = Fixture::new();
    let (mut store, _, secret, operation, facts) = f.setup();
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Totp(&code(&secret, f.clock.utc)),
                f.clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Denied
    );
    let clock = Clock {
        utc: f.clock.utc + 30,
        monotonic: 1030,
        ..f.clock
    };
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Totp(&code(&secret, clock.utc)),
                clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Accepted
    );
    drop(store);
    let mut store = Store::open(&f.root, f.id).unwrap();
    store
        .activate(
            f.authority.clone(),
            1,
            true,
            operation,
            facts,
            f.clock,
            &TestProtector,
        )
        .unwrap();
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Totp(&code(&secret, clock.utc)),
                clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Denied
    );
}
#[test]
fn recovery_is_repeatable_and_independent_of_wall_clock_rollback() {
    let f = Fixture::new();
    let (mut store, recovery, secret, _, facts) = f.setup();
    let clock = Clock {
        utc: f.clock.utc - 300,
        monotonic: 1030,
        ..f.clock
    };
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Totp(&code(&secret, clock.utc)),
                clock,
                &TestProtector
            )
            .unwrap(),
        Decision::ClockRollback
    );
    for _ in 0..2 {
        assert_eq!(
            store
                .verify(
                    &f.authority,
                    Factor::Recovery(&recovery),
                    clock,
                    &TestProtector
                )
                .unwrap(),
            Decision::Accepted
        );
    }
    let uses = store.recovery_uses().unwrap();
    assert_eq!(uses.len(), 2);
    assert_ne!(uses[0].operation_id, uses[1].operation_id);
    assert!(uses
        .iter()
        .all(|u| u.material == facts.recovery.material && u.version == 1));
    store
        .acknowledge_recovery_use(uses[0].operation_id)
        .unwrap();
    assert_eq!(store.recovery_uses().unwrap().len(), 1);
}
#[test]
fn failure_budget_survives_reopen_reboot_and_configuration() {
    let f = Fixture::new();
    let (mut store, recovery, _, _, facts) = f.setup();
    for _ in 0..5 {
        assert_eq!(
            store
                .verify(&f.authority, Factor::Totp("wrong"), f.clock, &TestProtector)
                .unwrap(),
            Decision::Denied
        );
    }
    store.configure(&f.authority, 2, true, &facts).unwrap();
    drop(store);
    let mut store = Store::open(&f.root, f.id).unwrap();
    let reboot = Clock {
        utc: f.clock.utc + 100,
        monotonic: 5,
        boot: Uuid::new_v4(),
    };
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Recovery(&recovery),
                reboot,
                &TestProtector
            )
            .unwrap(),
        Decision::Locked
    );
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Recovery(&recovery),
                Clock {
                    monotonic: 35,
                    ..reboot
                },
                &TestProtector
            )
            .unwrap(),
        Decision::Accepted
    );
}
#[test]
fn concurrent_connections_cannot_reuse_an_otp() {
    let f = Fixture::new();
    let (store, _, secret, _, _) = f.setup();
    drop(store);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let clock = Clock {
        utc: f.clock.utc + 30,
        monotonic: 1030,
        ..f.clock
    };
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let root = f.root.clone();
            let id = f.id;
            let authority = f.authority.clone();
            let code = code(&secret, clock.utc);
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(&root, id).unwrap();
                barrier.wait();
                store
                    .verify(&authority, Factor::Totp(&code), clock, &TestProtector)
                    .unwrap()
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        results.iter().filter(|r| **r == Decision::Accepted).count(),
        1
    );
    assert_eq!(
        results.iter().filter(|r| **r == Decision::Denied).count(),
        1
    );
}
#[test]
fn missing_wrong_installation_and_corrupt_database_fail_closed() {
    let f = Fixture::new();
    assert!(Store::open(&f.root, f.id).is_err());
    let (store, _, _, _, _) = f.setup();
    drop(store);
    assert!(Store::open(&f.root, Uuid::new_v4()).is_err());
    std::fs::write(f.root.join("device-access.sqlite"), b"corrupt").unwrap();
    assert!(Store::open(&f.root, f.id).is_err());
}
#[test]
fn unbinding_removes_only_the_exact_factor_authority() {
    let f = Fixture::new();
    let (mut store, recovery, _, _, _) = f.setup();
    let mut other = f.authority.clone();
    other.binding_generation += 1;
    assert!(store.revoke(&other, 2).is_err());
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Recovery(&recovery),
                f.clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Accepted
    );
    store.revoke(&f.authority, 2).unwrap();
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Recovery(&recovery),
                f.clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Disabled
    );
}

#[test]
fn commit_failure_never_accepts_or_consumes_an_otp() {
    let f = Fixture::new();
    let (mut store, _, secret, _, _) = f.setup();
    let connection = rusqlite::Connection::open(f.root.join("device-access.sqlite")).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_commit BEFORE UPDATE ON bindings BEGIN SELECT RAISE(ABORT,'injected storage failure'); END;").unwrap();
    let clock = Clock {
        utc: f.clock.utc + 30,
        monotonic: 1030,
        ..f.clock
    };
    let code = code(&secret, clock.utc);
    assert!(store
        .verify(&f.authority, Factor::Totp(&code), clock, &TestProtector)
        .is_err());
    connection
        .execute_batch("DROP TRIGGER fail_commit;")
        .unwrap();
    assert_eq!(
        store
            .verify(&f.authority, Factor::Totp(&code), clock, &TestProtector)
            .unwrap(),
        Decision::Accepted
    );
}

#[test]
fn revocation_survives_reopen_and_rejects_late_initialization() {
    let f = Fixture::new();
    let (mut store, recovery, _, operation, facts) = f.setup();
    store.revoke(&f.authority, 2).unwrap();
    store.revoke(&f.authority, 2).unwrap();
    drop(store);
    let mut store = Store::open(&f.root, f.id).unwrap();
    assert!(!store.enabled(&f.authority.sid).unwrap());
    assert!(store
        .activate(
            f.authority.clone(),
            3,
            true,
            operation,
            facts,
            f.clock,
            &TestProtector
        )
        .is_err());
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Recovery(&recovery),
                f.clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Disabled
    );
}

#[test]
fn native_source_and_enterprise_subject_are_part_of_consumption_identity() {
    let f = Fixture::new();
    let (mut store, recovery, _, _, _) = f.setup();
    for changed in 0..4 {
        let mut authority = f.authority.clone();
        match changed {
            0 => {
                authority.source =
                    rss_mdm_agent_wire::runtime::AccessWindowsAuthority::ActiveDirectory {
                        domain_sid: "S-1-5-21-1-2-3".into(),
                    }
            }
            1 => authority.principal = Uuid::new_v4(),
            2 => authority.instance = Uuid::new_v4(),
            _ => authority.join = "different-installation".into(),
        }
        assert!(store
            .verify(
                &authority,
                Factor::Recovery(&recovery),
                f.clock,
                &TestProtector
            )
            .is_err());
    }
    assert_eq!(
        store
            .verify(
                &f.authority,
                Factor::Recovery(&recovery),
                f.clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Accepted
    );
}

#[test]
fn registration_replacement_retires_old_bindings_and_cannot_roll_back() {
    let f = Fixture::new();
    let (mut store, _, _, operation, facts) = f.setup();
    let original = f.authority.registration();
    let mut next = original.clone();
    next.id = Uuid::new_v4();
    next.generation += 1;
    store.bind_registration(next.clone()).unwrap();
    assert!(store.bind_registration(original).is_err());
    assert!(store.authority(&f.authority.sid).unwrap().is_none());
    assert!(store
        .activate(
            f.authority.clone(),
            2,
            true,
            operation,
            facts,
            f.clock,
            &TestProtector
        )
        .is_err());
    store.retire_registration().unwrap();
    assert!(store.bind_registration(next).is_err());
}

#[test]
fn factor_replacement_preserves_a_locked_accounts_budget() {
    let f = Fixture::new();
    let (mut store, _, _, _, _) = f.setup();
    for _ in 0..5 {
        store
            .verify(
                &f.authority,
                Factor::Recovery("invalid"),
                f.clock,
                &TestProtector,
            )
            .unwrap();
    }
    let mut candidate = Candidate::generate().unwrap();
    let secret = candidate.totp_secret().to_vec();
    assert!(candidate
        .confirm(&code(&secret, f.clock.utc), f.clock)
        .unwrap());
    let recovery = candidate.recovery_secret().to_owned();
    let operation = Uuid::new_v4();
    store.stage(operation, &candidate, &TestProtector).unwrap();
    let mut authority = f.authority.clone();
    authority.factor_generation += 1;
    let mut facts = store.facts(&f.authority).unwrap();
    facts.factor_generation += 1;
    store
        .activate(
            authority.clone(),
            2,
            true,
            operation,
            facts,
            f.clock,
            &TestProtector,
        )
        .unwrap();
    assert_eq!(
        store
            .verify(
                &authority,
                Factor::Recovery(&recovery),
                f.clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Locked
    );
    drop(store);
    let mut store = Store::open(&f.root, f.id).unwrap();
    assert_eq!(
        store
            .verify(
                &authority,
                Factor::Recovery(&recovery),
                f.clock,
                &TestProtector
            )
            .unwrap(),
        Decision::Locked
    );
}

#[test]
fn join_change_requires_a_fresh_session_without_retiring_local_accounts() {
    let f = Fixture::new();
    let (mut store, _, _, _, _) = f.setup();
    assert!(store.require_join_session("entra:first", 100, 200).is_err());
    assert!(store.require_join_session("entra:first", 220, 230).is_ok());
    assert!(store
        .require_join_session("entra:second", 220, 300)
        .is_err());
    assert!(store.enabled(&f.authority.sid).unwrap());
    drop(store);
    let mut store = Store::open(&f.root, f.id).unwrap();
    assert!(store
        .require_join_session("entra:second", 220, 320)
        .is_err());
    assert!(store.require_join_session("entra:second", 310, 320).is_ok());
    assert!(store
        .require_join_session("entra:second", 310, 290)
        .is_err());
}

#[test]
fn old_schema_is_rejected_without_reinitializing_material() {
    let f = Fixture::new();
    let (store, _, _, _, _) = f.setup();
    drop(store);
    let path = f.root.join("device-access.sqlite");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 1).unwrap();
    drop(db);
    assert!(Store::open(&f.root, f.id).is_err());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM bindings", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

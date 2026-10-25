//! Real SQLite native metadata recovery; these tests do not execute Windows APIs.
use device_access::{Authority, Store};
use rss_mdm_agent_wire::runtime as w;
use uuid::Uuid;
struct Fixture {
    root: std::path::PathBuf,
    installation: Uuid,
    authority: Authority,
    task: w::AccessTaskSpec,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("rss-hello-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let object = Uuid::new_v4();
        let tenant = Uuid::new_v4();
        let bytes = object.to_bytes_le();
        let sid = format!(
            "S-1-12-1-{}-{}-{}-{}",
            u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            u32::from_le_bytes(bytes[12..16].try_into().unwrap())
        );
        let authority = Authority {
            instance: Uuid::new_v4(),
            principal: Uuid::new_v4(),
            join: "entra-join".into(),
            source: w::AccessWindowsAuthority::Entra {
                tenant_id: tenant,
                object_id: object,
            },
            login: "AzureAD\\user".into(),
            tenant,
            device: "device".into(),
            registration: Uuid::new_v4(),
            registration_generation: 1,
            binding: Uuid::new_v4(),
            binding_generation: 1,
            factor_generation: 1,
            sid,
        };
        let task = w::AccessTaskSpec {
            wire_version: 1,
            tenant_id: tenant,
            device_id: authority.device.clone(),
            registration_id: authority.registration,
            generation: 1,
            platform: w::TaskPlatform::Windows,
            architecture: w::TaskArchitecture::X86_64,
            task_id: Uuid::new_v4(),
            attempt_id: Uuid::new_v4(),
            permit: w::TaskPermit::Start,
            expires_at: 1800000030,
            deadline: 1800000060,
            binding_id: authority.binding,
            instance_id: authority.instance,
            principal_id: authority.principal,
            binding_generation: 1,
            factor_generation: 1,
            revision: 1,
            definition_digest: [7; 32],
            action: w::AccessAction::Apply {
                account: w::AccessAccount::Windows {
                    sid: authority.sid.clone(),
                    authority: authority.source.clone(),
                },
                configuration: w::AccessConfiguration {
                    mac: None,
                    windows: w::AccessWindowsConfiguration::Whfb,
                    self_enrollment: true,
                    enabled: true,
                    role: None,
                },
            },
            windows: Some(w::AccessWindowsPlan::Whfb {
                retire: false,
                previous_key: None,
            }),
            authorization: w::TaskAuthorization::Policy {
                policy_id: Uuid::new_v4(),
                policy_version: Uuid::new_v4(),
            },
        };
        task.validate().unwrap();
        Self {
            root,
            installation: Uuid::new_v4(),
            authority,
            task,
        }
    }
    fn store(&self) -> Store {
        let mut s = Store::create(&self.root, self.installation).unwrap();
        s.bind_registration(self.authority.registration()).unwrap();
        s
    }
    fn facts(&self, key: Option<Uuid>, time: i64) -> w::AccessWhfbFacts {
        let w::AccessWindowsAuthority::Entra {
            tenant_id,
            object_id,
        } = self.authority.source
        else {
            unreachable!()
        };
        w::AccessWhfbFacts {
            factor_generation: self.authority.factor_generation,
            tenant_id,
            object_id,
            policy: w::AccessObservation::Present,
            registration: if key.is_some() {
                w::AccessObservation::Present
            } else {
                w::AccessObservation::Absent
            },
            hardware_key: w::AccessObservation::Unknown,
            key_id: key,
            observed_at: time,
        }
    }
    fn next(&mut self) {
        self.task.revision += 1;
        self.task.definition_digest[0] += 1;
        self.task.task_id = Uuid::new_v4();
        self.task.attempt_id = Uuid::new_v4();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
fn current_format_reopens_pending_native_state_and_rejects_stale_observations() {
    let mut f = Fixture::new();
    let mut s = f.store();
    let record = s.prepare_hello(&f.task, f.authority.clone()).unwrap();
    let key = Uuid::new_v4();
    let facts = s
        .observe_hello(&record, f.facts(Some(key), 1800000000), true)
        .unwrap();
    assert_eq!(facts.hardware_key, w::AccessObservation::Unknown);
    drop(s);
    let mut s = Store::open(&f.root, f.installation).unwrap();
    let retained = s.hello(&f.authority.sid).unwrap().unwrap();
    assert!(retained.policy_owned);
    assert_eq!(retained.last_key, Some(key));
    assert!(s
        .observe_hello(&record, f.facts(None, 1799999999), false)
        .is_err());
    f.next();
    let newer = s.prepare_hello(&f.task, f.authority.clone()).unwrap();
    assert!(s
        .observe_hello(&record, f.facts(None, 1800000001), false)
        .is_err());
    assert!(newer.observation.is_none());
}
#[test]
fn replacement_generation_cannot_reuse_old_key_or_erase_key_fence() {
    let mut f = Fixture::new();
    let mut s = f.store();
    let record = s.prepare_hello(&f.task, f.authority.clone()).unwrap();
    let old = Uuid::new_v4();
    s.observe_hello(&record, f.facts(Some(old), 1800000000), true)
        .unwrap();
    f.next();
    f.authority.factor_generation += 1;
    f.task.factor_generation += 1;
    let replacement = s.prepare_hello(&f.task, f.authority.clone()).unwrap();
    assert_eq!(replacement.previous_key, Some(old));
    let reused = s
        .observe_hello(&replacement, f.facts(Some(old), 1800000001), false)
        .unwrap();
    assert_eq!(reused.registration, w::AccessObservation::Unknown);
    assert_eq!(
        s.hello(&f.authority.sid).unwrap().unwrap().last_key,
        Some(old)
    );
    let fresh = Uuid::new_v4();
    s.observe_hello(&replacement, f.facts(Some(fresh), 1800000002), false)
        .unwrap();
    assert_eq!(
        s.hello(&f.authority.sid).unwrap().unwrap().last_key,
        Some(fresh)
    );
}
#[test]
fn retirement_blocks_replay_and_new_binding_until_native_cleanup_observed() {
    let mut f = Fixture::new();
    let mut s = f.store();
    s.prepare_hello(&f.task, f.authority.clone()).unwrap();
    let old = f.task.clone();
    f.next();
    f.task.action = w::AccessAction::Unbind {
        account: w::AccessAccount::Windows {
            sid: f.authority.sid.clone(),
            authority: f.authority.source.clone(),
        },
        disposition: w::AccessAccountDisposition::Keep,
    };
    f.task.windows = Some(w::AccessWindowsPlan::Whfb {
        retire: true,
        previous_key: None,
    });
    let retired = s.prepare_hello(&f.task, f.authority.clone()).unwrap();
    assert!(s.prepare_hello(&old, f.authority.clone()).is_err());
    f.next();
    f.authority.binding = Uuid::new_v4();
    f.task.binding_id = f.authority.binding;
    f.task.action = old.action.clone();
    f.task.windows = old.windows.clone();
    assert!(s.prepare_hello(&f.task, f.authority.clone()).is_err());
    s.observe_hello(&retired, f.facts(None, 1800000000), true)
        .unwrap();
    s.prepare_hello(&f.task, f.authority.clone()).unwrap();
    s.retire_registration().unwrap();
    assert!(s.prepare_hello(&f.task, f.authority.clone()).is_err());
    assert!(s
        .observe_hello(&retired, f.facts(None, 1800000001), true)
        .is_err());
}

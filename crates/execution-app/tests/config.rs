use execution_app::{AppConfig, ConfigChange, ConfigState, Configuration, Error};
use execution_contract::ActorId;

#[test]
fn config_replace_is_atomic_audited_and_fail_closed() {
    let first = AppConfig::test_defaults(1);
    let mut state = Configuration::new(first, execution_app::test_execution_limits()).unwrap();
    let actor = ActorId::new("test-admin").unwrap();
    let mut changes = Vec::<ConfigChange>::new();
    let mut next = AppConfig::test_defaults(2);
    next.max_rules = 0;
    assert_eq!(
        state.replace(next, &actor, |_| Ok(())),
        Err(Error::Configuration)
    );
    assert_eq!(state.revision(), 1);
    assert_eq!(
        state.replace(AppConfig::test_defaults(2), &actor, |_| Err(
            Error::Unavailable
        )),
        Err(Error::Unavailable)
    );
    assert_eq!(state.revision(), 1);
    state
        .replace(AppConfig::test_defaults(2), &actor, |c| {
            changes.push(c.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(state.revision(), 2);
    assert_eq!(changes[0].previous_revision, 1);
    assert_eq!(changes[0].next_revision, 2);
    assert_eq!(changes[0].actor, actor);
    assert_eq!(
        state.replace(AppConfig::test_defaults(1), &actor, |_| Ok(())),
        Err(Error::Conflict)
    );
    state.load_failed(2);
    assert_eq!(state.state(), ConfigState::LastKnownGood);
    state.load_failed(3);
    assert_eq!(state.state(), ConfigState::Degraded);
    assert_eq!(state.active().unwrap_err(), Error::Degraded);
    state
        .replace(AppConfig::test_defaults(3), &actor, |_| Ok(()))
        .unwrap();
    assert_eq!(state.state(), ConfigState::Active);
}

#[test]
fn configuration_never_outgrows_the_immutable_store_envelope() {
    let mut bounds = execution_app::test_execution_limits();
    let mut production = AppConfig::test_defaults(1);
    production.max_timeout_ms = 86_400_000;
    production.max_output_bytes = 16_777_216;
    assert!(Configuration::new(production, bounds).is_err());
    bounds.max_timeout_ms = production.max_timeout_ms;
    bounds.max_output_bytes = production.max_output_bytes;
    let mut state = Configuration::new(production, bounds).unwrap();
    let mut next = production;
    next.revision = 2;
    next.max_output_bytes += 1;
    assert_eq!(
        state.replace(next, &ActorId::new("operator").unwrap(), |_| Ok(())),
        Err(Error::Configuration)
    );
    assert_eq!(state.revision(), 1);
}

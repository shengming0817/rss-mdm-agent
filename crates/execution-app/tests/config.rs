use execution_app::{AppConfig, Error};

#[test]
fn configuration_never_outgrows_the_immutable_store_envelope() {
    let mut bounds = execution_app::test_execution_limits();
    let mut config = AppConfig::test_defaults();
    config.max_timeout_ms = 86_400_000;
    config.max_output_bytes = 16_777_216;
    assert_eq!(config.validate(bounds), Err(Error::Configuration));
    bounds.max_timeout_ms = config.max_timeout_ms;
    bounds.max_output_bytes = config.max_output_bytes;
    config.validate(bounds).unwrap();
    config.max_output_bytes += 1;
    assert_eq!(config.validate(bounds), Err(Error::Configuration));
    config.max_output_bytes = 0;
    assert_eq!(config.validate(bounds), Err(Error::Configuration));
}

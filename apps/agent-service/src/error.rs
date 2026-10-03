//! Product error projection; communication has no dependency on application errors.
use agent_client::Error;
pub(crate) fn app_error(error: execution_app::Error) -> Error {
    use execution_app::Error as E;
    match error {
        E::Denied => Error::Denied,
        E::Unbound => Error::Identity,
        E::UnsupportedSchema { .. } => Error::Schema,
        E::Unsupported => Error::Unsupported,
        E::Clock => Error::Clock,
        E::Conflict => Error::Conflict,
        E::Capacity => Error::Capacity,
        E::Configuration => Error::Configuration,
        E::Unavailable => Error::Unavailable,
        _ => Error::Storage,
    }
}

#[cfg(test)]
#[test]
fn unsupported_execution_database_preserves_the_schema_diagnosis() {
    assert_eq!(
        app_error(execution_app::Error::UnsupportedSchema {
            found: 7,
            supported: 8
        }),
        Error::Schema
    );
}

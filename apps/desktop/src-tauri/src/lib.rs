//! Desktop fixed-service presentation seam. No execution authority.
#[cfg(all(feature = "dev-fixture", not(debug_assertions)))]
compile_error!("dev-fixture must never be included in a release build");
pub mod self_service;

pub mod composition;
pub mod organization_config;

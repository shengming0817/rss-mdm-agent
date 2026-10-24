//! Administrator-owned installation artifacts and native path protection. No service or protocol.
use serde::{Deserialize, Serialize};
const MAX_FRAME: usize = 65536;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{applications_directory, verify_apple_identity};
mod external;
mod policy;
pub use external::read_external;
mod target;
pub use target::Target;
#[cfg(any(windows, test))]
mod policy_acl;
#[cfg(windows)]
mod windows;
pub use policy::{deployment_path, protected, read_protected, Artifact};

/// Installation data is unavailable or fails native protection checks.
#[derive(Debug, thiserror::Error)]
#[error("installation protection failed")]
pub struct Rejected;

pub mod runtime;

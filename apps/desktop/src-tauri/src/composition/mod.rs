//! Desktop IPC and AI composition; execution belongs to the installed system service.
pub mod execution;
pub mod ipc;
pub mod lifecycle;
pub mod origin;
pub mod runtime;

pub mod users;

pub mod credentials;

mod control;

pub mod diagnostics;
mod host;
mod runtime_package;

mod private_link;

pub mod account;

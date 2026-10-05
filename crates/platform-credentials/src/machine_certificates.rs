//! Precise native certificate effects; intent and ownership stay with the product caller.
use std::io;

/// Machine certificate class, independent of product protocol.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CertificateStore {
    /// Machine trust anchor.
    Root,
    /// Machine intermediate, without standalone trust.
    Intermediate,
}

/// Native machine certificate adapter; never uses a user's default store.
pub struct MachineCertificates;

fn input(der: &[u8]) -> io::Result<()> {
    if der.is_empty() || der.len() > 32768 {
        Err(io::ErrorKind::InvalidInput.into())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
#[path = "machine_certificates_macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "machine_certificates_windows.rs"]
mod platform;

#[cfg(not(any(target_os = "macos", windows)))]
impl MachineCertificates {
    pub fn contains(&self, _: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn read(&self, _: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn add(&self, _: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn configure(&self, _: CertificateStore, der: &[u8]) -> io::Result<()> {
        input(der)?;
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn delete(&self, _: CertificateStore, der: &[u8]) -> io::Result<()> {
        input(der)?;
        Err(io::ErrorKind::Unsupported.into())
    }
}

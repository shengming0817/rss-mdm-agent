//! Windows Credential Provider V2. Statically links the offline owner; no service dependency.
//! ref: microsoft/Windows-classic-samples@434f6002bdf9cf9829406c3ff2b33387982d6168
//! Samples/CredentialProvider/cpp/{CSampleCredential.cpp,helpers.cpp} (COM and serialization).
#[cfg(windows)]
mod provider;
#[cfg(windows)]
mod serialization;

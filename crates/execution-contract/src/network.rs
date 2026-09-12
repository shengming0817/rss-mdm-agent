use crate::{ContractError, ErrorKind, Field, Rule};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::num::NonZeroU16;

/// Required network protocol. A runner that cannot enforce the scheme must reject the plan.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum NetworkScheme {
    /// Plain HTTP to the explicitly specified endpoint; no implicit port.
    Http,
    /// HTTPS with TLS to the explicitly specified endpoint; no implicit port.
    Https,
    /// TCP without an application-protocol assertion.
    Tcp,
    /// UDP without an application-protocol assertion.
    Udp,
}
/// Canonical DNS name or IP literal; cannot contain URL syntax, userinfo or a zone identifier.
/// DNS uses lowercase IDNA ASCII without a trailing root dot; IPv6 uses compressed brackets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct NetworkHost(#[schemars(length(min = 1, max = 1024))] String);
impl NetworkHost {
    /// Normalize a host with url's WHATWG/IDNA implementation. No DNS lookup is performed.
    /// Returns a NetworkHost diagnostic for invalid host syntax or excessive input size.
    pub fn new(input: &str) -> Result<Self, ContractError> {
        let invalid = || {
            ContractError::new(
                ErrorKind::InvalidValue,
                Field::NetworkHost,
                Rule::HostSyntax,
            )
        };
        if input.len() > 1024 {
            return Err(ContractError::new(
                ErrorKind::LimitExceeded,
                Field::NetworkHost,
                Rule::ByteLimit,
            ));
        }
        if input.is_empty()
            || input.chars().any(|c| {
                c.is_whitespace()
                    || c.is_control()
                    || matches!(c, '/' | '\\' | '@' | '?' | '#' | '%' | '*')
            })
        {
            return Err(invalid());
        }
        let host = url::Host::parse(input).map_err(|_| invalid())?;
        let canonical = match host {
            url::Host::Domain(domain) => {
                let domain = domain.strip_suffix('.').unwrap_or(&domain);
                if domain.len() > 253
                    || domain.split('.').any(|label| {
                        label.is_empty()
                            || label.len() > 63
                            || !label.as_bytes()[0].is_ascii_alphanumeric()
                            || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                            || !label
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    })
                {
                    return Err(invalid());
                }
                domain.to_ascii_lowercase()
            }
            other => other.to_string(),
        };
        Ok(Self(canonical))
    }
    /// Return the only host representation a frozen-plan consumer should interpret.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for NetworkHost {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(&String::deserialize(d)?)
            .map_err(|error| serde::de::Error::custom(error.for_serde()))
    }
}
/// One exact allowed endpoint. Scheme and port are mandatory; no default-port inference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDestination {
    /// Protocol the runner must enforce; HTTPS cannot be silently treated as unrestricted TCP.
    pub scheme: NetworkScheme,
    /// Canonical DNS/IPv4/bracketed-IPv6 host, without URL components.
    pub host: NetworkHost,
    /// Explicit nonzero destination port in 1..=65535.
    #[serde(deserialize_with = "port")]
    pub port: NonZeroU16,
}
fn port<'de, D: Deserializer<'de>>(d: D) -> Result<NonZeroU16, D::Error> {
    NonZeroU16::new(u16::deserialize(d)?).ok_or_else(|| {
        serde::de::Error::custom(
            ContractError::new(ErrorKind::InvalidValue, Field::NetworkPort, Rule::NonZero)
                .for_serde(),
        )
    })
}

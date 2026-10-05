//! Private decoders for the public PKI HTTP API; this is not a producer schema.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Purpose {
    Agent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    Prepared,
    Active,
    VerifyOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Issuer {
    pub digest: String,
    pub mode: Mode,
    pub certificate: String,
    pub root: String,
    pub crl: String,
    pub root_crl: String,
    pub provisioner: String,
    pub kid: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Trust {
    pub version: u8,
    pub origin: String,
    #[serde(deserialize_with = "canonical_uuid")]
    pub tenant: Uuid,
    pub purpose: Purpose,
    pub revision: u64,
    pub issuers: Vec<Issuer>,
}

fn canonical_uuid<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
    let text = String::deserialize(d)?;
    let value = Uuid::parse_str(&text).map_err(serde::de::Error::custom)?;
    if value.is_nil() || value.to_string() != text {
        return Err(serde::de::Error::custom("invalid PKI tenant"));
    }
    Ok(value)
}

impl Trust {
    pub fn installation_digest(&self) -> Result<String, crate::Error> {
        // Public HTTP installation binding: JSON array, in producer-issued issuer order.
        // CRL bytes have independent signed watermarks and are excluded from this binding.
        let issuers = self
            .issuers
            .iter()
            .map(|g| {
                (
                    &g.digest,
                    g.mode,
                    &g.certificate,
                    &g.root,
                    &g.provisioner,
                    &g.kid,
                )
            })
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&(
            "device-trust-install/v1",
            self.version,
            &self.origin,
            self.tenant,
            self.purpose,
            self.revision,
            issuers,
        ))
        .map_err(|_| crate::Error::Protocol)?;
        Ok(crate::store::hash(&bytes))
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Acknowledgement {
    pub operation: Uuid,
    pub version: u8,
    pub revision: u64,
    pub digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenewalLookup {
    pub wire_version: u8,
    pub operation_id: Uuid,
    pub secret: crate::Secret,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_installation_digest_vector_excludes_crl_bytes() {
        let mut trust: Trust = serde_json::from_value(serde_json::json!({
            "version":1,"origin":"https://example.test","tenant":"11111111-1111-1111-1111-111111111111",
            "purpose":"agent","revision":7,"issuers":[{"digest":"aa","mode":"active","certificate":"D","root":"R","crl":"one","root_crl":"two","provisioner":"rss-agent","kid":"K"}]
        })).unwrap();
        assert_eq!(
            trust.installation_digest().unwrap(),
            "da1f3a6a8ffa109f63d22db4d549089fb1a87ba1e12b3ae04e13ff998542caf8"
        );
        trust.issuers[0].crl = "updated".into();
        assert_eq!(
            trust.installation_digest().unwrap(),
            "da1f3a6a8ffa109f63d22db4d549089fb1a87ba1e12b3ae04e13ff998542caf8"
        );
    }
}

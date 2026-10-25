use crate::{Error, OperationKind, Result};
use passkey_crypto::coset::cbor;
use passkey_types::ctap2::{get_assertion as get, make_credential as make};
use serde::Serialize;
/// Native CTAP command buffers contain the CBOR map, not a CTAPHID command prefix.
pub(crate) enum Request {
    Create(make::Request),
    Assert(get::Request),
}
impl Request {
    pub(crate) fn decode(kind: OperationKind, bytes: &[u8]) -> Result<Self> {
        let mut remaining = bytes;
        let mut budget = 4096;
        scan(&mut remaining, 0, &mut budget)?;
        if !remaining.is_empty() || bytes.len() > 131072 {
            return Err(Error::Input);
        }
        let value: cbor::value::Value = cbor::de::from_reader(bytes).map_err(|_| Error::Input)?;
        unique(&value)?;
        match kind {
            OperationKind::Create => cbor::de::from_reader(bytes)
                .map(Self::Create)
                .map_err(|_| Error::Input),
            OperationKind::Assert => cbor::de::from_reader(bytes)
                .map(Self::Assert)
                .map_err(|_| Error::Input),
        }
    }
    pub(crate) fn rp(&self) -> &str {
        match self {
            Self::Create(r) => &r.rp.id,
            Self::Assert(r) => &r.rp_id,
        }
    }
}
fn unique(v: &cbor::value::Value) -> Result<()> {
    use cbor::value::Value;
    match v {
        Value::Map(items) => {
            for (i, (k, v)) in items.iter().enumerate() {
                if items[..i].iter().any(|(old, _)| old == k) {
                    return Err(Error::Input);
                }
                unique(v)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                unique(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn scan(input: &mut &[u8], depth: u8, budget: &mut usize) -> Result<()> {
    if depth > 16 || *budget == 0 {
        return Err(Error::Input);
    }
    *budget -= 1;
    let (&first, rest) = input.split_first().ok_or(Error::Input)?;
    *input = rest;
    let extra = match first & 31 {
        0..=23 => 0,
        24 => 1,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => return Err(Error::Input),
    };
    if input.len() < extra {
        return Err(Error::Input);
    }
    let mut n = if extra == 0 { (first & 31) as u64 } else { 0 };
    for b in &input[..extra] {
        n = (n << 8) | u64::from(*b);
    }
    *input = &input[extra..];
    match first >> 5 {
        0 | 1 => {}
        2 | 3 => {
            let n = usize::try_from(n).map_err(|_| Error::Input)?;
            if n > input.len() {
                return Err(Error::Input);
            }
            *input = &input[n..];
        }
        4 | 5 => {
            let count = n
                .checked_mul(if first >> 5 == 5 { 2 } else { 1 })
                .ok_or(Error::Input)?;
            if count > *budget as u64 {
                return Err(Error::Input);
            }
            for _ in 0..count {
                scan(input, depth + 1, budget)?;
            }
        }
        7 if matches!(first, 0xf4 | 0xf5 | 0xf6) => {}
        _ => return Err(Error::Input),
    }
    Ok(())
}
pub(crate) fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    cbor::ser::into_writer(value, &mut bytes).map_err(|_| Error::Protocol)?;
    Ok(bytes)
}

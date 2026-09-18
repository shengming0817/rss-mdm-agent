use serde::{
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Deserialize,
};
use serde_json::value::RawValue;
use std::{collections::BTreeSet, fmt};

// Validate structure without first materializing a Value: business numbers are
// subsequently decoded from their original tokens by the owning core.
struct Shape<'a> {
    left: &'a mut usize,
    depth: usize,
    max_depth: usize,
}
impl<'de> DeserializeSeed<'de> for Shape<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        if *self.left == 0 || self.depth > self.max_depth {
            return Err(serde::de::Error::custom("input budget"));
        }
        *self.left -= 1;
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Shape<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        while a
            .next_element_seed(Shape {
                left: self.left,
                depth: self.depth + 1,
                max_depth: self.max_depth,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = a.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(serde::de::Error::custom("duplicate key"));
            }
            a.next_value_seed(Shape {
                left: self.left,
                depth: self.depth + 1,
                max_depth: self.max_depth,
            })?;
        }
        Ok(())
    }
}
#[derive(Deserialize)]
struct RawEnvelope<'a> {
    method: Option<String>,
    #[serde(borrow)]
    params: Option<&'a RawValue>,
}
#[derive(Deserialize)]
struct RawCall<'a> {
    #[serde(borrow)]
    arguments: Option<&'a RawValue>,
}
pub(crate) fn inspect(
    bytes: &[u8],
    max_depth: usize,
    mut nodes: usize,
) -> Result<Option<String>, ()> {
    let mut d = serde_json::Deserializer::from_slice(bytes);
    Shape {
        left: &mut nodes,
        depth: 1,
        max_depth,
    }
    .deserialize(&mut d)
    .map_err(|_| ())?;
    d.end().map_err(|_| ())?;
    let envelope: RawEnvelope<'_> = serde_json::from_slice(bytes).map_err(|_| ())?;
    if envelope.method.as_deref() != Some("tools/call") {
        return Ok(None);
    }
    let params = envelope.params.ok_or(())?;
    let call: RawCall<'_> = serde_json::from_str(params.get()).map_err(|_| ())?;
    Ok(Some(call.arguments.map_or("{}", RawValue::get).to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_keys_are_rejected_before_sdk_maps_can_collapse_them() {
        let frame = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"execution_preview","arguments":{"arguments":{"count":1,"count":2}}}}"#;
        assert!(inspect(frame, 32, 1024).is_err());
    }

    #[test]
    fn original_argument_number_and_whitespace_survive_protocol_decode() {
        let frame = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"execution_preview","arguments":{ "count":9007199254740990.9 }}}"#;
        assert_eq!(
            inspect(frame, 32, 1024).unwrap().as_deref(),
            Some(r#"{ "count":9007199254740990.9 }"#)
        );
    }

    #[test]
    fn depth_and_nodes_are_checked_before_sdk_decode() {
        assert!(inspect(br#"{"x":[[[0]]]}"#, 3, 100).is_err());
        assert!(inspect(br#"{"x":[1,2,3]}"#, 8, 3).is_err());
    }

    #[test]
    fn escaped_method_and_duplicate_key_spellings_follow_json_semantics() {
        assert_eq!(
            inspect(
                br#"{"method":"tools/\u0063all","params":{"arguments":{}}}"#,
                8,
                32
            )
            .unwrap()
            .as_deref(),
            Some("{}")
        );
        assert!(inspect(
            br#"{"method":"tools/call","params":{"arguments":{"count":1,"\u0063ount":2}}}"#,
            8,
            32
        )
        .is_err());
    }
}

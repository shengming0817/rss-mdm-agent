//! The template file and its parameters are independently frozen; no caller-supplied CLI flags.
use execution_app::Error;
use execution_contract::{InputValue, Platform};
use rss_mdm_agent_wire::{osquery::SqlTemplate, runtime::TaskPlatform};
use std::collections::BTreeMap;
/// Current interpreter calling convention; the former version-only profile is not accepted.
pub const PROFILE: &str = "native-osquery-template";
const PREFIX: &[&str] = &[
    "--S",
    "--json",
    "--flagfile=",
    "--disable_extensions=true",
    "--disable_events=true",
    "--disable_distributed=true",
    "--disable_database=true",
];
/// Validate the template AST, bind literal parameters, and construct a bounded shell-mode invocation.
pub fn arguments(
    template: &[u8],
    parameters: &BTreeMap<String, InputValue>,
    max_rows: u16,
    platform: Platform,
) -> Result<Vec<String>, Error> {
    let text = std::str::from_utf8(template).map_err(|_| Error::Denied)?;
    let template = SqlTemplate::new(text).map_err(|_| Error::Denied)?;
    template
        .validate_platform(match platform {
            Platform::Windows => TaskPlatform::Windows,
            Platform::Macos => TaskPlatform::Macos,
            Platform::Linux => return Err(Error::Unsupported),
        })
        .map_err(|_| Error::Denied)?;
    let mut values = serde_json::Map::new();
    for (name, value) in parameters {
        let InputValue::Literal { value } = value else {
            return Err(Error::Denied);
        };
        values.insert(name.clone(), value.clone());
    }
    let query = template
        .render(&serde_json::Value::Object(values), u32::from(max_rows))
        .map_err(|_| Error::Denied)?;
    let mut args: Vec<_> = PREFIX.iter().map(|s| (*s).to_owned()).collect();
    args.push(query);
    Ok(args)
}
/// A frozen execution request can also retain non-executable journal metadata.
/// Reconstruct SQL from its AST-named inputs; the producer still validates its
/// separate SQL parameter map with `arguments`, including rejection of extras.
pub(crate) fn request_arguments(
    template: &[u8],
    parameters: &BTreeMap<String, InputValue>,
    max_rows: u16,
    platform: Platform,
) -> Result<Vec<String>, Error> {
    let text = std::str::from_utf8(template).map_err(|_| Error::Denied)?;
    let names = SqlTemplate::new(text)
        .and_then(|template| template.parameters())
        .map_err(|_| Error::Denied)?;
    let sql = names
        .into_iter()
        .map(|name| {
            parameters
                .get(&name)
                .cloned()
                .map(|value| (name, value))
                .ok_or(Error::Denied)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    arguments(template, &sql, max_rows, platform)
}
pub(crate) fn prefix_matches(args: &[String]) -> bool {
    args.len() == PREFIX.len() + 1 && args.iter().zip(PREFIX).all(|(a, b)| a == b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_collection_metadata_is_not_an_sql_parameter() {
        let template = b"SELECT version FROM osquery_info";
        let mut request = BTreeMap::from([(
            "collection-metadata".into(),
            InputValue::Literal {
                value: serde_json::json!({"private": "must not enter SQL"}),
            },
        )]);
        let args = request_arguments(template, &request, 1, Platform::Macos).unwrap();
        assert_eq!(
            args.last().unwrap(),
            "SELECT version FROM osquery_info LIMIT 2"
        );
        assert!(arguments(template, &request, 1, Platform::Macos).is_err());
        let named = b"SELECT version FROM osquery_info WHERE version = :version";
        assert!(request_arguments(named, &request, 1, Platform::Macos).is_err());
        request.insert(
            "version".into(),
            InputValue::Literal {
                value: serde_json::json!("' OR 1=1 --"),
            },
        );
        let args = request_arguments(named, &request, 1, Platform::Macos).unwrap();
        assert_eq!(
            args.last().unwrap(),
            "SELECT version FROM osquery_info WHERE version = ''' OR 1=1 --' LIMIT 2"
        );
        request.insert(
            "version".into(),
            InputValue::Literal {
                value: serde_json::json!({"not": "a scalar"}),
            },
        );
        assert!(request_arguments(named, &request, 1, Platform::Macos).is_err());
    }
}

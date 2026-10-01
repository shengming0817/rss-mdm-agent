//! The template file and its parameters are independently frozen; no caller-supplied CLI flags.
use execution_app::Error;
use execution_contract::{InputValue, Platform};
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
    let template = rss_mdm_resource::SqlTemplate::new(text).map_err(|_| Error::Denied)?;
    template
        .validate_platform(match platform {
            Platform::Windows => rss_mdm_resource::Platform::Windows,
            Platform::Macos => rss_mdm_resource::Platform::MacOS,
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
pub(crate) fn prefix_matches(args: &[String]) -> bool {
    args.len() == PREFIX.len() + 1 && args.iter().zip(PREFIX).all(|(a, b)| a == b)
}

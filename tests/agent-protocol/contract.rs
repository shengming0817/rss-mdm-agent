//! Offline consumer of the Agent projection exported during an explicit wire upgrade.
//! The snapshot is generated from the locked producer commit; tests never build the backend.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    process::Command,
    sync::{Arc, Mutex, OnceLock},
};

pub struct Contract {
    pub document: Value,
    validators: Mutex<BTreeMap<String, Arc<jsonschema::Validator>>>,
}
fn output(command: &mut Command) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "fixed HTTP contract source failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output.stdout)
}
pub fn from_metadata(metadata: &Value) -> Result<Contract, Box<dyn std::error::Error>> {
    let mut packages = metadata["packages"]
        .as_array()
        .ok_or("metadata packages")?
        .iter()
        .filter(|p| p["name"] == "rss-mdm-agent-wire");
    let package = packages.next().ok_or("wire missing")?;
    if packages.next().is_some() || package["version"] != "1.0.0" {
        return Err("wire identity is not unique".into());
    }
    let source = package["source"].as_str().ok_or("wire source")?;
    let revision = source
        .strip_prefix("git+https://dev.azure.com/shengming0923/rss/_git/rss-mdm?rev=")
        .ok_or("wire source URL")?
        .split_once('#')
        .ok_or("wire source commit")?;
    if revision.0 != revision.1
        || revision.0.len() != 40
        || !revision
            .0
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("wire needs one complete commit".into());
    }
    let root = Path::new(
        metadata["workspace_root"]
            .as_str()
            .ok_or("workspace root")?,
    );
    let directory = root.join("tests/agent-protocol");
    let provenance: Value =
        serde_json::from_slice(&std::fs::read(directory.join("http-v1-source.json"))?)?;
    if provenance["repository"] != "https://dev.azure.com/shengming0923/rss/_git/rss-mdm"
        || provenance["revision"] != revision.0
        || !["agent", "da"].iter().all(|k| {
            provenance["sources"][*k]
                .as_str()
                .is_some_and(|s| s.len() == 64)
        })
    {
        return Err("HTTP snapshot differs from the locked wire source".into());
    }
    let bytes = std::fs::read(directory.join("http-v1.json"))?;
    if provenance["sha256"] != format!("{:x}", Sha256::digest(&bytes)) {
        return Err("HTTP snapshot content digest mismatch".into());
    }
    let document: Value = serde_json::from_slice(&bytes)?;
    if document["formatVersion"] != 1
        || document["schemaDialect"] != "https://json-schema.org/draft/2020-12/schema"
        || !document["endpoints"].as_array().is_some_and(|e| {
            ["agent", "da"]
                .iter()
                .all(|listener| e.iter().any(|op| op["listener"] == *listener))
                && e.iter()
                    .all(|op| matches!(op["listener"].as_str(), Some("agent" | "da")))
        })
        || !document["schemas"].is_object()
    {
        return Err("invalid Agent HTTP snapshot".into());
    }
    Ok(Contract {
        document,
        validators: Mutex::new(BTreeMap::new()),
    })
}
pub fn fixed() -> &'static Contract {
    static FIXED: OnceLock<Contract> = OnceLock::new();
    FIXED.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bytes = output(Command::new("cargo").current_dir(root).args([
            "metadata",
            "--locked",
            "--all-features",
            "--format-version",
            "1",
        ]))
        .expect("locked Cargo metadata");
        from_metadata(&serde_json::from_slice(&bytes).unwrap()).expect("pinned Agent HTTP snapshot")
    })
}
impl Contract {
    pub fn endpoint(&self, method: &str, path: &str) -> Option<&Value> {
        self.document["endpoints"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["method"] == method && matches_path(e["path"].as_str().unwrap(), path))
    }
    pub fn validate(
        &self,
        description: &Value,
        value: &Value,
        direction: &str,
    ) -> Result<(), String> {
        if description["direction"] != direction {
            return Err("schema direction".into());
        }
        let id = description["schemaId"]
            .as_str()
            .ok_or("missing schema identity")?;
        let validator = {
            let mut validators = self.validators.lock().unwrap();
            if let Some(v) = validators.get(id) {
                v.clone()
            } else {
                let schema = self.document["schemas"].get(id).ok_or("unbound schema")?;
                let validator = Arc::new(
                    jsonschema::options()
                        .should_validate_formats(true)
                        .build(schema)
                        .map_err(|e| e.to_string())?,
                );
                validators.insert(id.into(), validator.clone());
                validator
            }
        };
        validator
            .validate(value)
            .map_err(|e| format!("schema mismatch at {}", e.instance_path))
    }
    pub fn request(
        &self,
        method: &str,
        uri: &axum::http::Uri,
        headers: &axum::http::HeaderMap,
        bytes: &[u8],
    ) -> Result<(), String> {
        let endpoint = self
            .endpoint(method, uri.path())
            .ok_or("method/path absent from fixed contract")?;
        let request = &endpoint["request"];
        if !request["path"].is_null() {
            let values: Vec<Value> = endpoint["path"]
                .as_str()
                .unwrap()
                .split('/')
                .zip(uri.path().split('/'))
                .filter(|(pattern, _)| pattern.starts_with('{'))
                .map(|(_, actual)| {
                    let encoded = format!("value={}", actual.replace('+', "%2B"));
                    Value::String(
                        url::form_urlencoded::parse(encoded.as_bytes())
                            .next()
                            .unwrap()
                            .1
                            .into_owned(),
                    )
                })
                .collect();
            let schema = &self.document["schemas"]
                [request["path"]["schemaId"].as_str().ok_or("path schema")?];
            let value = if schema["type"] == "array" {
                Value::Array(values)
            } else {
                values.into_iter().next().ok_or("path parameter")?
            };
            self.validate(&request["path"], &value, "request")?;
        }
        let mut query = serde_json::Map::new();
        for (key, value) in url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()) {
            let schema = &self.document["schemas"]
                [request["query"]["schemaId"].as_str().unwrap_or("")]["properties"][key.as_ref()];
            let types = &schema["type"];
            let numeric = types == "integer"
                || types
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v == "integer"));
            let value = if numeric {
                serde_json::from_str(&value).map_err(|_| "invalid numeric query")?
            } else {
                Value::String(value.into())
            };
            if query.insert(key.into(), value).is_some() {
                return Err("duplicate query parameter".into());
            }
        }
        if request["query"].is_null() {
            if !query.is_empty() {
                return Err("unexpected query".into());
            }
        } else {
            self.validate(&request["query"], &Value::Object(query), "request")?;
        }
        if request["body"].is_null() {
            if !bytes.is_empty() {
                return Err("unexpected request body".into());
            }
        } else {
            if media(headers) != request["body"]["content_type"].as_str() {
                return Err("request content type".into());
            }
            self.validate(
                &request["body"]["schema"],
                &serde_json::from_slice::<Value>(bytes).map_err(|_| "invalid JSON request")?,
                "request",
            )?;
        }
        for (name, description) in request["headers"].as_object().ok_or("request headers")? {
            if let Some(value) = headers.get(name) {
                self.validate(
                    description,
                    &Value::String(value.to_str().map_err(|_| "header encoding")?.into()),
                    "request",
                )?;
            }
        }
        for required in request["required_headers"].as_array().into_iter().flatten() {
            if !headers.contains_key(required.as_str().ok_or("required header")?) {
                return Err("missing required header".into());
            }
        }
        Ok(())
    }
    pub fn response(
        &self,
        endpoint: &Value,
        status: u16,
        headers: &axum::http::HeaderMap,
        bytes: &[u8],
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        for response in endpoint["responses"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["status"] == status)
        {
            let header_result: Result<(), String> = response["headers"]
                .as_object()
                .ok_or("response headers")?
                .iter()
                .try_for_each(|(name, schema)| {
                    let value = headers
                        .get(name)
                        .and_then(|v| v.to_str().ok())
                        .map(|v| Value::String(v.into()))
                        .unwrap_or(Value::Null);
                    self.validate(schema, &value, "response")
                });
            if let Err(error) = header_result {
                errors.push(error);
                continue;
            }
            if response["body"].is_null() {
                if bytes.is_empty() {
                    return Ok(());
                }
                errors.push("unexpected response body".into());
                continue;
            }
            if media(headers) != response["body"]["content_type"].as_str() {
                errors.push("response content type".into());
                continue;
            }
            if media(headers) != Some("application/json") {
                return Ok(());
            }
            let value =
                serde_json::from_slice::<Value>(bytes).map_err(|_| "invalid JSON response")?;
            match self.validate(&response["body"]["schema"], &value, "response") {
                Ok(()) => return Ok(()),
                Err(e) => errors.push(e),
            }
        }
        Err(format!(
            "response {status} does not match fixed operation: {errors:?}"
        ))
    }
}
fn media(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get("content-type")?
        .to_str()
        .ok()?
        .split(';')
        .next()
        .map(str::trim)
}
fn matches_path(template: &str, actual: &str) -> bool {
    let expected: Vec<_> = template.split('/').collect();
    let actual: Vec<_> = actual.split('/').collect();
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|(a, b)| a == &b || (a.starts_with('{') && a.ends_with('}') && !b.is_empty()))
}

//! Enforce this product's independent Cargo dependency boundary.
//! ref: Cargo reference/external-tools.md (metadata dependency graph); toml src/lib.rs.
use std::{fs, path::Path, process::Command};

fn manifest(value: &toml::Value) -> Result<(), &'static str> {
    if value.get("patch").is_some() || value.get("replace").is_some() {
        return Err("dependency overrides are forbidden");
    }
    fn scan(value: &toml::Value) -> Result<(), &'static str> {
        if let Some(table) = value.as_table() {
            if table
                .get("git")
                .and_then(toml::Value::as_str)
                .is_some_and(backend_source)
            {
                return Err("backend Git dependency is forbidden");
            }
            if table
                .get("package")
                .and_then(toml::Value::as_str)
                .is_some_and(|v| v.starts_with("rss-mdm-"))
            {
                return Err("backend package alias is forbidden");
            }
            for value in table.values() {
                scan(value)?;
            }
        }
        Ok(())
    }
    scan(value)
}
fn backend_source(source: &str) -> bool {
    let source = source.to_ascii_lowercase();
    source
        .split(['/', '?', '#'])
        .any(|part| part.trim_end_matches(".git") == "rss-mdm")
}
fn scan(directory: &Path, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if [".git", "target", "node_modules", "worktrees"]
            .iter()
            .any(|v| name == *v)
        {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            scan(&path, root)?;
        } else if name == "Cargo.toml" {
            let value: toml::Value = toml::from_str(&fs::read_to_string(&path)?)?;
            manifest(&value).map_err(|e| format!("{}: {e}", path.display()))?;
            // All local paths, including optional, target, build and dev inputs, stay in this product.
            fn paths(
                value: &toml::Value,
                owner: &Path,
                root: &Path,
            ) -> Result<(), Box<dyn std::error::Error>> {
                if let Some(table) = value.as_table() {
                    if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
                        let resolved = owner.join(path).canonicalize()?;
                        if !resolved.starts_with(root) {
                            return Err("dependency path escapes the product".into());
                        }
                    }
                    for value in table.values() {
                        paths(value, owner, root)?;
                    }
                }
                Ok(())
            }
            // Only dependency declarations contain package paths; binary source paths are independent.
            for (key, value) in value.as_table().ok_or("manifest table")? {
                if key.contains("dependencies") || key == "target" || key == "workspace" {
                    paths(value, path.parent().ok_or("manifest parent")?, root)?;
                }
            }
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    scan(&root, &root)?;
    let output = Command::new("cargo")
        .current_dir(&root)
        .args([
            "metadata",
            "--locked",
            "--all-features",
            "--format-version",
            "1",
        ])
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    for package in metadata["packages"].as_array().ok_or("packages")? {
        if package["name"]
            .as_str()
            .is_some_and(|v| v.starts_with("rss-mdm-") && v != "rss-mdm-desktop")
        {
            return Err("backend package in resolved dependency closure".into());
        }
        if let Some(source) = package["source"].as_str() {
            if backend_source(source)
                || package["name"]
                    .as_str()
                    .is_some_and(|v| v.starts_with("rss-mdm-"))
            {
                return Err("backend package in resolved dependency closure".into());
            }
        } else if !Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("package manifest")?,
        )
        .canonicalize()?
        .starts_with(&root)
        {
            return Err("resolved local dependency escapes the product".into());
        }
    }
    println!("Agent Cargo closure stays within the product and public dependencies");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_multiline_aliases_and_overrides() {
        for input in [
            "[dev-dependencies.alias]\ngit='https://host/_git/rss-mdm'\npackage='harmless-name'",
            "[target.'cfg(windows)'.build-dependencies.alias]\npackage='rss-mdm-inventory'\nversion='1'",
            "[patch.crates-io]\nserde={path='../other-product'}",
            "[replace]\n'package:1.0.0'={path='../other-product'}",
        ] {assert!(manifest(&toml::from_str(input).unwrap()).is_err());}
        assert!(
            manifest(&toml::from_str("[dependencies]\nagent-api={workspace=true}").unwrap())
                .is_ok()
        );
    }
}

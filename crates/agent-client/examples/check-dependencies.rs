//! Enforce the Agent's fixed protocol source and independent dependency closure.
//! ref: Cargo src/cargo/core/source_id.rs@rust-1.96.0 (Git reference and precise commit).
use std::{fs, path::Path, process::Command};

const WIRE: &str = "rss-mdm-agent-wire";
const WIRE_URL: &str = "https://dev.azure.com/shengming0923/rss/_git/rss-mdm";
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

struct Policy {
    rev: String,
}
impl Policy {
    fn from_workspace(value: &toml::Value) -> Result<Self, &'static str> {
        let dep = &value["workspace"]["dependencies"][WIRE];
        let rev = dep
            .get("rev")
            .and_then(toml::Value::as_str)
            .ok_or("protocol rev missing")?;
        if dep.get("git").and_then(toml::Value::as_str) != Some(WIRE_URL)
            || rev.len() != 40
            || !rev
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("protocol requires the approved URL and a complete commit");
        }
        Ok(Self {
            rev: rev.to_owned(),
        })
    }
    fn source(&self) -> String {
        format!("git+{WIRE_URL}?rev={}#{}", self.rev, self.rev)
    }
    fn declaration(&self, name: &str, value: &toml::Value) -> Result<(), &'static str> {
        let actual = value
            .get("package")
            .and_then(toml::Value::as_str)
            .unwrap_or(name);
        if name == WIRE || actual == WIRE {
            if name != WIRE || actual != WIRE {
                return Err("protocol aliases are forbidden");
            }
            if value.get("workspace").and_then(toml::Value::as_bool) == Some(true)
                && value.as_table().is_some_and(|t| t.len() == 1)
            {
                return Ok(());
            }
            let table = value.as_table().ok_or("protocol source missing")?;
            if table.len() != 2
                || value.get("git").and_then(toml::Value::as_str) != Some(WIRE_URL)
                || value.get("rev").and_then(toml::Value::as_str) != Some(&self.rev)
            {
                return Err("protocol source differs from the fixed workspace declaration");
            }
        } else if actual.starts_with("rss-mdm-")
            || value
                .get("git")
                .and_then(toml::Value::as_str)
                .is_some_and(backend_source)
        {
            return Err("backend implementation dependencies are forbidden");
        }
        Ok(())
    }
    fn manifest(&self, value: &toml::Value) -> Result<(), &'static str> {
        if value.get("patch").is_some() || value.get("replace").is_some() {
            return Err("dependency overrides are forbidden");
        }
        fn visit(policy: &Policy, value: &toml::Value) -> Result<(), &'static str> {
            if let Some(table) = value.as_table() {
                for (key, child) in table {
                    if matches!(
                        key.as_str(),
                        "dependencies" | "dev-dependencies" | "build-dependencies"
                    ) {
                        for (name, dep) in child.as_table().ok_or("dependency table")? {
                            policy.declaration(name, dep)?;
                        }
                    } else {
                        visit(policy, child)?;
                    }
                }
            }
            Ok(())
        }
        visit(self, value)
    }
    fn package(
        &self,
        package: &serde_json::Value,
        root: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let name = package["name"].as_str().ok_or("package name")?;
        if name == WIRE {
            if package["version"] != "1.0.0" || package["source"] != self.source() {
                return Err("resolved protocol identity differs from its fixed source".into());
            }
        } else if let Some(source) = package["source"].as_str() {
            if name.starts_with("rss-mdm-") || source != CRATES_IO {
                return Err("non-public package in resolved dependency closure".into());
            }
        } else if (name.starts_with("rss-mdm-")
            && !["rss-mdm-desktop", "rss-mdm-cli"].contains(&name))
            || !Path::new(
                package["manifest_path"]
                    .as_str()
                    .ok_or("package manifest")?,
            )
            .canonicalize()?
            .starts_with(root)
        {
            return Err("resolved local dependency escapes the product".into());
        }
        Ok(())
    }
}
fn backend_source(source: &str) -> bool {
    source
        .to_ascii_lowercase()
        .split(['/', '?', '#'])
        .any(|part| part.trim_end_matches(".git") == "rss-mdm")
}
fn configuration(value: &toml::Value) -> Result<(), &'static str> {
    if value.get("paths").is_some()
        || value.get("source").is_some()
        || value.get("patch").is_some()
        || value.get("replace").is_some()
    {
        return Err("Cargo dependency source overrides are forbidden");
    }
    Ok(())
}
fn paths(value: &toml::Value, owner: &Path, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(table) = value.as_table() {
        if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
            if !owner.join(path).canonicalize()?.starts_with(root) {
                return Err("dependency path escapes the product".into());
            }
        }
        for child in table.values() {
            paths(child, owner, root)?;
        }
    }
    Ok(())
}
fn scan(directory: &Path, root: &Path, policy: &Policy) -> Result<(), Box<dyn std::error::Error>> {
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
            scan(&path, root, policy)?;
        } else if name == "Cargo.toml" {
            let value: toml::Value = toml::from_str(&fs::read_to_string(&path)?)?;
            policy
                .manifest(&value)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            for (key, child) in value.as_table().ok_or("manifest table")? {
                if key.contains("dependencies") || key == "target" || key == "workspace" {
                    paths(child, path.parent().ok_or("manifest parent")?, root)?;
                }
            }
        } else if path.parent().is_some_and(|p| p.ends_with(".cargo"))
            && (name == "config.toml" || name == "config")
        {
            configuration(&toml::from_str(&fs::read_to_string(&path)?)?)?;
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let policy = Policy::from_workspace(&toml::from_str(&fs::read_to_string(
        root.join("Cargo.toml"),
    )?)?)?;
    scan(&root, &root, &policy)?;
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
    let packages = metadata["packages"].as_array().ok_or("packages")?;
    if packages.iter().filter(|p| p["name"] == WIRE).count() != 1 {
        return Err("unique protocol package missing".into());
    }
    for package in packages {
        policy.package(package, &root)?;
    }
    println!("Agent consumes one fixed protocol package and public dependencies");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> Policy {
        Policy {
            rev: "0".repeat(40),
        }
    }
    #[test]
    fn accepts_only_canonical_protocol_declarations() {
        let p = policy();
        for section in [
            "dependencies",
            "dev-dependencies",
            "build-dependencies",
            "target.'cfg(windows)'.dependencies",
            "workspace.dependencies",
        ] {
            let input = format!("[{section}]\n{WIRE}={{git='{WIRE_URL}',rev='{}'}}", p.rev);
            assert!(p.manifest(&toml::from_str(&input).unwrap()).is_ok());
            for bad in [
                "branch='develop'",
                "tag='v1'",
                "rev='short'",
                "rev='1111111111111111111111111111111111111111'",
                "path='../backend'",
            ] {
                let input = format!("[{section}]\n{WIRE}={{git='{WIRE_URL}',{bad}}}");
                assert!(
                    p.manifest(&toml::from_str(&input).unwrap()).is_err(),
                    "{input}"
                );
            }
        }
        assert!(p
            .manifest(
                &toml::from_str(&format!("[dependencies]\n{WIRE}={{workspace=true}}")).unwrap()
            )
            .is_ok());
    }
    #[test]
    fn rejects_hidden_backend_dependencies_and_overrides() {
        for input in [
            "[dev-dependencies.alias]\ngit='https://host/_git/rss-mdm'\npackage='harmless-name'",
            "[target.'cfg(windows)'.build-dependencies.alias]\npackage='rss-mdm-inventory'\nversion='1'",
            "[dependencies.alias]\npackage='rss-mdm-agent-wire'\nversion='1'\noptional=true",
            "[patch.crates-io]\nserde={path='../other-product'}",
            "[replace]\n'package:1.0.0'={path='../other-product'}",
        ] { assert!(policy().manifest(&toml::from_str(input).unwrap()).is_err()); }
        for input in [
            "paths=['../other']",
            "[source.crates-io]\nreplace-with='other'",
            "[patch.crates-io]\nserde={path='../other'}",
        ] {
            assert!(configuration(&toml::from_str(input).unwrap()).is_err());
        }
    }
    #[test]
    fn installed_cli_is_a_local_product_member_not_a_backend_exception() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let mut package = serde_json::json!({"name":"rss-mdm-cli","source":null,"manifest_path":root.join("apps/cli/Cargo.toml")});
        assert!(policy().package(&package, &root).is_ok());
        package["name"] = "rss-mdm-inventory".into();
        assert!(policy().package(&package, &root).is_err());
        package["name"] = "rss-mdm-cli".into();
        package["manifest_path"] = "/outside/product/Cargo.toml".into();
        assert!(policy().package(&package, &root).is_err());
    }
    #[test]
    fn rejects_wrong_resolved_identity_even_under_the_backend_url() {
        let p = policy();
        let mut pkg = serde_json::json!({"name":WIRE,"version":"1.0.0","source":p.source()});
        assert!(p.package(&pkg, Path::new(".")).is_ok());
        for (key, value) in [("name", "rss-mdm-inventory"), ("version", "1.0.1"), ("source", CRATES_IO), ("source", "git+https://host/_git/rss-mdm?branch=develop#0000000000000000000000000000000000000000")] {
            let mut bad = pkg.clone(); bad[key] = value.into(); assert!(p.package(&bad, Path::new(".")).is_err());
        }
        pkg["source"] = serde_json::Value::Null;
        assert!(p.package(&pkg, Path::new(".")).is_err());
    }
}

//! Agent-owned fixtures over actual TLS, SQLite and Git plumbing. No backend implementation input.
use super::*;
use std::{
    io::Write,
    process::{Command, Stdio},
};

pub const TOKEN: &str = "source-read-test-only-token-123456789";
#[derive(Clone)]
pub struct Source {
    pub document: Value,
    pub pack: Vec<u8>,
    pub commit: String,
    pub identifier: String,
    pub credential: Option<SoftwareReadCredential>,
    pub requests: Vec<String>,
    pub client_authenticated: Vec<bool>,
    pub denied: bool,
}
impl Source {
    pub fn response(
        &mut self,
        path: &str,
        headers: &HeaderMap,
        body: &[u8],
        authenticated: bool,
    ) -> Response {
        self.requests.push(path.into());
        self.client_authenticated.push(authenticated);
        if self.denied {
            return StatusCode::FORBIDDEN.into_response();
        }
        if self.credential.is_some() {
            let expected = format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("rss:{TOKEN}"))
            );
            assert_eq!(
                headers.get("authorization").and_then(|v| v.to_str().ok()),
                Some(expected.as_str())
            );
        } else {
            assert!(!headers.contains_key("authorization"));
        }
        if path.ends_with("information") {
            return axum::Json(json!({"Data":{"SourceIdentifier":self.identifier}}))
                .into_response();
        }
        if path.contains("packageManifests/") {
            return axum::Json(json!({"Data":self.document})).into_response();
        }
        if path.ends_with("git-upload-pack") {
            assert!(body
                .windows(self.commit.len())
                .any(|b| b == self.commit.as_bytes()));
            return [b"0008NAK\n".as_slice(), &self.pack]
                .concat()
                .into_response();
        }
        StatusCode::NOT_FOUND.into_response()
    }
}
pub fn git(root: &std::path::Path, args: &[&str], input: &[u8]) -> Vec<u8> {
    let mut process = Command::new("/usr/bin/git")
        .current_dir(root)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.test")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.test")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process.stdin.take().unwrap().write_all(input).unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
pub fn pack(documents: &[(&str, &[u8], &str)], parent: bool) -> (String, Vec<u8>) {
    let root = Root::new();
    git(&root.path, &["init", "--bare", "."], &[]);
    let mut trees = BTreeMap::<String, Vec<String>>::new();
    for (path, bytes, mode) in documents {
        let (folder, leaf) = path.split_once('/').unwrap();
        let blob =
            String::from_utf8(git(&root.path, &["hash-object", "-w", "--stdin"], bytes)).unwrap();
        trees
            .entry(folder.into())
            .or_default()
            .push(format!("{mode} blob {}\t{leaf}\n", blob.trim()));
    }
    let mut entries = String::new();
    for (folder, lines) in trees {
        let tree =
            String::from_utf8(git(&root.path, &["mktree"], lines.concat().as_bytes())).unwrap();
        entries.push_str(&format!("040000 tree {}\t{folder}\n", tree.trim()));
    }
    let tree = String::from_utf8(git(&root.path, &["mktree"], entries.as_bytes())).unwrap();
    let first = String::from_utf8(git(
        &root.path,
        &["commit-tree", tree.trim(), "-m", "fixture"],
        &[],
    ))
    .unwrap();
    let commit = if parent {
        String::from_utf8(git(
            &root.path,
            &[
                "commit-tree",
                tree.trim(),
                "-p",
                first.trim(),
                "-m",
                "child",
            ],
            &[],
        ))
        .unwrap()
    } else {
        first
    };
    let pack = git(
        &root.path,
        &["pack-objects", "--stdout", "--revs"],
        commit.as_bytes(),
    );
    (commit.trim().into(), pack)
}
impl Data {
    pub fn software_native(&mut self, origin: &url::Url, brew: bool) {
        self.software(1, false);
        let TaskPayload::Software(mut spec) = self.offer.take().unwrap().payload else {
            unreachable!()
        };
        spec.platform = if brew {
            TaskPlatform::Macos
        } else {
            TaskPlatform::Windows
        };
        let step = &mut spec.steps[0];
        let SoftwareTaskBehavior::Pkg(mut native) = step.action.behavior.clone() else {
            unreachable!()
        };
        if brew {
            let session_id = Uuid::new_v4();
            native.scope = SoftwareTaskScope::User;
            native.install.run_as = ExecutionIdentity::LoggedInUser;
            native.upgrade_invocation.run_as = ExecutionIdentity::LoggedInUser;
            spec.execution_context.interactive_user = Some(SoftwareInteractiveUser {
                windows_authority: None,
                identity: "501".into(),
                generated_uid: Some(Uuid::new_v4().to_string()),
                session_id,
                administrator: false,
            });
            step.target = SoftwareExecutionTarget::User {
                identity: "501".into(),
                session_id,
            };
        } else {
            step.action.package = "Fixture.App".into();
            // WinGet system tasks still require the existing independently observed admin login.
            spec.execution_context.interactive_user = Some(SoftwareInteractiveUser {
                windows_authority: None,
                identity: "S-1-5-21-100-200-300-1001".into(),
                generated_uid: None,
                session_id: Uuid::new_v4(),
                administrator: true,
            });
            native.detect = SoftwareTaskDetection::MsiProduct {
                product_code: "{00000000-0000-0000-0000-000000000001}".into(),
                version: "1.0".into(),
            };
        }
        step.action.behavior = if brew {
            SoftwareTaskBehavior::Brew(native)
        } else {
            SoftwareTaskBehavior::Winget(native)
        };
        let artifact = SoftwareExportArtifact {
            url: origin.join("native/artifact.pkg").unwrap().into(),
            length: self.bytes.len() as u64,
            sha256: Sha256::digest(&self.bytes).into(),
        };
        let document = json!({"PackageIdentifier":step.action.package,"Versions":[{"PackageVersion":step.action.version,"DefaultLocale":{"PackageLocale":"en-US","PackageName":"Fixture","Publisher":"Fixture","License":"Proprietary","ShortDescription":"Fixture package"},"Installers":[{"Architecture":"arm64","InstallerType":"msi","Scope":"machine","InstallerUrl":artifact.url,"InstallerSha256":""}]}]});
        let mut document = document;
        document["Versions"][0]["Installers"][0]["InstallerSha256"] =
            Value::String(artifact.sha256.iter().map(|b| format!("{b:02x}")).collect());
        let formula = b"class Fixture < Formula\nend\n";
        let (commit, pack) = if brew {
            pack(
                &[("Formula/fixture-0.rb", formula.as_slice(), "100644")],
                false,
            )
        } else {
            (String::new(), vec![])
        };
        let binding = SoftwareExportBinding {
            target: "native-fixture".into(),
            tenant_id: self.tenant,
            publication: [1; 32],
            target_digest: [2; 32],
            resource: step.definition.resource.clone(),
            resource_version: step.definition.version.clone(),
            resource_digest: step.definition.digest,
            definition_digest: [4; 32],
            document_sha256: if brew {
                rss_mdm_agent_wire::runtime::brew_documents_digest([(
                    "Formula/fixture-0.rb",
                    formula.as_slice(),
                )])
                .unwrap()
            } else {
                rss_mdm_agent_wire::runtime::winget_document_digest(&document).unwrap()
            },
            dependencies: vec![],
            artifacts: vec![artifact],
        };
        let credential = brew.then(|| SoftwareReadCredential {
            source: binding.target.clone(),
            reference: "tap-read".into(),
        });
        if let Some(credential) = &credential {
            spec.execution_context
                .source_credentials
                .push(credential.clone());
        }
        step.export = if brew {
            SoftwareTaskExport::Brew {
                binding,
                uri: origin.join("native/frozen.git").unwrap().into(),
                commit: commit.clone(),
                tap: "rss/fixture".into(),
                credential_reference: "tap-read".into(),
            }
        } else {
            SoftwareTaskExport::Winget {
                binding,
                uri: origin.join("native/frozen/").unwrap().into(),
                identifier: "rss-fixture".into(),
            }
        };
        spec.definition_digest = Sha256::digest(serde_json::to_vec(&spec.steps).unwrap()).into();
        self.offer = Some(self.signed(TaskPayload::try_from(*spec).unwrap()));
        self.software_source = Some(Source {
            document,
            pack,
            commit,
            identifier: "rss-fixture".into(),
            credential,
            requests: vec![],
            client_authenticated: vec![],
            denied: false,
        });
    }
}

//! The CLI resolves its policy independently of the caller's cwd/ProgramData.
use std::process::Command;

#[test]
fn caller_program_data_cannot_redirect_the_native_policy_loader() {
    let binary = env!("CARGO_BIN_EXE_rss-local-service");
    let load = |fake: bool| {
        let mut command = Command::new(binary);
        command.arg("--verification-candidate");
        if fake {
            command.env(
                "ProgramData",
                std::env::temp_dir().join("rss-untrusted-policy"),
            );
        }
        let output = command.output().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["version"], 1);
        assert!(!value["policyPath"]
            .as_str()
            .unwrap_or_default()
            .contains("rss-untrusted-policy"));
        if !output.status.success() {
            assert_eq!(value["phase"], "rejected");
            assert!(value.get("executable").is_none());
            assert!(value.get("negative").is_none());
        }
        value["policyPath"].clone()
    };
    assert_eq!(load(false), load(true));
}

#[test]
fn cli_rejects_arbitrary_policy_paths() {
    let output = Command::new(env!("CARGO_BIN_EXE_rss-local-service"))
        .args(["--verification-candidate", "attacker-policy.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

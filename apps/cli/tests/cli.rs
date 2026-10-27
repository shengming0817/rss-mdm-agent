use std::process::Command;

#[test]
fn actual_cli_help_and_json_errors_do_not_require_a_service_or_echo_input() {
    let cli = env!("CARGO_BIN_EXE_rssmdm");
    let help = Command::new(cli).arg("--help").output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for command in [
        "capabilities",
        "preview",
        "invoke",
        "query",
        "tasks",
        "batches",
        "checkin",
    ] {
        assert!(text.contains(command));
    }
    let rejected = Command::new(cli)
        .args(["--json", "invoke", "https://sensitive-canary.invalid/"])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    let response: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(response["error"]["code"], "invalid_input");
    assert!(!String::from_utf8(rejected.stderr)
        .unwrap()
        .contains("sensitive-canary"));
}

#[test]
fn cli_does_not_accept_argv_credentials_or_implicit_business_cancel() {
    for args in [
        vec!["invoke", "alias:repair", "--token", "sensitive-canary"],
        vec!["query", "submit", "arbitrary-sql"],
        vec!["tasks", "wait", "request:a", "--cancel"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_rssmdm"))
            .arg("--json")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(!String::from_utf8(result.stderr)
            .unwrap()
            .contains("sensitive-canary"));
    }
}

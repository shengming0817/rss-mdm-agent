#![cfg(target_os = "macos")]

#[test]
fn slow_native_helper_publication_is_coalesced_and_released_on_ack_or_loss() {
    let binary = std::env::temp_dir().join(format!("rss-publication-{}", std::process::id()));
    let compile = std::process::Command::new("/usr/bin/clang")
        .args([
            "-fobjc-arc",
            "-fblocks",
            "-framework",
            "Foundation",
            "-framework",
            "CoreGraphics",
        ])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/macos_publication.m"
        ))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let result = std::process::Command::new(&binary).output().unwrap();
    std::fs::remove_file(binary).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

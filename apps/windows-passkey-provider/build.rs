// ref: Microsoft grant-identity-to-nonpackaged-apps, embedded msix identity metadata.
fn main() {
    println!("cargo:rerun-if-env-changed=RSS_PASSKEY_PUBLISHER");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let publisher = std::env::var("RSS_PASSKEY_PUBLISHER")
        .expect("build through scripts/build-passkey-provider.mjs");
    let publisher = publisher
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let manifest = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<assemblyIdentity version="1.0.0.0" name="RSS.PersonalPasskeys"/>
<msix xmlns="urn:schemas-microsoft-com:msix.v1" publisher="{publisher}" packageName="RSS.PersonalPasskeys" applicationId="Provider"/>
<dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges><requestedExecutionLevel level="asInvoker" uiAccess="false"/></requestedPrivileges></security></trustInfo>
</assembly>"#
    );
    let path =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("provider.manifest");
    std::fs::write(&path, manifest).unwrap();
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", path.display());
}

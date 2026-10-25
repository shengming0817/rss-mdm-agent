fn main() {
    println!("cargo:rerun-if-changed=src/macos.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/macos.m")
            .compile("rss_installation_acl");
    }
}

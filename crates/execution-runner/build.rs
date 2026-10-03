fn main() {
    println!("cargo:rerun-if-changed=src/macos_files.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/macos_files.m")
            .flag("-mmacosx-version-min=13.0")
            .compile("rss_execution_files");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src/macos_service.m");
    println!("cargo:rerun-if-changed=src/macos_ipc.h");
    println!("cargo:rerun-if-changed=src/macos_security_probe.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/macos_service.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-mmacosx-version-min=13.0")
            .compile("rss_execution_xpc");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        // Separate archive: only the acceptance example references this object.
        cc::Build::new()
            .file("src/macos_security_probe.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-mmacosx-version-min=13.0")
            .compile("rss_security_probe");
    }
}

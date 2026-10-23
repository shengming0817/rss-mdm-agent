fn main() {
    println!("cargo:rerun-if-changed=src/local_software/native/launch.m");
    println!("cargo:rerun-if-changed=src/telemetry/environment.m");
    println!("cargo:rerun-if-changed=src/account_identity.m");
    println!("cargo:rerun-if-changed=src/collection_worker/macos/native.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/account_identity.m")
            .file("src/local_software/native/launch.m")
            .file("src/telemetry/environment.m")
            .file("src/collection_worker/macos/native.m")
            .flag("-Wno-deprecated-declarations")
            .flag("-fobjc-arc")
            .flag("-mmacosx-version-min=13.0")
            .compile("rss_account_identity");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Network");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=OpenDirectory");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
}

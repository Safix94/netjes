fn main() {
    println!("cargo:rerun-if-changed=native/macos_thumbnails.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("native/macos_thumbnails.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-mmacosx-version-min=13.0")
            .warnings_into_errors(true)
            .compile("netjes_macos_thumbnails");
        for framework in ["Foundation", "AppKit", "QuickLookThumbnailing"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    tauri_build::build()
}

fn main() {
    // macOS: burning .bin/.cue images through Apple's Disc Recording framework.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=native/drburn.c");
        cc::Build::new().file("native/drburn.c").flag("-Wno-deprecated-declarations").compile("chinotto_drburn");
        println!("cargo:rustc-link-lib=framework=DiscRecording");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
    tauri_build::build()
}

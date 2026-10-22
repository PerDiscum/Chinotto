//! Compiles the vendored libgpod (libgpod/src) into this crate.
//!
//! Where glib comes from:
//! - Linux: the system's glib (already required by WebKitGTK), found with
//!   pkg-config and linked dynamically.
//! - macOS: a static glib built by `scripts/build-glib-macos.sh` into
//!   `CHINOTTO_DEPS/<target triple>/`, linked statically together with its
//!   helper libraries, so the app bundle needs no extra libraries. If
//!   CHINOTTO_DEPS is set on Linux, a static glib there is used the same way.
//!
//! libxml2 and zlib are system libraries on both.

use std::path::PathBuf;

const SOURCES: &[&str] = &[
    "db-artwork-debug.c",
    "db-artwork-parser.c",
    "db-artwork-writer.c",
    "db-image-parser.c",
    "db-parse-context.c",
    "itdb_artwork.c",
    "itdb_chapterdata.c",
    "itdb_device.c",
    "itdb_hash58.c",
    "itdb_hash72.c",
    "itdb_hashAB.c",
    "itdb_iphone.c",
    "itdb_itunesdb.c",
    "itdb_photoalbum.c",
    "itdb_playlist.c",
    "itdb_plist.c",
    "itdb_sysinfo_extended_parser.c",
    "itdb_thumb.c",
    "itdb_track.c",
    "itdb_tzinfo.c",
    "itdb_zlib.c",
    "ithumb-writer.c",
    "pixmaps.c",
    "rijndael.c",
];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("libgpod/src");
    let target = std::env::var("TARGET").unwrap();
    let macos = target.contains("apple-darwin");

    println!("cargo:rerun-if-changed=libgpod");
    track_tree(&src.join("hashab"));
    track_tree(&src.join("podbox"));
    println!("cargo:rerun-if-changed=libgpod/src/podbox_nano6.cpp");
    println!("cargo:rerun-if-changed=libgpod/sqlite_stub.c");
    println!("cargo:rerun-if-env-changed=CHINOTTO_DEPS");

    let deps = std::env::var_os("CHINOTTO_DEPS").map(PathBuf::from);
    if macos && deps.is_none() {
        panic!(
            "CHINOTTO_DEPS is not set. On macOS, build the static glib first:\n  \
             scripts/build-glib-macos.sh\nthen set CHINOTTO_DEPS to the folder it prints."
        );
    }
    let glib_includes: Vec<PathBuf> = match deps {
        Some(deps) => link_static_glib(&deps.join(&target)),
        None => {
            let mut pc = pkg_config::Config::new();
            pc.cargo_metadata(true);
            let gobject = pc.probe("gobject-2.0").expect("glib (gobject-2.0) development files not found");
            let gmodule = pc.probe("gmodule-2.0").expect("glib (gmodule-2.0) development files not found");
            gobject.include_paths.into_iter().chain(gmodule.include_paths).collect()
        }
    };

    let mut build = cc::Build::new();
    build
        .include(manifest.join("libgpod"))
        .include(&src)
        .define("HAVE_CONFIG_H", None)
        .warnings(false)
        .flag_if_supported("-Wno-deprecated-declarations")
        .flag_if_supported("-Wno-incompatible-pointer-types")
        .flag_if_supported("-Wno-int-conversion");
    for p in &glib_includes {
        build.include(p);
    }

    if macos {
        let sdk = std::process::Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]).output().expect("xcrun");
        let sdk = String::from_utf8(sdk.stdout).unwrap();
        build.include(PathBuf::from(sdk.trim()).join("usr/include/libxml2"));
        println!("cargo:rustc-link-lib=xml2");
        println!("cargo:rustc-link-lib=z");
        // glib's macOS bits.
        for fw in ["CoreFoundation", "Foundation", "AppKit", "Carbon"] {
            println!("cargo:rustc-link-lib=framework={fw}");
        }
        println!("cargo:rustc-link-lib=iconv");
        println!("cargo:rustc-link-lib=resolv");
    } else {
        let xml = pkg_config::probe_library("libxml-2.0").expect("libxml2 development files not found");
        let z = pkg_config::probe_library("zlib").expect("zlib development files not found");
        for p in xml.include_paths.iter().chain(&z.include_paths) {
            build.include(p);
        }
    }

    for f in SOURCES {
        build.file(src.join(f));
    }
    build.file(manifest.join("libgpod/sqlite_stub.c"));
    build.compile("gpod");

    let mut hashab = cc::Build::new();
    hashab
        .include(src.join("hashab"))
        .flag_if_supported("-fwrapv")
        .warnings(false)
        .file(src.join("hashab/calcHashAB.c"))
        .file(src.join("hashab/generate_initial_buffer.c"))
        .file(src.join("hashab/generate_key_material.c"))
        .file(src.join("hashab/generate_buffer_from_state_mixing.c"))
        .file(src.join("hashab/data/data.c"))
        .compile("hashab_impl");

    let mut nano6 = cc::Build::new();
    nano6
        .cpp(true)
        .std("c++17")
        .include(src.join("podbox"))
        .include(src.join("hashab"))
        .warnings(false)
        .file(src.join("podbox/itdb/hash58.cpp"))
        .file(src.join("podbox/itdb/hash72.cpp"))
        .file(src.join("podbox/itdb/hashab.cpp"))
        .file(src.join("podbox/itdb/itunescdb.cpp"))
        .file(src.join("podbox/itdb/itunesdb.cpp"))
        .file(src.join("podbox/itdb/itunesdb_writer.cpp"))
        .file(src.join("podbox/itdb/itunessqlite.cpp"))
        .file(src.join("podbox_nano6.cpp"));
    nano6.compile("chinotto_nano6");
    println!("cargo:rustc-link-lib=sqlite3");
}

fn track_tree(root: &std::path::Path) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        }
    }
}

/// Link a static glib from `prefix` (as built by scripts/build-glib-macos.sh)
/// together with the helper libraries it was built with; returns include dirs.
fn link_static_glib(prefix: &std::path::Path) -> Vec<PathBuf> {
    // Meson puts libraries in lib/ (macOS) or lib/<multiarch>/ (Debian-style Linux).
    let libdir = std::iter::once(prefix.join("lib"))
        .chain(std::fs::read_dir(prefix.join("lib")).into_iter().flatten().flatten().map(|e| e.path()))
        .find(|d| d.join("libglib-2.0.a").is_file())
        .unwrap_or_else(|| panic!("no static glib (libglib-2.0.a) under {}", prefix.display()));
    println!("cargo:rustc-link-search=native={}", libdir.display());
    // Order matters for static linking: users before what they use.
    for lib in ["gmodule-2.0", "gobject-2.0", "glib-2.0", "pcre2-8", "ffi", "intl"] {
        if libdir.join(format!("lib{lib}.a")).is_file() {
            println!("cargo:rustc-link-lib=static={lib}");
        } else if matches!(lib, "pcre2-8" | "ffi") && !cfg!(target_os = "macos") {
            // Not bundled (e.g. a Linux test build against system helpers): use the system's.
            println!("cargo:rustc-link-lib={lib}");
        }
    }
    // include/ itself holds libintl.h on macOS (copied there by the glib script).
    vec![prefix.join("include/glib-2.0"), libdir.join("glib-2.0/include"), prefix.join("include")]
}

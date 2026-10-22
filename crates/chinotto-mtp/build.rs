//! Builds the bundled libusb (macOS: darwin backend; Linux: usbfs + netlink,
//! no udev) and libmtp (libusb 1.0 glue, no MTPZ, no XML) as static libraries,
//! so Chinotto needs nothing extra installed for MTP.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    println!("cargo:rerun-if-changed=libusb");
    println!("cargo:rerun-if-changed=libmtp");
    println!("cargo:rerun-if-changed=shim.c");

    let mut usb = cc::Build::new();
    usb.include("libusb").include("libusb/libusb").define("HAVE_CONFIG_H", None).warnings(false);
    for f in ["core.c", "descriptor.c", "hotplug.c", "io.c", "strerror.c", "sync.c", "os/events_posix.c", "os/threads_posix.c"] {
        usb.file(format!("libusb/libusb/{f}"));
    }
    match target_os.as_str() {
        "macos" => {
            usb.file("libusb/libusb/os/darwin_usb.c");
            for fw in ["IOKit", "CoreFoundation", "Security"] {
                println!("cargo:rustc-link-lib=framework={fw}");
            }
            // libusb's darwin backend links against objc for one call.
            println!("cargo:rustc-link-lib=objc");
        }
        "linux" => {
            usb.file("libusb/libusb/os/linux_usbfs.c").file("libusb/libusb/os/linux_netlink.c");
        }
        other => panic!("MTP isn't built for {other}"),
    }
    usb.compile("usb-1.0");

    let mut mtp = cc::Build::new();
    mtp.include("libmtp").include("libmtp/src").include("libusb/libusb").define("HAVE_CONFIG_H", None).warnings(false);
    for f in ["libmtp.c", "libusb1-glue.c", "ptp.c", "unicode.c", "util.c", "playlist-spl.c"] {
        mtp.file(format!("libmtp/src/{f}"));
    }
    mtp.file("shim.c");
    mtp.compile("mtp");

    if target_os == "macos" {
        // iconv is a separate library on macOS (part of glibc on Linux).
        println!("cargo:rustc-link-lib=iconv");
    }
}

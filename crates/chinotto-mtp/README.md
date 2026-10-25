# chinotto-mtp

MTP players through bundled builds of **libmtp 1.1.23** and **libusb 1.0.29**
(both LGPL-2.1-or-later; their licenses are in `libmtp/COPYING` and
`libusb/COPYING`). Only the files Chinotto needs are included.

Built by `build.rs` with the `cc` crate, as static libraries, so nothing
extra needs installing:

- libusb: the POSIX core plus the macOS (IOKit) or Linux (usbfs + netlink,
  no udev) backend; settings in `libusb/config.h`.
- libmtp: the libusb 1.0 glue, without MTPZ (Zune) and without XML (Windows
  Phone sync partners); settings in `libmtp/config.h`.

Changes to the upstream sources:

- `libmtp/src/libmtp.h` is `libmtp.h.in` with the version filled in.
- `libmtp/gphoto2-endian.h` is written by hand (upstream generates it at
  configure time); it assumes a little-endian CPU (x86-64, arm64).
- `libmtp/src/libusb1-glue.c`: Apple devices (vendor 0x05ac) are never
  probed: iPods in disk mode aren't MTP, and may be busy being written to.

`shim.c` adds a check that a device really offers an MTP interface (a
player set to mass storage keeps its USB ID, so libmtp would list it), a cheap
USB fingerprint (to probe only when something was
plugged in or out) and struct sizes for a layout test.

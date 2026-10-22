/* Chinotto: a cheap fingerprint of the USB devices plugged in (bus, address,
 * vendor, product of each), so MTP detection (which probes devices) only runs
 * when something changed. Nothing is opened. */
#include <stdint.h>
#include "libusb.h"

static libusb_context *ctx;

uint64_t chinotto_usb_fingerprint(void) {
  if (!ctx && libusb_init(&ctx) != 0) return 0;
  libusb_device **list;
  ssize_t n = libusb_get_device_list(ctx, &list);
  if (n < 0) return 0;
  uint64_t h = 1469598103934665603ULL; /* FNV-1a */
  for (ssize_t i = 0; i < n; i++) {
    struct libusb_device_descriptor d;
    if (libusb_get_device_descriptor(list[i], &d) != 0) continue;
    uint64_t v = ((uint64_t)libusb_get_bus_number(list[i]) << 48) | ((uint64_t)libusb_get_device_address(list[i]) << 32) |
                 ((uint64_t)d.idVendor << 16) | d.idProduct;
    for (int b = 0; b < 8; b++) {
      h ^= (v >> (b * 8)) & 0xff;
      h *= 1099511628211ULL;
    }
  }
  libusb_free_device_list(list, 1);
  return h ^ (uint64_t)n;
}

/* Sizes and field offsets of libmtp's structs, for a test that checks the
 * Rust declarations match. */
#include <stddef.h>
#include "libmtp.h"
size_t chinotto_mtp_layout(int which) {
  switch (which) {
    case 0: return sizeof(LIBMTP_raw_device_t);
    case 1: return sizeof(LIBMTP_track_t);
    case 2: return offsetof(LIBMTP_track_t, next);
    case 3: return sizeof(LIBMTP_playlist_t);
    case 4: return sizeof(LIBMTP_devicestorage_t);
    case 5: return sizeof(LIBMTP_mtpdevice_t);
    case 6: return offsetof(LIBMTP_mtpdevice_t, storage);
    case 7: return offsetof(LIBMTP_track_t, filetype);
    default: return 0;
  }
}

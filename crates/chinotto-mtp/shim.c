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

/* Whether the USB device at bus/address may offer MTP: it has an interface
 * (any configuration or setting) that isn't mass storage. Read from the
 * descriptors the system already has; the device isn't opened. A player in
 * mass-storage mode has only class 8 interfaces, even when libmtp knows its
 * USB ID. (Checking for the MTP interface itself is too strict: players
 * describe it differently.) */
int chinotto_has_mtp_interface(uint32_t bus, uint8_t address) {
  if (!ctx && libusb_init(&ctx) != 0) return 1; /* can't tell: don't hide it */
  libusb_device **list;
  ssize_t n = libusb_get_device_list(ctx, &list);
  if (n < 0) return 1;
  int found = -1;
  for (ssize_t i = 0; i < n && found < 0; i++) {
    if (libusb_get_bus_number(list[i]) != bus || libusb_get_device_address(list[i]) != address) continue;
    struct libusb_device_descriptor d;
    if (libusb_get_device_descriptor(list[i], &d) != 0) { found = 1; break; }
    found = 0;
    for (uint8_t c = 0; c < d.bNumConfigurations && !found; c++) {
      struct libusb_config_descriptor *cfg;
      if (libusb_get_config_descriptor(list[i], c, &cfg) != 0) { found = 1; break; }
      for (int j = 0; j < cfg->bNumInterfaces && !found; j++)
        for (int k = 0; k < cfg->interface[j].num_altsetting; k++) {
          const struct libusb_interface_descriptor *intf = &cfg->interface[j].altsetting[k];
          if (intf->bInterfaceClass != 8) { found = 1; break; }
        }
      libusb_free_config_descriptor(cfg);
    }
  }
  libusb_free_device_list(list, 1);
  return found != 0; /* not found in the list: don't hide it */
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

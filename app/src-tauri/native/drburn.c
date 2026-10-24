/* Chinotto: burning a .bin/.cue on macOS through Apple's Disc Recording
 * framework (what the Finder and Burn.app use), which shares the drive
 * properly with the system; cdrdao can't open a drive macOS is using.
 *
 * Each track's sectors are read from the image and handed to the drive in
 * the drive's own block format (burn.rs works out the layout from the .cue):
 * audio as 2352-byte blocks; mode 1 data as 2048 bytes; mode 2 (PlayStation,
 * CD-ROM XA) as 2332 bytes, subheader included. The drive writes the sync
 * pattern, header and error correction. Session-at-once, then (if asked) the
 * disc is read back and compared with the image. */
#include <DiscRecording/DiscRecording.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

typedef struct {
  const char *path;
  uint64_t offset;        /* byte offset of the track's INDEX 01 in its file */
  uint32_t file_sector;   /* bytes per sector in the file (2352, 2048, 2336) */
  uint32_t skip;          /* bytes skipped at the start of each file sector */
  uint32_t block_size;    /* bytes per block given to the drive */
  uint32_t block_type;
  uint32_t data_form;
  uint32_t track_mode;
  uint32_t session_format;
  uint32_t length;        /* blocks */
  uint32_t pregap;        /* blocks */
  int64_t pregap_offset;  /* byte offset of the pregap in the file; -1: silence */
} chinotto_dr_track;

typedef void (*chinotto_dr_progress)(void *ctx, int phase, double fraction);

#define MAX_TRACKS 99
static const chinotto_dr_track *g_tracks;
static int g_fds[MAX_TRACKS];
/* What went wrong in a callback, for the error message. */
static char g_problem[256];

/* Fill `buf` with `count` blocks of track `t`, from block `first` of the
 * track (or of its pregap). */
static int produce_blocks(int t, int pregap, uint64_t first, uint32_t count, uint8_t *buf) {
  const chinotto_dr_track *tr = &g_tracks[t];
  for (uint32_t b = 0; b < count; b++) {
    uint8_t *out = buf + (size_t)b * tr->block_size;
    int64_t base = pregap ? tr->pregap_offset : (int64_t)tr->offset;
    if (base < 0) {
      memset(out, 0, tr->block_size);
      continue;
    }
    off_t at = (off_t)base + (off_t)(first + b) * tr->file_sector + tr->skip;
    ssize_t n = pread(g_fds[t], out, tr->block_size, at);
    if (n != (ssize_t)tr->block_size) {
      snprintf(g_problem, sizeof g_problem, "Couldn't read track %d of the image at byte %lld.", t + 1, (long long)at);
      return -1;
    }
  }
  return 0;
}

static OSStatus track_callback(DRTrackRef track, DRTrackMessage message, void *io) {
  int t = -1;
  CFDictionaryRef props = DRTrackGetProperties(track);
  CFNumberRef n = props ? (CFNumberRef)CFDictionaryGetValue(props, CFSTR("ChinottoTrack")) : NULL;
  if (n) CFNumberGetValue(n, kCFNumberIntType, &t);
  if (t < 0 || t >= MAX_TRACKS) return kDRFunctionNotSupportedErr;
  switch (message) {
    case kDRTrackMessagePreBurn:
    case kDRTrackMessagePostBurn:
    case kDRTrackMessageVerificationStarting:
    case kDRTrackMessageVerificationDone:
      return noErr;
    case kDRTrackMessageProduceData:
    case kDRTrackMessageProducePreGap: {
      DRTrackProductionInfo *info = (DRTrackProductionInfo *)io;
      uint32_t bs = g_tracks[t].block_size;
      uint32_t count = info->reqCount / bs;
      if (produce_blocks(t, message == kDRTrackMessageProducePreGap, info->requestedAddress / bs, count, info->buffer) != 0)
        return kDRFunctionNotSupportedErr;
      info->actCount = count * bs;
      return noErr;
    }
    case kDRTrackMessageVerifyData:
    case kDRTrackMessageVerifyPreGap: {
      DRTrackProductionInfo *info = (DRTrackProductionInfo *)io;
      uint32_t bs = g_tracks[t].block_size;
      uint32_t count = info->reqCount / bs;
      static uint8_t expected[64 * 2352];
      for (uint32_t done = 0; done < count;) {
        uint32_t step = count - done > 64 ? 64 : count - done;
        if (produce_blocks(t, message == kDRTrackMessageVerifyPreGap, info->requestedAddress / bs + done, step, expected) != 0)
          return kDRVerificationFailedErr;
        if (memcmp(expected, (uint8_t *)info->buffer + (size_t)done * bs, (size_t)step * bs) != 0) {
          snprintf(g_problem, sizeof g_problem, "Track %d differs from the image near block %llu.", t + 1,
                   (unsigned long long)(info->requestedAddress / bs + done));
          return kDRVerificationFailedErr;
        }
        done += step;
      }
      return noErr;
    }
    default:
      return kDRFunctionNotSupportedErr;
  }
}

static void set_int(CFMutableDictionaryRef d, CFStringRef key, int64_t v) {
  CFNumberRef n = CFNumberCreate(NULL, kCFNumberSInt64Type, &v);
  CFDictionarySetValue(d, key, n);
  CFRelease(n);
}

static void copy_error(CFDictionaryRef status, char *err, size_t errlen) {
  char a[512] = "", b[512] = "", sense[256] = "", asc[256] = "";
  int code = 0;
  CFDictionaryRef e = status ? (CFDictionaryRef)CFDictionaryGetValue(status, kDRErrorStatusKey) : NULL;
  if (e) {
    CFStringRef s;
    CFNumberRef n;
    if ((s = (CFStringRef)CFDictionaryGetValue(e, kDRErrorStatusErrorStringKey))) CFStringGetCString(s, a, sizeof a, kCFStringEncodingUTF8);
    if ((s = (CFStringRef)CFDictionaryGetValue(e, kDRErrorStatusErrorInfoStringKey))) CFStringGetCString(s, b, sizeof b, kCFStringEncodingUTF8);
    if ((s = (CFStringRef)CFDictionaryGetValue(e, kDRErrorStatusSenseCodeStringKey))) CFStringGetCString(s, sense, sizeof sense, kCFStringEncodingUTF8);
    if ((s = (CFStringRef)CFDictionaryGetValue(e, kDRErrorStatusAdditionalSenseStringKey))) CFStringGetCString(s, asc, sizeof asc, kCFStringEncodingUTF8);
    if ((n = (CFNumberRef)CFDictionaryGetValue(e, kDRErrorStatusErrorKey))) CFNumberGetValue(n, kCFNumberIntType, &code);
  }
  /* "message (more) [drive: sense / additional sense] {our problem} (error 0x…)" */
  snprintf(err, errlen, "%s%s%s%s%s%s%s%s%s%s (error 0x%08x)", a[0] ? a : "The burn failed.", b[0] ? " " : "", b,
           sense[0] || asc[0] ? " [drive: " : "", sense, sense[0] && asc[0] ? " / " : "", asc, sense[0] || asc[0] ? "]" : "",
           g_problem[0] ? " " : "", g_problem, (unsigned)code);
}

/* Returns 0 when the disc is burned (and verified, if asked); else 1, with
 * a message in `err`. `speed_x`: x-factor (CD), 0 for the drive's slowest. */
int chinotto_dr_burn(const chinotto_dr_track *tracks, int count, double speed_x, int verify, chinotto_dr_progress progress,
                     void *ctx, char *err, size_t errlen) {
  if (count < 1 || count > MAX_TRACKS) {
    snprintf(err, errlen, "A disc holds 1 to 99 tracks.");
    return 1;
  }
  CFArrayRef devices = DRCopyDeviceArray();
  if (!devices || CFArrayGetCount(devices) == 0) {
    if (devices) CFRelease(devices);
    snprintf(err, errlen, "No disc burner found.");
    return 1;
  }
  DRDeviceRef device = (DRDeviceRef)CFArrayGetValueAtIndex(devices, 0);

  g_tracks = tracks;
  g_problem[0] = 0;
  for (int i = 0; i < count; i++) g_fds[i] = -1;
  for (int i = 0; i < count; i++) {
    g_fds[i] = open(tracks[i].path, O_RDONLY);
    if (g_fds[i] < 0) {
      snprintf(err, errlen, "Couldn't open %s.", tracks[i].path);
      for (int j = 0; j < i; j++) close(g_fds[j]);
      CFRelease(devices);
      return 1;
    }
  }

  CFMutableArrayRef layout = CFArrayCreateMutable(NULL, count, &kCFTypeArrayCallBacks);
  for (int i = 0; i < count; i++) {
    CFMutableDictionaryRef p = CFDictionaryCreateMutable(NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    set_int(p, kDRTrackLengthKey, tracks[i].length);
    set_int(p, kDRBlockSizeKey, tracks[i].block_size);
    set_int(p, kDRBlockTypeKey, tracks[i].block_type);
    set_int(p, kDRDataFormKey, tracks[i].data_form);
    set_int(p, kDRSessionFormatKey, tracks[i].session_format);
    set_int(p, kDRTrackModeKey, tracks[i].track_mode);
    set_int(p, kDRPreGapLengthKey, tracks[i].pregap);
    set_int(p, CFSTR("ChinottoTrack"), i);
    CFDictionarySetValue(p, kDRVerificationTypeKey, verify ? kDRVerificationTypeReceiveData : kDRVerificationTypeNone);
    DRTrackRef track = DRTrackCreate(p, track_callback);
    CFRelease(p);
    CFArrayAppendValue(layout, track);
    CFRelease(track);
  }

  DRBurnRef burn = DRBurnCreate(device);
  CFMutableDictionaryRef bp = CFDictionaryCreateMutable(NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
  float kps = (float)(speed_x > 0 ? speed_x : 1.0) * kDRDeviceBurnSpeedCD1x;
  CFNumberRef speed = CFNumberCreate(NULL, kCFNumberFloatType, &kps);
  CFDictionarySetValue(bp, kDRBurnRequestedSpeedKey, speed);
  CFRelease(speed);
  CFDictionarySetValue(bp, kDRBurnStrategyKey, kDRBurnStrategyCDSAO);
  CFDictionarySetValue(bp, kDRBurnStrategyIsRequiredKey, kCFBooleanTrue);
  CFDictionarySetValue(bp, kDRBurnVerifyDiscKey, verify ? kCFBooleanTrue : kCFBooleanFalse);
  CFDictionarySetValue(bp, kDRBurnAppendableKey, kCFBooleanFalse);
  CFDictionarySetValue(bp, kDRBurnCompletionActionKey, kDRBurnCompletionActionEject);
  DRBurnSetProperties(burn, bp);
  CFRelease(bp);

  int result = 1;
  OSStatus st = DRBurnWriteLayout(burn, layout);
  if (st != noErr) {
    snprintf(err, errlen, "The burn couldn't start (error %d).", (int)st);
  } else {
    for (;;) {
      usleep(500 * 1000);
      CFDictionaryRef status = DRBurnCopyStatus(burn);
      if (!status) continue;
      CFStringRef state = (CFStringRef)CFDictionaryGetValue(status, kDRStatusStateKey);
      CFNumberRef pct = (CFNumberRef)CFDictionaryGetValue(status, kDRStatusPercentCompleteKey);
      float f = -1;
      if (pct) CFNumberGetValue(pct, kCFNumberFloatType, &f);
      int verifying = state && CFEqual(state, kDRStatusStateVerifying);
      if (progress && f >= 0) progress(ctx, verifying ? 1 : 0, f);
      if (state && CFEqual(state, kDRStatusStateDone)) {
        result = 0;
        CFRelease(status);
        break;
      }
      if (state && CFEqual(state, kDRStatusStateFailed)) {
        copy_error(status, err, errlen);
        CFRelease(status);
        break;
      }
      CFRelease(status);
    }
  }
  CFRelease(burn);
  CFRelease(layout);
  for (int i = 0; i < count; i++) close(g_fds[i]);
  CFRelease(devices);
  g_tracks = NULL;
  return result;
}

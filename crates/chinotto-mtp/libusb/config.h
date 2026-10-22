/* libusb configuration for Chinotto's build (crates/chinotto-mtp/build.rs):
   POSIX, macOS (darwin backend) or Linux (usbfs + netlink; no udev). */
#define DEFAULT_VISIBILITY __attribute__ ((visibility ("default")))
#define PRINTF_FORMAT(a, b) __attribute__ ((__format__ (__printf__, a, b)))
#define ENABLE_LOGGING 1
#define HAVE_SYS_TIME_H 1
#define HAVE_CLOCK_GETTIME 1
#define PLATFORM_POSIX 1
#ifndef _GNU_SOURCE
#define _GNU_SOURCE 1
#endif
#ifdef __APPLE__
#include <AvailabilityMacros.h>
#define HAVE_PTHREAD_THREADID_NP 1
#define HAVE_NFDS_T 1
#define HAVE_IOKIT_USB_IOUSBHOSTFAMILYDEFINITIONS_H 1
#endif
#ifdef __linux__
#define HAVE_EVENTFD 1
#define HAVE_TIMERFD 1
#define HAVE_PIPE2 1
#define HAVE_PTHREAD_SETNAME_NP 1
#define HAVE_ASM_TYPES_H 1
#define HAVE_PTHREAD_CONDATTR_SETCLOCK 1
#endif

/* libmtp configuration for Chinotto's build (crates/chinotto-mtp/build.rs):
   libusb 1.0, iconv, no MTPZ (Zune), no XML (Windows Phone sync partners). */
#define HAVE_LIBUSB1 1
#define HAVE_ICONV 1
#define HAVE_LANGINFO_H 1
#define HAVE_STRNDUP 1
#define HAVE_UNISTD_H 1
#define HAVE_SYS_UIO_H 1
#define HAVE_MKSTEMP 1
#define HAVE_LIMITS_H 1
#define HAVE_STDINT_H 1
#define HAVE_INTTYPES_H 1
#define HAVE_STRING_H 1
#define HAVE_STDLIB_H 1
#define HAVE_SYS_TYPES_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_FCNTL_H 1
#define HAVE_SYS_TIME_H 1
#define HAVE_ERRNO_H 1
#define HAVE_CTYPE_H 1
#define HAVE_GETTIMEOFDAY 1
#define HAVE_BASENAME 1
#define HAVE_STRDUP 1
#define HAVE_USLEEP 1
#define VERSION "1.1.23"
#define PACKAGE "libmtp"
#define ICONV_CONST

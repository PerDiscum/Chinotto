/* Hand-written config.h for Chinotto's build of libgpod 0.8.3.
 * Replaces the autotools-generated one. Built without gdk-pixbuf (Chinotto
 * writes the ArtworkDB itself), without libimobiledevice/SQLite (iPhones and
 * 2009+ nanos), without HAL/udev/libusb. libxml2 is on: it parses
 * SysInfoExtended, where newer Classics keep the FireWire GUID. */
#define GETTEXT_PACKAGE "libgpod"
#define PACKAGE "libgpod"
#define PACKAGE_NAME "libgpod"
#define PACKAGE_VERSION "0.8.3"
#define VERSION "0.8.3"
#define HAVE_UNISTD_H 1
#define HAVE_LOCALTIME_R 1
#define HAVE_STRUCT_TM_TM_GMTOFF 1
#define HAVE_LIBXML 1
#define HAVE_ZLIB 1
#define HAVE_G_CHECKSUM_RESET 1
#define HAVE_G_INT64_EQUAL 1
#define HAVE_G_INT64_HASH 1
#define HAVE_G_MAPPED_FILE_UNREF 1
/* Only used for the nano 6G's hashAB blob, which Chinotto doesn't support. */
#define LIBGPOD_BLOB_DIR "/nonexistent/libgpod"
#define TMPMOUNTDIR "/tmp"

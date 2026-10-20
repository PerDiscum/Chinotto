/* Chinotto: stands in for itdb_sqlite.c, which writes the SQLite databases
 * used only by iPhones/iPod touch and nanos from the 5th generation on.
 * libgpod calls it only for those devices; the iPod Classic never reaches it. */
#include <config.h>
#include "itdb_private.h"

int itdb_sqlite_generate_itdbs(G_GNUC_UNUSED FExport *fexp)
{
    return -1;
}

# libgpod 0.8.3 (vendored)

`src/` is libgpod 0.8.3's `src/` directory as shipped by Ubuntu 24.04
(`libgpod_0.8.3-19.1ubuntu4`), which includes Debian's
`Upstream-change-to-speed-up-itdb_resolve_path-calls.patch`. Unmodified,
except that `itdb_sqlite.c` / `itdb_sqlite_queries.h` are removed and replaced
by `sqlite_stub.c` (see that file).

`config.h` replaces the autotools one; it lists what is switched on and off.

libgpod is LGPL-2.1-or-later (see `COPYING`). It is compiled from this source
as part of Chinotto's build, and Chinotto's own source is public, so anyone can
rebuild it with a modified libgpod.

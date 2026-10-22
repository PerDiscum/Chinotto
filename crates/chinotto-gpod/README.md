# chinotto-gpod

Changes iPod databases (edit tags, add/remove songs, playlists) through a
bundled build of libgpod 0.8.3. Nano 6 writing is experimental and also uses
the HashAB and SQLite companion database code described below. See `src/lib.rs`
for the API and safety rules (back up before every save, files deleted only
after a good save).

- `libgpod/` — the vendored C source (LGPL-2.1-or-later) and our `config.h`.
  Built without gdk-pixbuf, so libgpod never rewrites the ArtworkDB.
- `build.rs` — compiles it with the `cc` crate. glib: system on Linux; on macOS
  a static build from `scripts/build-glib-macos.sh` (set `CHINOTTO_DEPS`).
- `src/ffi.rs` — bindings generated once and committed. To regenerate:

      bindgen libgpod/src/itdb.h \
        --allowlist-function 'itdb_.*' --allowlist-type 'Itdb_.*' --allowlist-var 'ITDB_.*' \
        --blocklist-function 'itdb_.*(photo|artwork|thumb|chapter|device_get_ipod_info|info_get|sysinfo).*' \
        --rust-target 1.82 -o src/ffi.rs -- $(pkg-config --cflags glib-2.0 gobject-2.0)

  then put back the header comment and `#![allow(...)]` line at the top.
- `libgpod/src/hashab/` — HashAB implementation from `dstaley/hashab` at
  `cc3e8aac05172a52e32da259fb31ccca9c625218` (Unlicense; see its `LICENSE`).
- `libgpod/src/podbox/itdb/` — the iTunesDB and SQLite bundle implementation
  from Milan Marocchi's PodBox (MIT; see its `LICENSE`), used to stage and
  validate Nano 6 companion databases.

Tests in `tests/real_ipod.rs` work on a temporary copy of the database in
`CHINOTTO_TEST_IPOD` (never the original) with a made-up FireWire GUID.

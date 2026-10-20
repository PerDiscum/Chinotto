# Chinotto — project notes and handoff

Everything decided and learned so far, so work can continue in a new session.

## Decisions

- **Name:** Chinotto.
- **What it is:** a music library manager, player and sync app modelled on the
  workflows of **classic iTunes (9/10 era)**: source list, column browser
  (Genre → Artist → Album), song list, Get Info (Summary/Info/Sorting/Options/
  Lyrics/Artwork), smart playlists with the iTunes rule builder, device page
  with capacity bar and sync options. Own visual identity: no Apple artwork,
  logos or exact look.
- **No browser:** native desktop app. Stack: **Tauri** (system web engine, not a
  browser) + **Rust** core.
- **Platforms:** Mac and Linux first, Windows later.
- **Library files:** iTunes model. Folders indexed in place; optional "copy
  into library folder" and "keep library folder organised", both off by default.
- **Priority:** get the iPod perfect first. Then player and library, then smart
  playlists and sync settings, then other players (USB-drive/MSC with
  **M3U, M3U8, WPL**; MTP with native playlist objects or .pla), then Windows.
- **Playback:** Symphonia (MPL-2.0). **Conversion:** bundled FFmpeg as a
  separate helper process (LGPL build). Not VLC code (licence and size);
  libVLC remains a possible playback fallback.
- **Tags:** `lofty`. **Library DB:** SQLite. **iPod DB writing:** native libgpod
  (handles the Classic checksums) + our own artwork writer (no gdk-pixbuf).

## Roadmap

1. Foundation: native app with TunesReloaded's features, CI builds for Mac + Linux.
2. iPod-perfect: auto-detect on plug-in, model via USB, full metadata (album
   artist, disc, compilation, sort fields), artwork, playlists incl. live smart
   playlists, play counts/ratings read back, DB backup before every sync, safe
   eject, tests against a real database.
3. Library manager + player.
4. Sync engine: selective sync, diff-based copying, stats both ways.
5. Other players (MSC/MTP; per-device playlist format, path style, location presets incl. Rockbox).
6. Windows.

## Status (2026-10-20)

Roadmap step 1 is half done: the **Tauri app and the Mac/Linux builds exist**;
the app is **read-only**. TunesReloaded's *writing* features (add songs,
delete, edit tags, playlists, artwork) are the next chunk.

What works:
- Auto-detects mounted iPods (polls `/Volumes` on macOS; `/media`,
  `/media/<user>`, `/run/media/<user>`, `/mnt` on Linux, every 2 s) and opens
  a newly plugged-in one. Unplugging closes it. "Open iPod folder…" adds any
  folder with an `iPod_Control/iTunes/iTunesDB` (handy for copies on disk).
- Source list (device, Music, playlists, smart playlists), Genre → Artist →
  Album column browser (artist = album artist, falls back to artist;
  compilations grouped under "Compilations"), virtualised song list (23k rows
  are instant), sorting by column with iTunes-style tie-breaks, accent-
  insensitive search, multi-select (Shift/Cmd/Ctrl, arrows, Cmd/Ctrl-A).
- Covers: `art://localhost/<generation>-<mhii id>-<px>` serves the smallest
  stored thumbnail ≥ px as PNG, read straight from the `.ithmb` files.
- Device page: name (master playlist name), model from SysInfo, serial,
  capacity bar (Audio / Video / Other / Free from statvfs), Eject
  (`diskutil eject` on macOS; `udisksctl unmount` + `power-off` on Linux,
  falling back to `umount`).
- Start-up folder: first argument or `CHINOTTO_OPEN`.

Verified on Linux (Ubuntu 24.04, WebKitGTK 2.52) with the private copy of
CAESAR's databases, and **on the developer's Mac with CAESAR plugged in**
(universal .dmg from the "Build apps" workflow): mount detection, browsing,
real covers, toggling the browser/artwork panes all work. Not yet verified:
eject on real hardware.

Repository: github.com/PerDiscum/Chinotto (private). CI and "Build apps" pass on
macOS and Linux. Workflows use actions/checkout@v5 and actions/setup-node@v5
(the v4s run on the deprecated Node 20).

## App architecture

- `app/src-tauri/src/state.rs`: `AppState` holds hand-added folders and the
  open `Session` (parsed iTunesDB + ArtworkDB + SysInfo, behind an `Arc`).
- `commands.rs`: `list_devices`, `add_folder`, `open_ipod` (returns the whole
  view as one JSON blob: device, playlists with track ids, tracks with
  `skip_serializing_if` for absent strings; 64-bit ids as strings), `close_ipod`,
  `eject`, `startup_folder`.
- `devices.rs`: scanning, the watcher thread (emits `devices-changed`), eject.
- `artwork.rs`: the `art:` scheme handler.
- Front end: `lib/store.svelte.ts` (all state, runes), `lib/library.ts`
  (`TrackIndex`: strings ranked once with `Intl.Collator`, so sorts compare
  integers; `browse()` builds the three browser columns in one pass).
- Look: system UI font; smoked-brown source list; an amber "bottle label"
  status display in the toolbar; chinotto-brown selection; light and dark
  mode.
- Icon: the developer's own artwork (a smiling orange wearing earbuds),
  `app/src-tauri/icons/icon-original.png`. `icon-source.png` is it fitted to the
  macOS icon grid (824 px inside 1024, soft shadow); regenerate the set with
  `npx tauri icon src-tauri/icons/icon-source.png -o src-tauri/icons`, then
  delete the `android`, `ios` and `Square*`/`StoreLogo` outputs.
- Mac builds are ad-hoc signed (`signingIdentity: "-"`), not notarised.
- CI runs `cargo fmt --check` and `clippy -D warnings`; keep both clean.
- Rust is pinned to 1.97.0 (`rust-toolchain.toml` and both workflows) because
  newer releases add clippy lints that fail the build (1.99 did). Bump all
  three together, deliberately.

## Writing to the iPod (in progress)

Decided 2026-10-20: libgpod, as planned. Done so far:
- `crates/chinotto-gpod`: libgpod 0.8.3 vendored (Ubuntu's copy incl. Debian's
  resolve_path speed-up), compiled by `build.rs` with a hand-written config.h:
  no gdk-pixbuf (libgpod never touches ArtworkDB; `mhii_link` etc. survive),
  no SQLite/libplist/libimobiledevice (`sqlite_stub.c`; only nano 5G+/iPhone
  use them), libxml2 on (SysInfoExtended holds the FireWire GUID on newer
  Classics). TunesReloaded's WASM libgpod was built the same lean way.
- Safe API: `Database::open/edit/add_track/remove_tracks/create_playlist/
  rename_playlist/delete_playlist/set_playlist_tracks/save`. Tracks by dbid.
  `save` backs up iTunesDB to `iPod_Control/iTunes/Chinotto Backups/` (newest
  10 kept); removed tracks' files are deleted only after a good save.
- Tests on a temp copy of CAESAR's DB: unchanged save keeps all 23,516 tracks,
  covers links and playlists identical; edits/playlist/add/remove round-trip.
- macOS: `scripts/build-glib-macos.sh` builds static glib 2.82.5 for arm64 and
  x86_64 (meson, forcefallback for pcre2/libffi/intl/zlib); workflows cache it
  in `deps/` and set CHINOTTO_DEPS. **Not yet verified on GitHub.**

- Checksum safety net in `save`: if the old iTunesDB declared a checksum
  (header 0x30: 1 hash58, 2 hash72, 4 hashAB; hash58 at 0x58) and the new one
  doesn't, the backup is restored and an error returned. Needed because with
  a SysInfoExtended present libgpod takes the scheme from its `DBVersion` key
  alone (missing → no checksum).

Test device for writing: an 8 GB **iPod nano 3G named "CHINOTTO"** (mounts
as /Volumes/iPod), so CAESAR is never written during development. Its
FireWire GUID is only in SysInfoExtended (`FireWireGUID`), not in SysInfo, so
libxml2 support is required.

Still to do: hook it into the app (Get Info editing, add songs with tags from
`lofty`, delete, playlists, artwork via our ArtworkDB appender), and check
CAESAR's FireWire GUID is found (SysInfo or SysInfoExtended) before a real save.

## Original plan for this step

1. Link libgpod (LGPL; dynamic linking). Linux: system package
   (`libgpod-dev`). macOS: build libgpod + glib + libplist + sqlite in the
   workflow and bundle the dylibs into `Chinotto.app/Contents/Frameworks`
   (Tauri `bundle.macOS.frameworks`), fixing install names. This is the
   riskiest part; do it first and prove the CI .dmg still runs.
2. Commands: add files (tags via `lofty`, copy to `iPod_Control/Music/Fxx`),
   delete, edit metadata (album artist + sort fields per the Swinsian rules
   below), playlists, artwork through our ArtworkDB appender.
3. DB backup before every write; write to a temp file and rename.
4. Then the rest of roadmap step 2 (model via USB, play counts back, live
   smart playlists).

## Open items

- Try Eject on CAESAR from the app.
- Claude has no GitHub access from its sessions yet; fixes go to the developer
  as `git am` patch files unless GitHub is connected under Connectors.

## The developer's iPod (test device)

- iPod Classic 7G ("CAESAR"), large modded storage (~270 GB free of FAT32),
  previously managed with **Swinsian**. Library: 23,516 tracks, 18 playlists
  (master "CAESAR VERIDIS", 3 built-in "Videos" smart playlists).
- If macOS refuses to mount it (e.g. `com.apple.DiskManagement.disenter error
  49212`): Disk Utility → First Aid on CAESAR → Mount. Usually after unplugging
  without ejecting.

## iTunesDB facts (verified on that iPod)

- Sections in file order: 4 (albums), 1 (tracks), 3, 2 (playlists), 5 (smart).
- mhit header 624 bytes. Offsets: id 0x10, compilation 0x1E, rating 0x1F,
  size 0x24, length 0x28, track 0x2C, year 0x34, bitrate 0x38, samplerate
  0x3C (<<16), playcount 0x50, disc 0x5C, dbid 0x70 (u64), artwork_count 0x7C
  (u16), artwork_size 0x80 (= source width×height + 1), has_artwork 0xA4
  (1 yes / 2 no), media type 0xD0, mhii_link 0x160.
- String mhod: +0x18 encoding (1 UTF-16LE), +0x1C byte length, text at +0x28.
- mhyp: master flag 0x14, id 0x1C (u64); mhip track id at 0x18.
- libgpod drops mhod 102 (iTunes-only view settings) and duplicate
  smart-rule/index records on write; per-track data survives. The 3 "Videos"
  smart playlists trigger a harmless `itdb_splr_validate` warning.
- **Albums group by album name + album artist** (falls back to artist). Tracks
  added without album artist/sort fields showed as a duplicate album.

## Swinsian sort-field convention (matches all but custom values)

Lowercase; strip accents (NFD, drop combining marks except Japanese U+3099/U+309A);
fold full-width ASCII; Greek final ς → σ; drop a leading "the " for artist and
album-artist only (not albums). No sort title is written.

## ArtworkDB facts (verified)

- Formats: 1055 (128×128, 32,768 B), 1060 (320×320, 204,800 B), 1061 (56×56
  nominal, stored 56×55 = 6,160 B). RGB565 little-endian, row-major, no padding.
  54 images also use 1068/1069 (another source; left alone).
- mhii 764 bytes incl. children; covers shared per album via identical offsets.
- mhfd +0x1C holds the last used image id.
- Appending new images to large .ithmb files means the browser copies the whole
  file; we start a new `F<fmt>_<n+1>.ithmb` once a file is ≥ 64 MB.

## TunesReloaded (the starting point)

Browser app (libgpod compiled to WASM). Our fork added: track editor (title,
artist, album artist, album, genre, track #, year, composer, rating; multi-track),
album artist + sort fields on upload, artwork (upload covers, set/remove in
editor, preview), unsynced-changes badge, and a Go launcher (`TunesReloaded.app`
/ `.exe`) that serves the bundle locally with COOP/COEP and opens Vivaldi. The
WASM engine has no setters for album artist/sort/artwork fields; we write the
`Itdb_Track` struct directly (offsets: albumartist ptr 19×4, sort_artist 21×4,
sort_album 23×4, sort_albumartist 24×4, composer 9×4; cd_nr 120, track_nr 128,
year 144, dbid 232, artwork_count 252, artwork_size 256, has_artwork 304,
mhii_link 416), validated at runtime against the engine's JSON.

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

## Open items

- Network access was enabled in claude.ai Settings → Capabilities; needs a new session.
- Build route for Mac: GitHub repository + Actions (preferred) or local builds on the Mac.

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

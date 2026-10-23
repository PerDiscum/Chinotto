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

## Requested features (2026-10-20, from the developer)

- **Playback** with basic controls for ALAC/AAC, MP3, WAV, FLAC, OGG and WMA.
  Symphonia covers all but WMA (and AAC is LC only, no HE-AAC); those go
  through the bundled FFmpeg helper.
- **Last.fm scrobbling** of iPod plays, using code from LastPod (licence to
  check before copying anything; Chinotto is MIT). The developer will send
  LastPod's source. The iPod records play
  counts and the last play time only, so earlier plays' times are estimated.
- **Artwork auto-resize**: square images to 600×600; otherwise the shorter
  side to 600 px and the longer side in proportion. Only shrink: images
  already ≤ 600 px on the shorter side are left as they are.
- **Per-model device icons**: iPod 1G, 2G, 3G, 4G/Photo, 5G/Video, 6G, 7G,
  mini 1–2G, nano 1–5G, shuffles; later Sony, Sansa, Creative, Samsung,
  FiiO, generic, SD card. **All icons are supplied by the developer**;
  Chinotto provides the slots and a generic fallback.
  Two images per model: a **square sidebar icon** (`ipod-7g.svg`, or PNG
  ≥128 px, shown at 16–20 px) and a **full-length device-page graphic**
  (`ipod-7g-full.svg`, or PNG ≥600 px tall, shown up to ~280 px high beside
  the device details, aspect kept). Transparent backgrounds. Names: ipod-1g…
  ipod-7g (4g = 4G/Photo, 5g = Video), ipod-mini, ipod-nano-1g…5g,
  ipod-shuffle-1g…4g, generic; a missing image falls back to generic.
  **Delivered 2026-10-20** as one picture per model (used for both, in
  `app/src/assets/devices/`, trimmed and scaled to 512 px): ipod-3g, -4g,
  -7g (also used for 6g), -mini, -nano-2g, -nano-3g, -shuffle (all shuffles),
  generic, plus sansa-clip/-fuze/-e200, sony-nwz/-a10, fiio-x1,
  cowon-iaudio-x5, iriver-h100/-ifp, xclef for later. `deviceImages.ts` maps
  keys and aliases. Device page: picture top-right, 180 px high, no added
  shadow; sidebar shows it at 18 px.
- Model detection: `chinotto-ipod/src/models.rs`, generated from libgpod's
  tables (model number → generation; serial's last 3 chars → model number,
  for iPods like the nano 3G whose SysInfo has no model number).
- ⌘. (Ctrl+. on Linux) stops playback. Model detection from
  SysInfo/SysInfoExtended via libgpod's device table; generic icon fallback.

- **Column browser** (requested 2026-10-21): each browser column (Genre,
  Artist, Album) must scroll on its own, its list length changing with the
  filtering. The user chooses which columns show, from Artist, Album, Genre,
  Composer, Year and Format.
- **Song list**: add Format and Bitrate columns.
- **Artwork fixes** (reported 2026-10-21, done in 0014): the webview's own
  "Download Image" did nothing; now right-click a cover → Save Cover As…
  (largest stored size, PNG) and Paste Cover (clipboard via `arboard`);
  Get Info's Artwork tab has Paste and ⌘V.
- **Shuffle/repeat** (0014): buttons in the sidebar's bottom bar, remembered;
  repeat cycles off → all → one. Browser columns now scroll and the browser
  is resizable by the bar under it (0014).
  Covers now confirmed correct on the nano 3G and the classic 7G.

- **Other players (later):** USB-drive (MSC) players are added by hand through
  a wizard: choose the drive, the music folder, the playlist folder, the
  playlist format, and the player model and picture (settings depend on the
  device). Zune: possible later via libmtp's MTP-Z support (needs the user's
  own key file, which can't be shipped); not promised.

- **Icon chooser** (done, 0018): click the picture on the device page; the
  choice is saved per device in settings.json (`deviceIcons`, identity → key);
  "Use automatic" removes it.

- **Done in 0019:** playlist reordering (drag within a playlist shown in its
  own order, "#" column, unfiltered; sort is remembered per playlist and
  playlists start in their own order); song-list Format and Bit Rate columns
  and a column chooser (right-click the titles; hidden ones in localStorage);
  browser column chooser (Genre, Format, Year, Composer, Artist, Album, shown
  broadest first); device picture hides when the page is too narrow; new
  welcome text. "Add Drive…" still only opens iPods until the MSC wizard
  exists.

## Media library: decisions (2026-10-21, from the developer)

- **Several libraries**, each shown in the sidebar and collapsible (e.g. an
  archival lossless library plus a small everyday one). Each has its own
  music folders, songs and playlists.
- **Files are left alone**: read only, never moved, renamed or retagged
  (tag edits come later, if ever, and only when asked).
- **Scanning**: automatically every hour, plus a "Scan Now" for picking up
  new files on the fly. No file watcher needed.
- **Missing files**: never removed just because they can't be found. If a
  folder's drive isn't connected, its songs stay (shown as unavailable). Only
  when the drive *is* connected and the files are gone are they removed.
- **Playback formats**: WMA matters more than HE-AAC (FFmpeg helper; WMA first).
- **Library playlists**: yes.
- **To the iPod**: drag and drop from the library first; full sync later.
- **No iTunes library import** for now.

## Media library: step 1 (0023)

- `app/src-tauri/src/library.rs`: data model (`Library`, `LibTrack`, `Tags`),
  scan (walk folders; re-read tags only when size/mtime changed; unreadable
  files such as WMA listed by file name), the missing-files rule, folder covers
  (cover/folder/front/album .jpg/.png). Unit-tested.
- `libcommands.rs`: library list in AppState, commands (list/create/update/
  delete/scan/remove_unavailable/library_view), background scans with
  `library-progress` / `library-updated` / `libraries-changed` events, scan at
  start-up (+10 s) and hourly, `libart:` scheme (`<lib>-<song>-<px>` → JPEG).
  Stored in the app data folder, `libraries/<id>.json`.
- Library songs use the iPod's Track shape; dbid `lib:<lib>:<song>` (playback
  resolves it). Source `{kind: "library", lib, id: "all"}`; `store.index` is
  the shown collection's index. Read-only in Get Info.
- Not yet: library playlists, play counts/ratings in libraries, dragging to
  the iPod, WMA playback.

## Views (0026)

- Music of a library or an iPod has views: Songs, Albums, Artists, Genres,
  Composers, Compilations (`store.view`, remembered). Playlists are always songs.
- Albums: `AlbumGrid.svelte` (rows chunked by the measured column count, so
  the open album's band goes right under its row, iTunes 11 style).
  Artists/Genres/Composers: `GroupView.svelte`. Album songs:
  `AlbumTracks.svelte`; shared right-click/drag in `trackActions.ts`.
- Sorting rule everywhere (developer's): album artist, then the album's year,
  then album title; compilations last (also "Compilations" last in the column
  browser and the Artists list). `library.ts`: `albums()`, `groups()`,
  `albumKey()` (album + album artist; compilations by album only).

## Library playlists and editing (0027)

- Library playlists live in the library JSON (`playlists`, ids "p<n>" in the
  view; sidebar/drag ids "lib:<library>:p<n>"). Sidebar: the library's row is
  its Music; its playlists are nested under it.
- Editing writes into the music files (the developer's choice): `tagwrite.rs`
  copies the file beside itself (".<name>.chinotto-<pid>"), edits the copy
  with lofty split/merge on the format's own tag (keeps everything else),
  then renames it over the original. Ratings stay in the library JSON.
  Covers: front cover replaced, 600 px rule, JPEG/PNG. WMA can't be written.
  Tested on MP3, AAC, ALAC, FLAC, Ogg, WAV, AIFF (CHINOTTO_TEST_AUDIO).
- "Remove from Library" adds the paths to `excluded` (scans skip them);
  files are never deleted.
- Scans merge with changes made meanwhile (ratings, plays, removals, newer tags).

## Translations and copying to the iPod (0028)

- `app/src/lib/i18n.ts`: `t("English", {vars})`, `tn(n, one, other)`.
  English text is the key; `src/locales/<lang>.json` maps it (empty = English).
  `npm run strings` (scripts/strings.mjs) collects every t()/tn() text into
  en.json and adds new keys to the other files. See docs/TRANSLATING.md.
  The language is chosen in main.ts before App loads (system locales from the
  `system_locales` command via sys-locale; macOS also needs CFBundleLocalizations),
  so module-level t() calls are fine. Rust messages are still English.
- Library songs dragged onto the iPod's Music or one of its playlists (or
  "Copy to <iPod>" in the song menu) are copied with add_files; songs already
  there (same name/artist/album/length within 2 s) aren't copied again.

## Saving songs, playlists, reminders (0030)

- Song menu → Save As… (one song: the file, "Artist - Title.ext") or Save As
  Zip… (several: stored zip, "Artist/Album/NN Title.ext", duplicates get
  " (2)"); `export.rs`. Works for iPod and library songs.
- Smart playlists on the iPod (e.g. the nano's video playlists) can be
  renamed and deleted (`Database::any_playlist`); still not edited.
- The web view's own right-click menu (Back, Reload…) is suppressed outside
  text fields.
- **Conversion on transfer (done, 0033):** `convert.rs`. FLAC/Ogg/Opus (and
  ALAC if the device's setting says so, default yes) are decoded with
  Symphonia and encoded into a temporary folder (`TempDir`, removed after the
  iPod save), tags and cover copied over, then added as usual. Per-device
  setting (settings.json `conversions`, device page): AAC 256 (default), AAC
  128, MP3 320, MP3 192. MP3: LAME built in (mp3lame-encoder). AAC: macOS
  `/usr/bin/afconvert` (constrained VBR, from a 24-bit WAV); Linux `ffmpeg`
  from PATH (deb recommends ffmpeg). Rates above 48 kHz go to 44.1/48 kHz.
  Opus isn't decodable by Symphonia: fails with a message until FFmpeg.
- **To do, after conversion (requested 2026-10-21): Burn Playlist to Disc.**
  Right-click a playlist → Burn…: audio CD (decode to 16-bit/44.1 kHz WAV/AIFF
  in a temporary folder, warn past ~80 min, gap 2 s or none) or an MP3 data
  disc. macOS: `drutil burn -audio <folder>` (built in; CD-Text support
  unverified). Linux: wodim or cdrdao (tell the user which to install).
- Translation notes: German always capitalises Du/Dein/Deinem/Deiner/Deines.
  The developer translates; Claude only corrects grammar, no other changes.

## Scratch list (0032)

- Songs opened from outside the libraries never join one (the developer's
  top request): `scratch.rs` keeps them in memory only (dbid `scratch:<n>`,
  covers via `libart` with library id "scratch"). Sources: macOS
  `RunEvent::Opened` (double-click, Open With, Dock), command-line files at
  start, a second launch (tauri-plugin-single-instance, DBus on Linux),
  files dropped on the window unless an editable iPod is shown, and
  "Open File…" in the sidebar's folder menu. The sidebar shows "Scratch"
  only while it has songs; × clears it. The first opened song plays.
- File associations (bundle.fileAssociations) use rank Alternate, so
  Chinotto never becomes the default player by itself.

## Small fixes (done 2026-10-21, patch 0036)

- Keyboard shortcuts live in `lib/commands.svelte.ts` (`run` holds every
  command; `handleKey` maps keys; on macOS `installMenu` builds the menu bar
  from the same commands, with t() labels). ⌘I Song Details, ⌘N new playlist,
  ⌥⌘N new library, ⌘O open file (Scratch), ⇧⌘O add to iPod, ⇧⌘S Save As,
  ⌘E eject, ⌘F search, ⌘1–6 views, ⌘B column browser, ⌘G artwork, ⌘L show
  playing song, Space play/pause, ⌘. stop, ⌘←/→ previous/next, ⌘↑/↓ volume,
  ⌘A select all. On Linux the same keys use Ctrl. The web view's handler acts
  first (preventDefault), so the menu shortcut and the key never both fire.
- A click on an empty part of the window clears the song selection
  (App.svelte `mousedown`; buttons, rows, menus, dialogs are left alone).
- "Get Info" is now "Song Details" (the new string needs translating).
- Album covers (Albums/Compilations grid, Artists/Genres/Composers views) can
  be dragged: the album's songs are selected and dropped like songs (onto the
  iPod, its playlists or a library playlist). A click still opens the album.
- Compilation flag: dropped (already on the Info tab).

Order chosen: confirm editing on the nano → **playback** (done) → artwork (done) → icon chooser → shuffle/repeat → playlist reordering → column browser fixes and Format/Bitrate columns → media library.

## MTP, step 1: finding, reading, playing (patch 0053)

- crates/chinotto-mtp: bundled libmtp 1.1.23 + libusb 1.0.29 built with cc
  (see its README for the configuration and the small changes: Apple
  devices never probed; hand-written gphoto2-endian.h). Rust API: detect,
  Device::open (uncached), names/serial/battery/storages, tracks (with
  metadata), playlists, download. shim.c: a USB fingerprint (no device
  opened) so `detect` (which probes) runs only when USB devices change;
  a layout test checks the Rust struct declarations against C.
- app mtp.rs: the watcher adds MTP devices (`kind: "mtp"`, path
  "mtp:<bus>-<devnum>", picture guessed from the name). Clicking one opens it
  (mtp_open) — one at a time; it's held until "Let Other Programs Use It" or
  unplug — and its tracks/playlists become an ephemeral Library ("m<bus>x<dev>",
  `ephemeral`: never saved or scanned). Playing copies the song to
  <temp>/chinotto-mtp/<key>/ first (`mtp::fetch`, via track_file).
  MtpPage shows name, model, serial, battery, capacity, contents.
- Read-only for now: no copying to MTP, no deleting, no playlist changes,
  no covers shown (album art via LIBMTP_Get_Representative_Sample later).
- Platform notes: macOS's ptpcamerad (Image Capture) or Android File
  Transfer may hold the device → open fails with an explanation. Linux:
  the file manager (gvfs-mtp) holds it when it's mounted there; users need
  udev rules giving access (distros' libmtp package; the .deb recommends
  libmtp-common).
- Untested on real devices (none in the build sandbox), and the macOS build
  of libusb's darwin backend is only exercised by CI.

## Drag scrolling; Greek on Walkmans (patch 0051)

- While a song or album is dragged, the list it came from is held where it
  was (trackActions `holdScroll` resets any scroll), except the reorder
  scroll, which now waits 0.5 s at the very top/bottom of the rows
  (`allowDragScroll`). Before, passing the playlist's top edge on the way
  to the sidebar scrolled it.
- Sony profiles (NWZ, NW-A10/A20 — not the Android ones) have
  `plain_greek`: Greek letters lose their accents (tonos, dialytika,
  polytonic marks; final ς → σ; the Greek question mark becomes ";" by
  Unicode normalisation) in tags and folder names when copying and
  after edits (`msc::plain_greek`, NFD + drop marks after Greek letters).

## Editing on players, progress names, conversion rules (patch 0050)

- Player songs are editable when the player allows changes (store.writable
  for a player library = its playerStates writable). lib_edit_tracks asks
  `msc::edit_rules` (refuses when changes are off) and runs
  `msc::after_edit` per file: ID3v2.3 again (Sony, Sansa), small cover and a
  new folder.jpg for a new cover (Sansa). Player playlists stay read-only
  (`store.playlistsWritable`; editablePlaylist is null for players).
- Progress while copying shows "Title / Artist / Album" (`tags::label`) for
  the iPod, players and setup.
- Conversion: `Conversion { format, convert_alac, advanced, rules }`.
  `plan(path, plays)` → None (copy) or a target: simple = what the device
  can't play (+ ALAC if asked) → format; advanced = per kind
  (flac, alac, aac, mp3, ogg, opus, wav, aiff, wma) "keep" or aac256/aac128/
  mp3_320/mp3_192/alac; "keep" of something it can't play, ALAC for a device
  without ALAC → the simple choice; AAC on a device without AAC → MP3 320.
  New: Apple Lossless encoding (16-bit; afconvert / ffmpeg). WMA can't be
  decoded yet (FFmpeg helper later). iPods: `convert::IPOD_PLAYS`.
  UI: ConversionSettings.svelte on both device pages.

## Remember: covers on players that also do MTP

- The Sansa e260 (and Clip/Fuze, and Walkmans) can run as MSC or MTP. As MSC
  (files), SanDisk firmware shows only folder.jpg (patch 0049). Over MTP,
  covers go through the device's database (WMP sends an album object with a
  "representative sample" image), so the MTP code must send covers that way
  for these players, not as folder.jpg. Check per device when MTP comes.

## Sansa covers: folder.jpg (patch 0049)

- Tested on the developer's e260 v2 (SanDisk firmware 03.01.16A, Rockbox 3.15
  also installed) with four test albums: SanDisk's firmware showed only the
  albums with a `folder.jpg` (embedded covers ignored, whatever the size or
  text encoding); Rockbox showed all. So the Sansa profiles also write a
  small baseline `folder.jpg` (≤300 px, ≤30 KB) into each album folder when
  copying, if there isn't one; deleting an album's last song removes it.

## Same rules everywhere; Sansa covers (patch 0048)

- Any song can be dragged, from any source and view (song list, album panel,
  album covers, artist/genre/composer views); the drop target decides and
  says why when it can't (iPod changes off, a library playlist given another
  library's songs, player changes off — msc_add's error).
- Song menu everywhere: Copy to <iPod> (Music or a playlist) and Copy to
  <player> for each player; iPod songs can be copied to players
  (msc_add resolves iPod dbids via playback::track_file). Player songs: no
  playlist items; Delete key / menu → Delete from <player>….
- Album panel rows: Enter, ↑/↓ (⇧ extends), Delete like the song list.
- Sansa e200 profile: `small_covers: (300, 30 000 bytes)` → the embedded
  cover is re-encoded as a baseline JPEG ≤ 300 px and ≤ 30 KB (falling back to
  the folder's cover), plus ID3v2.3. From forum reports (a 300×300, 29 KB
  cover showed; 455×455, 42 KB didn't). Unconfirmed on the device.
- Part files while copying are `.<name>.part.<ext>` (the tag code goes by
  the extension).

## MSC players, step 2: player page, copying, deleting (patch 0047)

- PlayerPage (source `{kind: "player", path}`; clicking the player in the
  sidebar): model, capacity bar, contents, formats it plays, Eject, Player
  Settings…, Scan Now, Add Songs…; Allow changes and conversion, stored in
  Settings under identity `msc:<id>` (same as iPods use).
- Copying (msc_add): drag library/Scratch songs or albums onto the player in
  the sidebar (`data-drop-playlist="player:<path>"`), right-click → Copy to
  <player>, or Add Songs…. Converted when the profile's `plays` lacks the
  format (FLAC on an NWZ…; ALAC optional); AAC targets fall back to MP3 320
  for players without AAC. Placed at <music>/<Album Artist or
  Compilations>/<Album>/<NN Title>.ext (FAT-safe names), through a hidden
  .part file; same place + same size = already there. Sony profiles get
  ID3v2.3 tags on MP3s (`tagwrite::resave_id3v23`). Then a rescan.
- Deleting (msc_delete): right-click a player song → Delete from <player>…
  (confirm); only files inside its music folder; empty folders removed.
- Playlists on players: read-only (M3U/PLS files); the NWZ-E344 needs .pla,
  which are empty placeholders backed by its MTP database → wait for MTP.
- Not yet: tag editing on players (would need the ID3v2.3 rule), Sansa e200
  cover size rule.

## MSC players, step 1: reading (patch 0046)

- Add Drive… (Welcome / Open Device…) on a drive that isn't an iPod opens
  MscDialog: model profile (msc.rs `profiles`: Sony NWZ, Sony NW-A, Sansa
  e200, Sansa Clip/Fuze, Other), name, music folder, playlist folder,
  playlist format (m3u8/m3u/pls), picture. Saved on the player as
  `.chinotto-device.json` (MscConfig, with a generated id); right-click the
  player → Player Settings… changes it.
- The device scanner lists drives with that file as `kind: "msc"` with
  `library: "d<id>"` (no dash: libart URLs split on dashes). When one
  appears, `msc::attach` creates/updates that Library (`device` set,
  folders = [<mount>/<music folder>], `playlist_dir`) and scans it, so the
  songs list is cached between plug-ins and only new/changed files are read.
  After each scan, `msc::import_playlists` replaces its playlists with the
  playlist files (entries relative to the file, or /-rooted = drive top,
  drive letters and backslashes accepted, matched ignoring case).
- Front end: player libraries are hidden from Libraries (`store.ownLibraries`)
  and shown under Devices with Music + playlists; they use the library views,
  playback and covers. Read-only for now (`store.writable` false for them).
- Next (step 2): copy songs to players (with per-model conversion and
  tag/cover rules: ID3v2.3 for Walkman covers, Sansa e200 size to measure),
  write playlists in the chosen format, delete songs.

## Setting up iPods (patch 0044)

- Devices: a volume with `iPod_Control` but no iTunesDB is listed with
  `needsSetup` (devices.rs `needs_setup`); clicking it opens SetupDialog.
  File → Set Up iPod… picks any drive. Right-click an iPod → Rebuild Database….
  An iPod whose database fails to open shows Try Again / Rebuild on the
  welcome page (`store.openFailed`).
- setup.rs `run`: (rebuild: `chinotto_gpod::set_aside_database` moves
  iTunesDB, iTunesSD/PState/Stats/Play Counts/OTGPlaylistInfo and the Artwork
  folder into `Chinotto Backups/Before rebuild <time>/`) → FireWire ID into
  SysInfo (keeps other lines) → `chinotto_gpod::init_ipod` (libgpod
  itdb_init_ipod with the model number, e.g. xA477) → optionally every audio
  file in iPod_Control/Music added with `Database::add_existing` (ipod_path set,
  no copy) and their embedded covers (`write::add_embedded_covers`).
  Marks the iPod writable afterwards (setting it up is the go-ahead).
- FireWire ID = USB serial: Linux via /sys/class/block/<dev> up to the USB
  device (idVendor 05ac); macOS via `system_profiler SPUSBDataType /
  SPUSBHostDataType -json` (the device with this mount point, or the only
  Apple device with a 16-hex serial). **macOS path untested.** Required for
  classic 6G/7G and nano 3G (`models::needs_firewire_id`).
- Models offered: `models::setup_models` (one model number per kind; no
  shuffles, no nano 4G/5G: hash72/hashAB can't be made from scratch).
- Covers with no ArtworkDB / no covers yet: cover.rs makes a blank ArtworkDB
  (`artworkdb::blank`, laid out like libgpod/iTunes) and a template entry
  (`artworkdb::template_image`) from `models::cover_formats` (classic/nano3G:
  1055/1060/1061 as iTunes writes them, mhif list incl. 1068/1069; nano 1G/2G:
  1027/1031; 5G: 1028/1029; photo: 1016/1017, from libgpod's tables).
  Tested here on a folder set up as nano 3G (hash58 present, covers read back)
  and nano 2G (in the app). **Not yet tested on a real iPod.**
- Gap: an iPod with neither FireWire ID nor serial gets no identity, so
  "Allow changes" can't be turned on for it.
- Still to do: format/restore (partition + firmware, via ipodpatcher-like
  code), much riskier; the device layer for MSC/MTP comes with the MSC pass.

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

## Playback (2026-10-20)

- `crates/chinotto-player`: Symphonia 0.6 decoding (`decode.rs`) on a player
  thread into a ~0.5 s buffer; cpal 0.18 output whose callback drains it
  (volume applied there, squared for perceived loudness). Output runs at the
  file's rate when the device allows, else linear resampling (`convert.rs`).
  Small fixed buffer (2048 frames): PulseAudio otherwise asks for seconds of
  audio at a time. The output stream is paused whenever nothing plays (a
  running stream can keep a Mac awake). Events: Status (~4/s while playing),
  Ended, Error. `stop_and_wait` closes the file before ejecting.
- Linux: cpal's `pulseaudio` feature (pure Rust) so PulseAudio/PipeWire is
  used directly; ALSA otherwise. CI needs `libasound2-dev`.
- Formats tested: MP3, AAC, Apple Lossless, FLAC, Ogg Vorbis, AIFF, WAV
  (`tests/decode.rs` with `CHINOTTO_TEST_AUDIO`; real-time test
  `tests/playback.rs` with `CHINOTTO_TEST_PLAYBACK`). WMA and HE-AAC: not
  yet (FFmpeg helper, with the library). AAC encoder padding isn't trimmed
  (~20 ms extra at the end).
- App: `playback.rs` commands (play_track by dbid from the open iPod, play,
  pause, stop, seek, volume). Toolbar: previous / play-pause / next, volume
  (remembered in localStorage); the amber display shows the playing song with
  a seek bar. Double-click or Return plays from that song down the current
  list (that list is the queue); Space plays/pauses; previous restarts the
  song if >3 s in; unplayable songs (e.g. missing file) are skipped, up to 4
  in a row. Artwork pane shows "Now playing" (click to switch to selected).
- Not yet: shuffle/repeat, gapless, media keys, play counts from computer plays.

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
  `app/src-tauri/icons/icon-original.png`.
  - `icon-source.png`: the rounded artwork on a 1024 canvas, no shadow
    (Linux/Windows icons).
  - `icon-macos-fullbleed.png`: an opaque full-bleed square (8 px trimmed,
    transparent corners filled with nearby background). macOS 26 puts icons
    that aren't full squares on a grey plate and shrinks them; it rounds a
    full-bleed square itself. `icon.icns` is built from this one.
  - Regenerate: `npx tauri icon src-tauri/icons/icon-source.png -o src-tauri/icons`,
    then `npx tauri icon src-tauri/icons/icon-macos-fullbleed.png -o /tmp/x` and
    copy `/tmp/x/icon.icns` over; delete the `android`, `ios`, `Square*` and
    `StoreLogo` outputs.
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

In the app (2026-10-20, tested on a scratch copy laid out like the nano):
- Every iPod is read-only until "Allow changes to this iPod" is switched on on
  its device page. Remembered per iPod (FireWire GUID, else serial) in the
  app config folder's `settings.json` (`writableIpods`).
- `app/src-tauri/src/write.rs`: set_allow_changes, edit_tracks, delete_tracks,
  add_files (files or folders; progress via `add-progress` events; adds to the
  open playlist too), create/rename/delete_playlist, set_playlist_tracks.
  Each re-checks permission, applies via chinotto-gpod, saves, re-reads the
  iPod and returns `{view, extra}`; on any error libgpod's copy is dropped.
- `tags.rs`: lofty → TrackInfo; kinds MP3/AAC/Apple Lossless (m4a with a bit
  depth)/WAV/AIFF; FLAC/OGG/etc. reported as needing conversion; album artist
  falls back to artist; missing sort fields filled per the Swinsian rules.
- UI: Get Info (Info/Sorting/Details; multi-song with "Mixed"; only touched
  fields saved), right-click menus, Delete key (in a playlist: remove from it;
  elsewhere: delete from iPod after confirming), ⌘/Ctrl+I, ⌘/Ctrl+O, ⌘/Ctrl+N,
  drop files/folders on the window, drag songs onto playlists, double-click a
  playlist to rename. Selection is kept across saves by dbid.
- Song dragging is Chinotto's own (mouse events + `store.songDrag`, drop
  targets marked `data-drop-playlist`), not HTML5 drag-and-drop: Tauri's
  native file-drop handler intercepts HTML5 drags on macOS. OS file drops
  are recognised by carrying paths.
- Confirmed on the nano by the developer: adding via the file dialog, editing
  old and new songs, playlists (new and existing), eject.
- Artwork (2026-10-20): `app/src-tauri/src/cover.rs`. Image → 600 px rule
  (shrink only) → each of the iPod's common formats by fill-and-crop →
  RGB565 appended to `F<fmt>_<n>.ithmb` (new file at ≥64 MB) → ArtworkDB
  entries cloned from an existing one (`ArtworkDb::append`), written via temp
  + rename after backing it up to Chinotto Backups (`ArtworkDB <unix time>`,
  newest 5 kept) → songs linked with `Field::Artwork` in the libgpod save.
  Songs given the same image share thumbnails. Removing sets has_artwork=2 and
  leaves the thumbnails unused. Covers embedded in added files are used
  automatically. Get Info has an Artwork tab (choose, drop, remove; applied
  on OK). Needs an existing ArtworkDB with at least one cover to clone (an
  iPod with no covers at all can't get covers yet). Tested on a temp copy of
  CAESAR's DB (`cover::tests`) and in the app on the scratch copy.
- Not yet: BPM (not read by chinotto-ipod), playlist reordering by drag,
  smart playlist editing, conversion of FLAC etc. (FFmpeg helper).


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
- First real writes: on the nano CHINOTTO, after a "Build apps" run.
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

- Formats: 1055 (128×128, 32,768 B), 1060 (320×320, 204,800 B), 1061 (6,160 B =
  **56-pixel rows × 55 rows**, measured on the nano 3G's own covers, whose
  headers say 55×55; TunesReloaded's say 56×56, same storage). New thumbnails must start on a slot boundary
  (offset a multiple of the slot size): the iPod finds them by slot number,
  so appending at an unaligned file end distorts them (0012). 0010/0011 guessed
  the row width wrong; 0013 measures it. RGB565 little-endian, row-major, no padding.
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

## 0054: MTP only when the device offers it

`chinotto_has_mtp_interface` (shim.c) reads the USB descriptors (without opening the device) and `detect()` skips devices whose interfaces are all mass storage (class 8). (0057: checking for class 6/255 with 3 endpoints hid the NWZ-E344 in MTP mode on Linux.) A player in mass-storage mode (NWZ-E344 on a Mac, Sansa on "Auto Detect") keeps its USB ID, which libmtp knows, so it used to be listed as MTP as well as a drive, and opening it failed ("in use"). Now it appears only as the drive. To test MTP on the Sansa: SanDisk firmware, Settings → USB Mode → MTP.

## 0055: copying to and deleting from MTP devices

- chinotto-mtp: `Device::send_track` (LIBMTP_Send_Track_From_File, parent 0 = the device's music folder), `delete`, `supported_filetypes`, `filetype_for` (an .m4a goes as M4A, MP4 or AAC, whichever the device lists); errors carry libmtp's own message.
- mtp.rs: `add` and `delete`, reached through `msc_add` / `msc_delete` / `msc_state` / `msc_allow` / `msc_set_conversion` when the path starts with "mtp:", so the page, drag and drop, menus and the delete dialog are the MSC ones. Settings are kept under "mtp:<vid>:<pid>". What it plays comes from the file types it lists; the rules (ID3v2.3, plain Greek, small covers) from the MSC profile for its model (`profile_for`), without folder.jpg. Covers are embedded only; MTP album objects (representative samples) come with playlists. A song already on it (same title, artist and album) isn't sent again.
- Opening on Linux: if it fails, Chinotto unmounts gvfs's mtp:// mounts (`gio mount -u`) and tries once more.
- Editing songs on an MTP device isn't possible yet (`edit_rules` says so).
- Tested so far: NWZ-E344 offers MTP to Linux (disk to the Mac). The Sansa e260 v2 (with the Rockbox bootloader) doesn't answer MTP's OpenSession on either, with Fedora's libmtp too.

## 0056: reading songs again after an interrupted scan

- A file that disappears while its tags are read (a player unplugged mid-scan) is stored with mtime 0, so the next scan reads it again instead of keeping the file-name-only tags.
- "Read All Songs Again" (library and player menus in the sidebar): `scan_library(id, full: true)` re-reads every file's tags, not only new and changed ones.

## 0057: Walkman in MTP mode listed again; Add Drive refuses MTP paths

- The MTP filter now only hides devices that are mass storage and nothing else.
- Add Drive with a file-manager MTP path (gvfs `…/gvfs/mtp:host=…`) says to open the device under Devices instead.

## 0058: mass-storage-only players listed as MTP again on Linux

The NWZ-E344 has a single USB interface, class 8 (mass storage), and still answers MTP on it: on Linux libmtp unloads usb-storage (its device flags say to) and talks MTP over the same endpoints, and the Walkman then shows "USB (MTP)". macOS's mass-storage driver can't be unloaded (claim fails with -3), so the class-8-only filter now applies on macOS only. On Linux, opening such a player over MTP takes it away as a drive until it's replugged.

The Sansa e260 v2 here (Rockbox bootloader installed) isn't a guide for MTP: it doesn't answer OpenSession anywhere. The NWZ-E344 on Linux is the MTP test device.

## 0059: songs already on an MTP device are listed

Devices were opened with `LIBMTP_Open_Raw_Device_Uncached`, and libmtp's `flush_handles` does nothing on uncached devices, so `Get_Tracklisting` and `Get_Playlist_List` found no objects: only songs Chinotto had just sent showed. Now `LIBMTP_Open_Raw_Device` (cached). Opening reads every object's metadata once (the NWZ-E344 with ~1,000 songs: a few seconds); songs sent afterwards are added to the cache by libmtp.

## 0060: editing songs on MTP devices

`lib_edit_tracks` on the open MTP device's library goes to `mtp::edit`, which changes the device's database with `Device::update_track` (LIBMTP_Update_Track_Metadata). libmtp sets every property the device supports from the track struct, so the app keeps each song as read (`Opened::tracks`, updated on send/delete/edit) and sends it back with the changes. Kept: title, artist, album, genre, composer, year, track number, rating (a rating of 0 isn't sent: libmtp skips it). Other fields and covers are reported as not kept. The model's rules apply (plain Greek on Walkmans). Needs "Allow changes".

## 0062: recognize and browse iPod nano 6G

- Model IDs C525/C526 and C688–C699 identify the 6th-generation nano. Some
  volumes omit `SysInfo`; `iTunesCDB` plus the device-created
  `iTunes Library.itlp/Locations.itdb` identifies this database family.
- `iTunesCDB` can be read through libgpod and exposed as a browseable session.
  The app must not offer setup just because `iTunesDB` is absent.

## 0063: experimental iPod nano 6G writes

- libgpod now uses the vendored HashAB implementation from `dstaley/hashab`
  (Unlicense) instead of dynamically loading a missing `libhashab` blob. It
  signs the compressed iTunesCDB with the Nano 6 USB FireWire GUID.
- `chinotto-gpod` parses that newly written CDB with PodBox's MIT-licensed
  iTunesDB/SQLite code, verifies the CDB's HashAB signature to recover its
  nonce, stages and validates the companion `iTunes Library.itlp` databases,
  and swaps them in only after validation. A full Nano 6 backup of CDB, DB,
  and the SQLite bundle is kept before each save; failures restore the bundle.
- USB identity fallback is used when SysInfo omits the GUID. Do not write that
  value into SysInfo; it is only passed to libgpod for signing.
- This is experimental and has not been exercised on hardware in this patch.

## 0064–0066: Nano 6 database identity and playlist readback

- Some Nano 6 `Library.itdb` files have a `db_info.pid` that differs from the
  iTunesCDB header ID. This difference is valid: retain the companion database's
  own ID while keeping its master-container ID aligned. Do not rewrite
  `db_info.pid` from the CDB.
- libgpod prefers the type-3 dataset whenever it exists. Nano 6 CDBs can have
  an empty type-3 Podcasts dataset and real playlists in type 2, so prefer type
  3 only when its playlist count is nonzero; otherwise parse type 2.
- With the reader fix installed, playlist editing was confirmed working on the
  user's Nano 6.

## 0067: add songs to Nano 6

- Song imports are enabled for Nano 6. libgpod copies the song into the iPod's
  Music folder; the Nano staging bridge receives the in-memory track list and
  merges new tracks into the parsed CDB before regenerating the master and user
  playlist memberships.
- Rebuild and sign both iTunesCDB and the companion Library/Locations SQLite
  databases using the existing HashAB nonce. Covers, song edits, and song
  deletion remain unavailable; do not widen those operations without separate
  validation. Existing device data is backed up before each write.
- The user confirmed that an added song played from a playlist, but the Nano's
  Albums index was empty and Songs ordering was scrambled. The companion
  Library.itdb still contained the new track and its album/artist links; its
  derived browse flags and ordering indexes had been rebuilt incorrectly.

## 0068: restore Nano 6 browse indexes during writes

- Rank song, artist, album, genre, and composer fields with the Nano sort-key
  encoding, store zero-based physical order, and populate the companion
  database's album/artist `has_songs` flags and album counts.
- Validate required sort indexes and browse flags before installing staged
  databases. Compile-checked only; the repaired database still needs a
  real-device check with the user's Nano 6.

## 0069: restore Nano 6 artist, genre, and artwork metadata

- Rebuild `track_artist` rows and item links, set genre browse flags/counts,
  and keep album artwork item links consistent with the companion database.
- Preserve the iTunesCDB artwork fields in `Library.itdb`, including for new
  tracks, and allow embedded or manually assigned covers on Nano 6 using its
  observed artwork thumbnail formats.
- This targets the missing browse metadata and cover links seen after Chinotto
  writes the Nano databases. Compatibility with Apple Music.app sync still
  requires a real-device check; no Apple sync was performed during development.

## 0070: refresh existing Nano 6 artist sort indexes

- Existing `track_artist` rows are retained to preserve their persistent IDs,
  so refresh each row's display name and sort order during every database
  rebuild instead of leaving stale indexes in place.
- Mark only artists with linked tracks as populated, which also clears stale
  artist entries left by earlier database rewrites.

## 0071: clear CI formatting and Clippy blockers

- Format the Nano 6 FFI code with rustfmt and remove a redundant struct default
  update so CI can proceed through Rust formatting and Clippy.

## 0072: enable Nano 6 song editing and deletion

- Allow the existing edit and delete commands on Nano 6. Refresh modeled CDB
  track fields for existing persistent IDs, remove deleted tracks from the CDB,
  and drop deleted IDs from retained playlists before staging both databases.
- Keep CDB sort keys and editable text/numeric metadata synchronized with the
  companion SQLite indexes. Song files are still removed only after both
  staged databases are installed successfully.
- Nano 6 metadata edits and deletion still require a real-device check.

## 0073: identify legacy Sony OpenMG Walkmans safely

- Detect mounted roots containing `OMGAUDIO` as a separate `openmg` device
  kind, rather than attaching the generic MSC library to the usually empty
  `MUSIC` folder.
- Open a device-info dialog from the sidebar. Generic MSC setup is rejected by
  `msc_inspect`/`msc_save`, and the shared player resolver blocks generic
  settings, copy, and delete commands for these roots.
- This is only recognition and write protection. Initialization, parsing of
  OMGAUDIO's indexed database, and OpenMG-compatible song encoding/transfer
  are not implemented. The user's Sony NW-S716F was initialized with legacy
  device software; its live database had one playable `.OMA` track in
  `OMGAUDIO` while `MUSIC` remained empty. Do not treat it as a normal `NWZ`
  drag-and-drop Walkman.

## 0074: give OpenMG devices a dedicated page

- Selecting an `openmg` device now opens a full device page rather than a modal.
- The page shows the model, capacity, OMGAUDIO database location and eject
  control, and explains that generic writes remain disabled while native
  initialization and song transfer are under development.
- The selected device clears from the page if it is unplugged.

## 0075: fix the OpenMG toolbar type check

- Bind the narrowed OpenMG source path before looking up its device so
  `svelte-check` accepts the discriminated union.

## 0076: show OpenMG track metadata read-only

- Add a guarded `openmg_view` command that reads `OMGAUDIO/04CNTINF.DAT`
  only after confirming the selected root is an OpenMG device. Parse the
  documented fixed-size track records and the UTF-16BE tag blocks into title,
  artist, album, genre, and duration fields.
- Show those fields in a read-only table on the OpenMG device page. There is
  still no playback, initialization, or write support for OpenMG devices.
- The parser currently follows the one-track database observed on the user's
  NW-S716F. Confirm the record-to-audio-file relationship and multi-track
  behavior against a second track before building on this reader.

## 0077: document the OpenMG transfer investigation checkpoint

- The user added two more songs with third-party device software. Both now
  appear and play on the NW-S716F. The device currently reports six tracks in
  `04CNTINF.DAT`;
  the two new files are `10F00/10000005.OMA` and
  `10F00/10000006.OMA`.
- For all six observed tracks, the 11-byte key in `05CIDLST.DAT` matches the
  corresponding `.OMA` file at offset `0xC15` (the EA3 header starts at
  `0xC00`; `0xC14` is a preceding per-track byte). The audio payload begins at
  `0xC60`. This links the track table, key list, and audio files on
  this device, but does not yet define how to generate a valid new file/key.
- The latest transfer log shows one MP3 was copied without transcoding and
  another was transcoded to MP3 320 kbps before being written. Their playback
  on the Walkman confirms the result. Do not reuse third-party source code;
  treat the device output and database diffs as observations.
- The database changed in `04CNTINF.DAT`, `05CIDLST.DAT`, `03GINF01/02/03/2D`,
  `02TREINF.DAT`, and `01TREE01/02/03/04/2D`. The current six-track snapshots
  are under `/private/tmp/chinotto-openmg-baseline-6track-20261023` and the
  three- and four-track comparisons are in sibling snapshot folders. These
  are local research artifacts, not repository fixtures.
- A standard OMA muxer is not a drop-in writer for this device's files. An
  independent reverse-engineering project describes additional key locations
  and a device-specific OpenMG variant; those claims do not fully match the
  files visible in this user's currently working volume. Use this S716F's
  observed behavior as the target and resolve the difference before enabling
  writes. Do not infer a complete writer from the `04CNTINF`/`05CIDLST` diffs
  alone, and keep generic MSC writes blocked for OpenMG roots.
- Next work: continue decoding the device-specific `.OMA` header and the
  changed index records, then implement a staged/off-device generator and
  validate its output against the known-good six-track state before offering
  any device write action. No files were written to the physical Walkman
  during this investigation.

## 0078: show OpenMG audio-file linkage

- Extend the guarded OpenMG read command to match each `04CNTINF.DAT` row to
  its OMA file using the 11-byte key in `05CIDLST.DAT` and the key at offset
  `0xC15` in the file's EA3 header. Search the `10F*` audio folders instead of
  assuming track-number filenames are the authoritative link.
- Show the linked OMA filename on the existing read-only device page, or an
  explicit missing-link message. This provides a check of the on-disk track
  relationship before building a write path. It does not enable device writes.

## 0079: isolate one OpenMG track addition

- Captured a read-only six-track baseline of the user's NW-S716F database and
  artwork before the user added one track through third-party device software.
  The comparison is under `/private/tmp/chinotto-openmg-baseline-6track-current`
  and the live Walkman volume; no Chinotto command wrote to the device.
- The new `If We` track is the seventh `04CNTINF.DAT` record (Mariah Carey,
  `Glitter`, `Pop`, 260884 ms), paired with `10F00/10000007.OMA` through the
  11-byte key `aa1e4e90edf693b74f7493` at OMA offset `0xC15`.
- The addition appends one 656-byte track record to `04CNTINF.DAT` and one
  48-byte key record to `05CIDLST.DAT`. It adds one 0x90-byte record each to
  `03GINF02.DAT`, `03GINF03.DAT`, and `03GINF04.DAT` for the new artist,
  album, and genre; `03GINF01.DAT` gains a 0x310-byte combined group record,
  while `03GINF2D.DAT` gains two 0x210-byte hierarchy records. The matching
  `01TREE01/02/03/04/2D.DAT` tables and `02TREINF.DAT` also change to reflect
  the new group and track order.
- Artwork assets under `20P*` were added or updated as part of this operation,
  so the eventual writer must account for those references and images as well
  as the track and group tables. This single insertion isolates the new-group
  case; it still does not establish all details needed to generate a valid
  database or OMA file. Keep OpenMG writes disabled until an off-device
  generator can be checked against these observations.

## 0080: add an isolated OpenMG track-record encoder

- Add an internal helper that prepares one 656-byte `04CNTINF.DAT` track
  record by copying a known-good template, replacing the observed duration
  field, and encoding the four metadata tags as UTF-16BE.
- Preserve all unknown record bytes from the template. Reject tag values that
  exceed the fixed record capacity rather than silently truncating them.
- This is only an off-device building block. It is not called by a transfer
  command and does not update `05CIDLST`, group tables, tree indexes, artwork,
  or OMA files. OpenMG device writes remain disabled.

## 0081: prepare OpenMG track-key and counted-table rows

- Add an in-memory `05CIDLST.DAT` row helper that preserves the opaque row
  bytes and substitutes the 11-byte key obtained from the candidate OMA file.
- Add a generic counted-table append helper for the observed 48-byte OpenMG
  header layout. It validates the magic, declared row size, count, and exact
  current table length, then updates the count and payload-length fields.
- These helpers do not write files or update group/tree relationships. They
  are groundwork for an off-device generator only; device writing remains
  disabled until the full database can be generated and validated together.

## 0082: prepare OpenMG group metadata records

- Add template-based encoders for the fixed 144-byte artist, album, and genre
  rows and the 784-byte combined album-summary row observed in this device's
  `03GINF` tables. They update the metadata text and representative duration
  while preserving unknown bytes from a same-table template.
- These records still need stable group identifiers and corresponding
  `01TREE`/`03GINF2D` hierarchy references. The helpers are not connected to
  transfer and do not write to the Walkman.

## 0084: document OpenMG tree-index observations

- The before/after database captures show each `01TREE*.DAT` file remains
  0x4060 bytes. Its group list begins at 0x40 with 8-byte records, and its
  `TPLB` track-order table begins at 0x4050 with 16-bit track references.
  The track-order sequence changes when a new track is added and differs by
  tree file, so it cannot be safely regenerated from a single sort rule.
- `01TREE2D.DAT` adds two 8-byte nodes when the new artist and album are
  introduced. The node type values distinguish the two kinds, but the album
  node's reference field is inconsistent across the existing records. Keep
  preserving these records and do not synthesize hierarchy references until
  their meaning is resolved.
- This checkpoint records observed structure only. OpenMG transfer stays
  disabled; the current template encoders are not a complete database writer.

## 0085: clarify OpenMG group-duration fields

- Comparing the seven track durations with `03GINF01/02/03/04.DAT` shows each
  artist, album, and genre row stores the sum of its member tracks' durations.
  The combined album-summary row stores that album's total as well. For
  example, `Jagged Little Pill` is 514481 ms across its two tracks, and
  `Unknown Genre` is 713036 ms across its three tracks.
- Rename the template-encoder parameters to describe these totals. Database
  generation must recalculate the affected group totals when adding or removing
  tracks; no transfer path is enabled by this change.

## 0086: require explicit OpenMG album-art metadata

- The 784-byte combined album-summary row contains a 384-byte artwork section
  after its three text tags. Observed `PICP`/`PIC0` entries point to image files
  under `20P*` directories, so copying this tail from another album would leave
  incorrect artwork references.
- Require callers to provide a full artwork-metadata tail when preparing the
  row. This prevents the helper from silently carrying another album's art
  references. Image allocation and file generation remain unimplemented, and
  OpenMG transfer stays disabled.

## 0087: prepare OpenMG album hierarchy rows

- The ten-row `03GINF2D.DAT` hierarchy table contains separate 528-byte album
  rows with `TIT2`, `XSOT`, `PICP`, and `PIC0` tags. Its artwork points to
  `IT*` image files under `20P*` directories, while the combined album-summary
  table points to corresponding `1G*` image files.
- Add an in-memory encoder for an album hierarchy row. It updates the title and
  sort-title tags and requires correctly tagged 128-byte `PICP` and `PIC0`
  descriptors from the new album. It does not allocate image names, update the
  hierarchy indexes, generate images, or write to the device.

## 0089: OpenMG format solved; adding songs

Worked out from the NW-S716F's own files (seven songs added by other
software), and checked by rebuilding them: `crates/chinotto-omg`'s test
`rebuilds_device_tables` regenerates every song-dependent table *and* every
album picture from the song list alone, byte for byte (run it with
`CHINOTTO_TEST_OMG` pointing at a copy of OMGAUDIO; never commit the copy).

- **The audio isn't encrypted.** An `.OMA` is a 0xC00-byte `ea3` tag (ID3v2.3
  with another magic; TIT2, TPE1, TALB, TCON, TYER if any, TLEN; UTF-16BE),
  a 0x60-byte `EA3` header (`FF FF` = not encrypted; 12-byte content ID after
  `01 0F 50 00 00 04 00 00`; codec `03 84 D9|DE 00` for 128/320 kbps MP3;
  length in ms; frame count) and the MP3 frames as they are. The EKB in
  `00010021.DAT` is for ATRAC and irrelevant here.
- **Tables** share one layout (see `db.rs`). `04CNTINF` (songs) and
  `05CIDLST` (content IDs, linking rows to files) are the data. Trees 01–04 and
  2D, with their `03GINF` group tables, are derived from them (sort rules in
  `db.rs`: group names by UTF-16 code units; an album's songs in row order).
  `02TREINF` changes only in its total length (trees 01–04, 2C); `00GTRLST`
  doesn't change. 0084's "can't be regenerated from a single sort rule" is
  resolved: each tree has its own rule.
- **The Walkman rebuilds trees 0C, 0E, 0F and 10–15 itself.** They carry its
  clock's date (January 2007) yet included the newest song, so they aren't
  written.
- **Pictures**: `20Pnn/1Gnnnn{P0,01,02}.JPG` per album group (album tree
  numbering) and `20Pnn/ITnnnn…` per album row of tree 2D; 200, 96 and 32 px
  baseline JPEGs. Numbers shift when albums are added, so all picture folders
  are rewritten (the device had a stale `20P08` from earlier layouts).
- **Adding songs** (OpenMG page: "Allow changes", Add Songs…, or drag onto the
  Walkman): MP3s that are MPEG-1 Layer III, 44.1 kHz, CBR 128 or 320 kbps go as
  they are (tags and any Xing/Info frame stripped); everything else is
  converted to 320 kbps at 44.1 kHz (`convert::mp3_44k`). Each `.OMA` is
  written first, then `install` backs up every table and picture folder to
  `Chinotto Backups/OpenMG <time>` on the Walkman, stages the rebuilt ones in
  `.chinotto-omg-staging`, and swaps them in (each live file is renamed aside
  first; on failure the backup is copied back and the new `.OMA` files are
  removed). New albums get pictures from the song's cover. Tested on a copy
  of the device (`openmg::tests::installs_on_a_copy`), not yet on the Walkman.
- **Unconfirmed, so limited**: only the folder `10F00` (255 songs) is used;
  other bitrates' codec value; songs with several artists in one album (the
  album row takes the first song's artist and genre). Deleting and editing
  need rows renumbered and files renamed, and come next.
- ChatGPT's template helpers (0080–0087) are replaced by the crate and
  removed; the read-only view (`openmg_view`) is unchanged.

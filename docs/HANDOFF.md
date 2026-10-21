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

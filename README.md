# Chinotto

A music library manager and sync app for classic iPods and other digital
audio players, modelled on the workflows of classic iTunes (9/10 era).

## Ethos

This is the kind of program iTunes and MediaMonkey used to be, built for a
modern Mac or Linux machine instead of a Windows XP box from 2005 — the app
I wish I'd had 25 years ago. It's for the Mac user whose Walkman wants
SonicStage or Media Center and nothing else, the Linux user who's always had
to compromise on iPod support, and the nano owner who wanted a more recent
model with album art, not just the oldest ones that happen to be well
supported everywhere.

Every line of code here was written by AI — Claude (Sonnet and Opus),
ChatGPT ("Luna"), and GitHub Copilot — working from the developer's ideas,
testing, and direction. Beyond that, nobody involved feels they can take
credit for it, so it's released under the MIT license: use it, change it,
pull it apart, build on it.

Status: early. The desktop app recognises a player when it's plugged in —
a classic, pre-iOS iPod, an MTP device (such as a Creative Zen), or a Sony
Walkman using OpenMG (most NW-A/S/E models) — and lets you browse it: source
list with its playlists, Genre → Artist → Album column browser, sortable song
list, search, covers, and a device page with the capacity bar. Other
drive-like players (most Sansa models, older MSC Walkmans) can be added by
hand the same way, once told which folders hold the music and playlists.

Once you switch on "Allow changes" on a player's page, Chinotto can also edit
songs (Song Details), add and delete them, and make and change playlists.
Every save backs up the player's database first.

It also plays a player's songs: MP3 and AAC everywhere; Apple Lossless, WAV
and AIFF on iPods and MTP devices; FLAC and Ogg Vorbis too, for a library of
your own on your computer.

## Supported players

- **Classic iPods** (up to the iPod classic, nano 6G and shuffle 4G — not the
  iOS-based touch): full read/write support, including playlists, ratings,
  and artwork. The nano 6G's compressed database is handled separately (see
  Acknowledgements) and its write support is still experimental.
- **MTP devices** (Creative Zen, Sansa in MTP mode, and similar): browsing,
  playback, adding and deleting songs, editing tags and covers, and writing
  M3U/M3U8 playlists.
- **MSC (mass-storage) players** — anything that just shows up as a drive,
  such as most Sansa models and Walkmans not using OpenMG: add them once with
  **Add Drive…**, pointing Chinotto at the music and playlist folders; after
  that they're recognised automatically.
- **Sony Walkmans using OpenMG** (most NW-A, NW-S and NW-E series, the format
  used instead of plain MSC on those models): recognised and opened
  automatically, with full read/write support — adding, deleting, and editing
  songs and covers, with albums kept in track order. The OpenMG database
  format was reverse-engineered for this project; see `docs/HANDOFF.md` for
  the write-up.

Reading and writing MP3s, AACs, and other common formats is built in;
anything a player can't take as-is is converted first.

**A Walkman still locked to Media Center or SonicStage** (it won't show up
as a plain OpenMG drive at all) needs reformatting before Chinotto, or
anything else, can use it as a drive. Chinotto has no way to do that itself.
The developer used ManicStage once, on the test device, to set it up for
direct folder access — no code from ManicStage is used in Chinotto, and it
isn't needed again once the Walkman is reformatted.

## Tested devices

So far, this is the developer's own hardware, run through the app itself
(not just tested against copies of its database files):

| Device | Connection | Notes |
|---|---|---|
| iPod classic 7G | iPod, HFS+ | |
| iPod nano 3G | iPod, HFS+ | Needs its FireWire ID |
| iPod nano 2G | iPod, FAT32 | |
| iPod nano 6G | iPod, compressed database | Writing is still experimental |
| SanDisk Sansa e260 v2 | MSC | Official firmware; MTP mode didn't work right |
| Sony Walkman NWZ-E44/E45 series | MTP (Linux), MSC (Mac) | Playlists over MTP are still unreliable |
| Sony Walkman NW-A25 | MSC | |
| Sony NW-S716F (Japanese model) | OpenMG | |

**MTP is the least reliable connection of the four right now** — where a
player can also run as MSC (most Sony Walkmans, and some Sansas), that's the
one to use with Chinotto for the time being.

## Feature support by player type

|  | Classic iPod | MTP devices | MSC (drive) players | Sony Walkman (OpenMG) |
|---|---|---|---|---|
| Browse & play | ✅ | ✅ | ✅ | ✅ |
| Add / delete songs | ✅ | ✅ | ✅ | ✅ (up to 255 songs, one storage folder) |
| Edit tags & covers | ✅ | ✅ (title, artist, album, genre, composer, year, track number) | ✅ | ✅ (title, artist, album, genre, year, track and disc number) |
| Playlists | ✅ read and write | ⚠️ read; writing is unreliable | ✅ read and write, M3U/M3U8 players only | ❌ the format has none |
| Ratings | ✅ | ✅ | ❌ | ❌ |

The nano 6G's write support and MTP's playlist writing are both still
experimental (see Tested devices above).

## Layout

- `crates/chinotto-ipod` — iTunesDB / ArtworkDB parsing and writing, RGB565
  thumbnails, SysInfo. No external dependencies.
- `crates/chinotto-gpod` — changes the iPod's database through a bundled
  libgpod (LGPL; see its README), including the nano 6G's compressed
  database (see Acknowledgements).
- `crates/chinotto-mtp` — MTP device access (listing, reading, and writing
  songs, covers and playlists).
- `crates/chinotto-omg` — the Sony OpenMG `.OMA` file format and OMGAUDIO
  database, built from scratch for this project (see Acknowledgements for
  what it doesn't cover). No external dependencies.
- `crates/chinotto-player` — audio playback (Symphonia + cpal).
- `app/` — the desktop app (Tauri 2).
  - `app/src/` — the window's interface (Svelte 5 + TypeScript).
  - `app/src-tauri/` — the Rust side: device detection and watching
    (`devices.rs`), the iPod (`setup.rs`, `state.rs`, `write.rs`,
    `cover.rs`), MTP devices (`mtp.rs`), drive-based MSC players
    (`msc.rs`), OpenMG Walkmans (`openmg.rs`), playback (`playback.rs`),
    libraries of your own files (`library.rs`), and format conversion
    (`convert.rs`).
- `.github/workflows/` — `ci.yml` checks every push on Linux and macOS;
  `build.yml` builds the installable apps.

## iPod disk formats (Mac or Windows formatted)

Chinotto works with the iPod's files once your computer has mounted it, so
the formats it can use are the ones your system supports:

| iPod formatted as | macOS | Linux |
|---|---|---|
| Windows (FAT32) | read and write | read and write |
| Mac (HFS+), journaling off | read and write | read and write |
| Mac (HFS+), journaling on (iTunes' default) | read and write | **read only** |

**On Linux, FAT32 is the safe choice.** Every system reads and writes it with
no setup. Its limits don't matter much for an iPod: files can't be over 4 GB
(songs never are), and it copes less well with being unplugged without
ejecting, so eject first.

**A Mac-formatted iPod on Linux:** Linux's HFS+ driver won't write to a
journaled volume. Turn journaling off once, on a Mac (Disk Utility, or
`diskutil disableJournal /Volumes/<iPod name>`); after that Linux can write
to it too. Don't force a journaled iPod writable on Linux (`mount -o
force,rw`): the journal isn't kept up to date, and a Mac may later damage the
disk replaying it. The `hfsprogs` package adds `fsck.hfsplus`, which clears
the read-only state Linux uses after an iPod was unplugged without ejecting.

**Switching to FAT32** means restoring the iPod, which erases it: use iTunes
on a Windows PC (a virtual machine works); a Mac's Finder always restores to
the Mac format. Then copy the music back.

## Setting up an iPod without iTunes

An iPod with no music database (formatted or restored without iTunes, or
with its database lost) shows up under Devices with **Set Up…**; any other
drive can be chosen with **File → Set Up iPod…**. Chinotto creates the
database and folders, records the model, and can add back the songs already
in `iPod_Control/Music`, with their covers. Nothing else on the drive changes.

The iPod classic and nano 3G need their FireWire ID for the database
checksum (without it they show no music). It's the iPod's USB serial number;
Chinotto reads it from the system, or you can type it in.

**Rebuild Database…** (right-click the iPod) replaces a broken database the
same way. The old one isn't deleted: it's moved, with the artwork, to
`iPod_Control/iTunes/Chinotto Backups/Before rebuild <time>/`. Playlists
and ratings are carried over when the old database can still be read; from
a damaged one they're lost. Play counts are always lost.

## Getting the apps

On GitHub: Actions → **Build apps** → Run workflow. When it finishes, the
run page has two downloads: `Chinotto-macOS` (a universal `.dmg` for Apple
Silicon and Intel) and `Chinotto-Linux` (`.deb`, `.AppImage`, `.rpm`).
Pushing a tag such as `v0.1.0` also attaches them to a draft release.

The Mac app isn't notarised (that needs a paid Apple developer account), so
the first time macOS will refuse to open it. Either right-click the app →
Open, or System Settings → Privacy & Security → "Open Anyway". If macOS says
the app is damaged, run this once; it only removes the download flag from
the app itself:

    xattr -dr com.apple.quarantine /Applications/Chinotto.app

## Building it yourself

You need Rust (rustup.rs) and Node.js 20 or newer.

- **Linux** also needs the WebKitGTK and libxml2 development packages. Debian/Ubuntu:
  `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev libxdo-dev libxml2-dev zlib1g-dev libasound2-dev`
- **macOS** needs the Xcode command line tools (`xcode-select --install`),
  plus a static glib for libgpod, built once:
  `brew install meson ninja && scripts/build-glib-macos.sh deps`, then
  `export CHINOTTO_DEPS=$PWD/deps` before building.

Then:

    cd app
    npm install
    npm run tauri dev       # run with live reload
    npm run tauri build     # make the installable app (in target/release/bundle)

To open a copy of a player's folder at start-up (an iPod, or an MSC player
already set up with Chinotto), pass it as the first argument or set
`CHINOTTO_OPEN=/path/to/folder`.

## Tests against a real device

Some tests need a copy of a real player's database files. They are never
committed. Point `CHINOTTO_TEST_IPOD` at a folder laid out like an iPod root:

    $CHINOTTO_TEST_IPOD/iPod_Control/iTunes/iTunesDB
    $CHINOTTO_TEST_IPOD/iPod_Control/Artwork/ArtworkDB   (optional)

and `CHINOTTO_TEST_OMG` at a copy of a Sony Walkman's OMGAUDIO folder.
Without these, the tests that need them are skipped. `fixtures-private/` is
git-ignored for keeping such copies inside the checkout.

## Acknowledgements

- The nano 6G's compressed `iTunesCDB` and its companion SQLite library are
  read and written using Milan Marocchi's PodBox
  (`crates/chinotto-gpod/libgpod/src/podbox`, MIT — see the `LICENSE` file
  there).
- Signing that database uses the HashAB implementation from dstaley/hashab
  (`crates/chinotto-gpod/libgpod/src/hashab`, Unlicense/public domain — see
  the `LICENSE` file there).
- MTP album art and playlists follow the approach of XNJB by Richard Low
  (GPLv2): no code from it is used in Chinotto, but it shaped how Chinotto
  creates MTP album objects and playlists.
- ManicStage was used, once, to reformat the test Walkman out of Media
  Center/SonicStage lock-in and into plain OpenMG folder access (see
  "A Walkman still locked to Media Center or SonicStage" above): no code
  from it is used in Chinotto.

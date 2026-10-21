# Chinotto

A music library manager and sync app for classic iPods and other players,
modelled on the workflows of classic iTunes (9/10 era).

Status: early. The desktop app opens a classic iPod (automatically when it's
plugged in) and lets you browse it: source list with the iPod's playlists,
Genre → Artist → Album column browser, sortable song list, search, covers,
and a device page with the capacity bar.

Once you switch on "Allow changes to this iPod" on its page, it can also edit
songs (Song Details), add and delete them, and make and change playlists. Every
save backs up the iPod's database first (`iPod_Control/iTunes/Chinotto Backups`).

It also plays the iPod's songs (MP3, AAC, Apple Lossless, WAV, AIFF; FLAC and
Ogg Vorbis too, for when it has its own library).

## Layout

- `crates/chinotto-ipod` — iTunesDB / ArtworkDB parsing and writing, RGB565
  thumbnails, SysInfo. No external dependencies.
- `crates/chinotto-gpod` — changes the iPod's database through a bundled
  libgpod (LGPL; see its README).
- `crates/chinotto-player` — audio playback (Symphonia + cpal).
- `app/` — the desktop app (Tauri 2).
  - `app/src/` — the window's interface (Svelte 5 + TypeScript).
  - `app/src-tauri/` — the Rust side: device detection, opening an iPod,
    serving covers (`art:` URLs), eject.
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
or Finder on a Windows PC, or on a Mac with the iPod set to the Windows
format, then copy the music back.

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

To open a copy of an iPod's folder at start-up instead of a plugged-in iPod,
pass it as the first argument or set `CHINOTTO_OPEN=/path/to/folder`.

## Tests against a real iPod

Some tests need a copy of an iPod's database files. They are never committed.
Point `CHINOTTO_TEST_IPOD` at a folder laid out like an iPod root:

    $CHINOTTO_TEST_IPOD/iPod_Control/iTunes/iTunesDB
    $CHINOTTO_TEST_IPOD/iPod_Control/Artwork/ArtworkDB   (optional)

Without it, those tests are skipped. `fixtures-private/` is git-ignored for
keeping such a copy inside the checkout.

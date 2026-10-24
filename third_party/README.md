# Third-party sources

- `sources/cdrdao-1.2.6.tar.bz2`: cdrdao's release, unmodified (GPL v2 or
  later; https://github.com/cdrdao/cdrdao). `scripts/build-cdrdao.sh` builds it
  into the macOS app as a separate program that Chinotto runs to burn
  .bin/.cue images. Kept here so the exact source of what's shipped is always
  available; it's also attached to each release.
- `licenses/GPL-3.0.txt`: the GNU GPL 3, which the LGPL 3 (mp3lame-sys) builds
  on.

The other libraries built into Chinotto are in `crates/` with their licences.
`scripts/licenses.py` lists everything, with every licence text, in
`app/src-tauri/licenses/THIRD-PARTY-LICENSES.txt`, which ships inside the app.

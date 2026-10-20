# Chinotto

A music library manager and sync app for classic iPods and other players,
modelled on the workflows of classic iTunes (9/10 era).

Status: early. The first piece is `chinotto-ipod`, a plain-Rust reader/writer
for the iPod's own databases.

## Layout

- `crates/chinotto-ipod` — iTunesDB / ArtworkDB parsing and writing, RGB565
  thumbnails. No external dependencies.

## Tests against a real iPod

Some tests need a copy of an iPod's database files. They are never committed.
Point `CHINOTTO_TEST_IPOD` at a folder laid out like an iPod root:

    $CHINOTTO_TEST_IPOD/iPod_Control/iTunes/iTunesDB
    $CHINOTTO_TEST_IPOD/iPod_Control/Artwork/ArtworkDB   (optional)

Without it, those tests are skipped.

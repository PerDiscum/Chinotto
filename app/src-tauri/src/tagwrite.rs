//! Writing tags (and covers) into music files, for library songs.
//!
//! Safety: the file is copied next to itself, the copy is changed, and only
//! when that worked does the copy replace the original (a rename, which is
//! all-or-nothing). A failure leaves the original untouched.
//!
//! Each format's own tag is edited through lofty's split/merge, so anything
//! Chinotto doesn't know about (other frames, ReplayGain, lyrics…) is kept.

use std::fs::File;
use std::path::{Path, PathBuf};

use chinotto_gpod::Field;
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::AudioFile;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{ItemKey, MergeTag, SplitTag, Tag};

/// A cover change: a new picture (JPEG or PNG bytes), or none.
pub enum CoverChange {
    Set(Vec<u8>, MimeType),
    Remove,
}

/// Apply `fields` (and a cover change) to `tag`. Ratings aren't written to
/// files (players disagree on how), so `Field::Rating` is ignored here.
fn apply(tag: &mut Tag, fields: &[Field], cover: Option<&CoverChange>) {
    fn text(tag: &mut Tag, key: ItemKey, v: &Option<String>) {
        match v {
            Some(s) => {
                tag.insert_text(key, s.clone());
            }
            None => tag.remove_key(key),
        }
    }
    fn num(tag: &mut Tag, key: ItemKey, v: i32) {
        if v > 0 {
            tag.insert_text(key, v.to_string());
        } else {
            tag.remove_key(key);
        }
    }
    for f in fields {
        match f {
            Field::Title(v) => text(tag, ItemKey::TrackTitle, v),
            Field::Artist(v) => text(tag, ItemKey::TrackArtist, v),
            Field::AlbumArtist(v) => text(tag, ItemKey::AlbumArtist, v),
            Field::Album(v) => text(tag, ItemKey::AlbumTitle, v),
            Field::Genre(v) => text(tag, ItemKey::Genre, v),
            Field::Composer(v) => text(tag, ItemKey::Composer, v),
            Field::Comment(v) => text(tag, ItemKey::Comment, v),
            Field::Grouping(v) => text(tag, ItemKey::ContentGroup, v),
            Field::SortTitle(v) => text(tag, ItemKey::TrackTitleSortOrder, v),
            Field::SortArtist(v) => text(tag, ItemKey::TrackArtistSortOrder, v),
            Field::SortAlbum(v) => text(tag, ItemKey::AlbumTitleSortOrder, v),
            Field::SortAlbumArtist(v) => text(tag, ItemKey::AlbumArtistSortOrder, v),
            Field::SortComposer(v) => text(tag, ItemKey::ComposerSortOrder, v),
            Field::Year(y) => {
                tag.remove_key(ItemKey::RecordingDate);
                num(tag, ItemKey::Year, *y);
                if *y > 0 {
                    tag.insert_text(ItemKey::RecordingDate, y.to_string());
                }
            }
            Field::TrackNr(n) => num(tag, ItemKey::TrackNumber, *n),
            Field::TrackCount(n) => num(tag, ItemKey::TrackTotal, *n),
            Field::DiscNr(n) => num(tag, ItemKey::DiscNumber, *n),
            Field::DiscCount(n) => num(tag, ItemKey::DiscTotal, *n),
            Field::Bpm(n) => num(tag, ItemKey::Bpm, *n as i32),
            Field::Compilation(c) => {
                if *c {
                    tag.insert_text(ItemKey::FlagCompilation, "1".into());
                } else {
                    tag.remove_key(ItemKey::FlagCompilation);
                }
            }
            Field::Rating(_) | Field::Artwork(_) => {}
        }
    }
    match cover {
        Some(CoverChange::Set(bytes, mime)) => {
            // The new cover goes first, replacing the front cover. MP4 files
            // don't mark a picture as "front" (they read back as Other), so
            // those go too; back covers, booklets and the like stay.
            let keep: Vec<Picture> =
                tag.pictures().iter().filter(|p| !matches!(p.pic_type(), PictureType::CoverFront | PictureType::Other)).cloned().collect();
            while !tag.pictures().is_empty() {
                tag.remove_picture(0);
            }
            tag.push_picture(Picture::unchecked(bytes.clone()).pic_type(PictureType::CoverFront).mime_type(mime.clone()).build());
            for p in keep {
                tag.push_picture(p);
            }
        }
        Some(CoverChange::Remove) => {
            while !tag.pictures().is_empty() {
                tag.remove_picture(0);
            }
        }
        None => {}
    }
}

/// Split a format's own tag, edit it as a generic tag, merge it back.
fn edit<T: SplitTag>(t: T, fields: &[Field], cover: Option<&CoverChange>) -> <T::Remainder as MergeTag>::Merged {
    let (rest, mut tag) = t.split_tag();
    apply(&mut tag, fields, cover);
    rest.merge_tag(tag)
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Edit the file at `path` in place, as the format `ext` (the original's extension).
fn write_copy(path: &Path, ext: &str, fields: &[Field], cover: Option<&CoverChange>) -> Result<(), String> {
    let opts = ParseOptions::new();
    let w = WriteOptions::default();
    let open = || File::open(path).map_err(|e| e.to_string());
    match ext {
        "mp3" => {
            let mut f = lofty::mpeg::MpegFile::read_from(&mut open()?, opts).map_err(err)?;
            let t = f.remove_id3v2().unwrap_or_default();
            edit(t, fields, cover).save_to_path(path, w).map_err(err)
        }
        "m4a" | "m4b" | "aac" | "mp4" => {
            let mut f = lofty::mp4::Mp4File::read_from(&mut open()?, opts).map_err(err)?;
            let t = f.remove_ilst().unwrap_or_default();
            edit(t, fields, cover).save_to_path(path, w).map_err(err)
        }
        "flac" => {
            let mut f = lofty::flac::FlacFile::read_from(&mut open()?, opts).map_err(err)?;
            let t = f.remove_vorbis_comments().unwrap_or_default();
            edit(t, fields, cover).save_to_path(path, w).map_err(err)
        }
        "ogg" | "oga" => {
            let f = lofty::ogg::VorbisFile::read_from(&mut open()?, opts).map_err(err)?;
            let t = f.vorbis_comments().clone();
            edit(t, fields, cover).save_to_path(path, w).map_err(err)
        }
        "opus" => {
            let f = lofty::ogg::OpusFile::read_from(&mut open()?, opts).map_err(err)?;
            let t = f.vorbis_comments().clone();
            edit(t, fields, cover).save_to_path(path, w).map_err(err)
        }
        "wav" => {
            let mut f = lofty::iff::wav::WavFile::read_from(&mut open()?, opts).map_err(err)?;
            let t = f.remove_id3v2().unwrap_or_default();
            edit(t, fields, cover).save_to_path(path, w).map_err(err)
        }
        "aif" | "aiff" => {
            let mut f = lofty::iff::aiff::AiffFile::read_from(&mut open()?, opts).map_err(err)?;
            let t = f.remove_id3v2().unwrap_or_default();
            edit(t, fields, cover).save_to_path(path, w).map_err(err)
        }
        "wma" => Err("Chinotto can't change WMA files yet".into()),
        _ => Err("Chinotto can't change this kind of file".into()),
    }
}

/// Write `fields` and `cover` into the music file at `path`, safely (see the module notes).
pub fn write(path: &Path, fields: &[Field], cover: Option<&CoverChange>) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp: PathBuf = path.with_file_name(format!(".{name}.chinotto-{}", std::process::id()));
    std::fs::copy(path, &tmp).map_err(|e| format!("{name}: couldn't make a working copy ({e})"))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let result = write_copy(&tmp, &ext, fields, cover);
    match result {
        Ok(()) => std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{name}: couldn't replace the file ({e})")
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("{name}: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On copies of sample files in CHINOTTO_TEST_AUDIO (sine-440-3s.<ext>).
    #[test]
    fn writes_tags_and_covers_into_each_format() {
        let Some(src) = std::env::var_os("CHINOTTO_TEST_AUDIO").map(PathBuf::from) else { return };
        let dir = std::env::temp_dir().join(format!("chinotto-tagwrite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = {
            let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 30, 30]));
            let mut v = Vec::new();
            image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png).unwrap();
            v
        };
        let blue = {
            let img = image::RgbImage::from_pixel(8, 8, image::Rgb([30, 30, 200]));
            let mut v = Vec::new();
            image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png).unwrap();
            v
        };
        for ext in ["mp3", "m4a", "alac.m4a", "flac", "ogg", "wav", "aiff"] {
            let from = src.join(format!("sine-440-3s.{ext}"));
            if !from.is_file() {
                continue;
            }
            let to = dir.join(format!("t.{ext}"));
            std::fs::copy(&from, &to).unwrap();
            let fields = [
                Field::Title(Some("Écrit par Chinotto".into())),
                Field::AlbumArtist(Some("Various".into())),
                Field::Album(Some("Album".into())),
                Field::Year(2001),
                Field::TrackNr(3),
                Field::Compilation(true),
            ];
            write(&to, &fields, Some(&CoverChange::Set(png.clone(), MimeType::Png))).unwrap_or_else(|e| panic!("{ext}: {e}"));
            let kind = crate::tags::library_kind(&to).unwrap();
            let (t, pic) = crate::tags::read_tags(&to, kind).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert_eq!(t.title.as_deref(), Some("Écrit par Chinotto"), "{ext}");
            assert_eq!(t.album_artist.as_deref(), Some("Various"), "{ext}");
            assert_eq!((t.year, t.track_nr, t.compilation), (2001, 3, true), "{ext}");
            assert!(pic, "{ext}: cover");
            // A second cover replaces the first (it must not sit behind it).
            write(&to, &[], Some(&CoverChange::Set(blue.clone(), MimeType::Png))).unwrap_or_else(|e| panic!("{ext}: {e}"));
            let tagged = lofty::read_from_path(&to).unwrap();
            let all: Vec<_> = tagged.tags().iter().flat_map(|t| t.pictures().iter().map(|p| p.data().to_vec())).collect();
            assert_eq!(all, vec![blue.clone()], "{ext}: only the new cover");
            // No working copies left behind.
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "{ext}");
            std::fs::remove_file(&to).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Reading a music file's tags for adding it to an iPod, and the sort fields
//! Chinotto writes alongside (same convention as Swinsian, see HANDOFF.md).

use std::path::Path;

use chinotto_gpod::TrackInfo;
use lofty::file::FileType;
use lofty::prelude::*;
use lofty::tag::ItemKey;
use unicode_normalization::UnicodeNormalization;

/// File types a classic iPod plays. Anything else needs converting first.
pub fn ipod_kind(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "mp3" => Some("MPEG audio file"),
        "m4a" | "aac" => Some("AAC audio file"),
        "wav" => Some("WAV audio file"),
        "aif" | "aiff" => Some("AIFF audio file"),
        _ => None,
    }
}

/// Music files a library can hold (and Chinotto plays, or will: WMA).
pub fn library_kind(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "mp3" => Some("MPEG audio file"),
        "m4a" | "aac" => Some("AAC audio file"),
        "m4b" => Some("Audiobook"),
        "wav" => Some("WAV audio file"),
        "aif" | "aiff" => Some("AIFF audio file"),
        "flac" => Some("FLAC audio file"),
        "ogg" | "oga" => Some("Ogg Vorbis audio file"),
        "opus" => Some("Opus audio file"),
        "wma" => Some("Windows Media audio file"),
        _ => None,
    }
}

/// Tags and audio details of `path`, ready for [`chinotto_gpod::Database::add_track`].
pub fn read(path: &Path) -> Result<TrackInfo, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let kind = ipod_kind(path)
        .ok_or_else(|| format!("{name}: this kind of file doesn't play on an iPod (MP3, AAC, Apple Lossless, WAV and AIFF do)"))?;
    read_tags(path, kind).map(|(t, _)| t)
}

/// Tags and audio details of any music file `lofty` reads, described as
/// `kind`; also whether it has an embedded picture.
pub fn read_tags(path: &Path, mut kind: &'static str) -> Result<(TrackInfo, bool), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tagged = lofty::read_from_path(path).map_err(|e| format!("{name}: couldn't read it as audio ({e})"))?;
    let props = tagged.properties();
    // In an .m4a, AAC has no bit depth; Apple Lossless does.
    if tagged.file_type() == FileType::Mp4 && props.bit_depth().is_some() {
        kind = "Apple Lossless audio file";
    }

    let mut t = TrackInfo {
        filetype: Some(kind.to_string()),
        length_ms: props.duration().as_millis().min(i32::MAX as u128) as i32,
        bitrate_kbps: props.audio_bitrate().or(props.overall_bitrate()).unwrap_or(0) as i32,
        sample_rate: props.sample_rate().unwrap_or(44_100),
        ..Default::default()
    };

    let mut has_picture = false;
    if let Some(tag) = tagged.primary_tag().or(tagged.first_tag()) {
        has_picture = !tag.pictures().is_empty();
        let s = |k: ItemKey| tag.get_string(k).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
        t.title = tag.title().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        t.artist = tag.artist().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        t.album = tag.album().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        t.genre = tag.genre().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        t.comment = tag.comment().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        t.album_artist = s(ItemKey::AlbumArtist);
        t.composer = s(ItemKey::Composer);
        t.grouping = s(ItemKey::ContentGroup);
        t.sort_title = s(ItemKey::TrackTitleSortOrder);
        t.sort_artist = s(ItemKey::TrackArtistSortOrder);
        t.sort_album = s(ItemKey::AlbumTitleSortOrder);
        t.sort_album_artist = s(ItemKey::AlbumArtistSortOrder);
        t.sort_composer = s(ItemKey::ComposerSortOrder);
        t.track_nr = tag.track().unwrap_or(0) as i32;
        t.track_count = tag.track_total().unwrap_or(0) as i32;
        t.disc_nr = tag.disk().unwrap_or(0) as i32;
        t.disc_count = tag.disk_total().unwrap_or(0) as i32;
        t.year = tag
            .date()
            .map(|d| d.year as i32)
            .or_else(|| s(ItemKey::Year).and_then(|y| y.get(..4).and_then(|y| y.parse().ok())))
            .unwrap_or(0);
        t.compilation = s(ItemKey::FlagCompilation).is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        t.bpm = s(ItemKey::Bpm).and_then(|v| v.split('.').next().and_then(|n| n.parse().ok())).unwrap_or(0);
    }
    if t.title.is_none() {
        // Like iTunes: an untagged file is named after itself.
        t.title = path.file_stem().map(|n| n.to_string_lossy().into_owned());
    }
    fill_sort_fields(&mut t);
    Ok((t, has_picture))
}

/// The file's embedded cover (front cover if marked, else the first picture).
pub fn embedded_cover(path: &Path) -> Option<Vec<u8>> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or(tagged.first_tag())?;
    let pics = tag.pictures();
    let p = pics.iter().find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront).or(pics.first())?;
    Some(p.data().to_vec())
}

/// The iPod groups albums by album + album artist; without an album artist,
/// tracks of one album can show up as separate albums. Fill it (falling back
/// to the artist) and the sort fields the file doesn't have.
pub fn fill_sort_fields(t: &mut TrackInfo) {
    if t.album_artist.is_none() && !t.compilation {
        t.album_artist = t.artist.clone();
    }
    if t.sort_artist.is_none() {
        t.sort_artist = t.artist.as_deref().map(|v| sort_key(v, true));
    }
    if t.sort_album_artist.is_none() {
        t.sort_album_artist = t.album_artist.as_deref().map(|v| sort_key(v, true));
    }
    if t.sort_album.is_none() {
        t.sort_album = t.album.as_deref().map(|v| sort_key(v, false));
    }
    // No sort title is written (matches Swinsian).
}

/// Swinsian's sort-field convention: lowercase; accents stripped (NFD, combining
/// marks dropped except the Japanese voicing marks U+3099/U+309A); full-width
/// ASCII folded; Greek final sigma → σ; a leading "the " dropped for artists only.
pub fn sort_key(s: &str, drop_the: bool) -> String {
    let folded: String = s
        .chars()
        .map(|c| match c as u32 {
            0xFF01..=0xFF5E => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            0x3000 => ' ',
            _ => c,
        })
        .collect::<String>()
        .nfd()
        .filter(|c| {
            let u = *c as u32;
            let combining = (0x0300..=0x036F).contains(&u)
                || (0x1AB0..=0x1AFF).contains(&u)
                || (0x1DC0..=0x1DFF).contains(&u)
                || (0x20D0..=0x20FF).contains(&u)
                || (0xFE20..=0xFE2F).contains(&u);
            !combining
        })
        .collect::<String>()
        .nfc()
        .collect::<String>()
        .to_lowercase()
        .replace('ς', "σ");
    if drop_the {
        if let Some(rest) = folded.strip_prefix("the ") {
            return rest.to_string();
        }
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_keys_follow_swinsian() {
        assert_eq!(sort_key("The Beatles", true), "beatles");
        assert_eq!(sort_key("The Wall", false), "the wall");
        assert_eq!(sort_key("Björk", true), "bjork");
        assert_eq!(sort_key("Φοίβος Δεληβοριάς", true), "φοιβοσ δεληβοριασ");
        assert_eq!(sort_key("ＡＢＣ", true), "abc");
        // Japanese voicing marks survive (ガ stays ガ, not カ).
        assert_eq!(sort_key("ガラス", true), "ガラス");
    }

    #[test]
    fn album_artist_falls_back_to_artist() {
        let mut t = TrackInfo { artist: Some("The National".into()), album: Some("Boxer".into()), ..Default::default() };
        fill_sort_fields(&mut t);
        assert_eq!(t.album_artist.as_deref(), Some("The National"));
        assert_eq!(t.sort_artist.as_deref(), Some("national"));
        assert_eq!(t.sort_album_artist.as_deref(), Some("national"));
        assert_eq!(t.sort_album.as_deref(), Some("boxer"));
        assert_eq!(t.sort_title, None);
    }
}

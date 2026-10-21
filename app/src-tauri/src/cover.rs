//! Adding covers to an iPod: the image is shrunk by the 600 px rule, turned
//! into the iPod's thumbnail formats (RGB565), appended to its `.ithmb` files,
//! and listed in a new ArtworkDB. The songs' links to the new entries are then
//! saved into the iTunesDB through libgpod (`Field::Artwork`).
//!
//! New ArtworkDB entries are clones of an existing entry on the same iPod
//! (see chinotto-ipod's `ArtworkDb::append`), so they match whatever wrote the
//! iPod's artwork before. Songs given the same image share one copy of the
//! thumbnails, as Swinsian does for albums.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chinotto_ipod::artworkdb::{ithmb_name, ArtworkDb, NewImage, NewThumb};
use chinotto_ipod::device::Layout;
use chinotto_ipod::rgb565;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbaImage};

/// A new `.ithmb` file is started once the current one reaches this size.
const ITHMB_LIMIT: u64 = 64 * 1024 * 1024;

/// The developer's rule: square images to 600×600; otherwise the shorter side to
/// 600 px and the longer in proportion. Only ever shrinks.
pub fn limit_600(img: DynamicImage) -> DynamicImage {
    let (w, h) = img.dimensions();
    let short = w.min(h);
    if short <= 600 {
        return img;
    }
    let (nw, nh) = if w <= h { (600, (h as u64 * 600 / w as u64) as u32) } else { ((w as u64 * 600 / h as u64) as u32, 600) };
    img.resize_exact(nw, nh, FilterType::Lanczos3)
}

pub fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    image::load_from_memory(bytes).map_err(|e| format!("Not an image Chinotto can read ({e})"))
}

/// Scale to fill `w`×`h` and crop the overflow evenly (covers are square on the iPod).
fn fill(img: &DynamicImage, w: u32, h: u32) -> RgbaImage {
    img.resize_to_fill(w, h, FilterType::Lanczos3).to_rgba8()
}

/// The pixel layout of a thumbnail slot: (row width, rows).
///
/// Format 1061 (iPod classic / nano 3G) holds 6,160 bytes = 56 columns × 55
/// rows, whatever its header says: TunesReloaded-style entries say 56×56,
/// iTunes-style ones 55×55, and both are stored with 56-pixel rows (measured
/// on the developer's nano). So when the header is narrower than the slot and
/// one pixel more divides it evenly, rows are one pixel wider than the header.
pub fn slot_dims(w: usize, h: usize, size: usize) -> (usize, usize) {
    if w == 0 {
        return (0, 0);
    }
    let stride = if w * h * 2 < size && size.is_multiple_of((w + 1) * 2) { w + 1 } else { w };
    (stride, size / (stride * 2))
}

/// One image and the songs (dbids) that get it.
pub struct Assignment {
    pub image: DynamicImage,
    pub dbids: Vec<u64>,
}

/// Write the thumbnails and the new ArtworkDB. Returns, per song, the image id
/// and "source pixels" value to store in its iTunesDB entry.
pub fn add_covers(root: &Path, assignments: &[Assignment]) -> Result<HashMap<u64, (u32, u32)>, String> {
    let layout = Layout::new(root);
    let db_path = layout.artworkdb();
    // The model's formats, for an iPod with no covers yet.
    let model_formats = || {
        let gen = chinotto_ipod::device::SysInfo::read(root).and_then(|s| s.generation());
        gen.as_ref().and_then(chinotto_ipod::models::cover_formats)
    };
    let art = match std::fs::read(&db_path) {
        Ok(bytes) => ArtworkDb::parse(&bytes).map_err(|e| format!("The iPod's artwork database couldn't be read: {e}"))?,
        Err(_) => {
            // No artwork database yet (a freshly set-up iPod): start an empty one.
            let (_, files) =
                model_formats().ok_or("Chinotto doesn't know the cover sizes for this iPod model, so it can't add covers yet")?;
            std::fs::create_dir_all(layout.artwork_dir()).map_err(|e| format!("Couldn't create the Artwork folder: {e}"))?;
            let blank = chinotto_ipod::artworkdb::blank(files);
            std::fs::write(&db_path, &blank).map_err(|e| format!("Couldn't create the artwork database: {e}"))?;
            ArtworkDb::parse(&blank).map_err(|e| e.to_string())?
        }
    };
    let mut formats = art.common_formats();
    let ids: Vec<u32> = formats.iter().map(|f| f.format).collect();
    // A cover already on the iPod is the model for new ones; with none yet,
    // one is made from the model's formats.
    let template = match art.find_template(&ids) {
        Some(t) if !formats.is_empty() => Template::Existing(t.clone()),
        _ => {
            let (thumbs, _) = model_formats().ok_or("Couldn't find an existing cover on this iPod to model new ones on")?;
            formats = thumbs.to_vec();
            Template::New(chinotto_ipod::artworkdb::template_image(thumbs).map_err(|e| e.to_string())?)
        }
    };
    let mut file_index = art.last_file_index();

    backup(root, &db_path)?;

    let mut next_id = art.max_id() + 1;
    let mut entries = Vec::new();
    let mut links = HashMap::new();
    for a in assignments {
        let img = limit_600(a.image.clone());
        let (w, h) = img.dimensions();
        let source_pixels = w * h + 1;
        // Thumbnails once per image, shared by all its songs.
        let mut thumbs = Vec::new();
        for f in &formats {
            let (w, h) = slot_dims(f.width as usize, f.height as usize, f.size as usize);
            let rgba = fill(&img, w as u32, h as u32);
            let data = rgb565::encode(rgba.as_raw(), w, h, f.size as usize);
            let (file, offset) = append_ithmb(&layout, f.format, &mut file_index, &data)?;
            thumbs.push(NewThumb { format: f.format, file, offset, width: f.width, height: f.height });
        }
        for dbid in &a.dbids {
            entries.push(NewImage { id: next_id, song_dbid: *dbid, source_pixels, thumbs: thumbs.clone() });
            links.insert(*dbid, (next_id, source_pixels));
            next_id += 1;
        }
    }
    let out = match &template {
        Template::Existing(t) => art.append(t, &entries),
        Template::New(t) => art.append_new(t, &entries),
    }
    .map_err(|e| format!("Couldn't add the covers: {e}"))?;
    let tmp = db_path.with_extension("chinotto-tmp");
    std::fs::write(&tmp, &out).map_err(|e| format!("Couldn't save the artwork database: {e}"))?;
    std::fs::rename(&tmp, &db_path).map_err(|e| format!("Couldn't save the artwork database: {e}"))?;
    Ok(links)
}

enum Template {
    Existing(chinotto_ipod::artworkdb::Image),
    New((Vec<u8>, chinotto_ipod::artworkdb::Image)),
}

/// Append one thumbnail to the current `F<format>_<n>.ithmb`; returns (file name, offset).
fn append_ithmb(layout: &Layout, format: u32, index: &mut HashMap<u32, u32>, data: &[u8]) -> Result<(String, u32), String> {
    let n = index.entry(format).or_insert(1);
    let mut name = ithmb_name(format, *n);
    let mut path = layout.artwork_dir().join(&name);
    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut len = len;
    if len >= ITHMB_LIMIT {
        *n += 1;
        name = ithmb_name(format, *n);
        path = layout.artwork_dir().join(&name);
        len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    }
    // The iPod finds a thumbnail by slot number (offset ÷ slot size), so a new
    // one must start on a slot boundary: pad the file up to the next one.
    let slot = (data.len() as u64).max(1);
    let offset = len.div_ceil(slot) * slot;
    let mut f = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("Couldn't write {name}: {e}"))?;
    f.write_all(&vec![0u8; (offset - len) as usize]).map_err(|e| format!("Couldn't write {name}: {e}"))?;
    f.write_all(data).map_err(|e| format!("Couldn't write {name}: {e}"))?;
    let offset = u32::try_from(offset).map_err(|_| format!("{name} is too large"))?;
    Ok((name, offset))
}

fn backup(root: &Path, db_path: &Path) -> Result<(), String> {
    let dir = root.join(chinotto_gpod::BACKUP_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    std::fs::copy(db_path, dir.join(format!("ArtworkDB {stamp}")))
        .map_err(|e| format!("Couldn't back up the artwork database before changing it: {e}"))?;
    // Keep the newest 5 artwork backups (they're large).
    let mut ours: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("ArtworkDB ")))
        .collect();
    ours.sort();
    while ours.len() > 5 {
        let _ = std::fs::remove_file(ours.remove(0));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_thumbnail_is_55_wide() {
        assert_eq!(slot_dims(56, 56, 6160), (56, 55));
        assert_eq!(slot_dims(55, 55, 6160), (56, 55));
        assert_eq!(slot_dims(128, 128, 32768), (128, 128));
        assert_eq!(slot_dims(128, 120, 32768), (128, 128));
        assert_eq!(slot_dims(320, 320, 204800), (320, 320));
        assert_eq!(slot_dims(128, 128, 32768), (128, 128));
        assert_eq!(slot_dims(320, 320, 204800), (320, 320));
    }

    #[test]
    fn six_hundred_rule() {
        let sq = DynamicImage::new_rgb8(1200, 1200);
        assert_eq!(limit_600(sq).dimensions(), (600, 600));
        let tall = DynamicImage::new_rgb8(1000, 1500);
        assert_eq!(limit_600(tall).dimensions(), (600, 900));
        let wide = DynamicImage::new_rgb8(1800, 1200);
        assert_eq!(limit_600(wide).dimensions(), (900, 600));
        let small = DynamicImage::new_rgb8(500, 300);
        assert_eq!(limit_600(small).dimensions(), (500, 300), "never enlarged");
    }

    /// On a temporary copy of a real iPod's databases (CHINOTTO_TEST_IPOD).
    #[test]
    fn covers_round_trip_through_libgpod() {
        use chinotto_gpod::{Database, Field};
        use chinotto_ipod::itunesdb::ITunesDb;
        let Some(src) = std::env::var_os("CHINOTTO_TEST_IPOD").map(std::path::PathBuf::from) else { return };
        if !src.join("iPod_Control/Artwork/ArtworkDB").is_file() {
            return;
        }
        let root = std::env::temp_dir().join(format!("chinotto-cover-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["iTunes", "Artwork", "Device"] {
            std::fs::create_dir_all(root.join("iPod_Control").join(d)).unwrap();
        }
        std::fs::copy(src.join("iPod_Control/iTunes/iTunesDB"), root.join("iPod_Control/iTunes/iTunesDB")).unwrap();
        std::fs::copy(src.join("iPod_Control/Artwork/ArtworkDB"), root.join("iPod_Control/Artwork/ArtworkDB")).unwrap();
        std::fs::write(root.join("iPod_Control/Device/SysInfo"), "ModelNumStr: xA980\n").unwrap();
        std::fs::write(
            root.join("iPod_Control/Device/SysInfoExtended"),
            "<plist><dict><key>FireWireGUID</key><string>000A27001234ABCD</string><key>DBVersion</key><integer>3</integer></dict></plist>",
        )
        .unwrap();

        let before = ITunesDb::parse(&std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap()).unwrap();
        let (a, b) = (before.tracks[0].dbid, before.tracks[1].dbid);
        // 900x700 solid orange: shrinks to 771x600, square thumbnails crop the sides.
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(900, 700, image::Rgb([240, 120, 16])));
        let mut db = Database::open(&root).unwrap();
        let links = add_covers(&root, &[Assignment { image: img, dbids: vec![a, b] }]).unwrap();
        for (dbid, link) in &links {
            db.edit(&[*dbid], &[Field::Artwork(Some(*link))]).unwrap();
        }
        db.save().unwrap();
        drop(db);

        let after = ITunesDb::parse(&std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap()).unwrap();
        let art = ArtworkDb::parse(&std::fs::read(root.join("iPod_Control/Artwork/ArtworkDB")).unwrap()).unwrap();
        for dbid in [a, b] {
            let t = after.tracks.iter().find(|t| t.dbid == dbid).unwrap();
            assert_eq!((t.has_artwork, t.artwork_count), (1, 1));
            assert_eq!(t.artwork_size, 771 * 600 + 1);
            let im = art.images.iter().find(|i| i.id == t.mhii_link).expect("linked entry exists");
            assert_eq!(im.song_dbid, dbid);
            for th in &im.thumbs {
                assert_eq!(th.offset % th.size, 0, "{} starts on a slot boundary", th.file);
            }
            for th in &im.thumbs {
                let data = std::fs::read(root.join("iPod_Control/Artwork").join(&th.file)).unwrap();
                let px = &data[th.offset as usize..th.offset as usize + 2];
                let rgba = rgb565::decode(px, 1, 1);
                assert!(rgba[0] > 220 && (100..140).contains(&rgba[1]) && rgba[2] < 40, "{:?} in {}", &rgba[..3], th.file);
            }
        }
        // Both songs share one set of thumbnails.
        let ia = art.images.iter().find(|i| i.song_dbid == a).unwrap();
        let ib = art.images.iter().find(|i| i.song_dbid == b).unwrap();
        assert_eq!(ia.thumbs.iter().map(|t| t.offset).collect::<Vec<_>>(), ib.thumbs.iter().map(|t| t.offset).collect::<Vec<_>>());
        // Removing a cover.
        let mut db = Database::open(&root).unwrap();
        db.edit(&[a], &[Field::Artwork(None)]).unwrap();
        db.save().unwrap();
        drop(db);
        let after = ITunesDb::parse(&std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap()).unwrap();
        let t = after.tracks.iter().find(|t| t.dbid == a).unwrap();
        assert_eq!((t.has_artwork, t.artwork_count, t.mhii_link), (2, 0, 0));
        // Other songs' covers untouched.
        let c = &before.tracks[2];
        let c2 = after.tracks.iter().find(|t| t.dbid == c.dbid).unwrap();
        assert_eq!((c2.has_artwork, c2.mhii_link), (c.has_artwork, c.mhii_link));
        std::fs::remove_dir_all(&root).unwrap();
    }
}

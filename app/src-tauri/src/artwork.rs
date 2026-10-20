//! `art:` URL scheme: serves iPod cover thumbnails as PNG.
//!
//! URL path: `/<generation>-<image id>-<wanted width px>`. The smallest stored
//! thumbnail at least that wide is used (or the largest there is).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use chinotto_ipod::device::Layout;
use chinotto_ipod::rgb565;
use tauri::http::{header, Request, Response, StatusCode};

use crate::state::Session;

pub fn handle(session: Option<&Session>, req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    match render(session, req.uri().path()) {
        Some(png) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, "max-age=86400, immutable")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(png)
            .unwrap(),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap(),
    }
}

fn render(session: Option<&Session>, path: &str) -> Option<Vec<u8>> {
    let s = session?;
    let mut parts = path.trim_start_matches('/').split('-');
    let generation: u32 = parts.next()?.parse().ok()?;
    let image_id: u32 = parts.next()?.parse().ok()?;
    let want: u16 = parts.next()?.parse().ok()?;
    if generation != s.generation {
        return None;
    }
    let art = s.art.as_ref()?;
    let image = &art.images[*s.art_index.get(&image_id)?];

    let mut thumbs: Vec<_> = image.thumbs.iter().filter(|t| t.width > 0 && t.size > 0 && !t.file.is_empty()).collect();
    thumbs.sort_by_key(|t| t.width);
    let t = thumbs.iter().find(|t| t.width >= want).or(thumbs.last())?;

    let file = Layout::new(&s.root).artwork_dir().join(&t.file);
    let mut f = File::open(file).ok()?;
    f.seek(SeekFrom::Start(t.offset as u64)).ok()?;
    let mut raw = vec![0u8; t.size as usize];
    f.read_exact(&mut raw).ok()?;

    let w = t.width as usize;
    // Some slots hold fewer rows than the nominal height (56×56 stored as 56×55).
    let h = (t.height as usize).min(raw.len() / (w * 2));
    if h == 0 {
        return None;
    }
    let rgba = rgb565::decode(&raw, w, h);
    encode_png(&rgba, w as u32, h as u32)
}

fn encode_png(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

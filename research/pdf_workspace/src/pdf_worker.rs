//! Native code runs only in a disposable child. Stdout is a framed protocol.
use crate::model::*;
use crate::text_selection::{Glyph, TextPage, MAX_TEXT_BYTES, MAX_TEXT_CHARS};
use pdfium_render::prelude::{PdfBitmap, PdfBitmapFormat, PdfRenderConfig, Pdfium};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, BufRead, Read, Write},
    time::Instant,
};

pub const MAX_HEADER: u64 = 16384;
const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;

pub fn read_json<T: DeserializeOwned>(reader: &mut impl BufRead) -> Result<T, String> {
    let mut bytes = Vec::new();
    let n = reader
        .take(MAX_HEADER + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Worker pipe closed".into());
    }
    if n > MAX_HEADER as usize || bytes.last() != Some(&b'\n') {
        return Err("Invalid or oversized protocol header".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
pub fn write_json(value: &impl Serialize, writer: &mut impl Write) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|e| e.to_string())?;
    writer.write_all(b"\n").map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

pub fn run(path: &str) -> Result<(), String> {
    let library = std::env::var("IQS_PDFIUM_LIBRARY")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            std::env::current_exe()
                .map(|p| p.parent().unwrap().join("../Frameworks/libpdfium.dylib"))
        })
        .map_err(|e| e.to_string())?;
    let pdfium = Pdfium::new(Pdfium::bind_to_library(library).map_err(|e| e.to_string())?);
    // Hash exactly the bytes supplied to PDFium, not a separately re-opened path.
    let mut data = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_PDF_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX_PDF_BYTES {
        return Err("Research input limit is 512 MiB".into());
    }
    let fingerprint = format!("{:x}", Sha256::digest(&data));
    let document = pdfium
        .load_pdf_from_byte_vec(data, None)
        .map_err(|e| e.to_string())?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    loop {
        let req: Request = match read_json(&mut input) {
            Ok(r) => r,
            Err(e) if e == "Worker pipe closed" => return Ok(()),
            Err(e) => return Err(e),
        };
        let started = Instant::now();
        let mut header = Header {
            seq: req.seq,
            key: req.key.clone(),
            info: None,
            full: [0, 0],
            origin: [0, 0],
            size: [0, 0],
            bytes: 0,
            render_ms: 0.0,
            error: None,
        };
        let result = (|| -> Result<Vec<u8>, String> {
            if req.key.level < -3 || req.key.level > MAX_LEVEL {
                return Err("Invalid resolution level".into());
            }
            let page = document
                .pages()
                .get(req.key.page)
                .map_err(|e| e.to_string())?;
            let info = PageInfo {
                doc: fingerprint.clone(),
                page: req.key.page,
                pages: document.pages().len(),
                width: page.width().value as f64,
                height: page.height().value as f64,
                rotation: page.rotation().map_err(|e| e.to_string())?.as_degrees() as i32,
                crop: page.boundaries().crop().ok().map(|b| {
                    [
                        b.bounds.left().value,
                        b.bounds.bottom().value,
                        b.bounds.right().value,
                        b.bounds.top().value,
                    ]
                }),
            };
            if !info.width.is_finite()
                || !info.height.is_finite()
                || info.width <= 0.0
                || info.height <= 0.0
            {
                return Err("Invalid page dimensions".into());
            }
            header.info = Some(info.clone());
            if req.key.level == -2 {
                return Ok(Vec::new());
            }
            if req.key.doc != fingerprint {
                return Err("Source checksum mismatch".into());
            }
            if req.key.level == -3 {
                let result = extract_text(&page, &info);
                let bytes = serde_json::to_vec(&result).map_err(|e| e.to_string())?;
                if bytes.len() > MAX_TEXT_BYTES {
                    return Err("Text layer exceeds bounded payload limit".into());
                }
                return Ok(bytes);
            }
            let (w, h) = info.raster_size(req.key.edge());
            let x = req.key.col.checked_mul(TILE).ok_or("Invalid tile column")?;
            let y = req.key.row.checked_mul(TILE).ok_or("Invalid tile row")?;
            if x >= w || y >= h {
                return Err("Tile outside page".into());
            }
            let tw = (w - x).min(TILE);
            let th = (h - y).min(TILE);
            let mut bitmap = PdfBitmap::empty(tw as i32, th as i32, PdfBitmapFormat::BGRA)
                .map_err(|e| e.to_string())?;
            page.render_into_bitmap_with_config(
                &mut bitmap,
                &PdfRenderConfig::new()
                    .set_fixed_size(w as i32, h as i32)
                    .set_origin(-(x as i32), -(y as i32)),
            )
            .map_err(|e| e.to_string())?;
            header.full = [w, h];
            header.origin = [x, y];
            header.size = [tw, th];
            Ok(bitmap.as_rgba_bytes())
        })();
        let pixels = match result {
            Ok(p) => p,
            Err(e) => {
                header.error = Some(e);
                Vec::new()
            }
        };
        header.bytes = pixels.len();
        header.render_ms = started.elapsed().as_secs_f64() * 1000.0;
        write_json(&header, &mut output)?;
        output.write_all(&pixels).map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
}

pub fn read_pixels(reader: &mut impl BufRead, request: &Request) -> Result<Pixels, String> {
    let header: Header = read_json(reader)?;
    if header.seq != request.seq || header.key != request.key {
        return Err("Worker reply identity mismatch".into());
    }
    let is_text = request.key.level == -3;
    let max_bytes = if is_text { MAX_TEXT_BYTES } else { MAX_REPLY };
    if header.bytes > max_bytes || header.size.iter().any(|n| *n > TILE) {
        return Err("Oversized worker image".into());
    }
    let expected = header.size[0] as usize * header.size[1] as usize * 4;
    if !is_text && expected != header.bytes {
        return Err("Worker pixel length mismatch".into());
    }
    if request.key.level >= -1 && header.error.is_none() {
        if header.full.iter().any(|n| *n == 0 || *n > MAX_EDGE) || expected == 0 {
            return Err("Invalid worker image extent".into());
        }
        for i in 0..2 {
            if header.origin[i]
                .checked_add(header.size[i])
                .is_none_or(|n| n > header.full[i])
            {
                return Err("Invalid tile extent".into());
            }
        }
    }
    let mut rgba = vec![0; header.bytes];
    reader.read_exact(&mut rgba).map_err(|e| e.to_string())?;
    Ok(Pixels { header, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_headers_and_pixel_payloads_before_allocation() {
        assert!(
            read_json::<Request>(&mut io::Cursor::new(vec![b'x'; MAX_HEADER as usize + 1]))
                .is_err()
        );
        let req = Request {
            seq: 1,
            key: Key::info(0),
        };
        let h = Header {
            seq: 1,
            key: req.key.clone(),
            info: None,
            full: [1, 1],
            origin: [0, 0],
            size: [u32::MAX, 1],
            bytes: usize::MAX,
            render_ms: 0.0,
            error: None,
        };
        let mut buffer = Vec::new();
        write_json(&h, &mut buffer).unwrap();
        assert!(read_pixels(&mut io::Cursor::new(buffer), &req).is_err());
        let mut text_req = req;
        text_req.key.level = -3;
        let text_header = Header {
            key: text_req.key.clone(),
            size: [0, 0],
            bytes: MAX_TEXT_BYTES + 1,
            ..h
        };
        let mut oversized = Vec::new();
        write_json(&text_header, &mut oversized).unwrap();
        assert!(read_pixels(&mut io::Cursor::new(oversized), &text_req).is_err());
    }
}

// Shared by the viewer and the independent capture worker.
pub fn extract_text(page: &pdfium_render::prelude::PdfPage, info: &PageInfo) -> TextPage {
    let mut result = TextPage::default();
    match page.text() {
        Err(error) => result.error = Some(error.to_string()),
        Ok(text) => {
            result.truncated = text.len() as usize > MAX_TEXT_CHARS;
            let (w, h) = info.raster_size(1_000_000);
            let config = PdfRenderConfig::new().set_fixed_size(w as i32, h as i32);
            for character in text.chars().iter().take(MAX_TEXT_CHARS) {
                let Some(c) = character.unicode_char().filter(|c| *c != '\0') else {
                    continue;
                };
                let bounds = character.tight_bounds().ok().and_then(|bounds| {
                    let points = [
                        (bounds.left(), bounds.top()),
                        (bounds.right(), bounds.top()),
                        (bounds.right(), bounds.bottom()),
                        (bounds.left(), bounds.bottom()),
                    ];
                    let mut r = [
                        f32::INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    ];
                    for (x, y) in points {
                        let (x, y) = page.points_to_pixels(x, y, &config).ok()?;
                        let x = x as f32 / w as f32;
                        let y = y as f32 / h as f32;
                        r[0] = r[0].min(x);
                        r[1] = r[1].min(y);
                        r[2] = r[2].max(x);
                        r[3] = r[3].max(y);
                    }
                    (r.iter().all(|v| v.is_finite())
                        && r[2] > r[0]
                        && r[3] > r[1]
                        && !c.is_whitespace())
                    .then_some(r)
                });
                result.glyphs.push(Glyph(c, bounds));
            }
        }
    }

    result
}

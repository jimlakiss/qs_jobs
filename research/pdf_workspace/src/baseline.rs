//! Disposable Sprint 0 experiment. Full-page preview; this is not the M1 tiled renderer.
use pdfium_render::prelude::*;
use serde_json::json;
use std::{env, fs, time::Instant};

const PREVIEW_WIDTH: i32 = 2048;
const MAX_HEIGHT: i32 = 2048;

struct Request {
    path: String,
    page: i32,
}
struct Frame {
    page_count: i32,
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    open_ms: f64,
    render_ms: f64,
}
// Every PDFium handle is created, used and dropped on this single worker.
// No unsafe Send/Sync implementations, no PDFium calls on the UI thread.
fn render(pdfium: &Pdfium, req: &Request) -> Result<Frame, String> {
    let started = Instant::now();
    let document = pdfium
        .load_pdf_from_file(&req.path, None)
        .map_err(|e| e.to_string())?;
    let open_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let page = document.pages().get(req.page).map_err(|e| e.to_string())?;
    let bitmap = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(PREVIEW_WIDTH)
                .set_maximum_height(MAX_HEIGHT),
        )
        .map_err(|e| e.to_string())?;
    let rgba = bitmap.as_rgba_bytes();
    Ok(Frame {
        page_count: document.pages().len(),
        width: bitmap.width() as usize,
        height: bitmap.height() as usize,
        rgba,
        open_ms,
        render_ms: started.elapsed().as_secs_f64() * 1000.0,
    })
}

pub fn bind_pdfium() -> Result<Pdfium, String> {
    let path = env::var("IQS_PDFIUM_LIBRARY")
        .map_err(|_| "Set IQS_PDFIUM_LIBRARY to the pinned PDFium library file".to_string())?;
    Pdfium::bind_to_library(path)
        .map(Pdfium::new)
        .map_err(|e| e.to_string())
}

fn percentile(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() as f64 * q).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1)]
}

fn write_ppm(path: &str, width: usize, height: usize, rgba: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = std::io::BufWriter::new(fs::File::create(path)?);
    write!(out, "P6\n{width} {height}\n255\n")?;
    for p in rgba.chunks_exact(4) {
        out.write_all(&p[..3])?;
    }
    out.flush()
}

pub fn benchmark(path: String) -> Result<(), String> {
    let p = bind_pdfium()?;
    let mut samples = Vec::new();
    let mut opens = Vec::new();
    let mut last = None;
    for _ in 0..12 {
        let f = render(
            &p,
            &Request {
                path: path.clone(),
                page: 0,
            },
        )?;
        samples.push(f.render_ms);
        opens.push(f.open_ms);
        last = Some(f);
    }
    let f = last.unwrap();
    if let Ok(path) = env::var("IQS_RENDER_PPM") {
        write_ppm(&path, f.width, f.height, &f.rgba).map_err(|e| e.to_string())?;
    }
    println!(
        "{}",
        json!({"kind":"pdfium_preview", "path":path,
        "pdfium_binding":"0.9.4/pdfium_7881", "os":env::consts::OS, "arch":env::consts::ARCH,
        "page":1, "pages":f.page_count, "width":f.width, "height":f.height,
        "rgba_bytes":f.rgba.len(), "samples":samples.len(),
        "first_open_ms":opens[0], "first_render_ms":samples[0],
        "repeat_render_p50_ms":percentile(&samples[1..],0.5),
        "repeat_render_p95_ms":percentile(&samples[1..],0.95),
        "open_samples_ms":opens, "render_samples_ms":samples,
        "method":"reopen document per sample; OS caches uncontrolled; page 1 only; CPU raster+RGBA copy"})
    );
    Ok(())
}

pub fn reference(pdfium: &Pdfium, path: &str) -> Result<(usize, usize, Vec<u8>), String> {
    let frame = render(
        pdfium,
        &Request {
            path: path.into(),
            page: 0,
        },
    )?;
    Ok((frame.width, frame.height, frame.rgba))
}

/// Matched full-page comparison. No tiles, previews, UI or engine startup.
pub fn matched(path: &str) -> Result<(), String> {
    let pdfium = bind_pdfium()?;
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mut pages = Vec::new();
    let count = {
        let d = pdfium
            .load_pdf_from_byte_vec(bytes.clone(), None)
            .map_err(|e| e.to_string())?;
        d.pages().len()
    };
    for page_index in 0..count {
        let mut trials = Vec::new();
        let mut size = [0, 0];
        for _ in 0..5 {
            let input = bytes.clone();
            let opened = Instant::now();
            let document = pdfium
                .load_pdf_from_byte_vec(input, None)
                .map_err(|e| e.to_string())?;
            let open_ms = opened.elapsed().as_secs_f64() * 1000.0;
            let first = Instant::now();
            let page = document
                .pages()
                .get(page_index)
                .map_err(|e| e.to_string())?;
            let w = (page.width().value as f64 * 136.0 / 72.0).floor() as i32;
            let h = (page.height().value as f64 * 136.0 / 72.0).floor() as i32;
            if w <= 0 || h <= 0 || w as i64 * h as i64 > 100_000_000 {
                return Err("Invalid or excessive matched raster size".into());
            }
            size = [w, h];
            let config = PdfRenderConfig::new().set_fixed_size(w, h);
            let bitmap = page
                .render_with_config(&config)
                .map_err(|e| e.to_string())?;
            let first_render_ms = first.elapsed().as_secs_f64() * 1000.0;
            let copied = Instant::now();
            let pixels = bitmap.as_rgba_bytes();
            let first_readback_ms = copied.elapsed().as_secs_f64() * 1000.0;
            if pixels.len() != w as usize * h as usize * 4 {
                return Err("Unexpected raster byte count".into());
            }
            drop(pixels);
            drop(bitmap);
            let mut warm = Vec::new();
            for _ in 0..3 {
                let start = Instant::now();
                let bitmap = page
                    .render_with_config(&config)
                    .map_err(|e| e.to_string())?;
                let render_ms = start.elapsed().as_secs_f64() * 1000.0;
                let copied = Instant::now();
                let pixels = bitmap.as_rgba_bytes();
                let readback_ms = copied.elapsed().as_secs_f64() * 1000.0;
                std::hint::black_box(&pixels);
                warm.push(json!({"render_ms":render_ms,"readback_ms":readback_ms}));
            }
            trials.push(json!({"open_ms":open_ms,"first_render_ms":first_render_ms,
                "first_total_ms":open_ms+first_render_ms,"first_readback_ms":first_readback_ms,"warm":warm}));
        }
        pages.push(json!({"page":page_index+1,"width":size[0],"height":size[1],"trials":trials}));
    }
    println!(
        "{}",
        json!({"kind":"matched_full_page_raster","engine":"PDFium 7881 / Rust release",
        "dpi":136,"trials_per_page":5,"warm_renders_per_trial":3,"pages":pages,
        "method":"source bytes in memory before timing; fresh document per trial; engine startup and source disk IO excluded; full-page render without readback, with readback reported separately"})
    );
    Ok(())
}

//! End-to-end validation of the real subprocess/scheduler, not mock PDF renders.
use crate::{baseline, model::*, pipeline::*};
use serde_json::json;
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

fn next(controller: &Controller, session: u64) -> Result<std::sync::Arc<Pixels>, String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match controller.rx.try_recv() {
            Ok(Event::Ready(reply)) if reply.session == session => return Ok(reply.pixels),
            Ok(Event::Error {
                session: s,
                message,
            }) if s == session => return Err(message),
            _ => {}
        }
        if Instant::now() > deadline {
            return Err("Pipeline test timed out".into());
        }
        thread::sleep(Duration::from_millis(2));
    }
}
fn plan(controller: &Controller, session: u64, revision: u64, path: &Path, jobs: Vec<Key>) {
    controller.submit(Plan {
        session,
        revision,
        path: path.to_string_lossy().into_owned(),
        jobs,
    });
}
pub fn run(directory: &str) -> Result<(), String> {
    let pdfium = baseline::bind_pdfium()?;
    let root = Path::new(directory);
    let controller = Controller::new(|| {});
    let mut comparisons = Vec::new();
    for (index, name) in ["vector.pdf", "rotated_crop.pdf", "raster.pdf"]
        .iter()
        .enumerate()
    {
        let path = root.join(name);
        let session = index as u64 + 1;
        plan(&controller, session, 1, &path, vec![Key::info(0)]);
        let info = next(&controller, session)?
            .header
            .info
            .clone()
            .ok_or("Missing page metadata")?;
        let (w, h, reference) = baseline::reference(&pdfium, &path.to_string_lossy())?;
        let (tw, th) = info.raster_size(2048);
        if tw as usize != w || th as usize != h {
            return Err("Reference dimensions differ".into());
        }
        let mut keys = Vec::new();
        for row in 0..th.div_ceil(TILE) {
            for col in 0..tw.div_ceil(TILE) {
                keys.push(Key {
                    doc: info.doc.clone(),
                    page: 0,
                    level: 1,
                    col,
                    row,
                });
            }
        }
        plan(&controller, session, 2, &path, keys.clone());
        let mut stitched = vec![0; w * h * 4];
        let mut seen = std::collections::HashSet::new();
        while seen.len() < keys.len() {
            let pixels = next(&controller, session)?;
            let header = &pixels.header;
            if !seen.insert(header.key.clone()) {
                return Err("Duplicate pipeline tile".into());
            }
            let [x, y] = header.origin;
            let [width, height] = header.size;
            for row in 0..height as usize {
                let from = row * width as usize * 4;
                let to = ((y as usize + row) * w + x as usize) * 4;
                stitched[to..to + width as usize * 4]
                    .copy_from_slice(&pixels.rgba[from..from + width as usize * 4]);
            }
        }
        let difference: u64 = reference
            .iter()
            .zip(&stitched)
            .map(|(a, b)| a.abs_diff(*b) as u64)
            .sum();
        let mean = difference as f64 / reference.len() as f64;
        // Allows edge antialiasing differences; large origin/crop/rotation errors fail.
        if mean > 0.1 {
            return Err(format!(
                "{name}: stitched raster differs, mean channel delta {mean}"
            ));
        }
        comparisons.push(json!({"file":name,"tiles":keys.len(),"mean_channel_delta":mean,"width":w,"height":h,"rotation":info.rotation,"crop":info.crop}));
        plan(&controller, session, 3, &path, vec![info.preview()]);
        next(&controller, session)?;
        let hits = controller.metrics().cache_hits;
        plan(
            &controller,
            session,
            4,
            &path,
            vec![keys.last().unwrap().clone()],
        );
        next(&controller, session)?;
        if controller.metrics().cache_hits <= hits {
            return Err("Expected CPU cache reuse".into());
        }
    }
    let dense = root.join("dense.pdf");
    let vector = root.join("vector.pdf");
    plan(&controller, 10, 1, &dense, vec![Key::info(0)]);
    let info = next(&controller, 10)?
        .header
        .info
        .clone()
        .ok_or("Missing dense metadata")?;
    let key = Key {
        doc: info.doc,
        page: 0,
        level: 1,
        col: 0,
        row: 0,
    };
    plan(&controller, 10, 2, &dense, vec![key]);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !controller.is_busy() {
        if Instant::now() > deadline {
            return Err("Dense worker did not start".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
    thread::sleep(Duration::from_millis(2));
    let changed = Instant::now();
    plan(&controller, 11, 1, &vector, vec![Key::info(0)]);
    next(&controller, 11)?;
    let cancel_to_ready_ms = changed.elapsed().as_secs_f64() * 1000.0;
    if controller.metrics().cancellations == 0 || cancel_to_ready_ms > 2000.0 {
        return Err("Hard cancellation did not complete promptly".into());
    }
    plan(
        &controller,
        12,
        1,
        &root.join("malformed.pdf"),
        vec![Key::info(0)],
    );
    if next(&controller, 12).is_ok() {
        return Err("Malformed PDF should fail".into());
    }
    plan(&controller, 13, 1, &vector, vec![Key::info(0)]);
    next(&controller, 13)?;
    let metrics = controller.metrics();
    if metrics.cpu_cache_peak > CPU_BUDGET {
        return Err("CPU cache exceeded budget".into());
    }
    println!(
        "{}",
        json!({"kind":"progressive_pipeline_validation","success":true,
        "tile_comparisons":comparisons,"cancel_to_new_document_ready_ms":cancel_to_ready_ms,
        "malformed_recovery":true,"cpu_budget_bytes":CPU_BUDGET,"pipeline":metrics})
    );
    Ok(())
}

pub fn document(path: &str) -> Result<(), String> {
    let controller = Controller::new(|| {});
    let path = Path::new(path);
    let started = Instant::now();
    plan(&controller, 1, 1, path, vec![Key::info(0)]);
    let first = next(&controller, 1)?
        .header
        .info
        .clone()
        .ok_or("Missing metadata")?;
    let open_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut pages = vec![
        0,
        first.pages / 4,
        first.pages / 2,
        first.pages * 3 / 4,
        first.pages - 1,
    ];
    pages.sort();
    pages.dedup();
    let mut samples = Vec::new();
    let mut revision = 1;
    for page in pages {
        let info = if page == 0 {
            first.clone()
        } else {
            revision += 1;
            plan(&controller, 1, revision, path, vec![Key::info(page)]);
            next(&controller, 1)?
                .header
                .info
                .clone()
                .ok_or("Missing metadata")?
        };
        revision += 1;
        let t = Instant::now();
        plan(&controller, 1, revision, path, vec![info.preview()]);
        let preview = next(&controller, 1)?;
        let preview_roundtrip = t.elapsed().as_secs_f64() * 1000.0;
        revision += 1;
        let (w, h) = info.raster_size(4096);
        let key = Key {
            doc: info.doc.clone(),
            page,
            level: 2,
            col: (w / 2) / TILE,
            row: (h / 2) / TILE,
        };
        let t = Instant::now();
        plan(&controller, 1, revision, path, vec![key]);
        let tile = next(&controller, 1)?;
        samples.push(json!({"page":page+1,"width":info.width,"height":info.height,"rotation":info.rotation,
            "preview_render_ms":preview.header.render_ms,"preview_roundtrip_ms":preview_roundtrip,
            "centre_tile_render_ms":tile.header.render_ms,"centre_tile_roundtrip_ms":t.elapsed().as_secs_f64()*1000.0}));
    }
    println!(
        "{}",
        json!({"kind":"project_pdf_sample","success":true,"source_sha256":first.doc,
        "pages":first.pages,"bytes":std::fs::metadata(path).map_err(|e|e.to_string())?.len(),
        "initial_open_and_metadata_ms":open_ms,"samples":samples,"pipeline":controller.metrics(),
        "method":"read-only source; persistent child; five sampled pages; 512px preview and one 512px centre tile at 4096px long-edge; uncontrolled caches"})
    );
    Ok(())
}

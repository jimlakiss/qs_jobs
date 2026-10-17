//! M1a experiment: UI/GPU composition and killable PDFium subprocess.
mod baseline;
mod capture;
mod capture_text;
mod file_tree;
mod memory_workflow;
mod model;
mod pdf_worker;
mod pipeline;
mod staging;
mod text_selection;
mod validation;
mod viewer;
mod workspace;

use eframe::egui;
use model::*;
use pipeline::{Controller, Event, Plan};
use serde_json::json;
use std::{
    collections::HashMap,
    env, fs,
    io::Write,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_ZOOM: f64 = 50.0; // 5,000% of the selected fit mode.

fn percentile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v[((v.len() as f64 * q).ceil() as usize)
        .saturating_sub(1)
        .min(v.len() - 1)]
}
fn write_ppm(path: &str, image: &egui::ColorImage) -> std::io::Result<()> {
    let mut out = std::io::BufWriter::new(fs::File::create(path)?);
    write!(out, "P6\n{} {}\n255\n", image.size[0], image.size[1])?;
    for pixel in &image.pixels {
        out.write_all(&pixel.to_array()[..3])?;
    }
    out.flush()
}
struct Texture {
    handle: egui::TextureHandle,
    header: Header,
}
struct App {
    capture: capture::Capture,
    review_qa: bool,
    review_qa_ready: bool,
    text_mode: bool,
    text_page: Option<text_selection::TextPage>,
    text_selection: text_selection::Selection,
    text_notice: String,
    text_ocr: Option<capture::TextRecognition>,
    text_result: Option<String>,
    file_tree: Option<file_tree::FileTree>,
    memory_workflow: Option<memory_workflow::Workflow>,
    documents: Vec<String>,
    picker: Option<std::sync::mpsc::Receiver<Vec<String>>>,
    page_number: i32,
    fit_width: bool,
    rotations: HashMap<(String, i32), i32>,
    path: String,
    opened_path: String,
    page: i32,
    info: Option<PageInfo>,
    session: u64,
    revision: u64,
    controller: Controller,
    last_plan: Option<Plan>,
    textures: Cache<Key, Texture>,
    zoom: f64,
    pan: [f64; 2],
    drawing_rect: Option<egui::Rect>,
    drawing_id: Option<egui::Id>,
    wheel_pan: [f64; 2],
    status: String,
    overlays: HashMap<(String, i32), [f64; 4]>,
    started: Instant,
    previous_frame: Instant,
    intervals: Vec<f64>,
    busy_intervals: Vec<f64>,
    first_preview_ms: Option<f64>,
    tiles_received: u64,
    tile_ms: Vec<f64>,
    smoke: bool,
    restarted: bool,
    overlay_edits: u64,
    screenshot_requested: bool,
    outcome: Arc<Mutex<Option<bool>>>,
    fatal: bool,
}
impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        path: String,
        smoke: bool,
        page: i32,
        outcome: Arc<Mutex<Option<bool>>>,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        let mut app = Self {
            capture: Default::default(),
            review_qa: env::var_os("IQS_REVIEW_QA").is_some(),
            review_qa_ready: false,
            text_mode: false,
            text_page: None,
            text_selection: Default::default(),
            text_notice: String::new(),
            text_ocr: None,
            text_result: None,
            file_tree: None,
            memory_workflow: memory_workflow::Workflow::from_env(),
            documents: viewer::collect_paths(std::slice::from_ref(&path)),
            picker: None,
            page_number: page + 1,
            fit_width: false,
            rotations: HashMap::new(),
            opened_path: String::new(),
            path,
            page,
            info: None,
            session: 0,
            revision: 0,
            controller: Controller::new(move || ctx.request_repaint()),
            last_plan: None,
            textures: Cache::new(GPU_BUDGET),
            zoom: 1.0,
            pan: [0.0, 0.0],
            drawing_rect: None,
            drawing_id: None,
            wheel_pan: [0.0, 0.0],
            status: "Enter a PDF path to begin".into(),
            overlays: HashMap::new(),
            started: Instant::now(),
            previous_frame: Instant::now(),
            intervals: Vec::new(),
            busy_intervals: Vec::new(),
            first_preview_ms: None,
            tiles_received: 0,
            tile_ms: Vec::new(),
            smoke,
            restarted: false,
            overlay_edits: 0,
            screenshot_requested: false,
            outcome,
            fatal: false,
        };
        if !smoke && app.memory_workflow.is_none() && !app.review_qa {
            if let Some(path) = app.capture.enable_workspace() {
                if app.path.is_empty() && std::path::Path::new(&path).is_file() {
                    app.path = path;
                }
            }
        }
        if !smoke && app.memory_workflow.is_none() && !app.path.is_empty() {
            app.file_tree = Some(file_tree::FileTree::new(
                std::path::Path::new(&app.path),
                cc.egui_ctx.clone(),
            ));
        }
        if !app.path.is_empty() && app.memory_workflow.is_none() {
            app.opened_path = app.path.clone();
            app.session += 1;
            app.schedule(vec![Key::info(app.page)]);
        }
        app
    }
    fn open_paths(&mut self, paths: Vec<String>, ctx: &egui::Context) {
        let Some(first) = paths.first() else {
            return;
        };
        let input = std::path::Path::new(first);
        if let Some(tree) = &mut self.file_tree {
            tree.set_input(input);
        } else {
            self.file_tree = Some(file_tree::FileTree::new(input, ctx.clone()));
        }
        if paths.len() == 1 && input.is_dir() {
            self.path = first.clone();
            self.documents.clear();
            if self.opened_path.is_empty() {
                self.status = "Select a PDF from the folder tree".into();
            }
            return;
        }
        let paths = viewer::collect_paths(&paths);
        if paths.is_empty() {
            self.status = "Select a PDF from the folder tree".into();
            return;
        }
        self.documents = paths;
        self.select_document(self.documents[0].clone());
    }
    fn select_document(&mut self, path: String) {
        if let Some(info) = &self.info {
            self.capture.remember_document(&self.opened_path, info);
        }
        let same_set = self.capture.same_set(&path, &self.opened_path);
        self.capture.document_changed(same_set);
        self.path = path.clone();
        self.opened_path = path;
        self.page = 0;
        self.page_number = 1;
        self.zoom = 1.0;
        self.pan = [0.0, 0.0];
        self.reopen();
    }
    fn schedule(&mut self, jobs: Vec<Key>) {
        if self.opened_path.is_empty() || self.fatal {
            return;
        }
        if self.last_plan.as_ref().is_some_and(|p| {
            p.session == self.session && p.path == self.opened_path && p.jobs == jobs
        }) {
            return;
        }
        self.revision += 1;
        let plan = Plan {
            session: self.session,
            revision: self.revision,
            path: self.opened_path.clone(),
            jobs,
        };
        self.controller.submit(plan.clone());
        self.last_plan = Some(plan);
    }
    fn reopen(&mut self) {
        self.session += 1;
        self.info = None;
        self.text_page = None;
        self.text_selection.clear();
        self.text_ocr = None;
        self.text_result = None;
        self.text_notice.clear();
        self.fatal = false;
        self.status = "Opening in PDF worker…".into();
        self.schedule(vec![Key::info(self.page)]);
    }
    fn page_changed(&mut self) {
        if let Some(info) = &self.info {
            self.capture.remember_document(&self.opened_path, info);
        }
        self.capture.confirm_selection();
        self.page_number = self.page + 1;
        self.info = None;
        self.text_page = None;
        self.text_selection.clear();
        self.text_ocr = None;
        self.text_result = None;
        self.text_notice.clear();
        self.fatal = false;
        self.zoom = 1.0;
        self.pan = [0.0, 0.0];
        self.status = "Loading page…".into();
        self.schedule(vec![Key::info(self.page)]);
    }
    fn receive(&mut self, ctx: &egui::Context) {
        // At most two bounded pixel replies can be waiting. No render/pipe wait here.
        while let Ok(event) = self.controller.rx.try_recv() {
            match event {
                Event::Error { session, message } if session == self.session => {
                    self.status = format!("PDF worker: {message}. Use Restart worker to retry.");
                    self.fatal = true;
                }
                Event::Ready(reply) if reply.session == self.session => {
                    let p = reply.pixels;
                    if !self
                        .last_plan
                        .as_ref()
                        .is_some_and(|plan| plan.jobs.contains(&p.header.key))
                    {
                        continue;
                    }
                    if p.header.key.level == -3 {
                        match serde_json::from_slice::<text_selection::TextPage>(&p.rgba) {
                            Ok(text) => {
                                self.text_notice = if let Some(error) = &text.error {
                                    format!("Text unavailable: {error}")
                                } else if text.glyphs.iter().all(|g| g.1.is_none()) {
                                    "No embedded text on this sheet. OCR is needed.".into()
                                } else if text.truncated {
                                    "Text layer limited to the first 50,000 characters.".into()
                                } else {
                                    "Drag a box over text, then Copy or Cmd/Ctrl+C.".into()
                                };
                                self.text_page = Some(text);
                            }
                            Err(error) => {
                                self.text_notice = format!("Text decoding failed: {error}")
                            }
                        }
                    } else if p.header.key.level == -2 {
                        if let Some(info) = &p.header.info {
                            if info.page == self.page {
                                self.capture.activate_document(&self.opened_path, info);
                                self.info = Some(info.clone());
                                self.status = format!(
                                    "Page {} / {} · {:.1} × {:.1} displayed points · rotation {}°",
                                    info.page + 1,
                                    info.pages,
                                    info.width,
                                    info.height,
                                    info.rotation
                                );
                            }
                        }
                    } else if self
                        .info
                        .as_ref()
                        .is_some_and(|i| i.doc == p.header.key.doc && i.page == p.header.key.page)
                    {
                        let size = [p.header.size[0] as usize, p.header.size[1] as usize];
                        let image = egui::ColorImage::from_rgba_unmultiplied(size, &p.rgba);
                        let handle = ctx.load_texture(
                            format!("tile-{:?}", p.header.key),
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        if p.header.key.level == -1 && self.first_preview_ms.is_none() {
                            self.first_preview_ms =
                                Some(self.started.elapsed().as_secs_f64() * 1000.0);
                        }
                        if p.header.key.level >= 0 {
                            self.tiles_received += 1;
                            self.tile_ms.push(p.header.render_ms);
                        }
                        self.textures.insert(
                            p.header.key.clone(),
                            Texture {
                                handle,
                                header: p.header.clone(),
                            },
                            p.rgba.len(),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    fn overlay(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        view: View,
        info: &PageInfo,
    ) -> bool {
        let value = self
            .overlays
            .entry((info.doc.clone(), info.page))
            .or_insert([0.2, 0.2, 0.65, 0.65]);
        if self.smoke && self.started.elapsed().as_secs_f64() > 1.0 && !self.restarted {
            value[0] = 0.25;
            value[2] = 0.70;
            self.overlay_edits += 1;
        }
        let to_pos = |x: f64, y: f64| {
            let p = view.to_screen([x * info.width, y * info.height]);
            egui::pos2(p[0] as f32, p[1] as f32)
        };
        let a = to_pos(value[0], value[1]);
        let b = to_pos(value[2], value[3]);
        let mut dragging = false;
        for (index, pos) in [(0, a), (2, b)] {
            let hit = egui::Rect::from_center_size(pos, egui::vec2(16.0, 16.0));
            let response = ui.interact(
                hit,
                ui.id().with(("overlay-handle", index)),
                egui::Sense::drag(),
            );
            if response.dragged() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let p = view.normalized([pointer.x as f64, pointer.y as f64]);
                    value[index] = p[0];
                    value[index + 1] = p[1];
                    self.overlay_edits += 1;
                    dragging = true;
                }
            }
            painter.rect_filled(hit.shrink(3.0), 2.0, egui::Color32::LIGHT_BLUE);
        }
        painter.rect_stroke(
            egui::Rect::from_two_pos(a, b),
            0.0,
            egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE),
            egui::StrokeKind::Inside,
        );
        dragging
    }
    fn finish_smoke(&mut self, ctx: &egui::Context, success: bool) {
        let metrics = self.controller.metrics();
        let preserved = self.overlays.values().any(|r| (r[0] - 0.25).abs() < 1e-8);
        let success = success
            && self.first_preview_ms.is_some()
            && self.tiles_received > 0
            && preserved
            && self.restarted;
        println!(
            "{}",
            json!({"kind":"progressive_viewport_smoke","success":success,
            "first_preview_upload_ms":self.first_preview_ms,"tiles_received":self.tiles_received,"page":self.page+1,
            "tile_render_p95_ms":percentile(&self.tile_ms,0.95),
            "frame_interval_p50_ms":percentile(&self.intervals,0.5),"frame_interval_p95_ms":percentile(&self.intervals,0.95),
            "frame_interval_p99_ms":percentile(&self.intervals,0.99),"frames":self.intervals.len(),
            "busy_frame_interval_p95_ms":percentile(&self.busy_intervals,0.95),"busy_frames":self.busy_intervals.len(),
            "gpu_texture_peak_bytes":self.textures.peak,"gpu_budget_bytes":GPU_BUDGET,"cpu_budget_bytes":CPU_BUDGET,
            "overlay_edits":self.overlay_edits,"overlay_survived_restart":preserved,
            "pipeline":metrics,"method":"paced UI update intervals; scripted view/overlay changes during worker rendering; not GPU or input latency"})
        );
        *self.outcome.lock().unwrap() = Some(success);
        self.smoke = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
impl eframe::App for App {
    fn on_exit(&mut self) {
        self.capture.finish_workspace();
    }
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if self.info.is_some() && !self.capture.reviewing() {
            if let Some(rect) = self.drawing_rect {
                let pointer = ctx.input(|i| i.pointer.hover_pos());
                let pan = viewer::take_modified_scroll(&mut raw_input.events, rect, pointer);
                self.wheel_pan[0] += pan[0];
                self.wheel_pan[1] += pan[1];
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let interval = now.duration_since(self.previous_frame).as_secs_f64() * 1000.0;
        self.previous_frame = now;
        if self.started.elapsed() > Duration::from_millis(150) {
            self.intervals.push(interval);
            if self.intervals.len() > 3600 {
                self.intervals.remove(0);
            }
            if self.controller.is_busy() {
                self.busy_intervals.push(interval);
                if self.busy_intervals.len() > 3600 {
                    self.busy_intervals.remove(0);
                }
            }
        }
        self.receive(ctx);
        self.capture.poll();
        if let Some(result) = self.text_ocr.as_ref().and_then(|job| job.poll()) {
            self.text_ocr = None;
            match result {
                Ok(text) => {
                    self.text_notice = "OCR complete. Review the text, then copy.".into();
                    self.text_result = Some(text);
                }
                Err(e) => self.text_notice = format!("OCR: {e}"),
            }
        }
        if self.review_qa && !self.review_qa_ready {
            if let Some(info) = &self.info {
                self.capture.prepare_review_qa(&self.opened_path, info);
                self.review_qa_ready = true;
            }
        }
        if let Some(mut workflow) = self.memory_workflow.take() {
            if !workflow.applied {
                workflow.begin();
                let step = &workflow.steps[workflow.index];
                if step.page > 0 {
                    if self.opened_path.is_empty() {
                        self.opened_path = self.path.clone();
                        self.page = step.page - 1;
                        self.reopen();
                    } else if self.page != step.page - 1 {
                        self.page = step.page - 1;
                        self.page_changed();
                    }
                    self.fit_width = true;
                    self.zoom = step.zoom;
                }
            }
            if self.fatal || workflow.started.elapsed() > Duration::from_secs(60) {
                println!("{}", json!({"event":"failed","message":self.status}));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.memory_workflow = Some(workflow);
            }
        }
        if let Some(rx) = &self.picker {
            if let Ok(paths) = rx.try_recv() {
                self.picker = None;
                if !paths.is_empty() {
                    self.open_paths(paths, ctx);
                }
            }
        }
        let dropped: Vec<String> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.as_ref().map(|p| p.to_string_lossy().into_owned()))
                .collect()
        });
        if !dropped.is_empty() {
            self.open_paths(dropped, ctx);
        }
        let mut open = false;
        let mut restart = false;
        let mut page_delta = 0;
        let mut jump = None;
        let mut picker = None;
        let mut rotate = 0;
        let mut rotate_all = false;
        let mut copy_text = false;
        let mut ocr_text = false;
        let drawing_has_focus = self
            .drawing_id
            .is_some_and(|id| ctx.memory(|m| m.has_focus(id)));
        if !self.capture.reviewing()
            && self.text_result.is_none()
            && self.text_mode
            && (!ctx.wants_keyboard_input() || drawing_has_focus)
        {
            ctx.input(|i| {
                copy_text = i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                    || ((i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::C));
                if (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::A) {
                    self.text_selection.all = true;
                }
                if i.key_pressed(egui::Key::Escape) {
                    self.text_selection.clear();
                    self.text_ocr = None;
                }
            });
        }
        if !self.capture.reviewing()
            && self.text_result.is_none()
            && (!ctx.wants_keyboard_input() || drawing_has_focus)
        {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.capture.confirm_selection();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.capture.confirm_selection();
                self.capture.drawing = false;
                self.capture.selection.clear();
            }
            ctx.input(|i| {
                if i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::PageDown) {
                    page_delta = 1;
                }
                if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::PageUp) {
                    page_delta = -1;
                }
                if i.key_pressed(egui::Key::Home) {
                    jump = Some(0);
                }
                if i.key_pressed(egui::Key::End) {
                    jump = self.info.as_ref().map(|p| p.pages - 1);
                }
                if i.modifiers.command && i.key_pressed(egui::Key::O) {
                    picker = Some(false);
                }
            });
        }
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("iQs · Drawing viewer");
                if ui
                    .add_enabled(self.picker.is_none(), egui::Button::new("Open PDFs…"))
                    .clicked()
                {
                    picker = Some(false);
                }
                if ui
                    .add_enabled(self.picker.is_none(), egui::Button::new("Open folder…"))
                    .clicked()
                {
                    picker = Some(true);
                }
                ui.label("Local preview");
            });
            ui.horizontal(|ui| {
                ui.label("Path");
                let edit = ui.add_sized(
                    [ui.available_width() - 70.0, 22.0],
                    egui::TextEdit::singleline(&mut self.path),
                );
                open = ui.button("Open").clicked()
                    || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(self.page > 0, egui::Button::new("‹ Previous"))
                    .clicked()
                {
                    page_delta = -1;
                }
                let pages = self.info.as_ref().map_or(0, |p| p.pages);
                if ui
                    .add(egui::DragValue::new(&mut self.page_number).range(1..=pages.max(1)))
                    .changed()
                    && pages > 0
                {
                    jump = Some(self.page_number - 1);
                }
                ui.label(format!("/ {pages}"));
                if ui
                    .add_enabled(self.page + 1 < pages, egui::Button::new("Next ›"))
                    .clicked()
                {
                    page_delta = 1;
                }
                ui.separator();
                if ui
                    .selectable_label(!self.text_mode && !self.capture.open, "Pan")
                    .clicked()
                {
                    self.text_mode = false;
                    self.capture.set_open(false);
                    self.text_ocr = None;
                }
                if ui
                    .selectable_value(&mut self.text_mode, true, "Select text")
                    .clicked()
                {
                    self.capture.set_open(false);
                    self.text_ocr = None;
                    if self.text_page.is_none() {
                        self.text_notice = "Loading embedded text…".into();
                    }
                }
                copy_text |= ui
                    .add_enabled(
                        self.text_mode && self.text_ocr.is_none(),
                        egui::Button::new("Copy text").shortcut_text("⌘C"),
                    )
                    .clicked();
                ocr_text |= ui
                    .add_enabled(
                        self.text_mode
                            && self.text_ocr.is_none()
                            && self.text_selection.region_bounds().is_some(),
                        egui::Button::new("OCR selection"),
                    )
                    .on_hover_text(
                        "Recognise scanned text inside the Select Text box using local OCR.",
                    )
                    .clicked();
                if self.text_ocr.is_some() {
                    ui.spinner();
                    if ui.button("Cancel OCR").clicked() {
                        self.text_ocr = None;
                        self.text_notice = "OCR cancelled.".into();
                    }
                }
                if ui
                    .selectable_label(self.capture.open, "Capture fields")
                    .clicked()
                {
                    self.capture.set_open(!self.capture.open);
                    self.text_ocr = None;
                    self.text_mode = false;
                }
                if ui
                    .add_enabled(
                        self.capture.has_selection() && !self.text_mode,
                        egui::Button::new("Delete selected").shortcut_text("Delete"),
                    )
                    .on_hover_text("Remove selected capture boxes on this sheet. ⌘Z to undo.")
                    .clicked()
                {
                    if let Some(info) = &self.info {
                        self.capture.delete_selected(info);
                    }
                }
                ui.separator();
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom / 1.25).max(0.1);
                }
                ui.label(format!("{:.0}% of fit", self.zoom * 100.0));
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom * 1.25).min(MAX_ZOOM);
                }
                if ui.button("Fit page").clicked() {
                    self.fit_width = false;
                    self.zoom = 1.0;
                    self.pan = [0.0, 0.0];
                }
                if ui.button("Fit width").clicked() {
                    self.fit_width = true;
                    self.zoom = 1.0;
                    self.pan = [0.0, 0.0];
                }
                if ui.button("Rotate left").clicked() {
                    rotate = -1;
                }
                if ui.button("Rotate right").clicked() {
                    rotate = 1;
                }
                rotate_all = ui.button("Apply rotation to all sheets").clicked();
                restart = ui.button("Retry PDF").clicked();
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
            if self.text_mode {ui.label(&self.text_notice); ui.small("Drag a text box · ⌘C: copy PDF text / OCR a scan · OCR selection: force recognition · Esc: clear");}
            if !self.text_mode && self.capture.open { ui.small("Click / Shift-click boxes · Drag blank space: box-select · Space-drag: pan · ⌘A: all on sheet · Shift-click: multiple · Delete / Backspace: remove on this sheet · ⌘Z: undo"); }
            ui.small("Pan mode: drag to pan · Wheel / pinch to zoom · Ctrl+wheel: left/right · Option+wheel: up/down · Arrow keys: sheets");
        });
        if copy_text && self.text_ocr.is_none() {
            let text = self
                .text_page
                .as_ref()
                .map(|page| self.text_selection.copy_text(page))
                .unwrap_or_default();
            if !text.trim().is_empty() {
                ctx.copy_text(text);
                self.text_notice = "Selected text copied.".into();
            } else if self.text_selection.region_bounds().is_some() {
                ocr_text = true;
            } else {
                self.text_notice =
                    "Drag a box over text first. Copy uses PDF text, or OCR for a scan.".into();
            }
        }
        if ocr_text {
            if let (Some(info), Some(bounds)) = (&self.info, self.text_selection.region_bounds()) {
                let turns = *self
                    .rotations
                    .get(&(self.opened_path.clone(), self.page))
                    .unwrap_or(&0);
                match capture::TextRecognition::start(
                    &self.opened_path,
                    info,
                    bounds,
                    turns,
                    ctx.clone(),
                ) {
                    Ok(job) => {
                        self.text_ocr = Some(job);
                        self.text_notice = "Recognising selected text…".into();
                    }
                    Err(e) => self.text_notice = e,
                }
            }
        }
        if let Some(folder) = picker {
            self.picker = Some(viewer::pick_paths(folder));
        }
        if open {
            self.open_paths(vec![self.path.clone()], ctx);
        }
        if let Some(info) = &self.info {
            let id = (self.opened_path.clone(), self.page);
            if rotate != 0 {
                let r = self.rotations.entry(id.clone()).or_default();
                *r = (*r + rotate).rem_euclid(4);
                self.pan = [0.0, 0.0];
            }
            if rotate_all {
                let r = *self.rotations.get(&id).unwrap_or(&0);
                for p in 0..info.pages {
                    self.rotations.insert((self.opened_path.clone(), p), r);
                }
            }
        }
        let mut selected = None;
        egui::SidePanel::left("documents")
            .resizable(true)
            .default_width(230.0)
            .show(ctx, |ui| {
                ui.heading("Files");
                ui.label(format!(
                    "Drawing set · {} selected",
                    self.capture.set_paths.len()
                ));
                ui.small("Tick drawings for the set. Click a name to open it.");
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.opened_path.is_empty(),
                            egui::Button::new("Add current"),
                        )
                        .clicked()
                    {
                        self.capture.set_paths.insert(self.opened_path.clone());
                    }
                    if ui.button("Clear set").clicked() {
                        self.capture.clear_set();
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("document-list")
                    .max_height((ui.available_height() * 0.6).max(140.0))
                    .show(ui, |ui| {
                        if let Some(tree) = &mut self.file_tree {
                            if let Some(path) =
                                tree.ui(ui, &self.opened_path, &mut self.capture.set_paths)
                            {
                                selected = Some(path);
                            }
                        } else {
                            ui.label("Open a PDF or folder to browse its files.");
                        }
                        if self.documents.len() > 1 {
                            ui.separator();
                            ui.weak("Selected PDFs");
                            for path in &self.documents {
                                let name = std::path::Path::new(path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();
                                if ui
                                    .selectable_label(*path == self.opened_path, name)
                                    .on_hover_text(path)
                                    .clicked()
                                {
                                    selected = Some(path.clone());
                                }
                            }
                        }
                    });
                ui.separator();
                ui.heading("Sheets");
                egui::ScrollArea::vertical()
                    .id_salt("page-list")
                    .show(ui, |ui| {
                        if let Some(info) = &self.info {
                            for p in 0..info.pages {
                                if ui
                                    .selectable_label(p == self.page, format!("Sheet {}", p + 1))
                                    .clicked()
                                {
                                    jump = Some(p);
                                }
                            }
                        } else {
                            ui.label("Open a PDF to see its sheets.");
                        }
                    });
            });
        if let Some(info) = &self.info {
            self.capture.remember_document(&self.opened_path, info);
        }
        if let Some(path) = selected {
            self.select_document(path);
            page_delta = 0;
            jump = None;
        }
        if restart {
            self.reopen();
        }
        if let Some(info) = &self.info {
            let page = jump
                .unwrap_or(self.page + page_delta)
                .clamp(0, info.pages - 1);
            if page != self.page {
                self.page = page;
                self.page_changed();
            }
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        if self.smoke
            && elapsed > 3.0
            && !self.restarted
            && self.info.is_some()
            && self.overlay_edits > 0
        {
            self.restarted = true;
            self.reopen();
        }
        self.capture
            .panel(ctx, &self.opened_path, self.info.as_ref());
        if self.capture.drawing || self.capture.focus_drawing {
            self.text_mode = false;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let size = if self.memory_workflow.is_some() {
                egui::vec2(900.0, 650.0)
            } else {
                ui.available_size()
            };
            let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
            self.drawing_id = Some(response.id);
            let area = response.rect;
            self.drawing_rect = Some(area);
            let wheel_pan = std::mem::take(&mut self.wheel_pan);
            painter.rect_filled(area, 0.0, egui::Color32::from_rgb(24, 28, 34));
            let Some(info) = self.info.clone() else {
                painter.text(
                    area.center(),
                    egui::Align2::CENTER_CENTER,
                    if self.opened_path.is_empty() {
                        "Open PDFs or drop a drawing folder here"
                    } else {
                        &self.status
                    },
                    egui::FontId::proportional(18.0),
                    egui::Color32::LIGHT_GRAY,
                );
                return;
            };
            if self.smoke {
                self.zoom = if elapsed < 2.0 {
                    1.0
                } else if elapsed < 5.0 {
                    2.0
                } else {
                    4.0
                };
                self.pan = [(elapsed * 1.1).sin() * 35.0, 0.0];
            }
            let rotation = *self
                .rotations
                .get(&(self.opened_path.clone(), self.page))
                .unwrap_or(&0);
            let center = [area.center().x as f64, area.center().y as f64];
            let dimensions = if rotation % 2 == 0 {
                [info.width, info.height]
            } else {
                [info.height, info.width]
            };
            let fit = if self.fit_width {
                area.width() as f64 / dimensions[0]
            } else {
                (area.width() as f64 / dimensions[0]).min(area.height() as f64 / dimensions[1])
            } * 0.95;
            if let Some(workflow) = &self.memory_workflow {
                let step = &workflow.steps[workflow.index];
                self.pan = [
                    step.pan[0] * info.width * fit * self.zoom,
                    step.pan[1] * info.height * fit * self.zoom,
                ];
            }
            let wheel_pan = viewer::turn(wheel_pan, [0.0, 0.0], -rotation);
            self.pan[0] += wheel_pan[0];
            self.pan[1] += wheel_pan[1];
            let make_view = |zoom: f64, pan: [f64; 2]| View {
                origin: [
                    area.center().x as f64 + pan[0] - info.width * fit * zoom / 2.0,
                    area.center().y as f64 + pan[1] - info.height * fit * zoom / 2.0,
                ],
                scale: fit * zoom,
                page: [info.width, info.height],
            };
            let old = make_view(self.zoom, self.pan);
            if response.hovered() {
                let factor = ctx.input(|i| {
                    i.zoom_delta() as f64 * (i.smooth_scroll_delta.y as f64 * 0.002).exp()
                });
                let new_zoom = (self.zoom * factor).clamp(0.1, MAX_ZOOM);
                if let Some(pointer) = response.hover_pos() {
                    let pointer =
                        viewer::turn([pointer.x as f64, pointer.y as f64], center, -rotation);
                    let anchor = old.to_page(pointer);
                    let next = make_view(new_zoom, self.pan).to_screen(anchor);
                    self.pan[0] += pointer[0] - next[0];
                    self.pan[1] += pointer[1] - next[1];
                }
                self.zoom = new_zoom;
            }
            let view = make_view(self.zoom, self.pan);
            let end = view.to_screen([info.width, info.height]);
            let page_rect = egui::Rect::from_min_max(
                egui::pos2(view.origin[0] as f32, view.origin[1] as f32),
                egui::pos2(end[0] as f32, end[1] as f32),
            );
            viewer::paint_page(&painter, page_rect, center, rotation);
            let viewport = [
                area.left() as f64,
                area.top() as f64,
                area.right() as f64,
                area.bottom() as f64,
            ];
            let viewport = viewer::inverse_bounds(viewport, center, rotation);
            let tiles = visible_tiles(&info, view, viewport, ctx.pixels_per_point() as f64);
            let preview = info.preview();
            let mut keys = vec![preview.clone()];
            keys.extend(tiles.clone());
            let mut missing = Vec::new();
            for key in &keys {
                if let Some(texture) = self.textures.get(key) {
                    let h = &texture.header;
                    let a = [
                        h.origin[0] as f64 / h.full[0] as f64,
                        h.origin[1] as f64 / h.full[1] as f64,
                    ];
                    let b = [
                        (h.origin[0] + h.size[0]) as f64 / h.full[0] as f64,
                        (h.origin[1] + h.size[1]) as f64 / h.full[1] as f64,
                    ];
                    let rect = egui::Rect::from_min_max(
                        page_rect.min + page_rect.size() * egui::vec2(a[0] as f32, a[1] as f32),
                        page_rect.min + page_rect.size() * egui::vec2(b[0] as f32, b[1] as f32),
                    );
                    viewer::paint_texture(&painter, texture.handle.id(), rect, center, rotation);
                } else {
                    missing.push(key.clone());
                }
            }
            if self.text_mode && self.text_page.is_none() {
                let text = Key::text(info.doc.clone(), info.page);
                missing.insert(usize::from(!missing.is_empty()), text);
            }
            self.schedule(missing);
            if self.text_mode && !self.capture.reviewing() && self.text_result.is_none() {
                if response.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                }
                let events = ctx.input(|i| i.events.clone());
                if self
                    .text_selection
                    .handle_events(&events, area, view, center, rotation)
                {
                    response.request_focus();
                    self.text_ocr = None;
                    self.text_notice =
                        "Selection ready. ⌘C to copy; OCR selection for scanned text.".into();
                }
                self.text_selection.paint(
                    &painter,
                    self.text_page.as_ref().unwrap_or(&Default::default()),
                    view,
                    center,
                    rotation,
                );
            }
            let capture_drag = if !self.text_mode && self.text_result.is_none() {
                self.capture
                    .interact(&response, &painter, view, center, rotation, &info)
            } else {
                false
            };
            let dragging = self.smoke && self.overlay(ui, &painter, view, &info);
            if response.dragged() && !dragging && !self.text_mode && !capture_drag {
                let delta = response.drag_delta();
                let delta = viewer::turn([delta.x as f64, delta.y as f64], [0.0, 0.0], -rotation);
                self.pan[0] += delta[0];
                self.pan[1] += delta[1];
            }
        });
        self.capture.staging_ui(ctx);
        if (self.smoke && elapsed > 7.0 || self.review_qa && elapsed > 3.0)
            && !self.screenshot_requested
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.screenshot_requested = true;
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let path = env::var("IQS_SCREENSHOT_PPM")
                    .unwrap_or_else(|_| "results/viewport.ppm".into());
                if let Err(e) = write_ppm(&path, &image) {
                    eprintln!("Screenshot: {e}");
                }
            }
        }
        if self.review_qa && elapsed > 5.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.smoke && (elapsed > 9.0 || self.fatal) {
            self.finish_smoke(ctx, !self.fatal);
        }
        if let Some(mut workflow) = self.memory_workflow.take() {
            let step = &workflow.steps[workflow.index];
            let ready = step.page == 0
                || (self.info.is_some()
                    && !self.controller.is_busy()
                    && self.last_plan.as_ref().is_some_and(|p| p.jobs.is_empty()));
            if ready && !workflow.reported {
                println!(
                    "{}",
                    json!({"event":"ready","name":step.name,"textures":self.textures.bytes,"page":self.page+1})
                );
                if env::var_os("IQS_SCREENSHOT_PPM").is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                workflow.reported = true;
            }
            if workflow.advance(ready) {
                println!("{}", json!({"event":"complete","success":true}));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.memory_workflow = Some(workflow);
            }
        }
        if let Some(info) = &self.info {
            self.capture.remember_document(&self.opened_path, info);
        }
        if let Some(text) = self.text_result.as_mut() {
            let mut close = false;
            egui::Modal::new(egui::Id::new("selected-text-ocr")).show(ctx, |ui| {
                ui.set_width(600.0_f32.min(ctx.screen_rect().width() - 80.0));
                ui.heading("Recognised text");
                ui.label("Review or edit the OCR result, then copy it.");
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(text)
                                .desired_width(f32::INFINITY)
                                .desired_rows(12),
                        );
                    });
                ui.horizontal(|ui| {
                    if ui.button("Copy text").clicked() {
                        ctx.copy_text(text.clone());
                        self.text_notice = "Recognised text copied.".into();
                    }
                    close = ui.button("Done").clicked();
                });
            });
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.text_result = None;
            }
        }
        self.capture.autosave_workspace();
        // Keep cancellation/planning/metrics moving during native work and gestures.
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--export-workspace-csv") {
        let state = workspace::load(std::path::Path::new(
            args.get(1).ok_or("Saved session path required")?,
        ))?
        .ok_or("Saved session not found")?;
        let csv = capture::export_workspace_csv(state)?;
        viewer::atomic_export(
            std::path::Path::new(args.get(2).ok_or("CSV output path required")?),
            csv.as_bytes(),
        )?;
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--capture-worker") {
        if let Err(error) = capture::worker() {
            pdf_worker::write_json(&capture::Reply::Error(error), &mut std::io::stdout())?;
        }
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--pdf-worker") {
        pdf_worker::run(args.get(1).ok_or("Worker requires PDF path")?)?;
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--benchmark") {
        baseline::benchmark(args.get(1).ok_or("Benchmark requires PDF path")?.clone())?;
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--matched-benchmark") {
        baseline::matched(args.get(1).ok_or("Matched benchmark requires PDF path")?)?;
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--pipeline-test") {
        validation::run(
            args.get(1)
                .ok_or("Pipeline test requires fixtures directory")?,
        )?;
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--document-test") {
        validation::document(args.get(1).ok_or("Document test requires PDF path")?)?;
        return Ok(());
    }
    let mut smoke = false;
    let mut page = 0;
    let mut path = String::new();
    let mut paths = Vec::new();
    let mut options = args.iter();
    while let Some(arg) = options.next() {
        match arg.as_str() {
            "--smoke" => smoke = true,
            "--page" => {
                let value: i32 = options
                    .next()
                    .ok_or("--page needs a 1-based page number")?
                    .parse()?;
                if value < 1 {
                    return Err("Page must be positive".into());
                }
                page = value - 1;
            }
            a if a.starts_with("--") => return Err(format!("Unknown option: {a}").into()),
            _ => paths.push(arg.clone()),
        }
    }
    let documents = viewer::collect_paths(&paths);
    if let Some(first) = documents.first() {
        path = first.clone();
    }
    let outcome = Arc::new(Mutex::new(None));
    let result = outcome.clone();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 850.0]),
        ..Default::default()
    };
    eframe::run_native(
        "iQs Drawing Viewer",
        options,
        Box::new(move |cc| {
            let mut app = App::new(cc, path, smoke, page, result);
            app.documents = documents;
            if !smoke && app.memory_workflow.is_none() {
                if let Some(input) = paths.first().filter(|p| std::path::Path::new(p).is_dir()) {
                    app.file_tree = Some(file_tree::FileTree::new(
                        std::path::Path::new(input),
                        cc.egui_ctx.clone(),
                    ));
                }
            }
            Ok(Box::new(app))
        }),
    )?;
    if smoke && *outcome.lock().unwrap() != Some(true) {
        return Err("Progressive viewport smoke did not pass".into());
    }
    Ok(())
}

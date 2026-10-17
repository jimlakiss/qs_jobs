//! Named document/page captures. Recognition runs in a separate, cancellable process.
use crate::{
    model::PageInfo,
    pdf_worker,
    text_selection::{Selection, TextPage},
    viewer,
};
use eframe::egui;
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, Read, Write},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    Once,
    EveryPage,
}
impl Scope {
    fn label(self) -> &'static str {
        match self {
            Self::Once => "Once",
            Self::EveryPage => "Every page",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub page: i32,
    pub bounds: [f64; 4],
    pub turns: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub id: String,
    pub name: String,
    pub scope: Scope,
    pub region: Option<Region>,
    pub overrides: BTreeMap<i32, Region>,
    #[serde(default)]
    pub master_doc: Option<String>,
    #[serde(default)]
    pub excluded: BTreeSet<i32>,
    #[serde(default)]
    pub page_regions: BTreeMap<i32, Vec<Region>>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cell {
    pub text: String,
    pub method: String,
    pub confidence: Option<f32>,
    pub error: Option<String>,
    pub edited: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Data {
    pub fields: Vec<Field>,
    pub cells: BTreeMap<String, BTreeMap<i32, Cell>>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            fields: [
                ("prepared_by", "Prepared by", Scope::Once),
                ("project_id", "Project ID", Scope::Once),
                ("sheet_id", "Sheet ID", Scope::EveryPage),
                ("description", "Description", Scope::EveryPage),
                ("issue_id", "Issue ID", Scope::EveryPage),
                ("date", "Date", Scope::EveryPage),
                ("issue_description", "Issue description", Scope::EveryPage),
            ]
            .into_iter()
            .map(|(id, name, scope)| Field {
                id: id.into(),
                name: name.into(),
                scope,
                region: None,
                overrides: BTreeMap::new(),
                master_doc: None,
                excluded: BTreeSet::new(),
                page_regions: BTreeMap::new(),
            })
            .collect(),
            cells: BTreeMap::new(),
        }
    }
}
impl Field {
    fn region_for(&self, page: i32) -> Option<&Region> {
        if self.excluded.contains(&page) {
            None
        } else {
            self.overrides.get(&page).or(self.region.as_ref())
        }
    }
    fn regions_for(&self, page: i32) -> Vec<&Region> {
        if self.excluded.contains(&page) {
            return Vec::new();
        }
        let mut regions: Vec<&Region> = if let Some(regions) = self.page_regions.get(&page) {
            regions.iter().collect()
        } else {
            self.region_for(page).into_iter().collect()
        };
        regions.sort_by(|a, b| {
            if (a.bounds[1] - b.bounds[1]).abs() > 0.01 {
                a.bounds[1].total_cmp(&b.bounds[1])
            } else {
                a.bounds[0].total_cmp(&b.bounds[0])
            }
        });
        regions
    }
    fn cell_page(&self, page: i32) -> i32 {
        if self.scope == Scope::Once {
            -1
        } else {
            page
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct Task {
    pub path: String,
    pub fingerprint: String,
    pub pages: i32,
    pub fields: Vec<Field>,
    pub force_ocr: bool,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub known_fields: BTreeMap<String, Vec<Field>>,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Reply {
    Cell {
        doc: String,
        path: String,
        pages: i32,
        field: String,
        page: i32,
        value: Cell,
    },
    Done,
    Warning(String),
    Error(String),
}
struct Job {
    rx: mpsc::Receiver<Reply>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Job {
    fn start(task: Task, ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::sync_channel(16);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                    .arg("--capture-worker")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                let payload = serde_json::to_vec(&task).map_err(|e| e.to_string())?;
                if let Err(e) = child.stdin.take().unwrap().write_all(&payload) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e.to_string());
                }
                let stdout = child.stdout.take().unwrap();
                let (lines_tx, lines_rx) = mpsc::sync_channel(2);
                std::thread::spawn(move || {
                    let mut input = std::io::BufReader::new(stdout);
                    loop {
                        let mut line = Vec::new();
                        match input
                            .by_ref()
                            .take(1024 * 1024 + 1)
                            .read_until(b'\n', &mut line)
                        {
                            Ok(0) | Err(_) => break,
                            Ok(_) if line.len() > 1024 * 1024 => break,
                            Ok(_) => {
                                if lines_tx.send(line).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
                let mut last = Instant::now();
                let outcome = loop {
                    if stop.load(Ordering::Relaxed) {
                        break Ok(());
                    }
                    if last.elapsed() > Duration::from_secs(90) {
                        break Err("Capture timed out; retry the field or page.".into());
                    }
                    match lines_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(line) => {
                            last = Instant::now();
                            let reply: Reply = match serde_json::from_slice(&line) {
                                Ok(r) => r,
                                Err(e) => break Err(e.to_string()),
                            };
                            let done = matches!(reply, Reply::Done | Reply::Error(_));
                            let mut pending = reply;
                            loop {
                                match tx.try_send(pending) {
                                    Ok(()) => break,
                                    Err(mpsc::TrySendError::Full(r)) => {
                                        pending = r;
                                        if stop.load(Ordering::Relaxed) {
                                            break;
                                        }
                                        std::thread::sleep(Duration::from_millis(20));
                                    }
                                    Err(_) => break,
                                }
                            }
                            ctx.request_repaint();
                            if done {
                                break Ok(());
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(_) => break Err("Capture worker stopped unexpectedly.".into()),
                    }
                };
                let _ = child.kill();
                let _ = child.wait();
                outcome
            })();
            if let Err(e) = result {
                let _ = tx.try_send(Reply::Error(e));
                ctx.request_repaint();
            }
        });
        Self { rx, cancel }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub version: u32,
    documents: BTreeMap<String, Data>,
    templates: BTreeMap<String, Vec<Field>>,
    paths: BTreeMap<String, String>,
    page_counts: BTreeMap<String, i32>,
    set_paths: BTreeSet<String>,
    active_path: String,
    staging: Option<crate::staging::StagingState>,
}
impl Workspace {
    pub(crate) fn valid(&self) -> bool {
        let expected = Data::default();
        self.documents
            .values()
            .map(|data| &data.fields)
            .chain(self.templates.values())
            .all(|fields| {
                fields.len() == expected.fields.len()
                    && fields
                        .iter()
                        .zip(&expected.fields)
                        .all(|(field, reference)| {
                            field.id == reference.id
                                && field
                                    .region
                                    .as_ref()
                                    .is_none_or(|r| valid_bounds(r.bounds).is_some())
                                && field
                                    .overrides
                                    .values()
                                    .chain(field.page_regions.values().flatten())
                                    .all(|r| valid_bounds(r.bounds).is_some())
                        })
            })
    }
}
/// A transient Select Text request. It shares the isolated capture worker but
/// never writes field definitions, captured cells, or session state.
pub struct TextRecognition {
    job: Job,
}
impl TextRecognition {
    fn task(path: &str, info: &PageInfo, bounds: [f64; 4], turns: i32) -> Result<Task, String> {
        if valid_bounds(bounds) != Some(bounds) {
            return Err("Select a box on the drawing first.".into());
        }
        let mut field = Data::default().fields.remove(3);
        field.scope = Scope::Once;
        field.master_doc = Some(info.doc.clone());
        field.region = Some(Region {
            page: info.page,
            bounds,
            turns,
        });
        Ok(Task {
            path: path.into(),
            fingerprint: info.doc.clone(),
            pages: info.pages,
            fields: vec![field],
            force_ocr: true,
            targets: Vec::new(),
            known_fields: BTreeMap::new(),
        })
    }
    pub fn start(
        path: &str,
        info: &PageInfo,
        bounds: [f64; 4],
        turns: i32,
        ctx: egui::Context,
    ) -> Result<Self, String> {
        Ok(Self {
            job: Job::start(Self::task(path, info, bounds, turns)?, ctx),
        })
    }
    pub fn poll(&self) -> Option<Result<String, String>> {
        match self.job.rx.try_recv() {
            Ok(Reply::Cell { value, .. }) => Some(match value.error {
                Some(e) => Err(e),
                None if value.text.trim().is_empty() => {
                    Err("No text recognised in this box.".into())
                }
                None => Ok(value.text),
            }),
            Ok(Reply::Done) => Some(Err("No text recognised in this box.".into())),
            Ok(Reply::Error(e) | Reply::Warning(e)) => Some(Err(e)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("OCR worker stopped.".into())),
        }
    }
}

type RegionSnapshot = (BTreeMap<String, Data>, BTreeMap<String, Vec<Field>>);
struct RegionDrag {
    field: usize,
    slot: usize,
    anchor: [f64; 2],
    original: Region,
    preview: Region,
    corner: Option<usize>,
    companions: Vec<(usize, usize, Region, Region)>,
}
#[derive(Default)]
pub struct Capture {
    pub open: bool,
    pub drawing: bool,
    pub hide_boxes: bool,
    pub selection: Selection,
    pub documents: BTreeMap<String, Data>,
    pub selected: usize,
    highlighted: bool,
    selected_regions: BTreeSet<(usize, usize)>,
    pub override_page: bool,
    pub force_ocr: bool,
    pub notice: String,
    staging: Option<crate::staging::Staging>,
    job: Option<Job>,
    templates: BTreeMap<String, Vec<Field>>,
    paths: BTreeMap<String, String>,
    page_counts: BTreeMap<String, i32>,
    template_membership: BTreeMap<String, String>,
    move_region: Option<RegionDrag>,
    active_path: String,
    append_region: bool,
    undo: Vec<RegionSnapshot>,
    redo: Vec<RegionSnapshot>,
    pub set_paths: BTreeSet<String>,
    warnings: usize,
    completed: usize,
    autosaver: Option<crate::workspace::Saver>,
    last_autosave: Option<Instant>,
}
impl Capture {
    fn workspace_snapshot(&self) -> Workspace {
        Workspace {
            version: 1,
            documents: self.documents.clone(),
            templates: self.templates.clone(),
            paths: self.paths.clone(),
            page_counts: self.page_counts.clone(),
            set_paths: self.set_paths.clone(),
            active_path: self.active_path.clone(),
            staging: self.staging.as_ref().map(|s| s.snapshot()),
        }
    }
    fn restore_workspace(&mut self, state: Workspace) {
        self.documents = state.documents;
        for data in self.documents.values_mut() {
            if let Some(dates) = data.cells.get_mut("date") {
                for cell in dates.values_mut() {
                    cell.text = crate::capture_text::normalize("date", &cell.text);
                    if !cell.text.is_empty() && crate::capture_text::date_warning(&cell.text).is_none()
                        && cell.error.as_deref() == Some("Date is ambiguous or invalid; review the visible drawing (DD/MM/YYYY).") {
                        cell.error = None;
                    }
                }
            }
        }
        self.templates = state.templates;
        self.paths = state.paths;
        self.page_counts = state.page_counts;
        self.set_paths = state.set_paths;
        self.active_path = state.active_path;
        self.staging = state.staging.map(crate::staging::Staging::restore);
    }
    pub fn enable_workspace(&mut self) -> Option<String> {
        let result = (|| {
            let path = crate::workspace::path()?;
            if let Some(state) = crate::workspace::load(&path)? {
                self.restore_workspace(state);
                self.notice = "Saved workspace restored.".into();
            }
            self.autosaver = Some(crate::workspace::Saver::new(path));
            Ok::<_, String>(())
        })();
        if let Err(e) = result {
            self.notice = format!("Workspace could not be restored: {e}");
        }
        (!self.active_path.is_empty()).then(|| self.active_path.clone())
    }
    pub fn autosave_workspace(&mut self) {
        if self
            .last_autosave
            .is_none_or(|at| at.elapsed() > Duration::from_secs(1))
        {
            self.last_autosave = Some(Instant::now());
            if let Some(saver) = &self.autosaver {
                saver.submit(self.workspace_snapshot());
                if let Some(error) = saver.error.lock().unwrap().take() {
                    self.notice = format!("Workspace save failed: {error}");
                }
            }
        }
    }
    pub fn finish_workspace(&mut self) {
        if let Some(saver) = self.autosaver.take() {
            if let Err(error) = saver.finish(self.workspace_snapshot()) {
                eprintln!("Workspace save: {error}");
            }
        }
    }
    fn template_key(&self, path: &str) -> String {
        if self.set_paths.contains(path) {
            "selected-drawing-set".into()
        } else {
            format!("file:{path}")
        }
    }
    pub fn clear_set(&mut self) {
        self.set_paths.clear();
        self.templates.remove("selected-drawing-set");
    }
    pub fn same_set(&self, a: &str, b: &str) -> bool {
        self.set_paths.contains(a) && self.set_paths.contains(b)
    }
    pub fn remember_document(&mut self, path: &str, info: &PageInfo) {
        self.paths.insert(path.into(), info.doc.clone());
        self.page_counts.insert(info.doc.clone(), info.pages);
        let key = self.template_key(path);
        let locals = self
            .documents
            .entry(info.doc.clone())
            .or_default()
            .fields
            .clone();
        let templates = self
            .templates
            .entry(key.clone())
            .or_insert_with(|| Data::default().fields);
        // Only seed missing definitions. Navigation never replaces a master.
        for (master, local) in templates.iter_mut().zip(locals) {
            if master.region.is_none() && local.region.is_some() {
                *master = local;
                master.master_doc = Some(info.doc.clone());
                master.overrides.clear();
                master.excluded.clear();
                master.page_regions.clear();
            }
        }
        self.template_membership
            .insert(info.doc.clone(), key.clone());
        self.sync_group(&key);
    }
    fn sync_group(&mut self, key: &str) {
        let Some(templates) = self.templates.get(key).cloned() else {
            return;
        };
        let docs: BTreeSet<_> = self
            .paths
            .iter()
            .filter(|(path, _)| self.template_key(path) == key)
            .map(|(_, doc)| doc.clone())
            .collect();
        for doc in docs {
            if let Some(data) = self.documents.get_mut(&doc) {
                merge_templates(data, &templates);
            }
        }
    }
    fn publish_metadata(&mut self, path: &str, doc: &str) {
        let key = self.template_key(path);
        let Some(data) = self.documents.get(doc) else {
            return;
        };
        let templates = self
            .templates
            .entry(key.clone())
            .or_insert_with(|| Data::default().fields);
        for (master, field) in templates.iter_mut().zip(&data.fields) {
            master.name = field.name.clone();
            if master.scope != field.scope {
                master.scope = field.scope;
            }
        }
        self.sync_group(&key);
    }
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        self.drawing = false;
        self.confirm_selection();
    }
    pub fn confirm_selection(&mut self) {
        self.highlighted = false;
        self.selected_regions.clear();
        self.selection.clear();
        self.move_region = None;
    }

    pub fn activate_document(&mut self, path: &str, info: &PageInfo) {
        self.confirm_selection();
        self.active_path = path.into();
        self.append_region = false;
        if let Some(previous) = self.paths.get(path).cloned().filter(|doc| doc != &info.doc) {
            let key = self.template_key(path);
            if let Some(fields) = self.templates.get_mut(&key) {
                for field in fields {
                    if field.master_doc.as_deref() == Some(&previous) {
                        field.master_doc = Some(info.doc.clone());
                    }
                }
            }
        }
        self.remember_document(path, info);
        self.move_region = None;
    }
    fn checkpoint_regions(&mut self) {
        self.undo
            .push((self.documents.clone(), self.templates.clone()));
        if self.undo.len() > 50 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn undo_regions(&mut self) {
        if let Some((docs, templates)) = self.undo.pop() {
            self.redo.push((
                std::mem::replace(&mut self.documents, docs),
                std::mem::replace(&mut self.templates, templates),
            ));
            self.confirm_selection();
        }
    }
    fn redo_regions(&mut self) {
        if let Some((docs, templates)) = self.redo.pop() {
            self.undo.push((
                std::mem::replace(&mut self.documents, docs),
                std::mem::replace(&mut self.templates, templates),
            ));
            self.confirm_selection();
        }
    }
    fn commit_region(
        &mut self,
        path: &str,
        info: &PageInfo,
        index: usize,
        slot: usize,
        region: Region,
        append: bool,
    ) {
        self.checkpoint_regions();
        self.commit_region_inner(path, info, index, slot, region, append);
    }
    fn commit_region_inner(
        &mut self,
        path: &str,
        info: &PageInfo,
        index: usize,
        slot: usize,
        region: Region,
        append: bool,
    ) {
        let key = self.template_key(path);
        let field = self.documents.entry(info.doc.clone()).or_default().fields[index].clone();
        let is_master = field.region.is_none()
            || field.scope == Scope::Once
            || (field.master_doc.as_deref() == Some(&info.doc)
                && field.region.as_ref().is_some_and(|r| r.page == info.page)
                && !field.overrides.contains_key(&info.page)
                && !field.page_regions.contains_key(&info.page));
        if is_master && (!append || field.region.is_none()) {
            let templates = self
                .templates
                .entry(key.clone())
                .or_insert_with(|| Data::default().fields);
            let master = &mut templates[index];
            master.region = Some(region);
            master.master_doc = Some(info.doc.clone());
            master.name = field.name;
            master.scope = field.scope;
            // Invalidate derived captures across the set; explicit local overrides survive.
            let id = master.id.clone();
            let docs: BTreeSet<_> = self
                .paths
                .iter()
                .filter(|(path, _)| self.template_key(path) == key)
                .map(|(_, doc)| doc.clone())
                .collect();
            for doc in docs {
                if let Some(data) = self.documents.get_mut(&doc) {
                    data.cells.remove(&id);
                }
            }
            let templates = self.templates[&key].clone();
            merge_templates(self.documents.get_mut(&info.doc).unwrap(), &templates);
            self.sync_group(&key);
        } else {
            let data = self.documents.get_mut(&info.doc).unwrap();
            let field = &mut data.fields[index];
            let existing: Vec<_> = field.regions_for(info.page).into_iter().cloned().collect();
            field.excluded.remove(&info.page);
            if append {
                let mut regions = existing;
                regions.push(region);
                regions.sort_by(|a, b| {
                    a.bounds[1]
                        .total_cmp(&b.bounds[1])
                        .then_with(|| a.bounds[0].total_cmp(&b.bounds[0]))
                });
                field.page_regions.insert(info.page, regions);
            } else if let std::collections::btree_map::Entry::Occupied(mut entry) =
                field.page_regions.entry(info.page)
            {
                let mut regions = existing;
                if let Some(r) = regions.get_mut(slot) {
                    *r = region;
                }
                entry.insert(regions);
            } else {
                field.overrides.insert(info.page, region);
            }
            if let Some(cells) = data.cells.get_mut(&field.id) {
                cells.remove(&field.cell_page(info.page));
            }
        }
    }
    pub fn document_changed(&mut self, same_set: bool) {
        if !same_set {
            self.job = None;
            self.drawing = false;
            self.selected = 0;
            self.notice.clear();
        }
        self.override_page = false;
        self.selection.clear();
    }
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(job) = &self.job {
            while let Ok(reply) = job.rx.try_recv() {
                match reply {
                    Reply::Cell {
                        doc,
                        path,
                        pages,
                        field,
                        page,
                        value,
                    } => {
                        let folder = self.template_key(&path);
                        self.paths.insert(path, doc.clone());
                        self.page_counts.insert(doc.clone(), pages);
                        let data = self.documents.entry(doc).or_default();
                        if let Some(templates) = self.templates.get(&folder) {
                            merge_templates(data, templates);
                        }
                        data.cells.entry(field).or_default().insert(page, value);
                        self.completed += 1;
                        self.notice = format!("Captured {} regions", self.completed);
                    }
                    Reply::Warning(message) => {
                        self.warnings += 1;
                        self.notice = message;
                    }
                    Reply::Done => {
                        finished = true;
                        self.notice = format!(
                            "Complete: {} captures · {} file warnings. Review results below.",
                            self.completed, self.warnings
                        );
                    }
                    Reply::Error(e) => {
                        finished = true;
                        self.notice = e;
                    }
                }
            }
        }
        if finished {
            self.job = None;
        }
    }
    pub fn panel(&mut self, ctx: &egui::Context, path: &str, info: Option<&PageInfo>) {
        if !self.open {
            return;
        }
        let mut pending_task = None;
        let mut copy_table = false;
        let mut review = false;
        let mut metadata_changed = false;
        let mut remove_sheet = false;
        let mut reset_sheet = false;
        let mut restore_excluded = false;
        let mut clear_field = false;
        let mut imported = false;
        let mut undo = false;
        let mut redo = false;
        let mut document_edits: Vec<(String, Cell)> = Vec::new();
        egui::SidePanel::right("capture-panel")
            .default_width(330.0)
            .min_width(270.0)
            .show(ctx, |ui| {
                ui.heading("Capture fields");
                if self.autosaver.is_some() {
                    ui.small("Workspace saves automatically on this Mac.");
                }
                ui.horizontal(|ui| {
                    undo = ui
                        .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo region edit"))
                        .clicked();
                    redo = ui
                        .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo"))
                        .clicked();
                });
                let mut visible = !self.hide_boxes;
                if ui.checkbox(&mut visible, "Show capture boxes").changed() && !visible {
                    self.confirm_selection();
                }
                self.hide_boxes = !visible;
                let Some(info) = info else {
                    ui.label("Open a PDF to capture fields.");
                    return;
                };
                let busy = self.job.is_some();
                let data = self.documents.entry(info.doc.clone()).or_default();
                self.selected = self.selected.min(data.fields.len() - 1);
                ui.label("Choose a field, its scope, then draw its box.");
                ui.small(if self.set_paths.contains(path) {
                    format!(
                        "Drawing set: {} PDFs. Every page boxes repeat across these drawings.",
                        self.set_paths.len()
                    )
                } else {
                    "Current PDF only. Tick drawings in the left sidebar to make a set.".into()
                });
                ui.add_enabled_ui(!busy, |ui| {
                    for (i, field) in data.fields.iter_mut().enumerate() {
                        let old_scope = field.scope;
                        ui.horizontal(|ui| {
                            let text = format!(
                                "{} {}",
                                if field.region.is_some() { "●" } else { "○" },
                                field.name
                            );
                            if ui
                                .add_sized(
                                    [160.0, 24.0],
                                    egui::Button::new(text)
                                        .selected(self.selected == i && self.drawing),
                                )
                                .clicked()
                            {
                                self.selected = i;
                                self.highlighted = true;
                                self.selected_regions = if field.scope == Scope::EveryPage ||
                                    (field.master_doc.as_deref().is_none_or(|doc| doc == info.doc)
                                        && field.region.as_ref().is_some_and(|r| r.page == info.page)) {
                                    field.regions_for(info.page).iter().enumerate().map(|(slot, _)| (i, slot)).collect()
                                } else { BTreeSet::new() };
                                self.append_region = false;
                                self.override_page = false;
                                self.drawing = true;
                                self.hide_boxes = false;
                                self.selection.clear();
                            }
                            egui::ComboBox::from_id_salt(("scope", i))
                                .selected_text(field.scope.label())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut field.scope, Scope::Once, "Once");
                                    ui.selectable_value(
                                        &mut field.scope,
                                        Scope::EveryPage,
                                        "Every page",
                                    );
                                });
                        });
                        if old_scope != field.scope {
                            field.overrides.clear();
                            field.page_regions.clear();
                            field.excluded.clear();
                            metadata_changed = true;
                            data.cells.remove(&field.id);
                            self.selected = i;
                            self.highlighted = true;
                            self.selected_regions.clear();
                            self.drawing = true;
                            self.hide_boxes = false;
                            self.selection.clear();
                        }
                    }
                    let field = &mut data.fields[self.selected];
                    ui.separator();
                    ui.label(format!("Box for: {}", field.name));
                    ui.small(
                        if field.master_doc.as_deref() == Some(&info.doc)
                            && field.region.as_ref().is_some_and(|r| r.page == info.page)
                        {
                            "Master sheet: changes here update inherited regions."
                        } else {
                            "Inherited sheet: changes here create a local override."
                        },
                    );
                    ui.checkbox(
                        &mut self.append_region,
                        "Add another box to this field on this sheet",
                    );
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        metadata_changed |= ui
                            .add(egui::TextEdit::singleline(&mut field.name).desired_width(240.0))
                            .changed();
                    });
                    ui.small(if self.drawing {
                        "Release to keep changes · Shift-click: select boxes · Drag: move · ⌘C / ⌘V: copy / paste · Delete: remove on this sheet · ⌘Z / ⇧⌘Z: undo / redo · Enter / blank click: deselect · Esc: Pan"
                    } else {
                        "Select a field above, then drag its box on the drawing."
                    });
                    if self.highlighted && !self.selected_regions.is_empty() {
                        ui.small(format!("{} box(es) selected", self.selected_regions.len()));
                    }
                    if let Some(region) = field.region_for(info.page) {
                        ui.small(format!(
                            "Source sheet {} · {} page overrides",
                            region.page + 1,
                            field.overrides.len()
                        ));
                    }
                    if !field.excluded.is_empty() {
                        ui.colored_label(egui::Color32::YELLOW, format!("Excluded from extraction on {} sheet(s)", field.excluded.len()));
                        restore_excluded = ui.add_enabled(!busy, egui::Button::new("Restore excluded sheets for this field")).clicked();
                    }
                    ui.horizontal_wrapped(|ui| {
                        remove_sheet = ui.button("Remove on this sheet").clicked();
                        reset_sheet = ui.button("Use master on this sheet").clicked();
                        clear_field = ui.button("Clear field everywhere").clicked();
                    });
                    ui.checkbox(&mut self.force_ocr, "Force OCR (ignore embedded text)");
                });
                ui.small("Extract fields replaces results for all defined boxes.");
                let mut run = false;
                ui.horizontal(|ui| {
                    run = ui
                        .add_enabled(
                            !busy && data.fields.iter().any(|f| f.region.is_some()),
                            egui::Button::new("Extract fields"),
                        )
                        .clicked();
                    if busy && ui.button("Cancel").clicked() {
                        self.job = None;
                        self.notice = "Cancelled. Completed results retained.".into();
                    }
                });
                review = ui
                    .add_enabled(!busy, egui::Button::new("Review & export…"))
                    .clicked();
                ui.small(&self.notice);
                ui.horizontal(|ui| {
                    if ui.button("Copy table").clicked() {
                        copy_table = true;
                        self.notice = "Table copied (tab-separated).".into();
                    }
                    if ui.button("Copy JSON").clicked() {
                        ctx.copy_text(serde_json::to_string_pretty(data).unwrap_or_default());
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Save captures"))
                        .clicked()
                    {
                        self.notice = save(&info.doc, data)
                            .map(|_| "Saved locally for this PDF.".into())
                            .unwrap_or_else(|e| e);
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new("Load saved"))
                        .clicked()
                    {
                        match load(&info.doc) {
                            Ok(saved) => {
                                *data = saved;
                                imported = true;
                                self.selection.clear();
                                self.drawing = false;
                                self.notice = "Saved captures loaded.".into();
                            }
                            Err(e) => self.notice = e,
                        }
                    }
                });
                ui.separator();
                ui.label("Results · edit values to correct recognition");
                egui::ScrollArea::vertical()
                    .id_salt("capture-results")
                    .show(ui, |ui| {
                        for field in &data.fields {
                            ui.collapsing(&field.name, |ui| {
                                if let Some(cells) = data.cells.get_mut(&field.id) {
                                    for (page, cell) in cells {
                                        ui.label(if *page < 0 {
                                            "Document".into()
                                        } else {
                                            format!("Sheet {}", page + 1)
                                        });
                                        if ui
                                            .add_enabled(
                                                !busy,
                                                egui::TextEdit::singleline(&mut cell.text)
                                                    .desired_width(f32::INFINITY),
                                            )
                                            .changed()
                                        {
                                            cell.edited = true;
                                            if field.scope == Scope::Once {
                                                document_edits
                                                    .push((field.id.clone(), cell.clone()));
                                            }
                                        }
                                        ui.small(format!(
                                            "{}{}{}",
                                            cell.method,
                                            cell.confidence
                                                .map(|c| format!(" · {:.0}% confidence", c * 100.0))
                                                .unwrap_or_default(),
                                            if cell.edited { " · edited" } else { "" }
                                        ));
                                        if let Some(error) = &cell.error {
                                            ui.colored_label(egui::Color32::YELLOW, error);
                                        }
                                    }
                                } else {
                                    ui.weak("Not captured yet");
                                }
                            });
                        }
                    });
                if run {
                    self.drawing = false;
                    self.selection.clear();
                    self.completed = 0;
                    self.warnings = 0;
                    self.notice = "Extracting in background…".into();
                    pending_task = Some(Task {
                        path: path.into(),
                        fingerprint: info.doc.clone(),
                        pages: info.pages,
                        fields: data.fields.clone(),
                        force_ocr: self.force_ocr,
                        targets: if self.set_paths.contains(path) {
                            self.set_paths.iter().cloned().collect()
                        } else {
                            Vec::new()
                        },
                        known_fields: BTreeMap::new(),
                    });
                }
            });
        if undo {
            self.undo_regions();
        }
        if redo {
            self.redo_regions();
        }
        if let Some(info) = info {
            if remove_sheet || reset_sheet || clear_field || restore_excluded {
                self.confirm_selection();
                self.checkpoint_regions();
            }
            if imported {
                let key = self.template_key(path);
                let mut fields = self.documents[&info.doc].fields.clone();
                for field in &mut fields {
                    if field.region.is_some() {
                        field.master_doc.get_or_insert(info.doc.clone());
                    }
                    field.overrides.clear();
                    field.page_regions.clear();
                    field.excluded.clear();
                }
                self.templates.insert(key.clone(), fields);
                self.sync_group(&key);
            }
            if !document_edits.is_empty() {
                let key = self.template_key(path);
                let docs: BTreeSet<_> = self
                    .paths
                    .iter()
                    .filter(|(path, _)| self.template_key(path) == key)
                    .map(|(_, doc)| doc.clone())
                    .collect();
                for doc in docs {
                    if let Some(data) = self.documents.get_mut(&doc) {
                        for (id, cell) in &document_edits {
                            data.cells
                                .entry(id.clone())
                                .or_default()
                                .insert(-1, cell.clone());
                        }
                    }
                }
            }
            if metadata_changed {
                self.publish_metadata(path, &info.doc);
            }
            if clear_field {
                let key = self.template_key(path);
                if let Some(fields) = self.templates.get_mut(&key) {
                    fields[self.selected].region = None;
                    fields[self.selected].master_doc = None;
                }
                let docs: BTreeSet<_> = self
                    .paths
                    .iter()
                    .filter(|(path, _)| self.template_key(path) == key)
                    .map(|(_, doc)| doc.clone())
                    .collect();
                for doc in docs {
                    let Some(data) = self.documents.get_mut(&doc) else {
                        continue;
                    };
                    let field = &mut data.fields[self.selected];
                    field.overrides.clear();
                    field.excluded.clear();
                    field.page_regions.clear();
                    data.cells.remove(&field.id);
                }
                self.sync_group(&key);
            } else if remove_sheet || reset_sheet {
                let data = self.documents.get_mut(&info.doc).unwrap();
                let field = &mut data.fields[self.selected];
                field.overrides.remove(&info.page);
                field.page_regions.remove(&info.page);
                if remove_sheet {
                    field.excluded.insert(info.page);
                } else {
                    field.excluded.remove(&info.page);
                }
                data.cells.remove(&field.id);
            }
        }
        if restore_excluded {
            if let Some(info) = info {
                self.documents.get_mut(&info.doc).unwrap().fields[self.selected]
                    .excluded
                    .clear();
                self.notice =
                    "Excluded sheets restored for this field. Extract fields to refresh results."
                        .into();
            }
        }
        if review {
            if let Some(info) = info {
                self.open_staging(path, info);
            }
        }
        if copy_table {
            if let Some(info) = info {
                ctx.copy_text(self.copy_table(path, info));
            }
        }
        if let Some(mut task) = pending_task {
            task.known_fields = self
                .documents
                .iter()
                .map(|(doc, data)| (doc.clone(), data.fields.clone()))
                .collect();
            self.job = Some(Job::start(task, ctx.clone()));
        }
    }
    pub fn prepare_review_qa(&mut self, path: &str, info: &PageInfo) {
        self.activate_document(path, info);
        self.open = true;
        let data = self.documents.get_mut(&info.doc).unwrap();
        for page in 0..info.pages {
            for (id,text) in [("sheet_id",format!("A{:03}",page+1)),("description",format!("Level {} — general arrangement and reflected ceiling plan, external courtyard and adjacent rooms",page+1)),("issue_id","K".into())] {data.cells.entry(id.into()).or_default().insert(page,Cell{text,method:"Synthetic QA".into(),..Default::default()});}
        }
        self.open_staging(path, info);
    }
    pub fn reviewing(&self) -> bool {
        self.staging.as_ref().is_some_and(|s| s.open)
    }
    fn open_staging(&mut self, path: &str, info: &PageInfo) {
        let Some(data) = self.documents.get(&info.doc) else {
            return;
        };
        let paths: Vec<_> = if self.set_paths.contains(path) {
            self.set_paths.iter().cloned().collect()
        } else {
            vec![path.into()]
        };
        let sources = paths
            .into_iter()
            .map(|source_path| {
                let doc = if source_path == path {
                    Some(info.doc.clone())
                } else {
                    self.paths.get(&source_path).cloned()
                };
                let pages = if source_path == path {
                    info.pages
                } else {
                    doc.as_ref()
                        .and_then(|doc| self.page_counts.get(doc))
                        .copied()
                        .unwrap_or(1)
                };
                let data = doc.as_ref().and_then(|doc| self.documents.get(doc));
                (source_path, doc, pages, data)
            })
            .collect();
        let name = std::path::Path::new(path)
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.drawing = false;
        self.selection.clear();
        let mut stage = crate::staging::Staging::new(&data.fields, sources, name);
        if let Some(previous) = &self.staging {
            stage.preserve_layout(previous);
        }
        self.staging = Some(stage);
    }
    pub fn staging_ui(&mut self, ctx: &egui::Context) {
        if let Some(stage) = &mut self.staging {
            let active = stage.open;
            stage.ui(ctx);
            if active {
                stage.apply(&mut self.documents);
            }
        }
    }
    fn copy_table(&self, path: &str, info: &PageInfo) -> String {
        let Some(current) = self.documents.get(&info.doc) else {
            return String::new();
        };
        if !self.set_paths.contains(path) {
            return table(current, info.pages);
        }
        let mut out = vec![format!("File\t{}", table(current, 0))];
        for path in &self.set_paths {
            let name = std::path::Path::new(path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .replace(['\t', '\n', '\r'], " ");
            if let Some((doc, data)) = self
                .paths
                .get(path)
                .and_then(|doc| self.documents.get(doc).map(|data| (doc, data)))
            {
                for line in table(data, *self.page_counts.get(doc).unwrap_or(&1))
                    .lines()
                    .skip(1)
                {
                    out.push(format!("{name}\t{line}"));
                }
            } else {
                out.push(format!("{name}\t1{}", "\t".repeat(current.fields.len())));
            }
        }
        out.join("\n")
    }
    fn region_selected(&self, field: usize, slot: usize) -> bool {
        self.highlighted && self.selected_regions.contains(&(field, slot))
    }
    fn select_changed(&mut self, info: &PageInfo, changed: &[(usize, usize, Region)]) {
        self.selected_regions.clear();
        if let Some(data) = self.documents.get(&info.doc) {
            for (field, _, region) in changed {
                for (slot, r) in data.fields[*field]
                    .regions_for(info.page)
                    .iter()
                    .enumerate()
                {
                    if r.bounds == region.bounds {
                        self.selected_regions.insert((*field, slot));
                    }
                }
            }
        }
        self.highlighted = !self.selected_regions.is_empty();
    }
    fn commit_moves(&mut self, info: &PageInfo, changes: Vec<(usize, usize, Region)>) {
        if changes.is_empty() {
            return;
        }
        self.checkpoint_regions();
        let path = self.active_path.clone();
        let mut grouped: BTreeMap<usize, Vec<(usize, Region)>> = BTreeMap::new();
        for (field, slot, region) in &changes {
            grouped
                .entry(*field)
                .or_default()
                .push((*slot, region.clone()));
        }
        for (index, edits) in grouped {
            let regions: Vec<_> = self.documents[&info.doc].fields[index]
                .regions_for(info.page)
                .into_iter()
                .cloned()
                .collect();
            if regions.len() == 1 {
                self.commit_region_inner(&path, info, index, 0, edits[0].1.clone(), false);
            } else {
                // Patch all slots before sorting: boxes may cross each other in a drag.
                let mut regions = regions;
                for (slot, region) in edits {
                    if let Some(r) = regions.get_mut(slot) {
                        *r = region;
                    }
                }
                let data = self.documents.get_mut(&info.doc).unwrap();
                let field = &mut data.fields[index];
                field.page_regions.insert(info.page, regions);
                field.excluded.remove(&info.page);
                data.cells.remove(&field.id);
            }
        }
        self.select_changed(info, &changes);
    }
    pub fn has_selection(&self) -> bool {
        self.open
            && self.highlighted
            && !self.selected_regions.is_empty()
            && self.job.is_none()
            && !self.reviewing()
    }
    pub fn delete_selected(&mut self, info: &PageInfo) {
        if !self.has_selection() {
            return;
        }
        self.checkpoint_regions();
        if let Some(data) = self.documents.get_mut(&info.doc) {
            let fields: BTreeSet<_> = self.selected_regions.iter().map(|&(f, _)| f).collect();
            for index in fields {
                let field = &mut data.fields[index];
                let remaining: Vec<_> = field
                    .regions_for(info.page)
                    .into_iter()
                    .enumerate()
                    .filter(|(slot, _)| !self.selected_regions.contains(&(index, *slot)))
                    .map(|(_, r)| r.clone())
                    .collect();
                field.overrides.remove(&info.page);
                if remaining.is_empty() {
                    field.page_regions.remove(&info.page);
                    field.excluded.insert(info.page);
                } else {
                    field.page_regions.insert(info.page, remaining);
                }
                data.cells.remove(&field.id);
            }
        }
        self.confirm_selection();
        self.notice = "Selected boxes removed on this sheet. ⌘Z to undo.".into();
    }
    fn copy_regions(&self, info: &PageInfo) -> Option<String> {
        let data = self.documents.get(&info.doc)?;
        let regions: Vec<_> = self
            .selected_regions
            .iter()
            .filter_map(|&(f, slot)| {
                let field = data.fields.get(f)?;
                Some((
                    field.id.clone(),
                    (*field.regions_for(info.page).get(slot)?).clone(),
                ))
            })
            .collect();
        if regions.is_empty() {
            return None;
        }
        serde_json::to_string(&(info.doc.clone(), info.page, regions))
            .ok()
            .map(|s| format!("iQs regions v1\n{s}"))
    }
    fn paste_regions(&mut self, info: &PageInfo, clipboard: &str) {
        let Some(payload) = clipboard.strip_prefix("iQs regions v1\n") else {
            return;
        };
        if payload.len() > 256 * 1024 {
            return;
        }
        type Copied = (String, i32, Vec<(String, Region)>);
        let Ok((doc, page, regions)) = serde_json::from_str::<Copied>(payload) else {
            return;
        };
        if regions.is_empty() || regions.len() > 128 || !self.documents.contains_key(&info.doc) {
            return;
        }
        let fields = &self.documents[&info.doc].fields;
        let mut additions = Vec::new();
        // Same-sheet copies are offset together so the duplicate is visible.
        let same = doc == info.doc && page == info.page;
        let dx = if same {
            regions
                .iter()
                .map(|(_, r)| 1.0 - r.bounds[2])
                .fold(0.02, f64::min)
                .max(0.0)
        } else {
            0.0
        };
        let dy = if same {
            regions
                .iter()
                .map(|(_, r)| 1.0 - r.bounds[3])
                .fold(0.02, f64::min)
                .max(0.0)
        } else {
            0.0
        };
        for (id, mut region) in regions {
            if valid_bounds(region.bounds) != Some(region.bounds) {
                return;
            }
            let Some(index) = fields.iter().position(|f| f.id == id) else {
                return;
            };
            let b = region.bounds;
            region.bounds = [b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy];
            region.page = info.page;
            additions.push((index, 0, region));
        }
        self.checkpoint_regions();
        let path = self.active_path.clone();
        for (index, _, region) in &additions {
            let field = &self.documents[&info.doc].fields[*index];
            let transferable_once = field.scope == Scope::Once
                && (field.master_doc.as_deref() != Some(&info.doc)
                    || field.region.as_ref().is_some_and(|r| r.page != info.page));
            self.commit_region_inner(&path, info, *index, 0, region.clone(), !transferable_once);
        }
        self.select_changed(info, &additions);
        self.notice = "Boxes pasted with their field names. Drag to position; ⌘Z to undo.".into();
    }
    fn region_shortcuts(&mut self, ctx: &egui::Context, info: &PageInfo, events: &[egui::Event]) {
        let copy = events.iter().any(|e| matches!(e, egui::Event::Copy))
            || ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C));
        if copy {
            if let Some(text) = self.copy_regions(info) {
                ctx.copy_text(text);
                self.notice = "Selected boxes copied. ⌘V to paste on this or another sheet.".into();
            }
        }
        for event in events {
            if let egui::Event::Paste(text) = event {
                self.paste_regions(info, text);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            self.delete_selected(info);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            if ctx.input(|i| i.modifiers.shift) {
                self.redo_regions();
            } else {
                self.undo_regions();
            }
            self.confirm_selection();
        }
    }
    pub fn interact(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        view: crate::model::View,
        center: [f64; 2],
        turns: i32,
        info: &PageInfo,
    ) -> bool {
        // Capture visibility and editing are tool state, never extraction state.
        if !self.open {
            return false;
        }
        let mut consumed = self.move_region.is_some();
        let ctx = painter.ctx();
        let point =
            |p: egui::Pos2| view.normalized(viewer::turn([p.x as f64, p.y as f64], center, -turns));
        let to_pos = |p: [f64; 2]| {
            let p = viewer::turn(
                view.to_screen([p[0] * info.width, p[1] * info.height]),
                center,
                turns,
            );
            egui::pos2(p[0] as f32, p[1] as f32)
        };
        if !self.hide_boxes && !self.reviewing() && self.job.is_none() {
            let events = ctx.input(|i| i.events.clone());
            if response.has_focus() || !ctx.wants_keyboard_input() {
                self.region_shortcuts(ctx, info, &events);
            }
            let mut drawing_events = Vec::new();
            for event in events {
                if let egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } = &event
                {
                    if response.rect.contains(*pos) {
                        let p = point(*pos);
                        let hit = self.documents.get(&info.doc).and_then(|data| {
                            data.fields
                                .iter()
                                .enumerate()
                                .rev()
                                .find_map(|(index, field)| {
                                    if field.scope == Scope::Once
                                        && (field
                                            .master_doc
                                            .as_deref()
                                            .is_some_and(|doc| doc != info.doc)
                                            || field
                                                .region
                                                .as_ref()
                                                .is_some_and(|r| r.page != info.page))
                                    {
                                        return None;
                                    }
                                    field
                                        .regions_for(info.page)
                                        .into_iter()
                                        .enumerate()
                                        .find_map(|(slot, region)| {
                                            let b = region.bounds;
                                            let corners = [
                                                [b[0], b[1]],
                                                [b[2], b[1]],
                                                [b[2], b[3]],
                                                [b[0], b[3]],
                                            ];
                                            let corner = if self.region_selected(index, slot)
                                                && self.selected_regions.len() == 1
                                            {
                                                corners
                                                    .iter()
                                                    .position(|c| to_pos(*c).distance(*pos) < 8.0)
                                            } else {
                                                None
                                            };
                                            if corner.is_some()
                                                || egui::Rect::from_points(&corners.map(to_pos))
                                                    .expand(5.0)
                                                    .contains(*pos)
                                            {
                                                Some((index, slot, region.clone(), corner))
                                            } else {
                                                None
                                            }
                                        })
                                })
                        });
                        if let Some((field, slot, region, corner)) = hit {
                            consumed = true;
                            if modifiers.shift {
                                if !self.selected_regions.insert((field, slot)) {
                                    self.selected_regions.remove(&(field, slot));
                                }
                                self.highlighted = !self.selected_regions.is_empty();
                                self.selected = field;
                                self.selection.clear();
                                response.request_focus();
                                continue;
                            }
                            if !self.selected_regions.contains(&(field, slot)) {
                                self.selected_regions.clear();
                                self.selected_regions.insert((field, slot));
                            }
                            self.selected = field;
                            self.highlighted = true;
                            self.selection.clear();
                            let companions = self
                                .documents
                                .get(&info.doc)
                                .map(|data| {
                                    self.selected_regions
                                        .iter()
                                        .filter(|&&(f, s)| (f, s) != (field, slot))
                                        .filter_map(|&(f, s)| {
                                            data.fields
                                                .get(f)?
                                                .regions_for(info.page)
                                                .get(s)
                                                .map(|r| (f, s, (*r).clone(), (*r).clone()))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            self.move_region = Some(RegionDrag {
                                field,
                                slot,
                                anchor: p,
                                original: region.clone(),
                                preview: region,
                                corner: if self.selected_regions.len() == 1 {
                                    corner
                                } else {
                                    None
                                },
                                companions,
                            });
                            response.request_focus();
                            continue;
                        }
                    }
                }
                if let Some(drag) = &mut self.move_region {
                    match &event {
                        egui::Event::PointerMoved(pos)
                        | egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            ..
                        } => {
                            let p = point(*pos);
                            let mut b = drag.original.bounds;
                            if let Some(corner) = drag.corner {
                                let x = if corner == 0 || corner == 3 { 0 } else { 2 };
                                let y = if corner < 2 { 1 } else { 3 };
                                b[x] = p[0].clamp(0.0, 1.0);
                                b[y] = p[1].clamp(0.0, 1.0);
                                b = [
                                    b[0].min(b[2]),
                                    b[1].min(b[3]),
                                    b[0].max(b[2]),
                                    b[1].max(b[3]),
                                ];
                            } else {
                                let mut limits = b;
                                for (_, _, original, _) in &drag.companions {
                                    let r = original.bounds;
                                    limits = [
                                        limits[0].min(r[0]),
                                        limits[1].min(r[1]),
                                        limits[2].max(r[2]),
                                        limits[3].max(r[3]),
                                    ];
                                }
                                let dx = (p[0] - drag.anchor[0]).clamp(-limits[0], 1.0 - limits[2]);
                                let dy = (p[1] - drag.anchor[1]).clamp(-limits[1], 1.0 - limits[3]);
                                b = [b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy];
                                for (_, _, original, preview) in &mut drag.companions {
                                    let r = original.bounds;
                                    preview.bounds = [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy];
                                }
                            }
                            drag.preview.bounds = b;
                            if matches!(event, egui::Event::PointerButton { pressed: false, .. }) {
                                let drag = self.move_region.take().unwrap();
                                if drag.preview.bounds != drag.original.bounds
                                    && valid_bounds(drag.preview.bounds).is_some()
                                {
                                    let mut changes = vec![(drag.field, drag.slot, drag.preview)];
                                    changes.extend(
                                        drag.companions.into_iter().map(|(f, s, _, r)| (f, s, r)),
                                    );
                                    self.commit_moves(info, changes);
                                    self.notice =
                                        "Region moved. Inherited-sheet changes stay on this sheet."
                                            .into();
                                }
                            }
                        }
                        egui::Event::PointerGone | egui::Event::WindowFocused(false) => {
                            self.move_region = None;
                        }
                        _ => {}
                    }
                    continue;
                }
                drawing_events.push(event);
            }
            if self.move_region.is_none() && self.drawing {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                let was = self.selection.dragging;
                let focused = self.selection.handle_events(
                    &drawing_events,
                    response.rect,
                    view,
                    center,
                    turns,
                );
                if focused {
                    response.request_focus();
                }
                self.selection
                    .paint(painter, &TextPage::default(), view, center, turns);
                if (was || focused)
                    && drawing_events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::PointerButton {
                                button: egui::PointerButton::Primary,
                                pressed: false,
                                ..
                            }
                        )
                    })
                    && !self.selection.dragging
                {
                    if let Some(bounds) = self.selection.region_bounds().and_then(valid_bounds) {
                        let path = self.active_path.clone();
                        self.commit_region(
                            &path,
                            info,
                            self.selected,
                            0,
                            Region {
                                page: info.page,
                                bounds,
                                turns,
                            },
                            self.append_region,
                        );
                        self.selection.clear();
                        self.highlighted = true;
                        self.selected_regions = self.documents[&info.doc].fields[self.selected]
                            .regions_for(info.page)
                            .iter()
                            .enumerate()
                            .filter(|(_, r)| r.bounds == bounds)
                            .map(|(slot, _)| (self.selected, slot))
                            .collect();
                        self.notice="Region set. Master regions repeat; edits on inherited sheets stay local.".into();
                    } else {
                        self.confirm_selection();
                    }
                }
            } else if self.move_region.is_some() {
                ctx.set_cursor_icon(if self.move_region.as_ref().unwrap().corner.is_some() {
                    if self
                        .move_region
                        .as_ref()
                        .unwrap()
                        .corner
                        .is_some_and(|c| c == 0 || c == 2)
                    {
                        egui::CursorIcon::ResizeNwSe
                    } else {
                        egui::CursorIcon::ResizeNeSw
                    }
                } else {
                    egui::CursorIcon::Grabbing
                });
            }
        }
        if !self.drawing && !consumed && response.clicked() {
            self.confirm_selection();
        }
        if self.hide_boxes {
            return consumed;
        }
        if let Some(data) = self.documents.get(&info.doc) {
            for (index, field) in data.fields.iter().enumerate() {
                if field.scope == Scope::Once
                    && (field
                        .master_doc
                        .as_deref()
                        .is_some_and(|doc| doc != info.doc)
                        || field.region.as_ref().is_some_and(|r| r.page != info.page))
                {
                    continue;
                }
                for (slot, region) in field.regions_for(info.page).into_iter().enumerate() {
                    let region = self
                        .move_region
                        .as_ref()
                        .and_then(|d| {
                            if d.field == index && d.slot == slot {
                                Some(&d.preview)
                            } else {
                                d.companions
                                    .iter()
                                    .find(|(f, s, _, _)| *f == index && *s == slot)
                                    .map(|(_, _, _, preview)| preview)
                            }
                        })
                        .unwrap_or(region);
                    let b = region.bounds;
                    let points: Vec<_> = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]
                        .into_iter()
                        .map(to_pos)
                        .collect();
                    let local = field.overrides.contains_key(&info.page)
                        || field.page_regions.contains_key(&info.page);
                    let master = field.master_doc.as_deref() == Some(&info.doc)
                        && field.region.as_ref().is_some_and(|r| r.page == info.page);
                    let color = if self.region_selected(index, slot) {
                        egui::Color32::YELLOW
                    } else {
                        field_color(index)
                    };
                    painter.add(egui::Shape::convex_polygon(
                        points.clone(),
                        color.gamma_multiply(0.13),
                        egui::Stroke::NONE,
                    ));
                    let mut outline = points.clone();
                    outline.push(points[0]);
                    if !local && !master && field.scope == Scope::EveryPage {
                        for shape in egui::Shape::dashed_line(
                            &outline,
                            egui::Stroke::new(2.0, color),
                            6.0,
                            4.0,
                        ) {
                            painter.add(shape);
                        }
                    } else {
                        painter.add(egui::Shape::closed_line(
                            points.clone(),
                            egui::Stroke::new(2.0, color),
                        ));
                    }
                    let kind = if local {
                        "override"
                    } else if master {
                        "master"
                    } else {
                        "inherited"
                    };
                    painter.text(
                        points[0],
                        egui::Align2::LEFT_BOTTOM,
                        format!("{} · {} · {kind}", field.name, field.scope.label()),
                        egui::FontId::proportional(12.0),
                        color,
                    );
                    if self.region_selected(index, slot) && self.selected_regions.len() == 1 {
                        for p in points {
                            painter.rect_filled(
                                egui::Rect::from_center_size(p, egui::vec2(7.0, 7.0)),
                                1.0,
                                color,
                            );
                        }
                    }
                }
            }
        }
        consumed || self.drawing
    }
}
fn field_color(index: usize) -> egui::Color32 {
    let colors = [
        (52, 152, 219),
        (46, 204, 113),
        (231, 76, 60),
        (243, 156, 18),
        (26, 188, 156),
        (240, 150, 190),
        (155, 89, 182),
    ];
    let (r, g, b) = colors[index % colors.len()];
    egui::Color32::from_rgb(r, g, b)
}
fn merge_templates(data: &mut Data, templates: &[Field]) {
    for template in templates {
        if let Some(field) = data.fields.iter_mut().find(|f| f.id == template.id) {
            let scope_changed = field.scope != template.scope;
            if scope_changed {
                field.overrides.clear();
                field.page_regions.clear();
                field.excluded.clear();
            }
            let changed = scope_changed
                || (template.scope == Scope::EveryPage && field.region != template.region);
            field.name = template.name.clone();
            field.scope = template.scope;
            field.region = template.region.clone();
            field.master_doc = template.master_doc.clone();
            if changed {
                data.cells.remove(&field.id);
            }
        }
    }
}
fn valid_bounds(b: [f64; 4]) -> Option<[f64; 4]> {
    if b.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let b = b.map(|v| v.clamp(0.0, 1.0));
    (b[2] - b[0] > 0.00001 && b[3] - b[1] > 0.00001).then_some(b)
}
fn table(data: &Data, pages: i32) -> String {
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    let mut rows = vec![format!(
        "Sheet\t{}",
        data.fields
            .iter()
            .map(|f| clean(&f.name))
            .collect::<Vec<_>>()
            .join("\t")
    )];
    for page in 0..pages {
        rows.push(format!(
            "{}\t{}",
            page + 1,
            data.fields
                .iter()
                .map(|f| data
                    .cells
                    .get(&f.id)
                    .and_then(|v| v.get(&f.cell_page(page)))
                    .map(|v| clean(&v.text))
                    .unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\t")
        ));
    }
    rows.join("\n")
}
fn state_path(doc: &str) -> Result<std::path::PathBuf, String> {
    if doc.len() != 64 || !doc.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid document fingerprint".into());
    }
    let home = std::env::var_os("HOME").ok_or("Home folder unavailable")?;
    Ok(std::path::PathBuf::from(home)
        .join("Library/Application Support/iQsDrawingViewer/captures")
        .join(format!("{doc}.json")))
}
fn save(doc: &str, data: &Data) -> Result<(), String> {
    save_at(&state_path(doc)?, data)
}
fn save_at(p: &std::path::Path, data: &Data) -> Result<(), String> {
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = p.with_extension(format!("{}.json.tmp", std::process::id()));
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(data).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, p).map_err(|e| e.to_string())
}
fn load(doc: &str) -> Result<Data, String> {
    load_at(&state_path(doc)?)
}
fn load_at(path: &std::path::Path) -> Result<Data, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Saved captures exceed 16 MiB".into());
    }
    let data: Data = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let expected = Data::default();
    if data.fields.len() != expected.fields.len()
        || data
            .fields
            .iter()
            .zip(&expected.fields)
            .any(|(a, b)| a.id != b.id)
        || data.fields.iter().any(|f| {
            f.region
                .as_ref()
                .is_some_and(|r| valid_bounds(r.bounds).is_none())
                || f.overrides
                    .values()
                    .any(|r| valid_bounds(r.bounds).is_none())
                || f.page_regions
                    .values()
                    .flatten()
                    .any(|r| valid_bounds(r.bounds).is_none())
        })
    {
        return Err("Invalid saved capture fields".into());
    }
    Ok(data)
}

struct Processed {
    doc: String,
    path: String,
    pages: i32,
    once: Vec<(String, Cell)>,
}
/// Read-only export path used to verify the exact staging CSV against saved sessions.
pub fn export_workspace_csv(state: Workspace) -> Result<String, String> {
    let mut capture = Capture::default();
    capture.restore_workspace(state);
    let path = capture.active_path.clone();
    let doc = capture
        .paths
        .get(&path)
        .ok_or("No active drawing in saved session")?
        .clone();
    let pages = *capture
        .page_counts
        .get(&doc)
        .ok_or("Missing source page count")?;
    let info = PageInfo {
        doc,
        page: 0,
        pages,
        width: 1.0,
        height: 1.0,
        rotation: 0,
        crop: None,
    };
    capture.open_staging(&path, &info);
    capture
        .staging
        .as_ref()
        .ok_or("No captured fields to export")?
        .csv()
}
pub fn worker() -> Result<(), String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Capture request too large".into());
    }
    let task: Task = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let library = std::env::var_os("IQS_PDFIUM_LIBRARY")
        .map(std::path::PathBuf::from)
        .unwrap_or(
            std::env::current_exe()
                .map_err(|e| e.to_string())?
                .parent()
                .unwrap()
                .join("../Frameworks/libpdfium.dylib"),
        );
    let pdfium = Pdfium::new(Pdfium::bind_to_library(library).map_err(|e| e.to_string())?);
    let mut processed = vec![process_document(&pdfium, &task, &task.path, true)?];
    let current = std::fs::canonicalize(&task.path).map_err(|e| e.to_string())?;
    let mut seen = BTreeSet::new();
    seen.insert(current);
    for path in &task.targets {
        let canonical = std::fs::canonicalize(path).map_err(|e| e.to_string());
        if canonical.as_ref().is_ok_and(|p| seen.contains(p)) {
            continue;
        }
        if let Ok(path) = &canonical {
            seen.insert(path.clone());
        }
        match canonical.and_then(|_| process_document(&pdfium, &task, path, false)) {
            Ok(doc) => processed.push(doc),
            Err(error) => pdf_worker::write_json(
                &Reply::Warning(format!(
                    "{}: {error}",
                    std::path::Path::new(path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                )),
                &mut std::io::stdout(),
            )?,
        }
    }
    // Jobs document details are read once for the set, then shared across sheets.
    for source in &processed {
        for (field, value) in &source.once {
            for target in &processed {
                if target.doc != source.doc {
                    pdf_worker::write_json(
                        &Reply::Cell {
                            doc: target.doc.clone(),
                            path: target.path.clone(),
                            pages: target.pages,
                            field: field.clone(),
                            page: -1,
                            value: value.clone(),
                        },
                        &mut std::io::stdout(),
                    )?;
                }
            }
        }
    }
    pdf_worker::write_json(&Reply::Done, &mut std::io::stdout())
}
fn process_document(
    pdfium: &Pdfium,
    task: &Task,
    path: &str,
    primary: bool,
) -> Result<Processed, String> {
    let mut source = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(512 * 1024 * 1024 + 1)
        .read_to_end(&mut source)
        .map_err(|e| e.to_string())?;
    if source.len() > 512 * 1024 * 1024 {
        return Err("PDF exceeds 512 MiB limit".into());
    }
    let fingerprint = format!("{:x}", Sha256::digest(&source));
    if primary && fingerprint != task.fingerprint {
        return Err("PDF changed since opening; reopen it before capturing.".into());
    }
    let doc = pdfium
        .load_pdf_from_byte_vec(source, None)
        .map_err(|e| e.to_string())?;
    if (primary && task.pages != doc.pages().len()) || task.fields.len() > 32 {
        return Err("Invalid capture task".into());
    }
    let mut fields = task.fields.clone();
    if !primary {
        for field in &mut fields {
            field.excluded.clear();
            field.page_regions.clear();
            field.overrides = task
                .known_fields
                .get(&fingerprint)
                .and_then(|fields| fields.iter().find(|f| f.id == field.id))
                .map(|f| f.overrides.clone())
                .unwrap_or_default();
            if let Some(known) = task
                .known_fields
                .get(&fingerprint)
                .and_then(|fs| fs.iter().find(|f| f.id == field.id))
            {
                field.excluded = known.excluded.clone();
                field.page_regions = known.page_regions.clone();
            }
            if field.scope == Scope::Once
                && field
                    .master_doc
                    .as_deref()
                    .is_none_or(|doc| doc != fingerprint)
            {
                field.region = None;
            }
        }
    }
    for field in &mut fields {
        if field.scope == Scope::Once
            && field
                .master_doc
                .as_deref()
                .is_some_and(|doc| doc != fingerprint)
        {
            field.region = None;
        }
    }
    let mut once = Vec::new();
    // Page-major order: decode each text layer once, then discard before the next sheet.
    for index in 0..doc.pages().len() {
        let fields: Vec<_> = fields
            .iter()
            .filter(|f| {
                f.region.is_some()
                    && (f.scope == Scope::EveryPage || f.region.as_ref().unwrap().page == index)
            })
            .collect();
        if fields.is_empty() {
            continue;
        }
        let page = doc.pages().get(index).map_err(|e| e.to_string())?;
        let info = PageInfo {
            doc: fingerprint.clone(),
            page: index,
            pages: doc.pages().len(),
            width: page.width().value as f64,
            height: page.height().value as f64,
            rotation: 0,
            crop: None,
        };
        let text = if task.force_ocr {
            TextPage::default()
        } else {
            pdf_worker::extract_text(&page, &info)
        };
        for field in fields {
            let regions = field.regions_for(index);
            if regions.is_empty() {
                continue;
            }
            let mut parts = Vec::new();
            let mut methods = BTreeSet::new();
            let mut errors = Vec::new();
            let mut confidence = Vec::new();
            for region in regions {
                match recognize(&page, &info, &text, region, task.force_ocr, &field.id) {
                    Ok(value) => {
                        if !value.text.trim().is_empty() {
                            parts.push(value.text);
                        }
                        methods.insert(value.method);
                        if let Some(c) = value.confidence {
                            confidence.push(c);
                        }
                    }
                    Err(error) => errors.push(error),
                }
            }
            let clean = crate::capture_text::normalize(&field.id, &parts.join(" "));
            if field.id == "date" {
                if let Some(warning) = crate::capture_text::date_warning(&clean) {
                    errors.push(warning);
                }
            }
            let value = Cell {
                text: clean,
                method: methods.into_iter().collect::<Vec<_>>().join(" + "),
                confidence: (!confidence.is_empty())
                    .then(|| confidence.iter().sum::<f32>() / confidence.len() as f32),
                error: (!errors.is_empty()).then(|| errors.join("; ")),
                edited: false,
            };
            if field.scope == Scope::Once {
                once.push((field.id.clone(), value.clone()));
            }
            pdf_worker::write_json(
                &Reply::Cell {
                    doc: fingerprint.clone(),
                    path: path.into(),
                    pages: doc.pages().len(),
                    field: field.id.clone(),
                    page: field.cell_page(index),
                    value,
                },
                &mut std::io::stdout(),
            )?;
        }
    }
    Ok(Processed {
        doc: fingerprint,
        path: path.into(),
        pages: doc.pages().len(),
        once,
    })
}
fn recognize(
    page: &PdfPage,
    info: &PageInfo,
    text: &TextPage,
    region: &Region,
    force: bool,
    field_id: &str,
) -> Result<Cell, String> {
    let b = valid_bounds(region.bounds).ok_or("Invalid capture box")?;
    if !force {
        let selection = Selection {
            anchor: Some([b[0], b[1]]),
            end: Some([b[2], b[3]]),
            ..Default::default()
        };
        let value = selection.copy_text(text);
        if !value.trim().is_empty()
            && text.error.is_none()
            && !text.truncated
            && !crate::capture_text::verify_visible(field_id, &value)
        {
            return Ok(Cell {
                text: value,
                method: "PDF text".into(),
                ..Default::default()
            });
        }
    }
    let crop_w = (b[2] - b[0]) * info.width;
    let crop_h = (b[3] - b[1]) * info.height;
    let scale = 4.0_f64.min(2048.0 / crop_w.max(crop_h));
    let w = (info.width * scale).round().max(1.0) as i32;
    let h = (info.height * scale).round().max(1.0) as i32;
    let x = (b[0] * w as f64).floor() as i32;
    let y = (b[1] * h as f64).floor() as i32;
    let cw = ((b[2] * w as f64).ceil() as i32 - x).clamp(1, 2048);
    let ch = ((b[3] * h as f64).ceil() as i32 - y).clamp(1, 2048);
    let mut bitmap = PdfBitmap::empty(cw, ch, PdfBitmapFormat::BGRA).map_err(|e| e.to_string())?;
    page.render_into_bitmap_with_config(
        &mut bitmap,
        &PdfRenderConfig::new()
            .set_fixed_size(w, h)
            .set_origin(-x, -y),
    )
    .map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = std::env::var_os("IQS_OCR_HELPER")
        .map(std::path::PathBuf::from)
        .unwrap_or(exe.parent().unwrap().join("iqs-ocr"));
    let mut child = Command::new(helper)
        .args([cw.to_string(), ch.to_string(), region.turns.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Local OCR helper unavailable: {e}"))?;
    if let Err(e) = child
        .stdin
        .take()
        .unwrap()
        .write_all(&bitmap.as_rgba_bytes())
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e.to_string());
    }
    let started = Instant::now();
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Local OCR timed out".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Local OCR failed for this region".into());
    }
    #[derive(Deserialize)]
    struct Recognized {
        text: String,
        confidence: f32,
    }
    let result: Recognized = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    Ok(Cell {
        text: result.text,
        method: "Local OCR".into(),
        confidence: Some(result.confidence),
        error: None,
        edited: false,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn region_info(doc: &str, page: i32) -> PageInfo {
        PageInfo {
            doc: doc.into(),
            page,
            pages: 3,
            width: 100.0,
            height: 100.0,
            rotation: 0,
            crop: None,
        }
    }
    fn region_event(x: f32, y: f32, pressed: bool, shift: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::pos2(x, y),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers {
                shift,
                ..Default::default()
            },
        }
    }
    fn region_frame(
        ctx: &egui::Context,
        capture: &mut Capture,
        info: &PageInfo,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 300.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (response, painter) = ui
                        .allocate_painter(egui::vec2(200.0, 200.0), egui::Sense::click_and_drag());
                    capture.interact(
                        &response,
                        &painter,
                        crate::model::View {
                            origin: [0.0, 0.0],
                            scale: 1.0,
                            page: [100.0, 100.0],
                        },
                        [100.0, 100.0],
                        0,
                        info,
                    );
                });
            },
        )
    }
    fn region_key(key: egui::Key, command: bool, shift: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command,
                mac_cmd: command,
                shift,
                ..Default::default()
            },
        }
    }
    #[test]
    fn shift_click_group_drag_clamps_together_and_undoes_in_one_step() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            drawing: true,
            ..Default::default()
        };
        let master = region_info("master", 0);
        capture.activate_document("/set/a.pdf", &master);
        for (field, bounds, append) in [
            (2, [0.1, 0.1, 0.2, 0.2], false),
            (2, [0.4, 0.1, 0.5, 0.2], true),
            (3, [0.7, 0.1, 0.8, 0.2], false),
        ] {
            capture.commit_region(
                "/set/a.pdf",
                &master,
                field,
                0,
                Region {
                    page: 0,
                    bounds,
                    turns: 0,
                },
                append,
            );
        }
        // A later sheet inherits the masters; its extra box remains local.
        let info = region_info("master", 1);
        capture.commit_region(
            "/set/a.pdf",
            &info,
            2,
            0,
            Region {
                page: 1,
                bounds: [0.4, 0.1, 0.5, 0.2],
                turns: 0,
            },
            true,
        );
        for x in [15.0, 45.0, 75.0] {
            region_frame(
                &ctx,
                &mut capture,
                &info,
                vec![
                    region_event(x, 15.0, true, true),
                    region_event(x, 15.0, false, true),
                ],
            );
        }
        assert_eq!(capture.selected_regions.len(), 3);
        // Shift-click toggles a selected box without moving or drawing anything.
        for _ in 0..2 {
            region_frame(
                &ctx,
                &mut capture,
                &info,
                vec![
                    region_event(45.0, 15.0, true, true),
                    region_event(45.0, 15.0, false, true),
                ],
            );
        }
        assert_eq!(capture.selected_regions.len(), 3);
        let before = capture.documents["master"].fields.clone();
        let undo = capture.undo.len();
        region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![
                region_event(15.0, 15.0, true, false),
                egui::Event::PointerMoved(egui::pos2(95.0, 15.0)),
                region_event(95.0, 15.0, false, false),
            ],
        );
        assert_eq!(capture.undo.len(), undo + 1);
        let a = capture.documents["master"].fields[2].regions_for(1);
        assert!((a[0].bounds[0] - 0.3).abs() < 1e-8);
        assert!((a[1].bounds[0] - 0.6).abs() < 1e-8);
        assert!(
            (capture.documents["master"].fields[3].regions_for(1)[0].bounds[2] - 1.0).abs() < 1e-8
        );
        assert_eq!(
            capture.documents["master"].fields[3].region,
            before[3].region
        );
        assert_eq!(capture.selected_regions.len(), 3);
        capture.undo_regions();
        assert_eq!(
            serde_json::to_value(&capture.documents["master"].fields).unwrap(),
            serde_json::to_value(before).unwrap()
        );
    }
    #[test]
    fn region_clipboard_and_delete_keep_other_boxes_and_other_sheets() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            drawing: true,
            ..Default::default()
        };
        let info = region_info("test", 0);
        capture.activate_document("/test.pdf", &info);
        capture.commit_region(
            "/test.pdf",
            &info,
            3,
            0,
            Region {
                page: 0,
                bounds: [0.1, 0.1, 0.3, 0.3],
                turns: 0,
            },
            false,
        );
        region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![
                region_event(20.0, 20.0, true, false),
                region_event(20.0, 20.0, false, false),
            ],
        );
        let output = region_frame(&ctx, &mut capture, &info, vec![egui::Event::Copy]);
        let clipboard = output
            .platform_output
            .commands
            .into_iter()
            .find_map(|c| {
                if let egui::OutputCommand::CopyText(s) = c {
                    Some(s)
                } else {
                    None
                }
            })
            .unwrap();
        region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![egui::Event::Paste(clipboard.clone())],
        );
        assert_eq!(capture.documents["test"].fields[3].regions_for(0).len(), 2);
        assert_eq!(capture.selected_regions.len(), 1);
        region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![region_key(egui::Key::Backspace, false, false)],
        );
        let remaining = capture.documents["test"].fields[3].regions_for(0);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].bounds, [0.1, 0.1, 0.3, 0.3]);
        assert_eq!(capture.documents["test"].fields[3].regions_for(1).len(), 1);
        capture.undo_regions();
        assert_eq!(capture.documents["test"].fields[3].regions_for(0).len(), 2);
        // Cross-sheet paste retains exact coordinates and the field identity.
        let later = region_info("test", 2);
        capture.confirm_selection();
        region_frame(
            &ctx,
            &mut capture,
            &later,
            vec![egui::Event::Paste(clipboard)],
        );
        assert!(capture.documents["test"].fields[3]
            .page_regions
            .contains_key(&2));
        assert_eq!(capture.documents["test"].fields[3].regions_for(2).len(), 2);
        assert_eq!(capture.documents["test"].fields[2].regions_for(2).len(), 0);
    }
    #[test]
    fn deleting_last_box_then_pasting_does_not_resurrect_deleted_boxes() {
        let mut capture = Capture {
            open: true,
            drawing: true,
            ..Default::default()
        };
        let info = region_info("test", 0);
        capture.activate_document("/test.pdf", &info);
        capture.commit_region(
            "/test.pdf",
            &info,
            3,
            0,
            Region {
                page: 0,
                bounds: [0.1, 0.1, 0.3, 0.3],
                turns: 0,
            },
            false,
        );
        capture.selected_regions.insert((3, 0));
        capture.highlighted = true;
        let clipboard = capture.copy_regions(&info).unwrap();
        capture.delete_selected(&info);
        assert!(capture.documents["test"].fields[3]
            .regions_for(0)
            .is_empty());
        capture.paste_regions(&info, &clipboard);
        assert_eq!(capture.documents["test"].fields[3].regions_for(0).len(), 1);
        let before = serde_json::to_value(&capture.documents).unwrap();
        capture.paste_regions(&info,"iQs regions v1\n[\"x\",0,[[\"description\",{\"page\":0,\"bounds\":[0,0,2,1],\"turns\":0}]]]");
        assert_eq!(serde_json::to_value(&capture.documents).unwrap(), before);
    }
    #[test]
    fn visible_regions_select_and_delete_without_arming_a_field() {
        for append_region in [false, true] {
            let ctx = egui::Context::default();
            let mut capture = Capture {
                open: true,
                append_region,
                ..Default::default()
            };
            let master = region_info("test", 0);
            capture.activate_document("/test.pdf", &master);
            capture.commit_region(
                "/test.pdf",
                &master,
                3,
                0,
                Region {
                    page: 0,
                    bounds: [0.2, 0.2, 0.4, 0.4],
                    turns: 0,
                },
                false,
            );
            let info = region_info("test", 1);
            capture.append_region = append_region;
            assert!(!capture.drawing);
            // Include the border tolerance: small boxes remain easy to select.
            region_frame(
                &ctx,
                &mut capture,
                &info,
                vec![
                    region_event(17.0, 30.0, true, false),
                    region_event(17.0, 30.0, false, false),
                ],
            );
            assert!(capture.has_selection());
            assert!(capture.region_selected(3, 0));
            assert!(!capture.drawing);
            region_frame(
                &ctx,
                &mut capture,
                &info,
                vec![region_key(egui::Key::Delete, false, false)],
            );
            assert!(capture.documents["test"].fields[3]
                .regions_for(1)
                .is_empty());
            assert_eq!(capture.documents["test"].fields[3].regions_for(0).len(), 1);
            assert!(!capture.has_selection());
            capture.undo_regions();
            assert_eq!(capture.documents["test"].fields[3].regions_for(1).len(), 1);
        }
    }
    #[test]
    fn selecting_in_pan_mode_does_not_arm_blank_space_drawing() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            ..Default::default()
        };
        let info = region_info("test", 0);
        capture.activate_document("/test.pdf", &info);
        capture.commit_region(
            "/test.pdf",
            &info,
            3,
            0,
            Region {
                page: 0,
                bounds: [0.2, 0.2, 0.4, 0.4],
                turns: 0,
            },
            false,
        );
        let before = capture.undo.len();
        region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![
                region_event(30.0, 30.0, true, false),
                egui::Event::PointerMoved(egui::pos2(40.0, 40.0)),
                region_event(40.0, 40.0, false, false),
            ],
        );
        assert_eq!(capture.undo.len(), before + 1);
        assert!(!capture.drawing);
        let before = serde_json::to_value(&capture.documents).unwrap();
        region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![
                region_event(65.0, 65.0, true, false),
                egui::Event::PointerMoved(egui::pos2(90.0, 90.0)),
                region_event(90.0, 90.0, false, false),
            ],
        );
        assert_eq!(serde_json::to_value(&capture.documents).unwrap(), before);
        assert!(!capture.drawing);
    }
    #[test]
    fn closing_capture_fields_blocks_selection_delete_and_drag_without_changing_extraction() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            ..Default::default()
        };
        let info = region_info("test", 0);
        capture.activate_document("/test.pdf", &info);
        for index in [2, 3, 4] {
            capture.commit_region(
                "/test.pdf",
                &info,
                index,
                0,
                Region {
                    page: 0,
                    bounds: [0.2, 0.2, 0.4, 0.4],
                    turns: 0,
                },
                false,
            );
        }
        region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![
                region_event(30.0, 30.0, true, false),
                region_event(30.0, 30.0, false, false),
            ],
        );
        assert!(capture.has_selection());
        let before = serde_json::to_value(&capture.documents).unwrap();
        capture.set_open(false);
        assert!(!capture.has_selection());
        let output = region_frame(
            &ctx,
            &mut capture,
            &info,
            vec![
                region_event(30.0, 30.0, true, false),
                egui::Event::PointerMoved(egui::pos2(50.0, 50.0)),
                region_event(50.0, 50.0, false, false),
                region_key(egui::Key::Delete, false, false),
            ],
        );
        assert!(!output.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text.contains("Every page"))
        ));
        assert_eq!(serde_json::to_value(&capture.documents).unwrap(), before);
        for index in [2, 3, 4] {
            for page in 0..3 {
                assert_eq!(
                    capture.documents["test"].fields[index]
                        .regions_for(page)
                        .len(),
                    1
                );
            }
        }
    }
    #[test]
    fn restoring_written_dates_updates_values_and_only_clears_obsolete_validation_errors() {
        let mut original = Capture::default();
        let mut data = Data::default();
        data.cells.entry("date".into()).or_default().insert(
            0,
            Cell {
                text: "21 July 2026".into(),
                error: Some(
                    "Date is ambiguous or invalid; review the visible drawing (DD/MM/YYYY).".into(),
                ),
                ..Default::default()
            },
        );
        data.cells.get_mut("date").unwrap().insert(
            1,
            Cell {
                text: "22 July 2026".into(),
                error: Some("Local OCR failed for this region".into()),
                ..Default::default()
            },
        );
        original.documents.insert("test".into(), data);
        let mut restored = Capture::default();
        restored.restore_workspace(original.workspace_snapshot());
        let dates = &restored.documents["test"].cells["date"];
        assert_eq!(dates[&0].text, "21/07/2026");
        assert!(dates[&0].error.is_none());
        assert_eq!(dates[&1].text, "22/07/2026");
        assert!(dates[&1].error.is_some());
    }
    #[test]
    fn select_text_ocr_is_transient_single_page_and_keeps_rotation() {
        let info = region_info("test", 2);
        let task = TextRecognition::task("/test.pdf", &info, [0.1, 0.2, 0.4, 0.5], 3).unwrap();
        assert!(task.force_ocr);
        assert!(task.targets.is_empty());
        assert_eq!(task.fields.len(), 1);
        assert_eq!(task.fields[0].scope, Scope::Once);
        assert_eq!(
            task.fields[0].region,
            Some(Region {
                page: 2,
                bounds: [0.1, 0.2, 0.4, 0.5],
                turns: 3
            })
        );
        assert!(TextRecognition::task("/test.pdf", &info, [0.1, 0.2, 0.1, 0.2], 0).is_err());
    }

    #[test]
    fn workspace_restores_set_masters_overrides_and_review_with_latest_save_winning() {
        let mut capture = Capture::default();
        let path = "/set/one.pdf";
        let info = PageInfo {
            doc: "source".into(),
            page: 0,
            pages: 2,
            width: 100.0,
            height: 100.0,
            rotation: 0,
            crop: None,
        };
        capture.set_paths.insert(path.into());
        capture.activate_document(path, &info);
        capture.commit_region(
            path,
            &info,
            2,
            0,
            Region {
                page: 0,
                bounds: [0.1, 0.1, 0.2, 0.2],
                turns: 0,
            },
            false,
        );
        capture.documents.get_mut("source").unwrap().fields[2]
            .excluded
            .insert(1);
        capture.open_staging(path, &info);
        capture.staging.as_mut().unwrap().rows[0].filename = "reviewed.pdf".into();
        capture.staging.as_mut().unwrap().rows[0].filename_override = true;
        let folder =
            std::env::temp_dir().join(format!("iqs-workspace-test-{}", std::process::id()));
        let file = folder.join("workspace.json");
        let saver = crate::workspace::Saver::new(file.clone());
        saver.submit(capture.workspace_snapshot());
        capture.staging.as_mut().unwrap().export_name = "Latest reviewed set".into();
        saver.finish(capture.workspace_snapshot()).unwrap();
        let mut restored = Capture::default();
        restored.restore_workspace(crate::workspace::load(&file).unwrap().unwrap());
        assert!(restored.set_paths.contains(path));
        assert!(restored.documents["source"].fields[2].excluded.contains(&1));
        assert!(restored.templates["selected-drawing-set"][2]
            .region
            .is_some());
        assert_eq!(
            restored.staging.as_ref().unwrap().export_name,
            "Latest reviewed set"
        );
        assert!(!restored.reviewing());
        restored.open_staging(path, &info);
        assert_eq!(
            restored.staging.as_ref().unwrap().rows[0].filename,
            "reviewed.pdf"
        );
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn partial_navigation_cannot_erase_masters_and_inherited_edits_stay_local() {
        let mut capture = Capture::default();
        let a = "/set/a.pdf";
        let b = "/set/b.pdf";
        let c = "/set/c.pdf";
        capture.set_paths.extend([a.into(), b.into(), c.into()]);
        let info = |doc: &str| PageInfo {
            doc: doc.into(),
            page: 0,
            pages: 1,
            width: 100.0,
            height: 100.0,
            rotation: 0,
            crop: None,
        };
        let region = |x| Region {
            page: 0,
            bounds: [x, 0.1, x + 0.1, 0.2],
            turns: 0,
        };
        capture.activate_document(a, &info("a"));
        capture.commit_region(a, &info("a"), 2, 0, region(0.1), false);
        capture.activate_document(b, &info("b"));
        capture.commit_region(b, &info("b"), 3, 0, region(0.4), false);
        capture.commit_region(b, &info("b"), 2, 0, region(0.2), false);
        capture.remember_document(b, &info("b"));
        capture.activate_document(a, &info("a"));
        assert_eq!(
            capture.documents["a"].fields[2]
                .region
                .as_ref()
                .unwrap()
                .bounds,
            region(0.1).bounds
        );
        assert!(
            capture.documents["a"].fields[3].region.is_some(),
            "later field was lost on earlier sheet"
        );
        capture.commit_region(a, &info("a"), 2, 0, region(0.3), false);
        capture.activate_document(c, &info("c"));
        assert_eq!(
            capture.documents["c"].fields[2]
                .region
                .as_ref()
                .unwrap()
                .bounds,
            region(0.3).bounds
        );
        capture.activate_document(b, &info("b"));
        assert_eq!(
            capture.documents["b"].fields[2]
                .region_for(0)
                .unwrap()
                .bounds,
            region(0.2).bounds
        );
        capture.documents.get_mut("b").unwrap().fields[2]
            .excluded
            .insert(0);
        capture.remember_document(b, &info("b"));
        capture.activate_document(c, &info("c"));
        assert!(!capture.documents["c"].fields[2].regions_for(0).is_empty());
        capture.activate_document(b, &info("b"));
        assert!(capture.documents["b"].fields[2].regions_for(0).is_empty());
        capture.undo_regions();
        capture.redo_regions();
        assert_eq!(
            capture.documents["c"].fields[2]
                .region
                .as_ref()
                .unwrap()
                .bounds,
            region(0.3).bounds
        );
    }
    #[test]
    fn pointer_move_of_inherited_region_creates_local_override() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            drawing: true,
            selected: 2,
            ..Default::default()
        };
        let a = "/set/a.pdf";
        let b = "/set/b.pdf";
        capture.set_paths.extend([a.into(), b.into()]);
        let info = |doc: &str| PageInfo {
            doc: doc.into(),
            page: 0,
            pages: 1,
            width: 100.0,
            height: 100.0,
            rotation: 0,
            crop: None,
        };
        capture.activate_document(a, &info("a"));
        capture.commit_region(
            a,
            &info("a"),
            2,
            0,
            Region {
                page: 0,
                bounds: [0.1, 0.1, 0.4, 0.4],
                turns: 0,
            },
            false,
        );
        capture.activate_document(b, &info("b"));
        let pos = egui::pos2(25.0, 25.0);
        let end = egui::pos2(35.0, 35.0);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 300.0),
            )),
            events: vec![
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerMoved(end),
                egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(egui::vec2(200.0, 200.0), egui::Sense::click_and_drag());
                capture.interact(
                    &response,
                    &painter,
                    crate::model::View {
                        origin: [0.0, 0.0],
                        scale: 1.0,
                        page: [100.0, 100.0],
                    },
                    [100.0, 100.0],
                    0,
                    &info("b"),
                );
            });
        });
        let bounds = capture.documents["b"].fields[2].overrides[&0].bounds;
        assert!((bounds[0] - 0.2).abs() < 1e-8);
        assert_eq!(
            capture.documents["a"].fields[2]
                .region
                .as_ref()
                .unwrap()
                .bounds,
            [0.1, 0.1, 0.4, 0.4]
        );
    }
    #[test]
    fn clicking_field_arms_drawing_without_another_button() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            ..Default::default()
        };
        let info = PageInfo {
            doc: "test".into(),
            page: 0,
            pages: 3,
            width: 100.0,
            height: 100.0,
            rotation: 0,
            crop: None,
        };
        let frame = |capture: &mut Capture, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 850.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    capture.panel(ctx, "/tmp/test.pdf", Some(&info));
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.label("Drawing");
                    });
                },
            )
        };
        let output = frame(&mut capture, vec![]);
        let center = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.job.text == "○ Description" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("field button visible");
        let button = |pressed| egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let _ = frame(
            &mut capture,
            vec![egui::Event::PointerMoved(center), button(true)],
        );
        let _ = frame(&mut capture, vec![button(false)]);
        assert_eq!(capture.selected, 3);
        assert!(capture.drawing);
        assert!(!capture.hide_boxes);
    }
    #[test]
    fn explicit_set_templates_follow_members_and_exclude_neighbours() {
        let mut capture = Capture::default();
        let info = |doc: &str| PageInfo {
            doc: doc.into(),
            page: 0,
            pages: 1,
            width: 100.0,
            height: 100.0,
            rotation: 0,
            crop: None,
        };
        let first = "/test/set/first.pdf";
        let second = "/test/set/second.pdf";
        let excluded = "/test/set/other.pdf";
        capture.set_paths.extend([first.into(), second.into()]);
        capture.activate_document(first, &info("first"));
        let data = capture.documents.get_mut("first").unwrap();
        data.fields[2].name = "Drawing number".into();
        data.fields[2].region = Some(Region {
            page: 0,
            bounds: [0.1, 0.2, 0.3, 0.4],
            turns: 0,
        });
        data.fields[0].region = Some(Region {
            page: 0,
            bounds: [0.5, 0.6, 0.7, 0.8],
            turns: 0,
        });
        capture.remember_document(first, &info("first"));
        capture.document_changed(true);
        capture.activate_document(second, &info("second"));
        assert_eq!(
            capture.documents["second"].fields[2].region,
            capture.documents["first"].fields[2].region
        );
        assert_eq!(capture.documents["second"].fields[2].name, "Drawing number");
        assert_eq!(
            capture.documents["second"].fields[0].master_doc.as_deref(),
            Some("first")
        );
        capture
            .documents
            .get_mut("second")
            .unwrap()
            .cells
            .entry("sheet_id".into())
            .or_default()
            .insert(
                0,
                Cell {
                    text: "A102".into(),
                    ..Default::default()
                },
            );
        capture.activate_document(second, &info("second"));
        assert_eq!(
            capture.documents["second"].cells["sheet_id"][&0].text,
            "A102"
        );
        let copied = capture.copy_table(first, &info("first"));
        assert!(copied.starts_with("File\tSheet\t"));
        assert!(copied.contains("second.pdf\t1"));
        assert!(!copied.contains("other.pdf"));
        capture.activate_document(excluded, &info("excluded"));
        assert!(capture.documents["excluded"].fields[2].region.is_none());
        capture.set_paths.insert(excluded.into());
        capture.remember_document(excluded, &info("excluded"));
        assert!(capture.documents["excluded"].fields[2].region.is_some());
    }
    #[test]
    fn repeating_boxes_only_render_while_capture_fields_is_open() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            ..Default::default()
        };
        let mut data = Data::default();
        let region = Region {
            page: 0,
            bounds: [0.1, 0.2, 0.4, 0.5],
            turns: 0,
        };
        data.fields[0].region = Some(region.clone()); // Once: source sheet only.
        data.fields[2].region = Some(region);
        data.fields[2].overrides.insert(
            2,
            Region {
                page: 2,
                bounds: [0.5, 0.5, 0.7, 0.8],
                turns: 0,
            },
        );
        capture.documents.insert("test".into(), data);
        let render = |capture: &mut Capture, page, hidden| {
            capture.hide_boxes = hidden;
            let info = PageInfo {
                doc: "test".into(),
                page,
                pages: 3,
                width: 100.0,
                height: 100.0,
                rotation: 0,
                crop: None,
            };
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(300.0, 300.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let (response, painter) =
                            ui.allocate_painter(egui::vec2(200.0, 200.0), egui::Sense::hover());
                        capture.interact(
                            &response,
                            &painter,
                            crate::model::View {
                                origin: [0.0, 0.0],
                                scale: 1.0,
                                page: [100.0, 100.0],
                            },
                            [100.0, 100.0],
                            0,
                            &info,
                        );
                    });
                },
            );
            output
                .shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let first = render(&mut capture, 0, false);
        assert!(first.iter().any(|s| s.starts_with("Prepared by · Once")));
        assert!(first.iter().any(|s| s.starts_with("Sheet ID · Every page")));
        let next = render(&mut capture, 1, false);
        assert!(!next.iter().any(|s| s.contains("Prepared by")));
        assert!(next.iter().any(|s| s.starts_with("Sheet ID · Every page")));
        let override_page = render(&mut capture, 2, false);
        assert!(override_page
            .iter()
            .any(|s| s == "Sheet ID · Every page · override"));
        assert!(render(&mut capture, 2, true).is_empty());
        let before = serde_json::to_value(&capture.documents).unwrap();
        capture.set_open(false);
        assert!(render(&mut capture, 0, false).is_empty());
        assert!(render(&mut capture, 1, false).is_empty());
        assert_eq!(serde_json::to_value(&capture.documents).unwrap(), before);
        capture.set_open(true);
        assert!(render(&mut capture, 1, false)
            .iter()
            .any(|s| s.starts_with("Sheet ID · Every page")));
    }
    #[test]
    fn saved_captures_keep_names_boxes_and_edits() {
        let path = std::env::temp_dir().join(format!(
            "iqs-capture-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut data = Data::default();
        data.fields[0].name = "Author".into();
        data.fields[0].region = Some(Region {
            page: 2,
            bounds: [0.1, 0.2, 0.3, 0.4],
            turns: 1,
        });
        data.cells.entry("prepared_by".into()).or_default().insert(
            -1,
            Cell {
                text: "Corrected author".into(),
                edited: true,
                ..Default::default()
            },
        );
        save_at(&path, &data).unwrap();
        let loaded = load_at(&path).unwrap();
        assert_eq!(loaded.fields[0].name, "Author");
        assert_eq!(loaded.fields[0].region.as_ref().unwrap().page, 2);
        assert!(loaded.cells["prepared_by"][&-1].edited);
        data.fields[0].id = "unexpected_id".into();
        save_at(&path, &data).unwrap();
        assert!(load_at(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn fast_capture_drag_commits_box() {
        let ctx = egui::Context::default();
        let mut capture = Capture {
            open: true,
            drawing: true,
            selected: 2,
            ..Default::default()
        };
        let pos = egui::pos2(20.0, 20.0);
        let end = egui::pos2(40.0, 40.0);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 300.0),
            )),
            events: vec![
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerMoved(end),
                egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        let info = PageInfo {
            doc: "test".into(),
            page: 0,
            pages: 3,
            width: 100.0,
            height: 100.0,
            rotation: 0,
            crop: None,
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(egui::vec2(200.0, 200.0), egui::Sense::click_and_drag());
                capture.interact(
                    &response,
                    &painter,
                    crate::model::View {
                        origin: [0.0, 0.0],
                        scale: 1.0,
                        page: [100.0, 100.0],
                    },
                    [100.0, 100.0],
                    0,
                    &info,
                );
            });
        });
        assert_eq!(
            capture.documents["test"].fields[2]
                .region
                .as_ref()
                .unwrap()
                .bounds,
            [0.2, 0.2, 0.4, 0.4]
        );
        assert!(capture.drawing);
        assert!(capture.highlighted);

        // A blank click confirms the existing box without replacing it or
        // disarming the field for the next drag.
        let blank = egui::pos2(80.0, 80.0);
        let _ = ctx.run(
            egui::RawInput {
                events: vec![true, false]
                    .into_iter()
                    .map(|pressed| egui::Event::PointerButton {
                        pos: blank,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    })
                    .collect(),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (response, painter) = ui
                        .allocate_painter(egui::vec2(200.0, 200.0), egui::Sense::click_and_drag());
                    capture.interact(
                        &response,
                        &painter,
                        crate::model::View {
                            origin: [0.0, 0.0],
                            scale: 1.0,
                            page: [100.0, 100.0],
                        },
                        [100.0, 100.0],
                        0,
                        &info,
                    );
                });
            },
        );
        assert!(!capture.highlighted);
        assert!(capture.drawing);
        assert_eq!(
            capture.documents["test"].fields[2]
                .region
                .as_ref()
                .unwrap()
                .bounds,
            [0.2, 0.2, 0.4, 0.4]
        );
        capture.highlighted = true;
        capture.confirm_selection();
        assert!(!capture.highlighted);
        assert!(capture.drawing);
    }
    #[test]
    fn scopes_overrides_and_renames_preserve_identity() {
        let mut data = Data::default();
        let field = &mut data.fields[2];
        field.name = "Name".into();
        field.region = Some(Region {
            page: 0,
            bounds: [0.1, 0.1, 0.2, 0.2],
            turns: 0,
        });
        field.overrides.insert(
            2,
            Region {
                page: 2,
                bounds: [0.3, 0.3, 0.4, 0.4],
                turns: 1,
            },
        );
        assert_eq!(field.id, "sheet_id");
        assert_eq!(field.region_for(1).unwrap().page, 0);
        assert_eq!(field.region_for(2).unwrap().page, 2);
        assert_eq!(field.cell_page(2), 2);
        assert_eq!(data.fields[0].cell_page(2), -1);
        let decoded: Data = serde_json::from_str(&serde_json::to_string(&data).unwrap()).unwrap();
        assert_eq!(decoded.fields[2].name, "Name");
    }
    #[test]
    fn table_repeats_document_values_and_uses_renamed_headers() {
        let mut d = Data::default();
        d.fields[0].name = "Author".into();
        d.cells.entry("prepared_by".into()).or_default().insert(
            -1,
            Cell {
                text: "Test\tAuthor".into(),
                ..Default::default()
            },
        );
        d.cells.entry("sheet_id".into()).or_default().insert(
            1,
            Cell {
                text: "A102".into(),
                ..Default::default()
            },
        );
        let t = table(&d, 2);
        assert!(t.starts_with("Sheet\tAuthor\t"));
        assert!(t.contains("1\tTest Author\t\t\t"));
        assert!(t.contains("2\tTest Author\t\tA102"));
        assert!(valid_bounds([f64::NAN, 0.0, 1.0, 1.0]).is_none());
    }
}

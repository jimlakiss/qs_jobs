//! Review a fixed capture snapshot before export. Source PDFs are never renamed.
use crate::{
    capture::{Cell, Data, Field, Scope},
    viewer,
};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::mpsc,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: String,
    pub name: String,
    pub scope: Scope,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Row {
    pub path: String,
    pub doc: Option<String>,
    pub page: i32,
    #[serde(default)]
    pub global_page: i32,
    pub included: bool,
    pub filename: String,
    pub filename_override: bool,
    pub values: BTreeMap<String, String>,
    pub required: BTreeSet<String>,
    pub warnings: BTreeMap<String, String>,
    pub dirty: BTreeSet<String>,
}
impl Row {
    fn status(&self) -> &'static str {
        if self.doc.is_none() {
            return "Not captured";
        }
        let value = |id: &str| self.values.get(id).map(|s| s.trim()).unwrap_or("");
        if ["sheet_id", "description", "issue_id"]
            .iter()
            .all(|id| value(id).is_empty())
        {
            "OCR failed"
        } else if value("sheet_id").is_empty()
            || value("description").is_empty()
            || self.required.iter().any(|id| value(id).is_empty())
            || crate::capture_text::date_warning(value("date")).is_some()
            || !self.warnings.is_empty()
        {
            "Review"
        } else {
            "Ready"
        }
    }
    fn status_id(&self) -> &'static str {
        match self.status() {
            "Ready" => "ready",
            "Review" => "review",
            _ => "failed",
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct StagingState {
    columns: Vec<Column>,
    rows: Vec<Row>,
    export_name: String,
    template: String,
    filter: u8,
    sequence: bool,
    separator: String,
    widths: BTreeMap<String, f32>,
    show_all_fields: bool,
    original_order: Vec<(String, i32)>,
}
pub struct Staging {
    pub open: bool,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub export_name: String,
    template: String,
    filter: u8,
    sequence: bool,
    separator: String,
    widths: BTreeMap<String, f32>,
    show_all_fields: bool,
    original_order: Vec<(String, i32)>,
    drag_from: Option<usize>,
    drop_target: Option<(usize, bool)>,
    row_handles: Vec<(usize, egui::Rect)>,
    notice: String,
    saving: Option<mpsc::Receiver<Result<Option<String>, String>>>,
}
impl Staging {
    pub fn snapshot(&self) -> StagingState {
        StagingState {
            columns: self.columns.clone(),
            rows: self.rows.clone(),
            export_name: self.export_name.clone(),
            template: self.template.clone(),
            filter: self.filter,
            sequence: self.sequence,
            separator: self.separator.clone(),
            widths: self.widths.clone(),
            show_all_fields: self.show_all_fields,
            original_order: self.original_order.clone(),
        }
    }
    pub fn restore(state: StagingState) -> Self {
        Self {
            open: false,
            columns: state.columns,
            rows: state.rows,
            export_name: state.export_name,
            template: state.template,
            filter: state.filter.min(2),
            sequence: state.sequence,
            separator: state.separator,
            widths: state.widths,
            show_all_fields: state.show_all_fields,
            original_order: state.original_order,
            drag_from: None,
            drop_target: None,
            row_handles: Vec::new(),
            notice: "Saved review restored.".into(),
            saving: None,
        }
    }
    pub fn new(
        fields: &[Field],
        sources: Vec<(String, Option<String>, i32, Option<&Data>)>,
        name: String,
    ) -> Self {
        let columns = fields
            .iter()
            .map(|f| Column {
                id: f.id.clone(),
                name: f.name.clone(),
                scope: f.scope,
            })
            .collect();
        let mut rows = Vec::new();
        for (path, doc, pages, data) in sources {
            for page in 0..pages.max(1) {
                let mut row = Row {
                    path: path.clone(),
                    doc: doc.clone(),
                    page,
                    global_page: rows.len() as i32,
                    included: doc.is_some(),
                    filename: String::new(),
                    filename_override: false,
                    values: BTreeMap::new(),
                    required: BTreeSet::new(),
                    warnings: BTreeMap::new(),
                    dirty: BTreeSet::new(),
                };
                for field in fields {
                    let source_field = data
                        .and_then(|d| d.fields.iter().find(|f| f.id == field.id))
                        .unwrap_or(field);
                    let key = if source_field.scope == Scope::Once {
                        -1
                    } else {
                        page
                    };
                    let cell = data
                        .and_then(|d| d.cells.get(&field.id))
                        .and_then(|c| c.get(&key));
                    row.values.insert(
                        field.id.clone(),
                        if field.id == "sheet_id" {
                            normalize_sheet_id(cell.map(|c| c.text.as_str()).unwrap_or(""))
                        } else {
                            crate::capture_text::normalize(
                                &field.id,
                                cell.map(|c| c.text.as_str()).unwrap_or(""),
                            )
                        },
                    );
                    if source_field.region.is_some() {
                        row.required.insert(field.id.clone());
                    }
                    if let Some(error) = cell.and_then(|c| c.error.as_ref()) {
                        let now_valid_date = field.id == "date"
                            && !row.values[&field.id].is_empty()
                            && crate::capture_text::date_warning(&row.values[&field.id]).is_none()
                            && error == "Date is ambiguous or invalid; review the visible drawing (DD/MM/YYYY).";
                        if !now_valid_date {
                            row.warnings.insert(field.id.clone(), error.clone());
                        }
                    }
                }
                rows.push(row);
            }
        }
        let mut stage = Self {
            open: true,
            columns,
            rows: rows.clone(),
            export_name: name,
            template: "{sheet_id} - {description} - {issue_id}".into(),
            filter: 0,
            sequence: true,
            separator: " - ".into(),
            widths: BTreeMap::new(),
            show_all_fields: false,
            original_order: rows.iter().map(|r| (r.path.clone(), r.page)).collect(),
            drag_from: None,
            drop_target: None,
            row_handles: Vec::new(),
            notice: "Review values, filenames and included rows, then export.".into(),
            saving: None,
        };
        stage.generate_filenames();
        stage
    }
    pub fn preserve_layout(&mut self, previous: &Self) {
        let old: BTreeMap<_, _> = previous
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| ((row.path.as_str(), row.page), (i, row)))
            .collect();
        if self
            .rows
            .iter()
            .all(|row| old.contains_key(&(row.path.as_str(), row.page)))
            && self.rows.len() == previous.rows.len()
        {
            self.export_name = previous.export_name.clone();
            self.template = previous.template.clone();
            self.sequence = previous.sequence;
            self.separator = previous.separator.clone();
            self.widths = previous.widths.clone();
            self.show_all_fields = previous.show_all_fields;
            for row in &mut self.rows {
                if let Some((_, prior)) = old.get(&(row.path.as_str(), row.page)) {
                    if prior.doc == row.doc {
                        row.included = prior.included && row.doc.is_some();
                        row.filename = prior.filename.clone();
                        row.filename_override = prior.filename_override;
                    }
                }
            }
            self.rows
                .sort_by_key(|row| old.get(&(row.path.as_str(), row.page)).unwrap().0);
        }
        self.generate_filenames();
    }
    fn generate_filenames(&mut self) {
        for (index, row) in self.rows.iter_mut().enumerate() {
            if row.filename_override {
                continue;
            }
            let mut name = self.template.clone();
            for column in &self.columns {
                name = name.replace(
                    &format!("{{{}}}", column.id),
                    row.values.get(&column.id).map(String::as_str).unwrap_or(""),
                );
            }
            let parts: Vec<_> = name
                .split(" - ")
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            name = safe_name(&parts.join(" - "));
            if name.is_empty() {
                name = format!("page-{}", row.page + 1);
            }
            if self.sequence {
                name = format!("{:03}{}{name}", index + 1, self.separator);
            }
            row.filename = format!("{name}.pdf");
        }
    }
    pub fn set_value(&mut self, row: usize, id: &str, text: String) {
        let text = if id == "sheet_id" {
            normalize_sheet_id(&text)
        } else {
            crate::capture_text::normalize(id, &text)
        };
        let once = self
            .columns
            .iter()
            .any(|c| c.id == id && c.scope == Scope::Once);

        for (index, r) in self.rows.iter_mut().enumerate() {
            if index == row || (once && r.doc.is_some()) {
                r.values.insert(id.into(), text.clone());
                r.dirty.insert(id.into());
                r.warnings.remove(id);
            }
        }
        self.generate_filenames();
    }
    pub fn apply(&self, documents: &mut BTreeMap<String, Data>) {
        for row in &self.rows {
            let Some(data) = row.doc.as_ref().and_then(|doc| documents.get_mut(doc)) else {
                continue;
            };
            for id in &row.dirty {
                let Some(field) = data.fields.iter().find(|f| &f.id == id) else {
                    continue;
                };
                let key = if field.scope == Scope::Once {
                    -1
                } else {
                    row.page
                };
                let cell = data
                    .cells
                    .entry(id.clone())
                    .or_default()
                    .entry(key)
                    .or_insert_with(|| Cell {
                        method: "Manual".into(),
                        ..Default::default()
                    });
                cell.text = crate::capture_text::normalize(
                    id,
                    row.values.get(id).map(String::as_str).unwrap_or(""),
                );
                cell.edited = true;
                cell.error = None;
            }
        }
    }
    pub fn csv(&self) -> Result<String, String> {
        if !self.rows.iter().any(|r| r.included) {
            return Err("Select at least one row to export.".into());
        }
        let defaults = Data::default();
        let mut header = vec!["order".into(), "page".into(), "filename".into()];
        header.extend(self.columns.iter().map(|c| {
            if defaults
                .fields
                .iter()
                .any(|f| f.id == c.id && f.name == c.name)
            {
                c.id.clone()
            } else {
                c.name.clone()
            }
        }));
        header.extend(["status".into(), "source_file".into()]);
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut lines = vec![header.iter().map(|s| q(s)).collect::<Vec<_>>().join(",")];
        for (i, row) in self.rows.iter().filter(|r| r.included).enumerate() {
            let mut values = vec![
                (i + 1).to_string(),
                (row.global_page + 1).to_string(),
                row.filename.clone(),
            ];
            values.extend(self.columns.iter().map(|c| {
                crate::capture_text::normalize(
                    &c.id,
                    row.values.get(&c.id).map(String::as_str).unwrap_or(""),
                )
            }));
            values.push(row.status_id().into());
            values.push(file_name(&row.path));
            lines.push(values.iter().map(|s| q(s)).collect::<Vec<_>>().join(","));
        }
        Ok(format!("\u{feff}{}\r\n", lines.join("\r\n")))
    }
    pub fn json(&self) -> Result<String, String> {
        if !self.rows.iter().any(|r| r.included) {
            return Err("Select at least one row to export.".into());
        }
        let sheets: Vec<_> = self
            .rows
            .iter()
            .filter(|r| r.included)
            .enumerate()
            .map(|(i, row)| {
                let mut sheet = serde_json::Map::new();
                for (id, value) in &row.values {
                    sheet.insert(
                        id.clone(),
                        serde_json::json!(crate::capture_text::normalize(id, value)),
                    );
                }
                sheet.extend([
                    (String::from("order"), serde_json::json!(i + 1)),
                    ("page".into(), serde_json::json!(row.global_page + 1)),
                    ("source_page".into(), serde_json::json!(row.page + 1)),
                    (
                        "source_file".into(),
                        serde_json::json!(file_name(&row.path)),
                    ),
                    ("source_path".into(), serde_json::json!(row.path)),
                    ("source_sha256".into(), serde_json::json!(row.doc)),
                    ("filename".into(), serde_json::json!(row.filename)),
                    ("status".into(), serde_json::json!(row.status_id())),
                ]);
                sheet
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::json!({"schema_version":1,"exported_at":export_time(),"exported_at_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),"prepared_by":self.rows.iter().find(|r|r.included).and_then(|r|r.values.get("prepared_by")),"project_id":self.rows.iter().find(|r|r.included).and_then(|r|r.values.get("project_id")),"fields":self.columns,"sheets":sheets})).map_err(|e|e.to_string())
    }
    pub fn reorder(&mut self, from: usize, target: usize, above: bool) {
        if from >= self.rows.len() || target >= self.rows.len() || from == target {
            return;
        }
        let row = self.rows.remove(from);
        let adjusted = if target > from { target - 1 } else { target };
        let index = (adjusted + usize::from(!above)).min(self.rows.len());
        self.rows.insert(index, row);
        self.generate_filenames();
    }
    fn reset_order(&mut self) {
        self.rows.sort_by_key(|row| {
            self.original_order
                .iter()
                .position(|key| key == &(row.path.clone(), row.page))
                .unwrap_or(usize::MAX)
        });
        self.generate_filenames();
    }
    pub fn ui(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.saving {
            if let Ok(result) = rx.try_recv() {
                self.saving = None;
                self.notice = match result {
                    Ok(Some(path)) => format!("Saved: {path}"),
                    Ok(None) => "Save cancelled. Reviewed data retained.".into(),
                    Err(e) => format!("Export failed: {e}"),
                };
            }
        }
        if !self.open {
            return;
        }
        self.row_handles.clear();
        self.drop_target = None;
        let busy = self.saving.is_some();
        let mut close = false;
        let mut export = None;
        let mut edited = Vec::new();
        let size = ctx.screen_rect().size();
        egui::Modal::new(egui::Id::new("capture-staging")).show(ctx, |ui| {
            ui.set_width((size.x - 48.0).clamp(320.0, 2100.0));
            ui.heading("Staging area — review & edit filenames");
            ui.horizontal_wrapped(|ui| {
                let count = self.rows.iter().filter(|r| r.included).count();
                let ready = self
                    .rows
                    .iter()
                    .filter(|r| r.included && r.status() == "Ready")
                    .count();
                ui.label(format!(
                    "{ready} ready · {} need review · {count} selected",
                    count - ready
                ));
                ui.separator();
                ui.label("Drag a row handle to reorder. Drag a column divider to resize.");
            });
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let old = self.template.clone();
                    egui::ComboBox::from_id_salt("name-preset")
                        .selected_text("Filename template")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.template,
                                "{sheet_id} - {description} - {issue_id}".into(),
                                "Sheet · Description · Revision",
                            );
                            ui.selectable_value(
                                &mut self.template,
                                "{project_id} - {sheet_id} - {description} - {issue_id}".into(),
                                "Project · Sheet · Description · Revision",
                            );
                            ui.selectable_value(
                                &mut self.template,
                                "{sheet_id} - {description}".into(),
                                "Sheet · Description",
                            );
                        });
                    let changed = ui
                        .add(egui::TextEdit::singleline(&mut self.template).desired_width(330.0))
                        .changed()
                        || old != self.template;
                    if changed {
                        self.generate_filenames();
                    }
                    ui.label("Export name");
                    ui.add(egui::TextEdit::singleline(&mut self.export_name).desired_width(220.0));
                });
                ui.horizontal_wrapped(|ui| {
                    let mut names_changed =
                        ui.checkbox(&mut self.sequence, "Sequence prefix").changed();
                    egui::ComboBox::from_id_salt("seq-separator")
                        .selected_text(&self.separator)
                        .show_ui(ui, |ui| {
                            for sep in [" - ", "_", " "] {
                                names_changed |= ui
                                    .selectable_value(&mut self.separator, sep.into(), sep)
                                    .changed();
                            }
                        });
                    if names_changed {
                        self.generate_filenames();
                    }
                    if ui.button("Reset order").clicked() {
                        self.reset_order();
                    }
                    egui::ComboBox::from_id_salt("staging-filter")
                        .selected_text(
                            ["All sheets", "Needs review", "Ready"][self.filter as usize],
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.filter, 0, "All sheets");
                            ui.selectable_value(&mut self.filter, 1, "Needs review");
                            ui.selectable_value(&mut self.filter, 2, "Ready");
                        });
                    if ui.button("Select all").clicked() {
                        for row in &mut self.rows {
                            row.included = row.doc.is_some();
                        }
                    }
                    if ui.button("Select none").clicked() {
                        for row in &mut self.rows {
                            row.included = false;
                        }
                    }
                    ui.checkbox(&mut self.show_all_fields, "Show all fields");
                });
                // Match Jobs' useful default columns; additional fields remain available.
                let mut columns = vec![
                    ("drag".to_owned(), "".to_owned(), 32.0),
                    ("include".into(), "Include".into(), 58.0),
                    ("source".into(), "Source / page".into(), 210.0),
                ];
                for id in [
                    "sheet_id",
                    "description",
                    "issue_id",
                    "date",
                    "issue_description",
                ] {
                    if let Some(c) = self.columns.iter().find(|c| c.id == id) {
                        columns.push((
                            c.id.clone(),
                            c.name.clone(),
                            if id == "description" {
                                480.0
                            } else if id == "issue_description" {
                                340.0
                            } else if id == "date" {
                                155.0
                            } else if id == "issue_id" {
                                95.0
                            } else {
                                150.0
                            },
                        ));
                    }
                }
                if self.show_all_fields {
                    for c in &self.columns {
                        if ![
                            "sheet_id",
                            "description",
                            "issue_id",
                            "date",
                            "issue_description",
                        ]
                        .contains(&c.id.as_str())
                        {
                            columns.push((c.id.clone(), c.name.clone(), 220.0));
                        }
                    }
                }
                columns.push(("filename".into(), "Filename preview".into(), 600.0));
                columns.push(("status".into(), "Status".into(), 110.0));
                let widths: Vec<_> = columns
                    .iter()
                    .map(|(id, _, initial)| *self.widths.entry(id.clone()).or_insert(*initial))
                    .collect();
                let total = widths.iter().sum::<f32>() + 8.0 * (widths.len() - 1) as f32;
                egui::ScrollArea::horizontal()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .id_salt("staging-horizontal")
                    .show(ui, |ui| {
                        ui.set_min_width(total);
                        ui.horizontal(|ui| {
                            for ((id, label, _), width) in columns.iter().zip(&widths) {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(*width, 28.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().text(
                                    rect.left_center(),
                                    egui::Align2::LEFT_CENTER,
                                    label,
                                    egui::FontId::proportional(13.0),
                                    ui.visuals().text_color(),
                                );
                                if !["drag", "include"].contains(&id.as_str()) {
                                    let edge = egui::Rect::from_center_size(
                                        rect.right_center(),
                                        egui::vec2(8.0, 28.0),
                                    );
                                    let resize = ui.interact(
                                        edge,
                                        ui.id().with(("column-resize", id)),
                                        egui::Sense::drag(),
                                    );
                                    ui.painter().line_segment(
                                        [edge.center_top(), edge.center_bottom()],
                                        egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
                                    );
                                    if resize.hovered() || resize.dragged() {
                                        ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                    }
                                    if resize.dragged() {
                                        let delta = ctx.input(|i| i.pointer.delta().x);
                                        self.widths.insert(
                                            id.clone(),
                                            (*width + delta).clamp(70.0, 1600.0),
                                        );
                                    }
                                }
                            }
                        });
                        egui::ScrollArea::vertical()
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .id_salt("staging-vertical")
                            .max_height((size.y - 280.0).max(140.0))
                            .show(ui, |ui| {
                                let clip = ui.clip_rect();
                                for (index, row) in self.rows.iter_mut().enumerate() {
                                    if self.filter == 1 && row.status() == "Ready"
                                        || self.filter == 2 && row.status() != "Ready"
                                    {
                                        continue;
                                    }
                                    let top = ui.cursor().top();
                                    let full = egui::Rect::from_min_size(
                                        egui::pos2(ui.cursor().left(), top),
                                        egui::vec2(total, 34.0),
                                    );
                                    if index % 2 == 1 {
                                        ui.painter().rect_filled(
                                            full,
                                            0.0,
                                            ui.visuals().faint_bg_color,
                                        );
                                    }
                                    if self.drag_from == Some(index) {
                                        ui.painter().rect_filled(
                                            full,
                                            0.0,
                                            egui::Color32::from_rgba_unmultiplied(40, 140, 230, 40),
                                        );
                                    }
                                    ui.push_id((row.path.clone(), row.page), |ui| {
                                        ui.horizontal(|ui| {
                                            for ((id, _, _), width) in columns.iter().zip(&widths) {
                                                ui.allocate_ui_with_layout(
                                                    egui::vec2(*width, 34.0),
                                                    egui::Layout::left_to_right(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.set_min_size(egui::vec2(*width, 34.0));
                                                        ui.set_clip_rect(ui.clip_rect().intersect(
                                                            egui::Rect::from_min_size(
                                                                ui.cursor().min,
                                                                egui::vec2(*width, 34.0),
                                                            ),
                                                        ));
                                                        match id.as_str() {
                                                            "drag" => {
                                                                let (rect, handle) = ui
                                                                    .allocate_exact_size(
                                                                        egui::vec2(*width, 28.0),
                                                                        egui::Sense::drag(),
                                                                    );
                                                                self.row_handles
                                                                    .push((index, rect));
                                                                for x in [-3.0, 3.0] {
                                                                    for y in [-5.0, 0.0, 5.0] {
                                                                        ui.painter().circle_filled(
                                                                            rect.center()
                                                                                + egui::vec2(x, y),
                                                                            1.3,
                                                                            ui.visuals()
                                                                                .text_color(),
                                                                        );
                                                                    }
                                                                }
                                                                if handle.drag_started() {
                                                                    self.drag_from = Some(index);
                                                                }
                                                                if handle.hovered()
                                                                    || handle.dragged()
                                                                {
                                                                    ctx.set_cursor_icon(
                                                                        egui::CursorIcon::Grab,
                                                                    );
                                                                }
                                                                handle.on_hover_text(
                                                                    "Drag to reorder",
                                                                );
                                                            }
                                                            "include" => {
                                                                ui.add_enabled(
                                                                    row.doc.is_some(),
                                                                    egui::Checkbox::new(
                                                                        &mut row.included,
                                                                        "",
                                                                    ),
                                                                );
                                                            }
                                                            "source" => {
                                                                ui.add(
                                                                    egui::Label::new(format!(
                                                                        "{} · p{}",
                                                                        file_name(&row.path),
                                                                        row.page + 1
                                                                    ))
                                                                    .truncate(),
                                                                )
                                                                .on_hover_text(&row.path);
                                                            }
                                                            "filename" => {
                                                                if ui
                                                                    .add_sized(
                                                                        [*width, 28.0],
                                                                        egui::TextEdit::singleline(
                                                                            &mut row.filename,
                                                                        )
                                                                        .id_salt("filename"),
                                                                    )
                                                                    .changed()
                                                                {
                                                                    row.filename_override = true;
                                                                }
                                                            }
                                                            "status" => {
                                                                ui.label(row.status());
                                                            }
                                                            _ => {
                                                                let value = row
                                                                    .values
                                                                    .entry(id.clone())
                                                                    .or_default();
                                                                let response = ui.add_sized(
                                                                    [*width, 28.0],
                                                                    egui::TextEdit::singleline(
                                                                        value,
                                                                    )
                                                                    .id_salt(id),
                                                                );
                                                                if response.changed() {
                                                                    edited.push((
                                                                        index,
                                                                        id.clone(),
                                                                        value.clone(),
                                                                    ));
                                                                }
                                                                response
                                                                    .on_hover_text(value.as_str());
                                                            }
                                                        }
                                                    },
                                                );
                                            }
                                        })
                                    });
                                    if self.drag_from.is_some() {
                                        if let Some(pointer) =
                                            ctx.input(|i| i.pointer.interact_pos())
                                        {
                                            if full.contains(pointer) && clip.contains(pointer) {
                                                let above = pointer.y < full.center().y;
                                                self.drop_target = Some((index, above));
                                                let y =
                                                    if above { full.top() } else { full.bottom() };
                                                ui.painter().line_segment(
                                                    [
                                                        egui::pos2(full.left(), y),
                                                        egui::pos2(full.right(), y),
                                                    ],
                                                    egui::Stroke::new(
                                                        3.0,
                                                        egui::Color32::LIGHT_BLUE,
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                                if self.drag_from.is_some() {
                                    if let Some(pointer) = ctx.input(|i| i.pointer.interact_pos()) {
                                        if pointer.y < clip.top() + 24.0 {
                                            ui.scroll_with_delta(egui::vec2(0.0, 12.0));
                                        } else if pointer.y > clip.bottom() - 24.0 {
                                            ui.scroll_with_delta(egui::vec2(0.0, -12.0));
                                        }
                                    }
                                }
                            });
                    });
            });
            ui.label(&self.notice);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Save CSV…"))
                    .clicked()
                {
                    export = Some("csv");
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Save JSON…"))
                    .clicked()
                {
                    export = Some("json");
                }
                if ui.add_enabled(!busy, egui::Button::new("Close")).clicked() {
                    close = true;
                }
                if busy {
                    ui.spinner();
                    ui.label("Choose where to save…");
                }
            });
        });
        for (row, id, value) in edited {
            self.set_value(row, &id, value);
        }
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(from) = self.drag_from.take() {
                if let Some((target, above)) = self.drop_target.take() {
                    self.reorder(from, target, above);
                }
            }
        }
        if close {
            self.open = false;
        }
        if let Some(extension) = export {
            let result = if extension == "csv" {
                self.csv()
            } else {
                self.json()
            };
            match result {
                Ok(text) => {
                    let name = export_base_name(&self.export_name);
                    let name = if name.is_empty() { "drawings" } else { &name };
                    self.saving = Some(viewer::save_export(
                        format!("{name}.{extension}"),
                        text.into_bytes(),
                        extension,
                    ));
                    self.notice = "Choose where to save…".into();
                }
                Err(e) => self.notice = e,
            }
        }
    }
}
fn export_base_name(text: &str) -> String {
    let mut name = safe_name(text);
    for suffix in [".csv", ".json", ".zip"] {
        if name.to_ascii_lowercase().ends_with(suffix) {
            name.truncate(name.len() - suffix.len());
            break;
        }
    }
    name
}
fn normalize_sheet_id(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['O', 'o'], "0")
}
fn export_time() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut days = now.as_secs() / 86400;
    let mut year = 1970u64;
    let leap = |year: u64| {
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    };
    loop {
        let count = if leap(year) { 366 } else { 365 };
        if days < count {
            break;
        }
        days -= count;
        year += 1;
    }
    let months = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while days >= months[month] {
        days -= months[month];
        month += 1;
    }
    let time = now.as_secs() % 86400;
    format!(
        "{year:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        month + 1,
        days + 1,
        time / 3600,
        (time / 60) % 60,
        time % 60,
        now.subsec_millis()
    )
}
fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn safe_name(text: &str) -> String {
    text.chars()
        .filter(|c| !"/\\:*?\"<>|".contains(*c))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches([' ', '.', '-', '_'])
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Data {
        let mut data = Data::default();
        data.fields[0].name = "Author".into();
        data.fields[2].name = "Drawing number".into();
        data.cells.entry("prepared_by".into()).or_default().insert(
            -1,
            Cell {
                text: "Original".into(),
                ..Default::default()
            },
        );
        for page in 0..2 {
            data.cells.entry("sheet_id".into()).or_default().insert(
                page,
                Cell {
                    text: format!("A10{}", page + 1),
                    ..Default::default()
                },
            );
        }
        data
    }
    fn stage(data: &Data) -> Staging {
        Staging::new(
            &data.fields,
            vec![
                ("/test/a.pdf".into(), Some("source-a".into()), 2, Some(data)),
                ("/test/unopened.pdf".into(), None, 1, None),
            ],
            "Test".into(),
        )
    }
    #[test]
    fn long_description_stays_single_line_and_aligned_to_resizable_header() {
        let data = fixture();
        let mut stage = stage(&data);
        let text="A long description of the ground-floor general arrangement and reflected ceiling plan for several adjacent rooms";
        stage.set_value(0, "description", text.into());
        let ctx = egui::Context::default();
        let mut output = None;
        for _ in 0..2 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ctx| stage.ui(ctx),
            ));
        }
        let output = output.unwrap();
        let find = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => Some(t),
                    _ => None,
                })
                .unwrap()
        };
        let header = find("Description");
        let value = find(text);
        assert_eq!(value.galley.rows.len(), 1);
        assert!(
            (value.pos.x - header.pos.x).abs() < 12.0,
            "header and input are misaligned"
        );
    }
    #[test]
    fn matches_executable_jobs_filename_and_status_reference() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/extractor_reference.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let data = Data::default();
            let mut stage = Staging::new(
                &data.fields,
                vec![("/test/reference.pdf".into(), Some("ref".into()), 1, None)],
                "Reference".into(),
            );
            stage.template = case["template"].as_str().unwrap().into();
            stage.sequence = case["sequence"].as_bool().unwrap();
            stage.separator = case["separator"].as_str().unwrap().into();
            stage.rows[0].page = case["sheet"]["page"].as_i64().unwrap() as i32 - 1;
            for (id, value) in case["sheet"].as_object().unwrap() {
                if let Some(value) = value.as_str() {
                    stage.rows[0].values.insert(id.clone(), value.into());
                }
            }
            if let Some(doc) = case.get("document").and_then(|d| d.as_object()) {
                for (id, value) in doc {
                    stage.rows[0]
                        .values
                        .insert(id.clone(), value.as_str().unwrap().into());
                }
            }
            let index = case["index"].as_u64().unwrap() as usize;
            let row = stage.rows[0].clone();
            stage.rows = vec![row; index + 1];
            stage.generate_filenames();
            assert_eq!(
                stage.rows[index].filename,
                case["filename"].as_str().unwrap(),
                "{case}"
            );
            assert_eq!(
                stage.rows[index].status_id(),
                case["status"].as_str().unwrap()
            );
        }
    }
    #[test]
    fn pointer_drag_reorders_rows_and_resizing_changes_column_width() {
        let data = fixture();
        let mut stage = stage(&data);
        let ctx = egui::Context::default();
        let frame = |stage: &mut Staging, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| stage.ui(ctx),
            )
        };
        let _ = frame(&mut stage, vec![]);
        let output = frame(&mut stage, vec![]);
        let _ = output;
        let handles: Vec<_> = stage
            .row_handles
            .iter()
            .map(|(_, rect)| rect.center())
            .collect();
        assert_eq!(handles.len(), 3);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let start = handles[0];
        let end = handles[1] + egui::vec2(0.0, 10.0);
        let _ = frame(
            &mut stage,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let _ = frame(
            &mut stage,
            vec![egui::Event::PointerMoved(start + egui::vec2(0.0, 8.0))],
        );
        let _ = frame(&mut stage, vec![egui::Event::PointerMoved(end)]);
        let output = frame(&mut stage, vec![button(end, false)]);
        assert_eq!(stage.rows[0].page, 1);
        assert_eq!(stage.rows[1].page, 0);
        assert!(stage.rows[0].filename.starts_with("001"));
        assert!(stage.rows[1].filename.starts_with("002"));
        let header = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Description" => Some(t.pos),
                _ => None,
            })
            .unwrap();
        let initial = stage.widths["description"];
        let edge = header + egui::vec2(initial, 7.0);
        let end = edge + egui::vec2(120.0, 0.0);
        let _ = frame(
            &mut stage,
            vec![egui::Event::PointerMoved(edge), button(edge, true)],
        );
        let _ = frame(&mut stage, vec![egui::Event::PointerMoved(end)]);
        let _ = frame(&mut stage, vec![button(end, false)]);
        assert!(
            stage.widths["description"] > initial,
            "column divider did not resize"
        );
    }
    #[test]
    fn generated_names_follow_edits_sequence_and_manual_overrides() {
        let data = fixture();
        let mut stage = stage(&data);
        stage.set_value(0, "description", "Ground floor".into());
        assert!(stage.rows[0].filename.contains("Ground floor"));
        stage.rows[0].filename = "my-manual-name.pdf".into();
        stage.rows[0].filename_override = true;
        stage.set_value(0, "issue_id", "B".into());
        stage.reorder(0, 1, false);
        assert_eq!(stage.rows[1].filename, "my-manual-name.pdf");
        assert!(stage.rows[0].filename.starts_with("001"));
        stage.reset_order();
        assert_eq!(stage.rows[0].page, 0);
        assert_eq!(stage.rows[0].filename, "my-manual-name.pdf");
    }
    #[test]
    fn modal_keeps_export_actions_inside_viewport() {
        let data = fixture();
        let mut stage = stage(&data);
        let ctx = egui::Context::default();
        let mut output = None;
        for _ in 0..2 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 850.0),
                    )),
                    ..Default::default()
                },
                |ctx| stage.ui(ctx),
            ));
        }
        let output = output.unwrap();
        for label in ["Save CSV…", "Save JSON…", "Close"] {
            let rect = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
                    }
                    _ => None,
                })
                .expect("export control rendered");
            assert!(
                rect.left() >= 0.0
                    && rect.right() <= 1200.0
                    && rect.top() >= 0.0
                    && rect.bottom() <= 850.0,
                "{label} is outside viewport: {rect:?}"
            );
        }
    }
    #[test]
    fn review_edits_once_values_and_commits_to_capture_data() {
        let data = fixture();
        let mut stage = stage(&data);
        stage.set_value(1, "prepared_by", "Corrected author".into());
        assert_eq!(stage.rows[0].values["prepared_by"], "Corrected author");
        assert_eq!(stage.rows[1].values["prepared_by"], "Corrected author");
        stage.set_value(1, "sheet_id", "A202".into());
        assert_eq!(stage.rows[0].values["sheet_id"], "A101");
        let mut docs = BTreeMap::from([("source-a".into(), data)]);
        stage.apply(&mut docs);
        assert_eq!(
            docs["source-a"].cells["prepared_by"][&-1].text,
            "Corrected author"
        );
        assert!(docs["source-a"].cells["prepared_by"][&-1].edited);
        assert_eq!(docs["source-a"].cells["sheet_id"][&1].text, "A202");
    }
    #[test]
    fn exports_use_reviewed_order_inclusions_labels_and_stable_ids() {
        let data = fixture();
        let mut stage = stage(&data);
        stage.set_value(1, "description", "Café, \"North\"\nLevel 2".into());
        stage.rows.swap(0, 1);
        stage.rows[0].filename = "reviewed.pdf".into();
        stage.rows[1].included = false;
        let csv = stage.csv().unwrap();
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains("\"Author\""));
        assert!(csv.contains("\"Drawing number\""));
        assert!(csv.contains("\"Café, \"\"North\"\" Level 2\""));
        assert!(!csv.contains("A101"));
        assert!(!csv.contains("unopened.pdf"));
        let json: serde_json::Value = serde_json::from_str(&stage.json().unwrap()).unwrap();
        assert_eq!(json["sheets"].as_array().unwrap().len(), 1);
        assert_eq!(json["sheets"][0]["sheet_id"], "A102");
        assert_eq!(json["sheets"][0]["page"], 2);
        assert_eq!(json["sheets"][0]["order"], 1);
        assert_eq!(json["sheets"][0]["filename"], "reviewed.pdf");
        assert_eq!(json["fields"][2]["name"], "Drawing number");
        stage.rows[0].included = false;
        assert!(stage.csv().is_err());
        assert!(stage.json().is_err());
    }
    #[test]
    fn missing_issue_description_and_ambiguous_dates_require_review() {
        let data = fixture();
        let mut stage = stage(&data);
        let row = &mut stage.rows[0];
        row.values.insert("description".into(), "Floor plan".into());
        row.required.insert("issue_description".into());
        row.values.insert("issue_description".into(), String::new());
        assert_eq!(row.status(), "Review");
        row.values
            .insert("issue_description".into(), "PRELIMINARY".into());
        row.values
            .insert("date".into(), "27/03/2026 14/04/2026".into());
        assert_eq!(row.status(), "Review");
        row.values.insert("date".into(), "14/04/2026".into());
        assert_eq!(row.status(), "Ready");
        // Older saved values are also flattened at the export boundary.
        row.values
            .insert("description".into(), "Floor\nplan\t Level 1".into());
        let csv = stage.csv().unwrap();
        assert!(csv.contains("Floor plan Level 1"));
        assert!(!csv.replace("\r\n", "").contains(['\n', '\r', '\t']));
        let json: serde_json::Value = serde_json::from_str(&stage.json().unwrap()).unwrap();
        assert_eq!(json["sheets"][0]["description"], "Floor plan Level 1");
    }
    #[test]
    fn written_month_dates_clear_old_validation_errors_in_review_and_export() {
        let mut data = fixture();
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
        let mut stage = stage(&data);
        assert_eq!(stage.rows[0].values["date"], "21/07/2026");
        assert!(!stage.rows[0].warnings.contains_key("date"));
        assert!(stage.csv().unwrap().contains("21/07/2026"));
        stage.set_value(0, "date", "July 22nd, 2026".into());
        assert_eq!(stage.rows[0].values["date"], "22/07/2026");
    }
    #[test]
    fn reextraction_refreshes_generated_filenames_without_losing_review_layout() {
        let mut data = fixture();
        let previous = stage(&data);
        data.cells
            .entry("description".into())
            .or_default()
            .entry(0)
            .or_default()
            .text = "Corrected drawing title".into();
        let mut fresh = stage(&data);
        fresh.preserve_layout(&previous);
        assert!(fresh.rows[0].filename.contains("Corrected drawing title"));
    }
    #[test]
    fn reopened_staging_retains_layout_and_reads_latest_values() {
        let mut data = fixture();
        let mut previous = stage(&data);
        previous.rows.swap(0, 1);
        previous.rows[0].filename = "manual.pdf".into();
        previous.rows[0].filename_override = true;
        previous.rows[1].included = false;
        data.cells
            .get_mut("sheet_id")
            .unwrap()
            .get_mut(&1)
            .unwrap()
            .text = "Updated".into();
        let mut fresh = stage(&data);
        fresh.preserve_layout(&previous);
        assert_eq!(fresh.rows[0].page, 1);
        assert_eq!(fresh.rows[0].filename, "manual.pdf");
        assert!(!fresh.rows[1].included);
        assert_eq!(fresh.rows[0].values["sheet_id"], "Updated");
        assert!(!fresh.rows[2].included);
    }
}

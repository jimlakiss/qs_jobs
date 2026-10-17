//! Lazy, read-only filesystem navigation. Directory IO stays off the UI thread.
use eframe::egui;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
};

#[derive(Clone, Debug)]
struct Entry {
    path: PathBuf,
    name: String,
    directory: bool,
    link: bool,
    pdf: bool,
}
fn read_directory(path: &Path) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for item in path.read_dir().map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        let kind = item.file_type().map_err(|e| e.to_string())?;
        let path = item.path();
        let pdf = kind.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        entries.push(Entry {
            path,
            name: item.file_name().to_string_lossy().into_owned(),
            directory: kind.is_dir(),
            link: kind.is_symlink(),
            pdf,
        });
    }
    entries.sort_by_cached_key(|e| (!e.directory, e.name.to_lowercase()));
    Ok(entries)
}
struct Request {
    generation: u64,
    path: PathBuf,
}
struct Reply {
    generation: u64,
    path: PathBuf,
    entries: Result<Vec<Entry>, String>,
}
pub struct FileTree {
    root: PathBuf,
    generation: Arc<AtomicU64>,
    requests: mpsc::SyncSender<Request>,
    replies: mpsc::Receiver<Reply>,
    entries: HashMap<PathBuf, Result<Vec<Entry>, String>>,
    pending: HashSet<PathBuf>,
    show_hidden: bool,
    set_anchor: Option<String>,
}
impl FileTree {
    pub fn new(input: &Path, ctx: egui::Context) -> Self {
        let generation = Arc::new(AtomicU64::new(0));
        let active = generation.clone();
        let (requests, rx) = mpsc::sync_channel::<Request>(32);
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(request) = rx.recv() {
                if request.generation != active.load(Ordering::Relaxed) {
                    continue;
                }
                let entries = read_directory(&request.path);
                if tx
                    .send(Reply {
                        generation: request.generation,
                        path: request.path,
                        entries,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        let mut tree = Self {
            root: PathBuf::new(),
            generation,
            requests,
            replies,
            entries: HashMap::new(),
            pending: HashSet::new(),
            show_hidden: false,
            set_anchor: None,
        };
        tree.set_input(input);
        tree
    }
    pub fn set_input(&mut self, input: &Path) {
        let root = if input.is_dir() {
            input
        } else {
            input.parent().unwrap_or(input)
        };
        self.root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        self.refresh();
    }
    fn refresh(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.entries.clear();
        self.pending.clear();
    }
    fn request(&mut self, path: &Path) {
        if self.entries.contains_key(path) || self.pending.contains(path) {
            return;
        }
        if self
            .requests
            .try_send(Request {
                generation: self.generation.load(Ordering::Relaxed),
                path: path.to_path_buf(),
            })
            .is_ok()
        {
            self.pending.insert(path.to_path_buf());
        }
    }
    fn folder(
        &mut self,
        ui: &mut egui::Ui,
        path: &Path,
        depth: usize,
        selected: &str,
        members: &mut BTreeSet<String>,
    ) -> Option<String> {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let mut opened = None;
        egui::CollapsingHeader::new(format!("{name}/"))
            .id_salt((self.generation.load(Ordering::Relaxed), path))
            .default_open(depth == 0)
            .show(ui, |ui| {
                self.request(path);
                match self.entries.get(path).cloned() {
                    None => {
                        ui.spinner();
                        ui.small("Reading folder…");
                    }
                    Some(Err(error)) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                    Some(Ok(entries)) => {
                        if entries.is_empty() {
                            ui.weak("Empty folder");
                        }
                        let siblings: Vec<String> = entries.iter().filter(|e|e.pdf && (self.show_hidden || !e.name.starts_with('.'))).map(|e|e.path.to_string_lossy().into_owned()).collect();
                        if ui.small_button("Add PDFs here to set").clicked() {members.extend(siblings.iter().cloned());}
                        for entry in entries {
                            if !self.show_hidden && entry.name.starts_with('.') {
                                continue;
                            }
                            if entry.directory {
                                if let Some(path) =
                                    self.folder(ui, &entry.path, depth + 1, selected, members)
                                {
                                    opened = Some(path);
                                }
                            } else if entry.pdf {
                                let path = entry.path.to_string_lossy().into_owned();
                                ui.horizontal(|ui| {
                                    let mut member = members.contains(&path);
                                    if ui.checkbox(&mut member, "").on_hover_text("Include in drawing set. Shift-click selects a range in this folder.").clicked() {
                                        select_members(members, &path, member, ui.input(|i|i.modifiers.shift), self.set_anchor.as_deref(), &siblings);
                                        self.set_anchor = Some(path.clone());
                                    }
                                    let current = path == selected;
                                    if ui.selectable_label(members.contains(&path), format!("{} {}",if current {"▶"}else{"PDF"},entry.name)).on_hover_text(&path).clicked() {
                                        if ui.input(|i|i.modifiers.command || i.modifiers.ctrl || i.modifiers.shift) {
                                            select_members(members,&path,!members.contains(&path),ui.input(|i|i.modifiers.shift),self.set_anchor.as_deref(),&siblings);
                                            self.set_anchor = Some(path.clone());
                                        } else {opened=Some(path.clone());}
                                    }
                                });
                            } else {
                                let suffix = if entry.link { " (link)" } else { "" };
                                ui.add_enabled(
                                    false,
                                    egui::Label::new(format!("{}{}", entry.name, suffix)),
                                )
                                .on_disabled_hover_text(
                                    "Only PDF files can be previewed. Links are not followed.",
                                );
                            }
                        }
                    }
                }
            })
            .header_response
            .on_hover_text(path.display().to_string());
        opened
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        selected: &str,
        members: &mut BTreeSet<String>,
    ) -> Option<String> {
        while let Ok(reply) = self.replies.try_recv() {
            if reply.generation == self.generation.load(Ordering::Relaxed) {
                self.pending.remove(&reply.path);
                self.entries.insert(reply.path, reply.entries);
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.refresh();
            }
            if ui
                .add_enabled(self.root.parent().is_some(), egui::Button::new("Up"))
                .on_hover_text("Browse the parent folder")
                .clicked()
            {
                self.root = self.root.parent().unwrap().to_path_buf();
                self.refresh();
            }
            ui.checkbox(&mut self.show_hidden, "Hidden");
        });
        ui.small(self.root.display().to_string());
        let root = self.root.clone();
        self.folder(ui, &root, 0, selected, members)
    }
}

fn select_members(
    members: &mut BTreeSet<String>,
    path: &str,
    include: bool,
    range: bool,
    anchor: Option<&str>,
    siblings: &[String],
) {
    if range {
        if let Some((a, b)) = anchor
            .and_then(|a| siblings.iter().position(|p| p == a))
            .zip(siblings.iter().position(|p| p == path))
        {
            for path in &siblings[a.min(b)..=a.max(b)] {
                if include {
                    members.insert(path.clone());
                } else {
                    members.remove(path);
                }
            }
            return;
        }
    }
    if include {
        members.insert(path.into());
    } else {
        members.remove(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_selection_adds_ranges_and_removes_only_chosen_drawings() {
        let paths = vec![
            "a.pdf".into(),
            "b.pdf".into(),
            "c.pdf".into(),
            "d.pdf".into(),
        ];
        let mut members = BTreeSet::new();
        select_members(&mut members, "a.pdf", true, false, None, &paths);
        select_members(&mut members, "c.pdf", true, true, Some("a.pdf"), &paths);
        assert_eq!(members.len(), 3);
        assert!(!members.contains("d.pdf"));
        select_members(&mut members, "b.pdf", false, false, None, &paths);
        assert_eq!(members, BTreeSet::from(["a.pdf".into(), "c.pdf".into()]));
    }
    #[test]
    fn directory_listing_is_shallow_sorted_and_does_not_follow_links() {
        let root = std::env::temp_dir().join(format!("iqs-file-tree-{}", std::process::id()));
        std::fs::create_dir_all(root.join("child")).unwrap();
        std::fs::write(root.join("z.PDF"), b"test").unwrap();
        std::fs::write(root.join("a.txt"), b"test").unwrap();
        std::fs::write(root.join("child/nested.pdf"), b"test").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let entries = read_directory(&root).unwrap();
        assert_eq!(entries[0].name, "child");
        assert!(entries.iter().any(|e| e.name == "z.PDF" && e.pdf));
        assert!(entries.iter().any(|e| e.name == "a.txt" && !e.pdf));
        assert!(!entries.iter().any(|e| e.name == "nested.pdf"));
        #[cfg(unix)]
        assert!(entries
            .iter()
            .any(|e| e.name == "loop" && e.link && !e.directory));
        assert!(read_directory(&root.join("child")).unwrap()[0].pdf);
        std::fs::remove_dir_all(root).unwrap();
    }
}

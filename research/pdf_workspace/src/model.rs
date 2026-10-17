use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash};

pub const TILE: u32 = 512;
pub const PREVIEW_EDGE: u32 = 512;
pub const MAX_LEVEL: i8 = 9;
pub const MAX_EDGE: u32 = 1024 << MAX_LEVEL;
pub const MAX_REPLY: usize = TILE as usize * TILE as usize * 4;
pub const CPU_BUDGET: usize = 16 * 1024 * 1024;
pub const GPU_BUDGET: usize = 32 * 1024 * 1024;
pub const MAX_JOBS: usize = GPU_BUDGET / MAX_REPLY;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub doc: String,
    pub page: i32,
    // -3: text; -2: metadata; -1: preview; 0..MAX_LEVEL: 1024..524288 virtual long edge.
    pub level: i8,
    pub col: u32,
    pub row: u32,
}
impl Key {
    pub fn info(page: i32) -> Self {
        Self {
            doc: String::new(),
            page,
            level: -2,
            col: 0,
            row: 0,
        }
    }
    pub fn text(doc: String, page: i32) -> Self {
        Self {
            doc,
            page,
            level: -3,
            col: 0,
            row: 0,
        }
    }
    pub fn edge(&self) -> u32 {
        if self.level < 0 {
            PREVIEW_EDGE
        } else {
            1024 << self.level.min(MAX_LEVEL)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageInfo {
    pub doc: String,
    pub page: i32,
    pub pages: i32,
    pub width: f64,
    pub height: f64,
    pub rotation: i32,
    pub crop: Option<[f32; 4]>,
}
impl PageInfo {
    pub fn raster_size(&self, edge: u32) -> (u32, u32) {
        let scale = edge as f64 / self.width.max(self.height);
        (
            (self.width * scale).round().max(1.0) as u32,
            (self.height * scale).round().max(1.0) as u32,
        )
    }
    pub fn preview(&self) -> Key {
        Key {
            doc: self.doc.clone(),
            page: self.page,
            level: -1,
            col: 0,
            row: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub seq: u64,
    pub key: Key,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub seq: u64,
    pub key: Key,
    pub info: Option<PageInfo>,
    pub full: [u32; 2],
    pub origin: [u32; 2],
    pub size: [u32; 2],
    pub bytes: usize,
    pub render_ms: f64,
    pub error: Option<String>,
}
pub struct Pixels {
    pub header: Header,
    pub rgba: Vec<u8>,
}

// Double-precision, displayed-page coordinates (top-left origin), independent
// of GPU pixels. Raw PDF user-space/world-unit conversion is a separate adapter.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub origin: [f64; 2],
    pub scale: f64,
    pub page: [f64; 2],
}
impl View {
    pub fn to_screen(self, p: [f64; 2]) -> [f64; 2] {
        [
            self.origin[0] + p[0] * self.scale,
            self.origin[1] + p[1] * self.scale,
        ]
    }
    pub fn to_page(self, p: [f64; 2]) -> [f64; 2] {
        [
            (p[0] - self.origin[0]) / self.scale,
            (p[1] - self.origin[1]) / self.scale,
        ]
    }
    pub fn normalized(self, p: [f64; 2]) -> [f64; 2] {
        let p = self.to_page(p);
        [
            (p[0] / self.page[0]).clamp(0.0, 1.0),
            (p[1] / self.page[1]).clamp(0.0, 1.0),
        ]
    }
}

// Priority: nearest visible tile to viewport centre; strict maximum work list.
pub fn visible_tiles(info: &PageInfo, view: View, viewport: [f64; 4], dpi: f64) -> Vec<Key> {
    let desired = (info.width.max(info.height) * view.scale * dpi).clamp(1024.0, MAX_EDGE as f64);
    let mut level = (desired / 1024.0)
        .log2()
        .ceil()
        .clamp(0.0, MAX_LEVEL as f64) as i8;
    let a = view.normalized([viewport[0], viewport[1]]);
    let b = view.normalized([viewport[2], viewport[3]]);
    // Clamping alone would request an edge tile for a completely offscreen page.
    let end = view.to_screen([info.width, info.height]);
    if end[0] <= viewport[0]
        || end[1] <= viewport[1]
        || view.origin[0] >= viewport[2]
        || view.origin[1] >= viewport[3]
    {
        return Vec::new();
    }
    // Lower the refinement bucket rather than cycling more visible textures
    // through the cache than its budget can hold simultaneously.
    let (w, h) = loop {
        let (w, h) = info.raster_size(1024 << level);
        let columns =
            (b[0] * w as f64 / TILE as f64).ceil() - (a[0] * w as f64 / TILE as f64).floor();
        let rows = (b[1] * h as f64 / TILE as f64).ceil() - (a[1] * h as f64 / TILE as f64).floor();
        if columns * rows < MAX_JOBS as f64 || level == 0 {
            break (w, h);
        }
        level -= 1;
    };
    let c = [
        (a[0] + b[0]) * 0.5 * w as f64,
        (a[1] + b[1]) * 0.5 * h as f64,
    ];
    let x0 = (a[0] * w as f64 / TILE as f64).floor() as u32;
    let y0 = (a[1] * h as f64 / TILE as f64).floor() as u32;
    let x1 = ((b[0] * w as f64 / TILE as f64).ceil() as u32).min(w.div_ceil(TILE));
    let y1 = ((b[1] * h as f64 / TILE as f64).ceil() as u32).min(h.div_ceil(TILE));
    let mut result = Vec::new();
    for row in y0..y1 {
        for col in x0..x1 {
            result.push(Key {
                doc: info.doc.clone(),
                page: info.page,
                level,
                col,
                row,
            });
        }
    }
    result.sort_by(|a, b| {
        let d = |k: &Key| {
            ((k.col as f64 + 0.5) * TILE as f64 - c[0]).powi(2)
                + ((k.row as f64 + 0.5) * TILE as f64 - c[1]).powi(2)
        };
        d(a).total_cmp(&d(b))
    });
    result.truncate(MAX_JOBS - 1);
    result
}

pub struct Cache<K, V> {
    entries: HashMap<K, (V, usize, u64)>,
    pub bytes: usize,
    pub peak: usize,
    budget: usize,
    clock: u64,
}
impl<K: Eq + Hash + Clone, V> Cache<K, V> {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            peak: 0,
            budget,
            clock: 0,
        }
    }
    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.clock += 1;
        self.entries.get_mut(key).map(|(v, _, t)| {
            *t = self.clock;
            &*v
        })
    }
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }
    pub fn insert(&mut self, key: K, value: V, bytes: usize) {
        if bytes > self.budget {
            return;
        }
        if let Some((_, old, _)) = self.entries.remove(&key) {
            self.bytes -= old;
        }
        while self.bytes + bytes > self.budget {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, (_, _, t))| *t)
                .map(|(k, _)| k.clone())
                .unwrap();
            self.bytes -= self.entries.remove(&oldest).unwrap().1;
        }
        self.clock += 1;
        self.bytes += bytes;
        self.peak = self.peak.max(self.bytes);
        self.entries.insert(key, (value, bytes, self.clock));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_evicts_least_recently_used_before_exceeding_budget() {
        let mut c = Cache::new(8);
        c.insert(1, 1, 4);
        c.insert(2, 2, 4);
        c.get(&1);
        c.insert(3, 3, 4);
        assert!(c.contains(&1));
        assert!(!c.contains(&2));
        assert!(c.contains(&3));
        assert_eq!(c.peak, 8);
        c.insert(4, 4, 9);
        assert!(!c.contains(&4));
        c.insert(1, 9, 2);
        assert_eq!(c.bytes, 6);
    }
    #[test]
    fn viewport_round_trip_and_zoom_anchor_are_stable() {
        for scale in [0.001, 0.5, 3.7, 100.0] {
            let v = View {
                origin: [-37.4, 83.9],
                scale,
                page: [648.0, 1296.0],
            };
            let p = [123.456, 718.22];
            let q = v.to_page(v.to_screen(p));
            assert!((q[0] - p[0]).abs() < 1e-8 && (q[1] - p[1]).abs() < 1e-8);
        }
    }
    #[test]
    fn high_zoom_requests_fresh_detail_without_full_page_allocation() {
        let info = PageInfo {
            doc: "zoom".into(),
            page: 0,
            pages: 1,
            width: 1191.0,
            height: 842.0,
            rotation: 0,
            crop: None,
        };
        let fit = 2400.0 * 0.95 / info.width;
        let mut previous_edge = 0;
        for zoom in [16.0, 32.0, 50.0] {
            let scale = fit * zoom;
            let view = View {
                origin: [
                    1200.0 - info.width * scale / 2.0,
                    800.0 - info.height * scale / 2.0,
                ],
                scale,
                page: [info.width, info.height],
            };
            let tiles = visible_tiles(&info, view, [0.0, 0.0, 2400.0, 1600.0], 2.0);
            assert!(!tiles.is_empty() && tiles.len() < MAX_JOBS);
            let edge = tiles[0].edge();
            assert!(edge >= previous_edge);
            assert!(tiles
                .iter()
                .all(|k| k.edge() == edge && k.level <= MAX_LEVEL));
            previous_edge = edge;
        }
        assert!(previous_edge > 32768);
    }
    #[test]
    fn tile_selection_only_covers_visible_page_and_remains_bounded() {
        let info = PageInfo {
            doc: "a".into(),
            page: 0,
            pages: 1,
            width: 720.0,
            height: 360.0,
            rotation: 0,
            crop: None,
        };
        let v = View {
            origin: [-512.0, 0.0],
            scale: 2.0,
            page: [720.0, 360.0],
        };
        let tiles = visible_tiles(&info, v, [0.0, 0.0, 300.0, 300.0], 2.0);
        assert!(!tiles.is_empty());
        assert!(tiles.iter().all(|k| k.col >= 2 && k.col <= 4 && k.row <= 1));
        assert!(visible_tiles(
            &info,
            View {
                origin: [1000.0, 1000.0],
                ..v
            },
            [0.0, 0.0, 300.0, 300.0],
            1.0
        )
        .is_empty());
        let huge = visible_tiles(&info, v, [0.0, 0.0, 30000.0, 30000.0], 100.0);
        assert!(huge.len() < MAX_JOBS);
        assert!(huge.iter().all(|key| key.level < 5));
    }
}

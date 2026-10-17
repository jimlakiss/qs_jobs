//! Embedded text lives in displayed-page coordinates, separately from raster tiles.
use crate::{model::View, viewer};
use eframe::egui;
use serde::{Deserialize, Serialize};
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TEXT_CHARS: usize = 50_000;
/// Unicode character plus optional normalized displayed-page bounds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Glyph(pub char, pub Option<[f32; 4]>);
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct TextPage {
    pub glyphs: Vec<Glyph>,
    pub truncated: bool,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct Selection {
    pub anchor: Option<[f64; 2]>,
    pub end: Option<[f64; 2]>,
    pub all: bool,
    pub dragging: bool,
    pub tolerance: [f64; 2],
}
impl Selection {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn handle_events(
        &mut self,
        events: &[egui::Event],
        viewport: egui::Rect,
        view: View,
        center: [f64; 2],
        rotation: i32,
    ) -> bool {
        let point = |pos: egui::Pos2| {
            view.normalized(viewer::turn(
                [pos.x as f64, pos.y as f64],
                center,
                -rotation,
            ))
        };
        // Keep hit tolerance constant on screen, including at high zoom.
        let tolerance = [
            3.0 / (view.scale * view.page[0]),
            3.0 / (view.scale * view.page[1]),
        ];
        if self.dragging {
            self.tolerance = tolerance;
        }
        let update_end = |selection: &mut Self, p: [f64; 2]| {
            if let Some(a) = selection.anchor {
                let distance = ((p[0] - a[0]) * view.page[0] * view.scale)
                    .hypot((p[1] - a[1]) * view.page[1] * view.scale);
                if distance >= 3.0 || selection.end != selection.anchor {
                    selection.end = Some(p);
                }
            }
        };
        let mut focused = false;
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } if viewport.contains(*pos) => {
                    let p = point(*pos);
                    *self = Self {
                        anchor: Some(p),
                        end: Some(p),
                        all: false,
                        dragging: true,
                        tolerance,
                    };
                    focused = true;
                }
                egui::Event::PointerMoved(pos) if self.dragging => update_end(self, point(*pos)),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } if self.dragging => {
                    update_end(self, point(*pos));
                    self.dragging = false;
                }
                egui::Event::PointerGone | egui::Event::WindowFocused(false) => {
                    self.dragging = false
                }
                _ => {}
            }
        }
        focused
    }
    pub fn region_bounds(&self) -> Option<[f64; 4]> {
        self.bounds()
    }
    fn bounds(&self) -> Option<[f64; 4]> {
        let a = self.anchor?;
        let b = self.end?;
        if a == b {
            return None;
        }
        let mut r = [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[0].max(b[0]),
            a[1].max(b[1]),
        ];
        // A narrow sweep still selects text; a deliberate large box stays exact.
        for axis in 0..2 {
            if r[axis + 2] - r[axis] < 2.0 * self.tolerance[axis] {
                r[axis] -= self.tolerance[axis];
                r[axis + 2] += self.tolerance[axis];
            }
        }
        Some(r)
    }

    pub fn includes(&self, glyph: &Glyph) -> bool {
        let Some(b) = glyph.1 else {
            return false;
        };
        let x = (b[0] + b[2]) as f64 / 2.0;
        let y = (b[1] + b[3]) as f64 / 2.0;
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return false;
        }
        if self.all {
            return true;
        }
        self.bounds().is_some_and(|r| {
            let width = (b[2] - b[0]) as f64;
            let height = (b[3] - b[1]) as f64;
            if width <= 0.0 || height <= 0.0 {
                return false;
            }
            let overlap_x = (r[2].min(b[2] as f64) - r[0].max(b[0] as f64)).max(0.0);
            let overlap_y = (r[3].min(b[3] as f64) - r[1].max(b[1] as f64)).max(0.0);
            let relevant_area = width.min(r[2] - r[0]) * height.min(r[3] - r[1]);
            relevant_area > 0.0 && overlap_x * overlap_y >= 0.25 * relevant_area
        })
    }

    pub fn copy_text(&self, page: &TextPage) -> String {
        let mut out = String::new();
        let mut previous = None;
        for (index, glyph) in page.glyphs.iter().enumerate() {
            if glyph.0.is_control() || !self.includes(glyph) {
                continue;
            }
            if let Some(prev) = previous {
                let gap = &page.glyphs[prev + 1..index];
                if gap.iter().all(|g| g.0.is_whitespace()) {
                    for g in gap {
                        out.push(g.0);
                    }
                } else if !gap.is_empty() {
                    out.push('\n');
                }
            }
            out.push(glyph.0);
            previous = Some(index);
        }
        out.replace("\r\n", "\n")
            .replace('\r', "\n")
            .trim()
            .to_owned()
    }
    pub fn paint(
        &self,
        painter: &egui::Painter,
        page: &TextPage,
        view: View,
        center: [f64; 2],
        rotation: i32,
    ) {
        if !self.all {
            if let Some(b) = self.bounds() {
                let points = vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]
                    .into_iter()
                    .map(|p| {
                        let p = viewer::turn(
                            view.to_screen([p[0] * view.page[0], p[1] * view.page[1]]),
                            center,
                            rotation,
                        );
                        egui::pos2(p[0] as f32, p[1] as f32)
                    })
                    .collect();
                painter.add(egui::Shape::convex_polygon(
                    points,
                    egui::Color32::from_rgba_unmultiplied(
                        60,
                        145,
                        255,
                        if self.dragging { 18 } else { 5 },
                    ),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 115, 220)),
                ));
            }
        }
        for glyph in &page.glyphs {
            if !self.includes(glyph) {
                continue;
            }
            let Some(b) = glyph.1 else {
                continue;
            };
            let to_pos = |x: f32, y: f32| {
                let p = viewer::turn(
                    view.to_screen([x as f64 * view.page[0], y as f64 * view.page[1]]),
                    center,
                    rotation,
                );
                egui::pos2(p[0] as f32, p[1] as f32)
            };
            let points = vec![
                to_pos(b[0], b[1]),
                to_pos(b[2], b[1]),
                to_pos(b[2], b[3]),
                to_pos(b[0], b[3]),
            ];
            if !egui::Rect::from_points(&points).intersects(painter.clip_rect()) {
                continue;
            }
            painter.add(egui::Shape::convex_polygon(
                points,
                egui::Color32::from_rgba_unmultiplied(60, 145, 255, 80),
                egui::Stroke::NONE,
            ));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fast_drag_and_rotated_selection_follow_pointer_events() {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let view = View {
            origin: [0.0, 0.0],
            scale: 1.0,
            page: [100.0, 100.0],
        };
        for rotation in 0..4 {
            let screen = |p| {
                let p = viewer::turn(p, [50.0, 50.0], rotation);
                egui::pos2(p[0] as f32, p[1] as f32)
            };
            let start = screen([10.0, 10.0]);
            let end = screen([40.0, 30.0]);
            let events = vec![
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(end),
                egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let mut selection = Selection::default();
            assert!(selection.handle_events(&events, viewport, view, [50.0, 50.0], rotation));
            assert_eq!(selection.anchor, Some([0.1, 0.1]));
            assert_eq!(selection.end, Some([0.4, 0.3]));
            assert!(!selection.dragging);
        }
    }
    #[test]
    fn edge_overlap_thin_sweeps_and_clicks_are_predictable() {
        let glyph = Glyph('A', Some([0.2, 0.2, 0.3, 0.4]));
        let mut selection = Selection {
            anchor: Some([0.25, 0.15]),
            end: Some([0.4, 0.45]),
            ..Default::default()
        };
        assert!(selection.includes(&glyph));
        selection.anchor = Some([0.299, 0.15]);
        assert!(!selection.includes(&glyph)); // Merely touching an edge is insufficient.
        selection = Selection {
            anchor: Some([0.1, 0.3]),
            end: Some([0.4, 0.3]),
            tolerance: [0.003, 0.003],
            ..Default::default()
        };
        assert!(selection.includes(&glyph)); // Horizontal sweeps have a small screen-space band.
        selection.end = selection.anchor;
        assert!(!selection.includes(&glyph)); // A click clears; it doesn't select a random character.
    }
    #[test]
    fn completed_selection_does_not_change_when_zooming() {
        let mut selection = Selection {
            anchor: Some([0.1, 0.3]),
            end: Some([0.4, 0.3]),
            tolerance: [0.003, 0.003],
            ..Default::default()
        };
        let before = selection.bounds();
        let view = View {
            origin: [0.0, 0.0],
            scale: 50.0,
            page: [100.0, 100.0],
        };
        selection.handle_events(&[], egui::Rect::EVERYTHING, view, [50.0, 50.0], 0);
        assert_eq!(selection.bounds(), before);
    }
    #[test]
    fn rectangular_copy_keeps_spaces_and_excludes_other_fields() {
        let page = TextPage {
            glyphs: vec![
                Glyph('A', Some([0.1, 0.1, 0.2, 0.2])),
                Glyph(' ', None),
                Glyph('1', Some([0.3, 0.1, 0.4, 0.2])),
                Glyph('\r', None),
                Glyph('\n', None),
                Glyph('X', Some([0.8, 0.8, 0.9, 0.9])),
            ],
            ..Default::default()
        };
        let mut selection = Selection {
            anchor: Some([0.0, 0.0]),
            end: Some([0.5, 0.5]),
            all: false,
            dragging: false,
            tolerance: [0.0, 0.0],
        };
        assert_eq!(selection.copy_text(&page), "A 1");
        std::mem::swap(&mut selection.anchor, &mut selection.end);
        assert_eq!(selection.copy_text(&page), "A 1");
        selection.all = true;
        assert_eq!(selection.copy_text(&page), "A 1\nX");
    }
}

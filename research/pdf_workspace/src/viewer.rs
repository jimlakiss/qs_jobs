//! Local drawing-viewer controls. Rotation affects composition, never source bytes.
use eframe::egui::{self, Color32, Pos2, Rect};
use std::{path::Path, sync::mpsc};

pub fn collect_paths(inputs: &[String]) -> Vec<String> {
    let mut paths = Vec::new();
    for input in inputs {
        let path = Path::new(input);
        if path.is_dir() {
            if let Ok(entries) = path.read_dir() {
                let mut children: Vec<_> = entries
                    .flatten()
                    .map(|e| e.path().to_string_lossy().into_owned())
                    .filter(|p| Path::new(p).is_file())
                    .collect();
                children.sort_by_key(|p| p.to_lowercase());
                paths.extend(collect_paths(&children));
            }
        } else if path.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            paths.push(
                path.canonicalize()
                    .unwrap_or_else(|_| path.to_path_buf())
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

pub fn pick_paths(folder: bool) -> mpsc::Receiver<Vec<String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        let paths = {
            let chooser = if folder {
                "choose folder with prompt \"Open drawing folder\""
            } else {
                "choose file with prompt \"Open PDF drawings\" of type {\"com.adobe.pdf\"} with multiple selections allowed"
            };
            let script = format!("set picked to {chooser}\nif class of picked is not list then set picked to {{picked}}\nset output to \"\"\nrepeat with itemPath in picked\nset output to output & POSIX path of itemPath & linefeed\nend repeat\nreturn output");
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        #[cfg(not(target_os = "macos"))]
        let paths = {
            let _ = folder;
            Vec::new()
        };
        let _ = tx.send(paths);
    });
    rx
}

// Consume modified wheel events before egui converts Ctrl+wheel to zoom.
// Keep this local to the drawing viewport so the file tree scrolls normally.
pub fn take_modified_scroll(
    events: &mut Vec<egui::Event>,
    viewport: Rect,
    mut pointer: Option<Pos2>,
) -> [f64; 2] {
    let mut pan = [0.0, 0.0];
    events.retain(|event| {
        if let egui::Event::PointerMoved(pos) = event {
            pointer = Some(*pos);
        }
        if matches!(event, egui::Event::PointerGone) {
            pointer = None;
        }
        if let egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
        } = event
        {
            if pointer.is_some_and(|p| viewport.contains(p)) && (modifiers.ctrl || modifiers.alt) {
                let unit_scale = match unit {
                    egui::MouseWheelUnit::Point => 1.0,
                    egui::MouseWheelUnit::Line => 40.0,
                    egui::MouseWheelUnit::Page => viewport.height() as f64,
                };
                let distance = (delta.x as f64 + delta.y as f64) * unit_scale;
                if modifiers.ctrl {
                    pan[0] += distance;
                } else {
                    pan[1] += distance;
                }
                return false;
            }
        }
        true
    });
    pan
}

pub fn turn(p: [f64; 2], center: [f64; 2], quarters: i32) -> [f64; 2] {
    let [x, y] = [p[0] - center[0], p[1] - center[1]];
    let [x, y] = match quarters.rem_euclid(4) {
        1 => [-y, x],
        2 => [-x, -y],
        3 => [y, -x],
        _ => [x, y],
    };
    [center[0] + x, center[1] + y]
}
fn corners(rect: Rect, center: [f64; 2], rotation: i32) -> Vec<Pos2> {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
    .into_iter()
    .map(|p| {
        let p = turn([p.x as f64, p.y as f64], center, rotation);
        egui::pos2(p[0] as f32, p[1] as f32)
    })
    .collect()
}
pub fn inverse_bounds(b: [f64; 4], center: [f64; 2], rotation: i32) -> [f64; 4] {
    let points = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]
        .map(|p| turn(p, center, -rotation));
    [
        points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
        points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
        points
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max),
        points
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max),
    ]
}
pub fn paint_page(painter: &egui::Painter, rect: Rect, center: [f64; 2], rotation: i32) {
    painter.add(egui::Shape::convex_polygon(
        corners(rect, center, rotation),
        Color32::WHITE,
        egui::Stroke::NONE,
    ));
}
pub fn paint_texture(
    painter: &egui::Painter,
    texture: egui::TextureId,
    rect: Rect,
    center: [f64; 2],
    rotation: i32,
) {
    let mut mesh = egui::Mesh::with_texture(texture);
    for (pos, uv) in corners(rect, center, rotation).into_iter().zip([
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 1.0),
    ]) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modified_wheel_pans_without_leaking_into_zoom_or_sidebar() {
        let viewport = Rect::from_min_size(egui::pos2(200.0, 100.0), egui::vec2(900.0, 650.0));
        let wheel = |modifiers, unit, delta| egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
        };
        let mut events = vec![
            egui::Event::PointerMoved(egui::pos2(500.0, 300.0)),
            wheel(
                egui::Modifiers::CTRL,
                egui::MouseWheelUnit::Line,
                egui::vec2(0.0, 1.0),
            ),
            wheel(
                egui::Modifiers::ALT,
                egui::MouseWheelUnit::Point,
                egui::vec2(0.0, -25.0),
            ),
            egui::Event::Zoom(1.1),
        ];
        assert_eq!(
            take_modified_scroll(&mut events, viewport, None),
            [40.0, -25.0]
        );
        assert!(!events
            .iter()
            .any(|e| matches!(e, egui::Event::MouseWheel { .. })));
        assert!(events.iter().any(|e| matches!(e, egui::Event::Zoom(_))));
        let mut sidebar = vec![wheel(
            egui::Modifiers::CTRL,
            egui::MouseWheelUnit::Line,
            egui::vec2(0.0, 1.0),
        )];
        assert_eq!(
            take_modified_scroll(&mut sidebar, viewport, Some(egui::pos2(50.0, 300.0))),
            [0.0, 0.0]
        );
        assert_eq!(sidebar.len(), 1);
        let mut ordinary = vec![wheel(
            egui::Modifiers::NONE,
            egui::MouseWheelUnit::Line,
            egui::vec2(0.0, 1.0),
        )];
        assert_eq!(
            take_modified_scroll(&mut ordinary, viewport, Some(viewport.center())),
            [0.0, 0.0]
        );
        assert_eq!(ordinary.len(), 1);
        let mut page = vec![wheel(
            egui::Modifiers::ALT,
            egui::MouseWheelUnit::Page,
            egui::vec2(0.0, 1.0),
        )];
        assert_eq!(
            take_modified_scroll(&mut page, viewport, Some(viewport.center())),
            [0.0, 650.0]
        );
    }
    #[test]
    fn rotation_preserves_pointer_and_visible_bounds() {
        for r in 0..4 {
            let point = [123.25, 487.75];
            let center = [600.0, 400.0];
            assert_eq!(turn(turn(point, center, r), center, -r), point);
        }
        assert_eq!(
            inverse_bounds([0.0, 0.0, 1200.0, 800.0], [600.0, 400.0], 1),
            [200.0, -200.0, 1000.0, 1000.0]
        );
    }
}

/// Native Save As runs off the render thread. Cancellation leaves the export intact.
pub fn save_export(
    name: String,
    bytes: Vec<u8>,
    extension: &'static str,
) -> mpsc::Receiver<Result<Option<String>, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        let result = (|| {
            let script = "on run argv\ntry\nset picked to choose file name with prompt \"Save reviewed drawing data\" default name (item 1 of argv)\nreturn POSIX path of picked\non error number -128\nreturn \"\"\nend try\nend run";
            let output = std::process::Command::new("osascript")
                .args(["-e", script, "--", &name])
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
            }
            let path = String::from_utf8(output.stdout)
                .map_err(|e| e.to_string())?
                .trim_end_matches(['\n', '\r'])
                .to_owned();
            if path.is_empty() {
                return Ok(None);
            }
            if !Path::new(&path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(extension))
            {
                return Err(format!("Choose a file ending in .{extension}."));
            }
            atomic_export(Path::new(&path), &bytes)?;
            Ok(Some(path))
        })();
        #[cfg(not(target_os = "macos"))]
        let result = {
            let _ = (name, bytes, extension);
            Err("Native Save As is currently available on macOS only.".into())
        };
        let _ = tx.send(result);
    });
    rx
}
pub(crate) fn atomic_export(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("Export folder unavailable")?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(".iqs-export-{}-{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod export_tests {
    use super::*;
    #[test]
    fn export_replaces_complete_file_without_temp_debris() {
        let folder = std::env::temp_dir().join(format!("iqs-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("reviewed.json");
        std::fs::write(&path, b"old").unwrap();
        atomic_export(&path, b"{\"sheets\":[]}").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"sheets\":[]}");
        assert_eq!(folder.read_dir().unwrap().count(), 1);
        std::fs::remove_dir_all(folder).unwrap();
    }
}

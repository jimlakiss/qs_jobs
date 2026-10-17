# Local Rust drawing viewer — 17 October 2026

The requested reference is `DocumentGroupsController#viewer`, which selects imported PDFs belonging to a document group, sorts by filename and renders `project_documents/viewer`. It is distinct from the extracted working-document navigation dataset. The shared template also includes extraction and measurement tools; this first app implements its basic viewing workflow.

## Available now

- Native Rust/egui/wgpu app with the existing isolated PDFium renderer and progressive tiles.
- Open multiple local PDFs or PDFs directly inside a folder; drag-and-drop and path input.
- Document sidebar, sheet list, direct sheet number, previous/next and arrow/Page Up/Page Down/Home/End navigation.
- Drag pan, pointer-anchored scroll/pinch zoom, zoom buttons, fit page and fit width.
- Per-sheet view rotation and apply the current additional rotation to every sheet. The PDF's own rotation remains respected.
- Retry PDF action after a worker error.
- Local Mac app bundle with PDFium and its license notices. No Rails changes or network services.

The sheet list currently has no thumbnails. Source PDFs are opened read-only. Rotation, document selection and navigation state last only for the session. The app does not authenticate with Jobs or fetch document groups from Rails. Extraction regions, OCR, CSV/JSON export, splitting/naming, calibrated 2D measurement and 3D are unimplemented. No complete Jobs parity or workflow speed advantage is claimed. Zoom is relative to the fit mode, not physical DPI.

## Verification

- Five Rust unit tests pass, including quarter-turn inverse transforms and viewport bounds used for rotated tile selection.
- Cargo Clippy with warnings denied, formatting and release build pass.
- Bundled executable run without `IQS_PDFIUM_LIBRARY`: structural PDF sheet 2 smoke passed, including tile refinement and worker restart. First preview texture upload: 225.5 ms; paced UI update interval p95: 17.3 ms. These are a single scripted run, not input-to-display latency or a comparative benchmark.
- Manually inspected the live app with structural sheet 1 and 2; clicked sheet 2, rotated it clockwise and switched to a second fixture PDF. Both sheet switching and document switching produced the expected drawing without the old PDF.js page-2 failure.
- Existing earlier checks cover all eight structural pages. Native dialog selection, folder import, drag/drop and all keyboard/gesture combinations have not each received an end-to-end UI test in this increment.

Build: `research/pdf_workspace/build_macos_app.sh`. Output: `research/pdf_workspace/target/iQs Drawing Viewer.app`. Native Mac development build; distribution signing and complete dependency/license review remain separate work.

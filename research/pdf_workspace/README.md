# iQs PDF workspace experiment

M1a research prototype. This standalone Cargo workspace does not modify Jobs runtime code, database schema or API behavior. Findings: [inventory](../../docs/specifications/jobs_feature_inventory.md), [engine comparison](../../docs/research/pdf_engine_comparison.md), [ADR](../../docs/decisions/0001-pdf-measurement-research-boundaries.md), [progressive viewport results](../../docs/research/pdf_progressive_m1a.md).

The current interactive app adds a local document-group-style viewing shell: native macOS PDF/folder selection, drag-and-drop, multiple documents, a lazy filesystem tree and sheet list, page jump and keyboard navigation, fit-page/fit-width, zoom buttons and view-only quarter-turn rotation per sheet or across the document. The demo overlay is hidden in normal use and retained only for the smoke test. This shell does not yet include thumbnails, Split & Name, measurement tools or Rails document-group authentication/data loading. Zoom percentages are relative to the selected fit mode; they are not calibrated physical DPI. Opening a PDF roots the file tree at its containing folder; opening a folder roots it there. Subfolders load on expansion through a background worker, and nested PDFs can be opened directly. Other files are shown but cannot be previewed; Refresh reloads listings, Up changes to the parent folder, and Hidden toggles hidden entries. Links are listed without traversal. Listings are not watched automatically. Rotation and the document list are session-only.

Build the local Mac app with `./build_macos_app.sh`, then open `target/iQs Drawing Viewer.app`. The bundle includes the pinned PDFium runtime and its bundled license notices; it is ad-hoc signed for local development, not notarized for distribution. The renderer remains a subprocess of the native Rust app. No browser/webview is used.

Implemented: native eframe/wgpu viewer, disposable PDFium subprocess, 512-pixel coarse preview, centre-first visible tiles, byte-budgeted LRU caches, stale-work rejection, hard cancellation/restart, PDF path input, page navigation, pointer-anchored zoom/pan, and editable independent blue overlay handles. The child retains one immutable document snapshot, identified by SHA-256 of the exact bytes supplied to PDFium.

This experiment does not implement vector-path extraction/snapping, OCR, calibrated measurement tools, undo, disk persistence, sync or a layer-visibility panel. Overlay edits survive worker restart in memory; they are not saved measurements. Native crashes/pipe errors are surfaced for restart. Source opening is read-only; no PDF is copied into fixtures unless explicitly added separately.

Resource policy: 16 MiB CPU raster cache, 32 MiB logical GPU texture payload, one native job in flight, at most two bounded replies (1 MiB for a raster tile; up to 8 MiB for an on-demand text layer), and at most 32 desired jobs including the preview. Raster resolution uses power-of-two long-edge buckets from 1024 to 524288 virtual pixels; only 512 × 512 tile bitmaps are allocated. Resolution drops if the visible working set would exceed the texture budget. Limits exclude original PDF bytes, native parser/font/image allocations, temporary upload buffers, graphics-driver memory and render meshes. The 512 MiB input limit is a separate research policy, not a whole-process memory ceiling.

The monitor kills obsolete in-flight work on document reopen immediately, or obsolete viewport work after an 80 ms job-age grace. A wanted request has a 15-second deadline; timeout is surfaced for manual restart. Shutdown kills/reaps the child before exiting. Reopening/worker restart can reuse checksum-matching raster cache entries without altering overlay data.

## Reproduce on the current macOS ARM64 host

From this directory:

```sh
python3 generate_fixtures.py
node legacy_parity.mjs
DEVELOPER_DIR=/Library/Developer/CommandLineTools cargo build --release --locked
```

`DEVELOPER_DIR` selects the installed Command Line Tools for this build only. The current Xcode installation otherwise reports an unaccepted licence. No system settings or licence acceptance are changed. On another machine use its working native compiler environment.

Obtain the pinned research runtime (not fetched by the app):

```sh
mkdir -p runtime results
curl -fL 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7881/pdfium-mac-arm64.tgz' -o runtime/pdfium-mac-arm64-7881.tgz
shasum -a 256 runtime/pdfium-mac-arm64-7881.tgz
```

Expected archive hash: `52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40`. Verify before extracting/loading:

```sh
tar -xzf runtime/pdfium-mac-arm64-7881.tgz -C runtime
export IQS_PDFIUM_LIBRARY="$PWD/runtime/lib/libpdfium.dylib"
target/release/iqs-pdf-spike fixtures/vector.pdf
```

For a selected page of any local source:

```sh
target/release/iqs-pdf-spike '/absolute/path/to/drawings.pdf' --page 12
```

Page numbers are 1-based. The toolbar can change pages, open a different path and restart the worker. Drag the page to pan, wheel/pinch at the pointer to zoom, and drag the two blue handles to change the demonstration overlay.

Windows needs a matching PDFium 7881 DLL and `IQS_PDFIUM_LIBRARY` set to its absolute filename; build and DX12 behavior have not been validated. Linux likewise remains untested. Do not reuse the macOS binary on those platforms.

## Progressive validation

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools cargo test --locked
DEVELOPER_DIR=/Library/Developer/CommandLineTools cargo clippy --locked -- -D warnings
python3 run_progressive_checks.py
python3 run_progressive_checks.py --project-pdf '/absolute/path/to/drawings.pdf'
```

The runner exercises the real worker/scheduler, compares stitched tiles to full reference rasters, checks cache reuse, interrupts dense work, tests malformed-file recovery, and runs a nine-second desktop smoke. The optional project test samples five distributed pages, then runs the interactive smoke on the slowest sampled preview page. It reads the original in place; only metrics/logs/screenshots go into ignored `results/`.

The smoke pans/zooms and changes an overlay while rendering continues, then restarts the worker and checks retained overlay coordinates. It reports coarse-preview upload time, tile render time, update intervals during active native work, cache peaks and restart/cancellation counts. It does not inject real mouse gestures or establish input-to-presentation latency. The 100 ms RSS sampler inspects only this test process and its direct worker children, and can miss transient spikes or GPU allocations.

## Original baseline benchmarks

```sh
python3 run_benchmarks.py --viewport
```

The viewport option opens a window briefly, animates texture pan/zoom, requests a screenshot and exits. macOS GUI services and `/usr/bin/time -l` memory instrumentation need a normal desktop process environment; the restricted agent sandbox can block them. Without `--viewport`, only CPU PDF renders and legacy quantity/snap checks run. Results, downloaded runtime and build outputs are ignored by Git.

Use the project's Ruby 3.4.7 and a modern Node runtime. If a runner resolves macOS's older system Ruby, set `IQS_RUBY` to the project Ruby executable and optionally `IQS_NODE` to the desired Node executable; neither script changes your runtime configuration.

Each valid PDF case renders page 1 twelve times, reopening its document each time; reports distinguish the first call from repeated calls. These are not controlled cold-cache measurements. Raster timing includes page acquisition, rendering and RGBA copy, and excludes UI texture upload/presentation. High-water RSS is process-wide, not the tile/cache budget or GPU allocation. Malformed input must return an error, not succeed or crash.

`--benchmark` retains the original full-page 2048-pixel reference path in `src/baseline.rs`. The desktop window and `--smoke` now use the progressive pipeline; old Sprint 0 GUI figures describe the earlier one-texture experiment. Current GUI metrics are **update-to-update intervals**, not GPU timing, input latency or a general 60 FPS guarantee.

Inspect the generated screenshot on macOS:

```sh
sips -s format png results/progressive-project.ppm --out results/progressive-project.png
```

Fixtures are original deterministic synthetic data with hashes/provenance in `fixtures/manifest.json`. `vector.pdf` is a known rectangle; `dense.pdf` contains 20,000 strokes; `multipage.pdf` is a deliberately small 100-page shared-content PDF; `rotated_crop.pdf` probes rotation/crop/UserUnit; `raster.pdf` is image-only; `malformed.pdf` is invalid. They are a seed corpus, not representative real project coverage. No private files are automatically scanned or copied.

## Validation boundaries

The legacy script evaluates selected actual JS functions and invokes the actual Ruby builder, comparing both with ten analytic cases. It does not boot Rails or establish end-to-end browser fidelity. The snap benchmark runs the existing linear search in a Node VM, not in a browser.

The experiment's direct dependency pins and downloaded runtime provenance are documented in the engine comparison. Added `serde` (MIT/Apache-2.0) for the typed, length-bounded IPC protocol and `sha2` 0.10.9 (MIT/Apache-2.0) for source-content identity; resolved versions are in Cargo.lock. Transitive licensing/security review, complete notices and reproducible distribution remain open before production adoption. Do not ship the downloaded research runtime as an approved product dependency.

The [structural PDF regression](../../docs/research/pdf_structural_regression_2026-10-17.md) documents a reproduced PDF.js 3.11.174 failure in the actual Jobs viewer and a local comparison with 5.6.205. `diagnose_jobs_viewer.mjs` is a diagnostic harness only; it does not update production dependencies or publish/upload project files.

Viewport math uses double-precision, top-left displayed-page coordinates. PDFium handles source rotation/cropping for rendering, and replies retain crop/rotation metadata. Stitched pixels are checked against the full-render reference for a rotated/cropped/UserUnit fixture. Raw PDF user-space ↔ canonical unrotated page ↔ calibrated world-unit conversion is still a measurement-adapter task; do not infer that the raster check verifies calibrated quantities.

## Matched viewing memory workload

`run_memory_comparison.py SOURCE --pages N --trials 2` runs a fresh native viewer and/or a dedicated headed Chrome instance through the same 900 × 650 logical-pixel drawing viewport, fit-width, sheet visits, zoom and pan positions, return visits and idle period. Set `IQS_PDFIUM_LIBRARY`, `IQS_TEST_NODE_MODULES` and optionally `IQS_NODE`/`IQS_CHROME` as for the other harnesses. `--engines rust pdfjs3 pdfjs5` selects engines. Results are under ignored `results/memory-comparison-footprint-N.json`.

The opt-in `IQS_MEMORY_WORKFLOW` environment variable activates native scripted navigation; normal interactive app behavior is unchanged. The browser harness uses actual Jobs HTML/JS with local pinned engine assets, imported-document configuration and no authenticated write endpoints. It blocks nonlocal page requests. It keeps normal thumbnail loading and default browser canvas behavior, with snapping and overlays off. Browser pan steps scroll the existing canvas; native pans update visible tile requests.

On macOS, the sampler measures the selected process tree with both `ps` RSS and `footprint --noCategories`. The physical-footprint figure uses the tool's combined `total footprint`, rather than adding historical individual peaks. The Node/Python controllers are outside the counted browser/native trees. Sampling can miss short-lived peaks; idle phases do not prove long-term memory stability. The dedicated browser's fixed overhead is reported separately via the empty-viewer baseline. This is an architecture/workflow comparison, not a claim that Rust as a language inherently uses less memory than JavaScript.

Measured memory findings: [matched viewing comparison](../../docs/research/pdf_viewing_memory_comparison_2026-10-17.md) and [compact results](../../docs/research/pdf_viewing_memory_results_2026-10-17.json).

High zoom: the UI allows 5,000% of fit. Refinement now extends beyond the original 32,768-pixel ceiling to a 524,288-pixel virtual page edge. Only visible 512-pixel tiles are allocated; the cache budgets still apply and can reduce backing resolution on large/high-DPI viewports. Vector detail can refine at higher zoom; embedded scans remain limited by their source pixels.

Filesystem tree implementation: `src/file_tree.rs`. Read-only directory listing runs on a single worker with a bounded request queue; generation checks discard obsolete listings after changing roots or refreshing. Unit coverage includes nested listings, case-insensitive PDF recognition, directory-first sorting and symlink loops. A live QA run opened a PDF two folders below the starting PDF while retaining the original tree root.

Drawing navigation: ordinary wheel/pinch zooms; Control + wheel pans horizontally; Option/Alt + wheel pans vertically. Modified wheel events over the drawing are consumed before egui zoom handling, so scrolling does not also zoom. Directions remain screen-relative when the sheet is rotated. The file tree keeps its normal scrolling behavior.


## Embedded text selection and copy

Choose **Select text**, drag a rectangular region around a label/field, then use **Copy text** or Cmd+C (Ctrl+C is also accepted). Cmd/Ctrl+A selects page text when the drawing has focus; Escape clears selection. Choose **Pan** to return to drag-panning. Modified-wheel panning and wheel/pinch zoom remain available. Selection highlights follow the page through zoom, source crop/rotation and additional view rotation. Selections are temporary highlights, not saved PDF annotations or editable PDF form fields.

A blue selection box appears during dragging and remains around the selected region after release. Characters that substantially overlap its edges are included; narrow horizontal or vertical sweeps have a small screen-space tolerance. A click clears the selection, and completed selections retain the same page bounds when zooming.

PDFium extracts Unicode characters and normalized displayed-page bounds in the isolated worker, only when text mode needs the current sheet. Text shares the 16 MiB CPU cache budget; the UI retains only the current decoded text layer. Transport is capped at 8 MiB, extraction at 50,000 characters per page, with a visible truncation notice. Raster replies remain capped at 1 MiB. Reading order follows PDFium; complex multi-column/CAD object ordering may need cleanup after copying. For scans or vector outlines without embedded Unicode, **Copy text / Cmd+C** falls back to local OCR for the selected box. **OCR selection** forces recognition even when an embedded text layer exists. Recognition runs in the background; its modal preview lets you correct and copy the result. Cancel OCR, start a new selection, or change sheets to cancel a pending request. OCR selection results are temporary and do not change the saved extraction fields.

Validation: `check_text_layer.py` checks extraction and rendered-glyph alignment on synthetic normal and rotated/cropped/UserUnit fixtures, verifies empty text on a raster-only fixture, and can inspect the supplied structural PDF via `--source`. Use the bundled Python runtime with Pillow. Live QA selected **A101**, copied with Cmd+C, pasted exactly **A101**, and confirmed the highlight stayed aligned after view rotation. Quick press/move/release handling has a regression test.

## Capture fields and local OCR (macOS)

Tick the PDFs forming a **Drawing set** in the left sidebar. Selected members are highlighted and the current PDF has a ▶ marker. Normal clicks open a drawing; Cmd/Ctrl-click toggles membership, Shift-click selects a range within a folder, and **Add PDFs here to set** explicitly adds the listed folder PDFs. Only selected drawings enter the shared extraction; unselected neighbours are excluded.

**Click a field, then draw directly.** Initial fields and names match Jobs, with editable display names and stable IDs. Prepared by and Project ID default to **Once**; Sheet ID, Description, Issue ID, Date and Issue description default to **Every page**. The first definition of each field is its master. A master changes inherited regions; drawing, moving or resizing an inherited region creates a local sheet override. Inherited outlines are dashed; masters and local overrides are solid and labelled. Drag inside a region to move it, or drag a selected corner to resize. The field tool remains armed until Pan, Select text, Escape, extraction or closing the panel.

**Remove on this sheet** suppresses a region locally. **Use master on this sheet** removes local overrides/exclusions. **Clear field everywhere** clears the selected field in the current set. **Add another box to this field on this sheet** supports multi-box text, collected top-to-bottom then left-to-right. Region edits have Undo/Redo (50 snapshots). Shared definitions, page overrides and exclusions are separate: navigating to a sheet with partial local state cannot replace the shared masters.

Choose **Extract fields** after placing regions. Once fields are read from their source master sheet once for the selected set and shared across its drawings, including when extraction starts from another PDF. Repeating fields resolve each sheet's overrides/exclusions before reading. PDF text is preferred, with local OCR fallback and **Force OCR** available. Extraction replaces results for defined boxes; reviewed exports never rerun extraction. Failed source files report warnings while remaining selected files continue.

### Staging and export

Choose **Review & export…** to review the complete selected set in one modal. Its default columns follow Jobs: source/page, sheet number, description, revision, filename preview and status. **Show all fields** reveals the remaining fields. Text inputs are single-line, Description and Filename are wide, and header dividers resize columns. Headers remain above the vertically scrolling rows; the horizontal scrollbar is always available when columns exceed the viewport.

Drag a row's six-dot handle to insert it above or below another row. The insertion line and autoscroll work with filtered rows. **Reset order** restores source order. Select all/none, All/Needs review/Ready filters, the Jobs filename presets, custom tokens, sequence-prefix toggle and separator are included. Field edits refresh generated filenames automatically; manually edited filenames stay manual, including after reorder. Sequence numbers follow displayed row order. Editing a Once field applies across the selected set's reviewed rows.

**Save CSV…** / **Save JSON…** use a native Save As dialog. Both export only included rows in their reviewed order. CSV retains the Jobs column order and default field IDs, substitutes custom display labels for fields you rename, and appends source-file metadata. It uses UTF-8 with BOM and quoted values, including commas and quotes; field values are plain single-line text. JSON preserves Jobs field IDs, top-level prepared-by/project values, ordered sheets, filename and lowercase readiness status, with additional field-label/source/checksum metadata. `page` retains the original global sheet index, while `source_page` records the page within its source PDF. The export manifest is not the authenticated Rails extraction endpoint. Proposed filenames do not rename or split source PDFs.

### Local state

The normal app automatically saves the selected set, source identities, masters, local regions/exclusions, captured values, review order/inclusion, manual filenames, naming controls and column widths to `~/Library/Application Support/iQsDrawingViewer/viewer-workspace.json`. A bounded background writer checks snapshots once per second, skips identical state, writes atomically and flushes the latest state on normal exit. Reopening without a file restores the last drawing. A failed/corrupt load is reported and does not overwrite the saved workspace. Benchmark and review-QA modes disable this autosave.

**Save captures** / **Load saved** additionally retain explicit per-PDF JSON snapshots under `~/Library/Application Support/iQsDrawingViewer/captures/<PDF-SHA256>.json`. Source PDFs are never modified by capture or data export.

Validation now includes actual pointer events for row drag/drop, column resize, field arming and inherited-region movement; partial navigation/master preservation; local exclusions; long single-line input/header alignment; workspace round-trip and final-save ordering; and CSV/JSON export. `reference_extractor_cases.mjs` executes the actual Jobs `generateFilename()` and `getStatus()` functions to generate `fixtures/extractor_reference.json`; native tests compare against those results. `IQS_REVIEW_QA=1 IQS_SCREENSHOT_PPM=results/review.ppm <binary> fixtures/multipage.pdf` renders a deterministic 100-row staging scene and exits after five seconds, without writing user workspace state. Native rendering was visually checked; interactive macOS automation and the Save As dialog remained unavailable while the Mac was locked.

Recognition uses Apple's Vision framework through the bundled `iqs-ocr` Swift helper. It runs locally without a network OCR service or downloaded OCR engine. Building the macOS bundle now requires the Command Line Tools Swift compiler as well as Rust. The normal viewer remains Rust/PDFium/egui. Other platforms require a separate recognition adapter.

Extraction uses its own disposable PDFium subprocess, separate from the interactive viewer worker. It verifies the source fingerprint, loads one page text layer at a time, and renders only each requested crop (up to 2048 × 2048 RGBA, nominally 288 DPI). Batch progress streams back through bounded queues. Cancellation kills the capture worker; the OCR helper exits when its parent disappears. Each OCR call has a 30-second deadline and the supervisor stops a capture worker silent for 90 seconds. PDFium/parser allocations are additional to crop/text buffers. This does not change the existing viewer memory benchmark into an OCR benchmark.

Boxes use normalized displayed-page coordinates and follow crop/source rotation and view rotation. OCR applies the view orientation recorded when the box was drawn. Sheets with different title-block placement/orientation may need overrides. OCR currently targets printed English text, with dictionary correction disabled to preserve drawing IDs; handwriting, poor scans and dense tables need review. This increment does not add custom field counts, cloud synchronization or authenticated Rails integration. Local PDF splitting/ZIP packaging, vector snapping is not yet equivalent to the complete Jobs viewer.

Validation: `cargo test --locked` includes scopes, overrides, rename identity, tabular output, saved-state round trips and a fast drag that commits a capture box, direct field-button arming, explicit set membership, excluded neighbours, range selection and set table output. `check_capture.py` exercises the actual bundled worker on embedded text, forced OCR, an image-only PDF, repeated sheets, a one-off capture from a later sheet, a blank override, source crop/rotation/UserUnit, view rotation, and changed-source rejection. Run it with the bundled Python runtime (Pillow/pypdf); Apple's Vision recognition needs normal macOS service access and fails inside the restricted execution sandbox. Live UI inspection for this increment was unavailable because the Mac was locked.

The supplied eight-sheet structural PDF also completed both embedded extraction and forced OCR on its first sheet (1,525 and 1,183 characters respectively). These counts verify operation, not OCR accuracy. The regression script accepts `--source /path/to/file.pdf` for the same check.

Capture region changes are kept when the mouse is released. Yellow means selected: press Enter or click blank drawing space to restore the field colour. The field stays ready for another drag. Escape or Pan stops drawing.

Region keyboard controls (drawing focus): Shift-click adds/removes individual boxes from the selection; drag a selected box to move the group with spacing preserved at page edges. Cmd+C / Cmd+V copies/pastes boxes with stable field IDs on the same or another sheet; same-sheet copies are offset. Delete/Backspace removes selected boxes only on this sheet. Cmd+Z / Shift+Cmd+Z undo/redo each group action in one step. Sidebar field selection still arms drawing directly. Shortcuts do not intercept typing in field-name or staging editors.

While **Capture Fields** is selected, visible capture boxes can be clicked directly in Pan mode without arming a field, including while Add another box is enabled. Turning **Capture Fields** off hides boxes and disables region clicks, dragging and shortcuts, leaving drawing navigation/measurement unobstructed. Hiding capture mode does not delete or exclude any regions from extraction. Selection does not arm drawing: blank-space drags box-select in Select / move regions; Space-drag pans. Delete/Backspace or the toolbar’s **Delete selected** removes the selected boxes on this sheet; Undo restores them. Select Text remains a separate tool.

Plain extraction/export: layout newlines, tabs, nonbreaking spaces and invisible formatting controls are removed from field values. Multiple embedded text runs (including hidden superseded labels) are verified against rendered pixels with local OCR. Dates are normalised to DD/MM/YYYY when unambiguous; invalid or multiple dates require review. Missing defined fields, including Date and Issue Description, no longer count as Ready. Both columns are visible in the default staging view. Excluded sheets are shown explicitly with a per-field restore action. Reopening review refreshes generated filenames from the latest values while preserving manual filename overrides.

The read-only `--export-workspace-csv SESSION_JSON OUTPUT_CSV` diagnostic uses the same staging/export code as the UI without opening a viewer or modifying saved session state. The hidden-revision case in `check_capture.py` verifies that overwritten drawing titles and dates are read from visible pixels.

Date input rules: written English months (full/abbreviated, either order), ordinal days, optional weekday names, ISO year-first dates and numeric dates convert to DD/MM/YYYY. Numeric dates use Australian day-first order; clearly month-first values such as 07/21/2026 are accepted. Invalid dates, multiple values and two-digit years remain flagged for review rather than guessing. Existing written-month values clear obsolete date-validation warnings when review is reopened.

## Local macOS installation

Run `./build_macos_app.sh`, then `./install_macos_app.sh` to install or update `/Applications/iQs Drawing Viewer.app`. Installation quits the viewer normally before updating and preserves `~/Library/Application Support/iQsDrawingViewer/`. Launch the installed app, then choose Options → Keep in Dock from its Dock menu. `IQS_INSTALL_DIR="$HOME/Applications" ./install_macos_app.sh` supports a per-user installation. The versioned logo source is `native/app-icon.png`; every build generates the macOS ICNS asset and embeds it in the app bundle. The bundle identifier stays unchanged so session storage remains compatible. This is a local ad-hoc signed installation; notarised distribution and automatic updates remain separate work.

Bulk region actions: with Capture Fields active and drawing focus, Cmd+A (or **Select all on sheet**) selects every visible region, including inherited and extra boxes. Delete removes those selections only on the current sheet. **Clear all regions** clears all masters, local overrides, exclusions and captured values for the current drawing set (or current PDF when outside the set), while keeping field names/scopes and unrelated drawings. Each bulk action has one-step Undo.

Capture multi-selection: **Select / move regions** allows blank-space drag boxes to select intersecting regions. Shift-click toggles individual boxes; Shift-drag adds boxes, including while a field tool is armed. Drag any selected box to move the group; Space-drag pans without changing geometry. Pan and Select Text exit capture mode. Regression tests separate press, motion and release into distinct frames and check selection persistence, three-box inherited moves, one-step Undo, additive marquee selection and all four page rotations.

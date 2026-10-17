# PDF / 2D / 3D measurement: Sprint 0 findings

17 October 2026. First research pass completed; **Sprint 0's full exit gate remains open** for representative drawings, Windows validation and dependency/distribution review.

Follow-up: the [M1a progressive viewport](pdf_progressive_m1a.md) now tests an approved real project PDF and replaces the initial thread/full-page UI path with a tiled worker-process experiment. Results below are the original Sprint 0 baseline.

## Result

The existing Jobs implementation is mapped, and a working isolated Rust desktop experiment renders PDFs through PDFium, composes them through wgpu/Metal, and pans/zooms an independent overlay. The production application and sibling roadmap are unchanged.

Start with the [feature inventory](../specifications/jobs_feature_inventory.md), [engine comparison](pdf_engine_comparison.md), [architecture decision](../decisions/0001-pdf-measurement-research-boundaries.md), and [experiment/run instructions](../../research/pdf_workspace/README.md). The reproducible [recorded results](pdf_workspace_results_2026-10-17.json) retain individual samples, fixture hashes, runtime identity and test environment.

## Findings that affect architecture

1. Existing measurement parity means length, area/perimeter and count. Volume and 3D are new scope in the inspected workflow.
2. Preserve the entire viewer-state snapshot for migration. The extraction's measurement records alone omit page dimensions and calibration zones.
3. Preserve legacy scale-selection rules explicitly: first point for a line, vertex mean for an area, newest containing zone wins. Later changes need versioned rules and an auditable recalculation.
4. Use one authoritative construction-object model for 2D geometry, future 3D generation and BOQ quantities. PDF rasters, overlays and meshes are projections of that model.
5. The present JS snap search scans arrays. Its dense synthetic query baseline exceeds the proposed 10 ms p95 target before browser input/paint overhead; indexed snapping deserves a measured prototype.
6. Rendering time varies sharply with content complexity. Keep native work off the UI thread and introduce progressive tiles, explicit budgets and cancellation before testing production-size sets.

## Recorded local measurements

Release build, Rust 1.90.0, macOS 26.6.2, Apple M4 Max, 128 GiB RAM. PDFium 7881; pdfium-render 0.9.4; eframe 0.32.3/wgpu 25.0.2. Synthetic PDFs only; no real customer drawings. These figures are observations on a powerful development machine, not product guarantees.

Raster results render page 1 twelve times at up to 2048 × 2048, reopening the document each time. First call is reported separately; subsequent eleven calls are repeated, uncontrolled OS/font-cache measurements. Times include CPU rasterization and RGBA copy, excluding GPU upload/presentation.

| Synthetic PDF | First raster + copy | Repeat p95 | Process peak RSS |
|---|---:|---:|---:|
| Small vector rectangle/text | 51.9 ms | 1.53 ms | 62.8 MiB |
| Dense repeated strokes | 757.6 ms | 771.91 ms | 70.7 MiB |
| Rotated, cropped, UserUnit=2 | 16.2 ms | 1.49 ms | 63.0 MiB |
| 100 shared-content pages, page 1 only | 12.9 ms | 1.65 ms | 62.8 MiB |
| Image-only grayscale | 6.4 ms | 5.75 ms | 59.6 MiB |

The intentionally malformed file returned a format error without crashing. Successful rendering of the rotated/cropped fixture does not establish extraction or coordinate accuracy. The multipage fixture is small and does not represent a dense architectural set.

The native GPU smoke completed 148 sampled frame intervals with a single PDF request during scripted pan/zoom:

| Metric | Observation |
|---|---:|
| Update interval p50 / p95 / p99 | 16.38 / 20.40 / 27.66 ms |
| Process peak RSS | 155.7 MiB |
| PDF raster requests during the test | 1 |

These are paced application update intervals, not GPU timing or input-to-display latency. The test reuses a texture and does not prove responsiveness while several dense pages are rendering. A screenshot was captured and visually checked for PDF content, toolbar and independent blue overlay.

Legacy checks: ten analytic cases pass in both actual JS functions and the actual Ruby quantity builder at relative tolerance `1e-9`. Synthetic snap search at 20,000 points plus 19,999 segments measured 13.26 ms p95 in a Node VM. This is a computational baseline, not a browser benchmark or a new Rust index result.

Validation performed: release compilation; Clippy with warnings denied; Rust formatting; Python/Node syntax; fixture hash verification; five render cases plus malformed input; JS/Ruby differential quantity checks; native Metal smoke and visual inspection. Rails tests were inspected but not run because no production behavior changed.

## Smallest next milestone: M1a progressive viewport

Deliver one desktop window that opens a user-selected real drawing, shows a coarse preview promptly, then refines **visible tiles only** while pan/zoom and overlay editing stay responsive.

- Introduce a `PdfWorker` request/reply protocol: document ID/checksum, page ID, rotation, render scale bucket, tile rectangle, generation and cancellation token. Pixel replies must include the exact transform and request identity.
- Prove worker-process isolation and cancellation. Killing/restarting an unhealthy worker must not lose document edits. Bound requests, CPU pixels and GPU textures separately; tune initial budgets against reference hardware.
- Implement a priority queue for visible tiles and an LRU cache keyed by source identity/page/render configuration. Stale requests must not populate the active viewport. Separate thumbnail/extraction priorities from visible page rendering.
- Verify forward/inverse transforms across crop origins, rotation, UserUnit, high-DPI display and page/world calibration. Import legacy normalized geometry through an explicit adapter.
- Record time to usable preview, tile latency, interaction intervals, cancellation delay and memory high-water marks over repeated page/zoom/open/close cycles. Report cached and uncached behavior separately.
- Validate the same executable workflow on macOS and Windows against a small approved real-project corpus. Add dense hatches, raster scans, font edge cases and encrypted PDFs with expected outcomes.

Acceptance: open and navigate those drawings, edit a simple independent overlay during refinement, cancel stale work promptly, keep allocations within configured budgets, and recover from malformed/native-worker failure. No new measurement migration or 3D kernel is needed to prove this slice.

## Decisions still open

The PDFium/eframe experiment supports further evaluation, not final adoption. An equivalent Tauri spike remains conditional on resolving UI suitability/transfer cost with the shared worker protocol. Hayro still needs fidelity/extraction evaluation. Windows and representative project PDFs are untested. Full transitive security/licence review, runtime notices, reproducible iQs builds, schema/migration and sync-conflict decisions remain outstanding.

For 3D, the initial roadmap describes generating views from measured construction objects. Imported IFC or arbitrary mesh measurement should be specified separately before choosing a solid/BIM kernel.

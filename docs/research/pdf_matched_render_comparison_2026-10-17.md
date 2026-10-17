# Equal-resolution renderer benchmark

17 October 2026. The earlier 512-pixel progressive-preview timing and Jobs full-page timing were not comparable. This follow-up renders the same structural PDF at the **same full-page dimensions** in each engine.

## Answer against the existing Jobs renderer

For pages **1, 5, 6, 7 and 8**, which PDF.js 3.11.174 renders successfully:

| Full-page rendering task | Existing PDF.js 3.11.174 | Rust/PDFium 7881 | Faster renderer |
|---|---:|---:|---|
| First render after fresh document opening | 116.2 ms | 52.2 ms | Rust/PDFium, **2.2×** |
| Repeat render of the same parsed page | 9.9 ms | 42.3 ms | PDF.js, **4.3×** |

Values are arithmetic means of the matching pages' median timings. This does not establish a blanket speed advantage for Rust/PDFium. PDF.js 3.11.174 fails on **pages 2, 3 and 4**, so those pages have no finite speed ratio and are excluded from this comparison.

## Against the newer PDF.js renderer

All eight pages render in PDF.js 5.6.205 and PDFium:

| Full-page rendering task, all eight pages | PDF.js 5.6.205 | Rust/PDFium 7881 |
|---|---:|---:|
| First render after fresh document opening | 197.1 ms | 204.3 ms |
| Repeat render of the same parsed page | 51.5 ms | 182.1 ms |

Initial averages are similar on this workload; repeat full-page rendering favors PDF.js by about **3.5×**. PDFium is particularly slower on the dense vector pages 3–4. This is evidence for keeping the renderer decision open while evaluating fidelity, failure handling, extraction APIs and deployment, rather than choosing based on language alone.

## Per-page render timings

First render / repeat-render medians, excluding document opening and pixel readback:

| Page | Existing PDF.js first / repeat | Rust/PDFium first / repeat | PDF.js 5.6.205 first / repeat |
|---|---:|---:|---:|
| 1 | 65.6 / 22.4 ms | 27.3 / 15.1 ms | 75.0 / 25.0 ms |
| 2 | Rendering failure | 68.0 / 32.5 ms | 224.1 / 75.2 ms |
| 3 | Rendering failure | 634.5 / 588.2 ms | 327.6 / 127.9 ms |
| 4 | Rendering failure | 671.4 / 624.6 ms | 370.6 / 118.2 ms |
| 5 | 153.7 / 5.6 ms | 75.6 / 66.1 ms | 175.6 / 25.3 ms |
| 6 | 119.2 / 7.7 ms | 52.6 / 42.0 ms | 141.3 / 11.8 ms |
| 7 | 119.9 / 6.7 ms | 56.3 / 47.8 ms | 131.0 / 15.4 ms |
| 8 | 122.4 / 7.2 ms | 49.0 / 40.6 ms | 131.8 / 12.9 ms |

## Method and limits

- Same user-supplied PDF, source checksum recorded; Apple M4 Max / 128 GiB RAM / macOS 26.6.2.
- Same 136 DPI and **2249 × 1590 pixels** on pages 1–4; **2248 × 1589** on pages 5–8. Cross-engine dimensions were checked for every successful page.
- Five fresh-document trials per page, each followed by three repeat renders: five first-render samples and fifteen repeat samples per successful page.
- Source bytes are already in memory and engine/worker initialization completes before timing. File IO, browser/window startup, thumbnails, UI layout, snapping, tiles and tile/texture caches are excluded.
- First-render timing starts before page acquisition and includes page preparation, bitmap/canvas allocation, operator preparation and rendering. Document opening and CPU pixel readback are recorded separately. The timed Rust path uses the prototype's pinned PDFium engine in release mode, in a full-page benchmark path, rather than the progressive UI path.
- Repeat renders retain the same parsed page and engine caches, but allocate/rasterize a full page each time. This is not an image-cache hit or a pan/zoom frame.
- Browser render timing is the PDF.js render-promise duration through issued canvas work; it is not a GPU presentation timestamp. Readback figures below provide a separate CPU-ready bitmap endpoint.
- Engines ran sequentially, in the recorded order, with uncontrolled OS/font caches. Ratios characterize this document, machine and backend configuration; they do not establish a universal language performance ratio.

For a stricter **CPU-ready RGBA bitmap** endpoint, adding document opening and pixel readback to first render gives **130.3 ms PDF.js 3.11.174 versus 52.5 ms Rust/PDFium** on the five common pages: about **2.5×** in favor of Rust. Repeat render plus readback gives **19.9 ms versus 42.6 ms**, about **2.1×** in favor of PDF.js. Forced readback is not the normal Jobs display path, so these are supplementary figures rather than an on-screen latency claim.

## Implication for the architecture

The prototype's demonstrated workflow benefits come from progressive previews, rendering only visible tiles, bounded caching and interruptible worker processes. A raw full-page engine test does not quantify those UI benefits. PDF.js also reuses its rendered canvas during some interactions, so neither engine should be forced to rerasterize every pan/zoom frame in a production viewport.

Preserve PDFium as a tested candidate, and keep the newer PDF.js or another renderer under consideration. The existing PDF.js failure still needs repair, including the snapping adapter before an engine upgrade. See the [structural regression findings](pdf_structural_regression_2026-10-17.md).

## Reproduce

`src/baseline.rs` adds `--matched-benchmark` for the pinned native renderer. `matched_render_benchmark.mjs` runs both browser engines and the native benchmark sequentially, with five trials per page. It uses the same local-only assets/runtime setup as `diagnose_jobs_viewer.mjs` and does not modify Jobs or upload the PDF.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools cargo build --release --locked
IQS_TEST_NODE_MODULES='/absolute/path/to/runtime/node_modules' \
IQS_PDFIUM_LIBRARY="$PWD/runtime/lib/libpdfium.dylib" \
node matched_render_benchmark.mjs '/absolute/path/to/drawings.pdf'
```

All individual timings, failures, methods and source identity are retained in the [recorded benchmark JSON](pdf_matched_render_results_2026-10-17.json). Raw runtime assets/results remain ignored. No production application code changed.

# Structural PDF: reproduced Jobs rendering stall

17 October 2026. Read-only test of the user-supplied structural sketch set. The source PDF remains outside the repository. No production Jobs code was changed.

The [equal-resolution benchmark](pdf_matched_render_comparison_2026-10-17.md) follows this functional test. Progressive-preview timings below are not raw renderer speed comparisons. The expanded benchmark also reproduces the old engine's failure independently on pages 3–4.

## Finding

**The current Jobs viewer stalls while rendering page 2 with PDF.js 3.11.174.** Its page-2 thumbnail render triggers:

```text
TypeError: Cannot read properties of undefined (reading 'restore')
    at CanvasGraphics.restore
    at CanvasGraphics.endDrawing
    at InternalRenderTask.cancel
```

The thumbnail promise remains unresolved. `handleSelectedPDFs` awaits `buildThumbnails()` before `renderPage(1)`, and this eight-page document takes the branch that renders every thumbnail first. As a result the first drawing never appears. In the original run the diagnostic's 45-second wait expired. This reproduced a stuck rendering/loading workflow; the JavaScript heartbeat continued, so it was not a permanent main-thread CPU lock in this test.

Skipping thumbnails in the diagnostic allowed page 1 to open, but **full-size page 2 produced the same exception and exceeded a 20-second deadline**. Moving thumbnail work into the background alone therefore does not resolve this file's rendering failure.

The exception identifies a failure during PDF.js canvas-state cleanup. The exact PDF graphics operation that originally triggers cleanup has not been isolated; do not attribute it to a specific transparency group or drawing object without another reduced test.

A **minimal page-2 renderer with no Jobs code, thumbnails, snapping or previous page rendering** produces the same exception in PDF.js 3.11.174. This isolates the engine rendering failure from Jobs' additional loading/measurement workflows.

## Engine comparison

| Test | Result |
|---|---|
| Actual Jobs HTML/JS, PDF.js 3.11.174, normal thumbnail loading | Stalls on page-2 thumbnail |
| Same viewer/engine, thumbnail rendering bypassed diagnostically | Page 1 opens; full-size page 2 stalls |
| Minimal PDF.js 3.11.174 renderer, page 2 only | Same rendering exception without Jobs code |
| Same viewer with bundled PDF.js 5.6.205 module/worker/assets | All eight thumbnails and full-size pages render; no page errors |
| Rust/PDFium worker, all eight pages | Every page's full coarse preview and centre tile render successfully |
| Rust/PDFium desktop, page 1 | First coarse-preview upload 154 ms; p95 update interval 17.54 ms; restart preserves overlay |
| Rust/PDFium desktop, failing source page 2 | First coarse-preview upload 218 ms; p95 update interval 17.35 ms; zoom/restart and overlay-retention checks pass |

The browser comparison used a fresh headless Chrome 155.0.8059.80 profile, the actual repository HTML/JS, and local PDF.js assets. ERB host configuration was replaced, authenticated API/write URLs were omitted, and unused OCR/export dependencies were omitted. No PDF was uploaded to an external service. It does not test the live Rails deployment, existing saved viewer state or the user's production browser session.

## Important upgrade regression

**PDF.js 5.6.205 fixes the tested rendering failure, but it is not a safe drop-in upgrade for measurement.** With the existing Jobs snap decoder, page 1 produced 4,749 snap points and 7,694 segments on 3.11.174. With 5.6.205 it produced zero snap points/segments, including on the vector pages.

The current decoder expects `constructPath` arguments containing separate path-operation and coordinate arrays. The newer operator representation differs. The adapter needs updating and geometry/transform regression checks before switching the production viewer to that engine. Rendering success is not measurement parity.

## File characteristics and native timings

The PDF is **3,447,925 bytes, eight A3 pages**, not encrypted and without document JavaScript. Page 2 has source rotation 90 degrees. Pages 1–4 contain many nested Form XObjects (reusable drawing fragments, not fillable form fields); later pages include large compressed JPEG/CCITT raster content. File size alone does not represent rendering complexity.

Native worker opening plus metadata took 84.8 ms. Per-page coarse-preview rendering ranged from **18.6 to 117.9 ms**; individual centre tiles at a 4096-pixel page long edge took **7.4 to 61.8 ms**. These are local release-build observations on an Apple M4 Max, with uncontrolled caches, not end-to-end promises. Full-page previews were 512 pixels on the long edge; a centre tile is not a full-resolution audit of the entire page.

Individual timings, source checksum, browser error stacks and operation counts are in the [recorded results](pdf_structural_regression_results_2026-10-17.json). Raw logs/screenshots and downloaded dependencies are ignored under `research/pdf_workspace/results` and `runtime`.

## Recommended Jobs repair

1. Upgrade the PDF.js renderer, worker and associated assets together, with the necessary module initialization changes.
2. Adapt vector snapping to the selected operator format and verify known source geometry, nested forms, rotations and raster-only behavior.
3. Display the first page before background thumbnails, and add bounded error/timeout handling so a failed thumbnail/page cannot leave the viewer waiting indefinitely.

Keep this file as a private regression case identified by checksum, rather than committing the project drawing. The Rust prototype also benefits from this case: it exercises a genuinely failing browser renderer, not merely a large file.

## Reproduce the browser comparison

`research/pdf_workspace/diagnose_jobs_viewer.mjs` serves the actual viewer locally with no Rails state/write endpoints. It blocks nonlocal browser requests. Set `IQS_TEST_NODE_MODULES` to the bundled runtime's Node modules directory and extract the pinned `pdfjs-dist-3.11.174.tgz` into ignored `runtime/pdfjs-3.11.174` first. Archive SHA-256: `11402926f38e7d5c71e63f4c3fdcfcb9172afd5bd202900994cecfa8ee7a98c4`.

```sh
node diagnose_jobs_viewer.mjs '/absolute/path/to/drawings.pdf'
node diagnose_jobs_viewer.mjs '/absolute/path/to/drawings.pdf' --skip-thumbnails
node diagnose_jobs_viewer.mjs '/absolute/path/to/drawings.pdf' --raw-page2
node diagnose_jobs_viewer.mjs '/absolute/path/to/drawings.pdf' --modern
```

The `--modern` comparison uses PDF.js 5.6.205 from the bundled runtime, not a production dependency change. `IQS_CHROME` can select a local Chrome executable. Expected rendering failures are recorded as `success:false` in the report, rather than treated as an unhandled diagnostic-process crash. The current report/screenshot filenames identify this structural regression; archive them before testing another source.

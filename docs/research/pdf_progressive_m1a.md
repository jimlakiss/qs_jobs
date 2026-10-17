# M1a: progressive PDF viewport

17 October 2026. Status: implemented and validated as an isolated research vertical slice on macOS. Production M1 remains open.

## What works

The native window now opens a read-only local PDF, shows a coarse preview, and progressively overlays visible 512-pixel tiles. Pan/zoom uses GPU composition. The editable blue overlay is owned by the UI and survives PDF-worker restart. Page selection and a restart control are available in the toolbar; `--page` opens a specified 1-based page.

PDFium runs in a separate disposable process, retains one immutable document snapshot and returns typed metadata plus bounded raw RGBA buffers. Each raster key includes the source checksum, page, resolution bucket and tile position. The UI rejects obsolete session/page replies. A separate monitor can kill a blocked native call; closing the application kills and reaps its child.

The scheduler prioritizes the preview, then tiles nearest the visible centre. Latest desired work replaces obsolete plans; already delivered work is not repeatedly resent when the camera changes. If the visible working set would exceed the texture budget, it lowers resolution instead of cycling tiles through an undersized cache.

The [implementation and commands](../../research/pdf_workspace/README.md), [worker/cache decision](../decisions/0002-progressive-pdf-worker-experiment.md), and [recorded measurements](pdf_progressive_results_2026-10-17.json) provide reproduction details. The Rails application and original roadmap remain unchanged.

## Real project test

Used the original user-supplied architectural tender PDF: **84,541,144 bytes, 45 pages**, read directly from Dropbox. No source drawing was added to the repository or uploaded. Page samples were **1, 12, 23, 34 and 45**. Page 45 had the slowest sampled coarse preview and was selected for the interactive test.

Release build on Apple M4 Max, 128 GiB RAM, macOS 26.6.2. The source fingerprint and individual samples are in the recorded JSON, without the private source path.

| Observation | Recorded result |
|---|---:|
| Open + first-page metadata, separate sampling run | 183.5 ms |
| Coarse preview render, sampled pages | 8.8–91.3 ms |
| Centre tile render at 4096-pixel page long edge | 4.6–48.6 ms |
| Page 45 first coarse-preview upload in desktop test | 349.7 ms |
| Page 45 desktop tile-render p95 | 56.4 ms |
| Desktop update interval p50 / p95 / p99 | 16.61 / 19.60 / 25.18 ms |
| Page 45 update interval p95 | 19.60 ms |
| Update interval p95 while native work was active | 18.80 ms |
| Viewer + worker sampled peak RSS | 313.4 MiB |
| Edited overlay retained after worker restart | Passed |

This is a nine-second scripted pan/zoom/overlay test, including a worker restart. It measures application update intervals, not GPU timestamps, actual mouse input latency or a general 60 FPS guarantee. RSS was sampled every 100 ms across the viewer and its direct children; it can miss transient spikes and excludes graphics-driver allocations. Opening and desktop timings come from separate runs with uncontrolled OS/font caches.

The PDF's default visible content rendered successfully. A layer-visibility panel and comparison against every source layer/state are not implemented. Five page samples and one desktop page do not establish correctness for all 45 pages or every interaction.

## Correctness and recovery checks

- **48 stitched tiles** across vector, image-only and rotated/cropped/UserUnit synthetic PDFs exactly matched the independent full-page reference raster: mean channel difference **0.0** for each fixture.
- CPU cache reuse passed. Recorded CPU raster cache peak stayed at **16 MiB**, and logical texture payload stayed at or below **32 MiB**.
- A dense in-flight native render was interrupted; replacement-document metadata arrived in **22.4 ms** from the plan change. This is cancellation-through-recovery time; the separate `cancel_kill_ms` field measures only the OS kill call.
- An invalid PDF produced an error; a subsequent valid PDF opened through a fresh worker. The UI offers restart for worker errors and retains in-memory overlay state.
- Dense synthetic desktop work also passed: first preview **233 ms**, overall p95 update interval **19.46 ms**, with native rendering active for part of the run.
- Four focused Rust tests cover byte-budgeted LRU eviction, coordinate round trips, bounded visible-tile selection and protocol-allocation bounds. Release build, Clippy with warnings denied, Rust formatting and Python syntax checks passed.

Rendering never occurs on the UI thread. The experiment still uses the original full-page renderer only for baseline CLI measurements and independent validation references.

## Resource policy and limits

Cache ceilings are **16 MiB CPU** and **32 MiB texture payload**. One native request is in flight; the reply queue holds at most two 1 MiB images. Input is capped at 512 MiB. Native parser/image/font allocations, the original document snapshot, temporary uploads and driver allocations are outside cache accounting; these cache figures are not whole-process memory ceilings.

Document reopen cancels obsolete work immediately; obsolete viewport work has an 80 ms job-age grace. A request exceeding 15 seconds is stopped and reported for manual retry. Those are provisional research settings, not production performance promises.

Viewport math uses double-precision top-left displayed-page coordinates, and replies retain source crop/rotation metadata. Pixel equality for the UserUnit/rotation fixture establishes rendering consistency only. The raw PDF-to-canonical-page-to-calibrated-world transform and legacy measurement import remain to be implemented before measurement accuracy is claimed.

## Remaining M1 gates

Windows packaging/DX12 testing; longer navigation/zoom/open/close stress sessions; additional real drawing sets, malformed/encrypted/font cases; layer/form fidelity review; native allocation limits or stronger OS containment; and the complete dependency/binary redistribution audit.

The next implementation slice should add PDF text/path extraction behind this worker boundary and verify source-to-page transforms against known geometry before building snapping and calibrated measurement tools. Keep the viewport and its cache/worker tests as the rendering baseline.

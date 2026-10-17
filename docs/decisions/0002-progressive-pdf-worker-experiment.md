# ADR 0002: progressive PDF worker experiment

Date: 17 October 2026. Status: accepted for M1a research; final PDF/UI stack and production packaging remain unapproved.

## Decision

Extend the isolated eframe/PDFium experiment with a child-process boundary, length-bounded typed IPC, content-addressed raster keys, centre-prioritized visible tiles and independent byte-budgeted CPU/GPU caches. Do not introduce PDFium types or GPU textures into authoritative measurement data.

The child reads and hashes one immutable snapshot, then retains the parsed document. Requests carry a sequence and page/resolution/tile key; replies echo identity and exact full-raster dimensions, tile origin/extent and source metadata. The parent validates header length, payload length and extents before allocating pixels. This permits hard cancellation and native failure recovery while preserving UI-owned overlays.

The latest desired-work plan is bounded. Work that remains desired survives camera-plan updates; obsolete work is dropped. Caches use SHA-256 source identity, zero-based source page, resolution and tile position. Rendering options and source orientation are fixed by the immutable PDF in this experiment; editable rotation/layer/form state must become explicit cache-key configuration before those features are added.

## Budgets and lifecycle

- 512 × 512 preview/tiles; at most 1 MiB RGBA per reply.
- CPU raster LRU 16 MiB; logical GPU texture LRU 32 MiB; at most two queued replies.
- One native job in flight and at most 32 desired jobs; adapt resolution so the visible textures plus preview fit together.
- Reopen cancels obsolete work immediately. Viewport invalidation has an 80 ms job-age grace. A wanted request has a 15-second deadline and surfaces timeout for explicit retry.
- Shutdown kills/reaps the native child. Worker restart preserves in-memory UI overlay geometry and can reuse checksum-matching raster entries.

The PDF byte snapshot has a separate 512 MiB input policy. Cache limits do not cap native parsing allocations, source bytes, GPU driver memory or total application RSS. Process isolation contains a native crash; it is not an OS security sandbox.

## Consequences and evidence

The child-process protocol adds restart/IPC overhead but enables actual interruption of native calls. Fewer/lower-resolution tiles may be shown on unusually large displays to respect the working-set budget. Measurement/world transforms, long-term persistence and layer controls remain separate work.

Stitched tile/reference comparisons, cache reuse, in-flight cancellation, malformed-input recovery, synthetic background rendering and an approved 45-page real project set were tested on macOS. See the [results](../research/pdf_progressive_m1a.md). Windows, exhaustive fidelity, distribution and transitive dependency review remain gates before a production stack-selection ADR.

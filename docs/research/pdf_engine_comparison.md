# PDF engine and viewport research

Date: 17 October 2026. Status: provisional research, not production approval.

## Working recommendation

Use **PDFium + a Rust adapter** for the first native viewport experiment. Keep PDF.js as the existing behavior baseline and a serious Tauri alternative. Evaluate Hayro against the same corpus before committing to the long-term renderer. Use measured fidelity, extraction correctness, deployment effort and bounded-work behavior to decide.

The roadmap's Rust-first principle is compatible with a C/C++ PDF engine behind Rust interfaces. Rendering, extracting paths and calculating construction quantities remain separate responsibilities.

## Comparison

| Candidate | Rendering and extraction | Deployment/maintenance | Licence posture and research disposition |
|---|---|---|---|
| PDFium | Bitmap rendering; positioned text API; path segments/page objects; forms require recursive object traversal and composed transforms | C++ library supplied separately from bindings; match runtime ABI, maintain builds/updates on both OSs; serialize calls or isolate processes | Upstream licence contains BSD-style and Apache terms; third-party notices still required. First prototype candidate; binary audit outstanding |
| PDF.js | Existing browser renderer and positioned text; current snapping interprets operator lists | Existing JS integration; offline bundles and workers needed in Tauri; operator decoding is an adapter maintenance burden | Apache-2.0 project licence; dependency/asset notices still need review. Baseline and web viewport candidate |
| Hayro | Pure Rust interpreter/rasterizer with an abstract device model; exact snapping/text extraction suitability needs a prototype | Avoids a separate C++ runtime; current upstream calls it experimental and lists unsupported cases | MIT/Apache-2.0; transitive licences still matter. Second native engine candidate, fidelity gate required |
| MuPDF | Integrated renderer with structured text and graphics facilities to investigate | Native library and commercial-support route; benchmark only if licensing route is deliberately chosen | Upstream offers AGPL or commercial licensing. Do not integrate into distributed proprietary components under this proposal without a recorded licensing decision |
| Poppler | Existing PDF rendering/text ecosystem; suitable independent reference where available | Platform/build dependencies and redistribution need evaluation | Upstream identifies the project as GPL. Excluded from the proposed distributed proprietary stack pending an explicit decision |

These are source/documentation assessments. Only the isolated PDFium experiment has been implemented in this pass; no head-to-head fidelity or speed ranking has been established.

## PDFium extraction and worker boundary

`FPDFPath_CountSegments`, `FPDFPath_GetPathSegment`, point/type/close queries and page-object transforms provide the building blocks for a path adapter. Positioned text APIs include character boxes. These APIs do not automatically yield a clean construction drawing: preserve curves, nested forms, clipping, source paths and composed transforms; distinguish text outlines, hatches and image content.

PDFium concurrency must be explicit. The current binding describes serialized access; it does not imply parallel rendering. The spike disables the binding's `thread_safe` feature and constructs all handles on one worker. For M1, a worker process provides a stronger boundary for crash containment, cancellation and memory ceilings. A Rust thread alone does not contain a native crash, and ignoring a stale reply is not cancellation.

Primary sources: [path/object API](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_edit.h), [positioned text API](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_text.h), [binding threading guidance](https://github.com/ajrcarey/pdfium-render#multi-threading).

## UI decision remains open

| Approach | What it offers for iQs | What must be measured |
|---|---|---|
| eframe/egui + wgpu | Rust-owned viewport and interaction; GPU texture/overlay composition; direct core integration | QS text/table editing, keyboard/IME, accessibility, high-DPI input, native gestures and Windows behavior |
| Tauri + web viewport | Reuse of HTML/forms and existing extraction UX; Rust core boundary | WebView differences, texture transfer or native-surface integration, event latency, offline packaging and memory |

Tauri uses a core process and OS webviews; selecting it does not mean embedding Chromium identically on both platforms. The comparison must keep the PDF engine and corpus constant when isolating UI costs. Avoid transferring full frames or all geometry through JSON on every pointer event.

Sources: [Tauri architecture](https://v2.tauri.app/concept/architecture/), [eframe 0.32.3 manifest](https://github.com/emilk/egui/blob/0.32.3/crates/eframe/Cargo.toml). The local experiment pins eframe 0.32.3 to match the installed compiler; this is not a recommendation that an old UI version become the production baseline.

## Initial dependency decision record

| Dependency | Pin/features in experiment | Assessment and exit strategy |
|---|---|---|
| pdfium-render | 0.9.4, `pdfium_7881`, default features off | MIT/Apache-2.0; separate runtime, unsafe FFI and ABI maintenance. Do not expose its object types in iQs domain data; replace adapter independently |
| PDFium runtime | bblanchon `chromium/7881`, mac-arm64, no V8/XFA | Research binary only; provenance/hash retained below. Reproducible iQs-controlled build, full notices, supported-platform distribution and security update process remain gates |
| eframe/egui | 0.32.3; fonts, wgpu, Linux window backends | MIT/Apache-2.0 project; assess font and transitive notices. Experimental UI only, separate from schema/rules |
| wgpu | 25.0.2; Metal/macOS, DX12/Windows, Vulkan/Linux | MIT/Apache-2.0 project; backend/driver and shader behavior require platform testing |
| serde_json | Resolved in checked-in Cargo.lock | MIT/Apache-2.0 project; experiment metrics only; schema still requires independent versioning |

The Cargo.lock records dependency resolution, not approval or a security audit. A full transitive SBOM, vulnerability review, font/codec notices and binary redistribution audit remain incomplete. No dependency is added to the Rails application.

M1a adds `serde` 1.0.229 with derive (MIT/Apache-2.0) for typed IPC headers and `sha2` 0.10.9 (MIT/Apache-2.0) for source checksums. These licences were checked in the downloaded crate manifests. Headers/pixel lengths are checked before parent allocations. Both dependencies remain internal to the experiment and can be replaced without changing domain objects. Their transitive dependencies remain part of the outstanding full audit.

Downloaded archive SHA-256: `52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40`.
Extracted `libpdfium.dylib` SHA-256: `1bc45b15466b34cef96641ce25c77a876e70010c6b114f909dda2f5325fc5bd7`.
Archive VERSION: 151.0.7881.0; build args identify ARM64/macOS with `pdf_enable_v8=false`, `pdf_enable_xfa=false`.

The archive's root LICENSE describes its packaging repository; it must not be mistaken for the complete upstream/third-party licence set.

## Decision gates

1. Both macOS and Windows: vector, dense hatch, raster, rotated/cropped/UserUnit, embedded fonts, malformed and encrypted documents.
2. Verify paths and text positions against known geometry, including nested form transforms and clipping. Raster-only pages must return no vector snaps.
3. Measure first usable viewport, uncached/cached page changes, tile throughput, cancellation, frame intervals and process memory over repeated sessions.
4. Compare eframe and Tauri only after a shared engine/worker protocol exists; implement the second spike when UX or transfer cost remains a consequential uncertainty.
5. Complete binary/source dependency review and a reproducible packaging demonstration before a production adoption ADR.

Primary references checked for this research: [PDFium licence](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/LICENSE), [Rust binding licence](https://github.com/ajrcarey/pdfium-render/blob/master/LICENSE.md), [binary build project](https://github.com/bblanchon/pdfium-binaries), [PDF.js licence](https://github.com/mozilla/pdf.js/blob/v3.11.174/LICENSE), [Hayro status and licence](https://github.com/LaurenzV/hayro), [MuPDF licensing](https://mupdf.readthedocs.io/en/latest/license.html), [Poppler upstream](https://poppler.freedesktop.org/), [Poppler source licence](https://gitlab.freedesktop.org/poppler/poppler/-/blob/master/COPYING), [serde_json licence](https://github.com/serde-rs/json/blob/master/LICENSE-MIT). wgpu's downloaded 25.0.2 Cargo manifest also declares `MIT OR Apache-2.0`.

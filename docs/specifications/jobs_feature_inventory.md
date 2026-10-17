# Jobs PDF and measurement inventory

Research date: 17 October 2026. Source baseline: `643a123b93719b03baf0f70660d65f100791da16`.
Status: source inspection plus targeted quantity checks; browser workflow acceptance remains open.
Roadmap reference: `../Strategies & Roadmaps/rust_roadmap_V1.0.md` relative to the repository root.

## Implementation map

| Component | Existing implementation | Role |
|---|---|---|
| Viewer and measurement tools | `public/pdf_viewer/pdf_viewer.js` (5,021 lines at baseline) | PDF.js rendering, regions/OCR, normalized geometry, snapping, tools, autosave |
| Staging and export | `public/pdf_viewer/pdf_splitter.js` (912 lines) | Sheet review, filenames, splitting, export packages, upload |
| Host page | `app/views/project_documents/viewer.html.erb` | UI/SVG/canvas, CDN scripts, `window.qsJobsDocument` config |
| Rails boundary | `app/controllers/project_documents_controller.rb`, `config/routes.rb` | Attachment access, state/extraction/export persistence, source-page remapping |
| Group viewer | `app/controllers/document_groups_controller.rb` | Opens document groups through the shared viewer |
| Stored records | `DocumentViewerState`, `DocumentExtraction`, `ProjectDocument`, `DocumentGroup` | JSONB draft state, extracted metadata, generated file provenance and groups |
| Quantity projection | `app/models/measured_dimensions_builder.rb` | Recalculates saved measurements for project display |
| Project display | `app/views/projects/show.html.erb`, project helpers/controller | Measured Dimensions groups and links back to source/working PDF |

The measurement implementation is in a public JavaScript file rather than Stimulus modules. Export code depends on viewer globals and window hooks. There is no Rust workspace or 3D implementation in the baseline repository.

## Feature and parity checklist

These are acceptance requirements for a replacement; source presence does not establish that all UI paths are bug-free.

| ID | Observed behavior and source anchors | Acceptance scenario |
|---|---|---|
| V01 | Single-file picker/drop, initial authenticated project PDF, multipdf assembly: `handleSelectedPDFs`, `loadInitialProjectPdf`, `loadMultiplePDFs` | Open one file and a group; verify page order and source-document mapping |
| V02 | Page input, next/previous, thumbnails; thumbnail and project session caches; file and session entry limits of 5 | Switch among six working PDFs and return; correct state and page, evicted resources released |
| V03 | PDF.js page canvas; hidden canvas rendered then copied to visible canvas; previous render task cancelled; dimension cap 16,384 | Navigate during rendering; reject stale results; no blank flash or unbounded allocation |
| V04 | Fit width, zoom presets, wheel/pinch/pan; intermediate CSS zoom and debounced rerasterization; configurable screen DPI | Cursor-to-page mapping remains accurate during intermediate zoom and on high-DPI screens |
| V05 | Extra rotation in 90° increments, page/all-page rotation, thumbnail/snap/field invalidation: `rotatePage`, `rotateAllPagesToCurrent` | Rotate a marked cropped page four times; PDF, markers, dimensions and quantities must return consistently; current marker handling needs characterization |
| E01 | Rectangular metadata regions: prepared_by, project_id, sheet_id, description, issue_id, date, issue_description | Extract a known title block; preserve region identity, normalized bounds and field cleanup |
| E02 | Region templates/ghost exclusions, per-page overrides, selection, move, clipboard and pointer paste | Apply template across pages, override one page, delete ghost; save/reopen and compare |
| E03 | Text extraction tests transformed text item origins against region bounds; OCR fallback or OCR-only mode | Compare digital text, rotated text and raster title blocks; record extraction source and manual corrections |
| E04 | Tesseract worker with serialized jobs, crop preprocessing, multipass recognition and field heuristics | Two simultaneous extraction requests cannot corrupt worker parameters; verify IDs containing 0/O |
| E05 | Editable/reorderable staging rows, inclusion/filtering/status, naming presets and sequence prefixes | Reorder/exclude sheets; exported page numbers still identify original source pages |
| E06 | pdf-lib single-page split, extra rotation preserved, ZIP with PDFs + CSV + metadata JSON + viewer-state JSON | Open generated pages, check rotation, filenames, manifests, marker snapshot and provenance |
| M01 | Extraction and measurement modes gated by imported versus `extracted_document` metadata: `initDocumentModeAvailability` | Imported file opens extraction; working PDF opens measurement; navigation stays within working documents |
| M02 | Named measurement groups are `label` strings, with child dimension `name`/legacy `dimensionName`; no group entity ID | Rename a group and a child; preserve grouping and links after save/reopen |
| M03 | Linear tool supports two-point lines and modifier-assisted open polylines: `msrFinishLinear`, `msrLengthMeters` | Horizontal, vertical, diagonal and three-segment lines match known dimensions |
| M04 | Polygon and rectangle area; closed perimeter also calculated: `msrFinishArea`, `msrAreaSqMeters`, `msrPerimeterMeters` | Known rectangle and reversed winding give equal area/perimeter; characterize concave and self-crossing inputs |
| M05 | Count stores individual marker points; named count group receives added points; individual markers removable | Add three markers, move/remove one, undo/redo, reopen; group count equals marker cardinality |
| M06 | Page or polygon/rectangle scale zones; ratio or two-point known-length calibration; `mpp` is a dimensionless paper-to-world ratio | Calibrate 1:100 and a known reference; same geometry yields same quantity; overlapping zone precedence explicit |
| M07 | Scale chosen from first point for lines, arithmetic mean of vertices for areas; latest containing zone wins | Line crosses two zones: preserve legacy result until a deliberate replacement rule is approved |
| M08 | Endpoint and nearest-line-segment snapping; CTM stack and viewport transform; curve endpoints only; arrays scanned linearly | Transformed vector endpoint/edge snaps within threshold; raster-only page yields no fabricated vector snaps |
| M09 | Selection, dragging/nudging, labels, point deletion, group rename/delete, visibility through app mode and SVG | Move selected marker/group geometry, cancel in-progress drawing, delete/recover; verify persisted coordinates |
| M10 | Snapshot undo/redo; capped at 50 snapshots; new edit clears redo | Undo creation/move/calibration and redo; check quantities, selection, page rotation and save state |
| P01 | Viewer state v1 serialized and PATCHed after 1,200 ms debounce, explicit checkpoints and unload hook | Save/reopen exact geometry; inject failures and edit during an in-flight save; verify no unsaved change lost |
| P02 | Extraction POST stores metadata, sheet rows, region templates, measurements and staging | Extraction snapshot is associated with source attachment; confirm separate draft/calibration state retained |
| P03 | Working PDF export links to source attachment and extraction; selected source-page state can be remapped to page 1 | Open a measured dimension via its split PDF; correct marker selected and original provenance preserved |
| P04 | Rails recomputes project rows from saved viewer state; groups area/perimeter/length/count | Saved JS quantities and Rails projection match independent numbers, including missing-scale behavior |

## Existing formats

Page numbers are 1-based object keys (JSON strings). `measurementsByPage[page]` contains:

```json
{"id":42,"type":"area","points":[{"x":0.1,"y":0.1},{"x":0.2,"y":0.1},{"x":0.2,"y":0.2}],"label":"FLOOR AREA","name":"Bedroom 1"}
```

Points are normalized against the displayed, effectively rotated page, with a top-left origin. This is a legacy coordinate frame to import explicitly, not a proposed world-coordinate format. Region rectangles use normalized `x/y/w/h`. Page dimensions are stored in `pageBaseDimsByPage` from a scale-1 PDF.js viewport. Extra rotations are stored separately in `pageRotations`.

```json
{"id":7,"vertices":[{"x":0,"y":0},{"x":1,"y":0},{"x":1,"y":1},{"x":0,"y":1}],"mpp":100,"label":"1:100"}
```

Despite the name, `mpp` is **not metres per pixel**. Distance = `hypot(dx_normalized × width_pt, dy_normalized × height_pt) × 0.0254 / 72 × mpp`. Area uses the absolute shoelace area in page points × `(0.0254/72)² × mpp²`. Reference calibration stores the resulting ratio and label; the actual reference segment and entered length are not retained in the zone record.

Viewer state v1 keys: `version`, `currentPage`, `documentDetails`, `sheetDetailsByPage`, `regionsByPage`, `regionTemplates`, `pageRotations`, `ghostExclusions`, `staging`, `measurementsByPage`, `scaleZonesByPage`, `pageBaseDimsByPage`.

`DocumentViewerState.data` is JSONB, unique per project/Active Storage attachment. It is a whole-state overwrite. There is no version conflict token, offline mutation log or merge protocol in this endpoint. The JSON `version:1` identifies the format, not an edit revision.

Extraction JSON and CSV manifests are sheet metadata; they are not complete project/measurement backups. The ZIP's separate `*-viewer-state.json` carries calibration and marker state. `DocumentExtraction.measurements` alone is insufficient to calculate lengths/areas because it omits scale zones and page dimensions.

## API boundaries

Under `/projects/:project_id/documents/:id`:

| Method/action | Payload/response | Current trust model |
|---|---|---|
| GET `viewer` | HTML with signed/proxied PDF URL, endpoint URLs, initial state and measurement target | Rails session; controller requires admin |
| GET `viewer_state` | `{viewer_state: {...}}`; optional source attachment/page remapping | Project-scoped attachment lookup |
| PATCH `viewer_state` | `{viewer_state: {...}, reason: ...}` → `{ok, saved_at}` | Session + CSRF; replaces snapshot |
| POST `save_extraction` | `{extraction: {document_details,sheets,regions,measurements,staging_data}}` | Existing extraction per attachment updated |
| POST `upload_export` | Multipart `file`, `kind`, `document_group_name` | Active Storage attachment + generated-file metadata |

Desktop authentication, authorization scope, stable domain IDs, API versioning and revision conflicts must be designed; these browser endpoints are evidence of current semantics, not an approved desktop sync API.

## Gaps and correctness questions

- No volume, PIOs, deductions, wall joins, 3D model, solid kernel or automated BOQ linkage found in this workflow. These are new scope. The roadmap's Volume parity wording needs qualification.
- No dedicated midpoint, intersection, perpendicular, curve-nearest or indexed overlay snapping found. The operator loop stops when raw point count reaches 20,000, but one compound path can exceed that threshold. It is not a strict memory budget or an R-tree.
- Rotating a page invalidates caches but does not explicitly transform stored measurement/zone points. Non-square pages, CropBox origins, `/UserUnit`, existing measurements and exported rotations need dedicated tests before importing legacy coordinates.
- A line crossing scale boundaries still uses its first point's zone. Area uses the mean of vertices, which can lie outside a concave polygon. Boundary points follow the existing ray-cast behavior; no explicit edge policy is defined.
- Missing scale/dimensions leaves lines/areas unquantified; count still works. Degenerate geometry, invalid coordinates and zero-length reference calibration need explicit validation policies.
- Numeric IDs are allocated by `msrIdCounter`; `restoreViewerState` does not visibly reconcile that counter against restored measurements/zones. Characterize potential collisions before reusing IDs across documents.
- `saveViewerState` returns the existing promise if a save is in flight; there is no explicit dirty-generation acknowledgement. An unload keepalive can exceed the browser's 64 KiB body limit. Recovery and concurrent edits require testing.
- Undo stores complete snapshots; staging, document details and all extraction/UI state are not in the same undo transaction. Large projects need bounded commands or another measured strategy.
- Script versions are loaded from CDNs: PDF.js 3.11.174, Tesseract.js 5.0.4, pdf-lib 1.17.1, JSZip 3.10.1. Production desktop distribution needs pinned local assets and a dependency review.

## Validation so far

`node research/pdf_workspace/legacy_parity.mjs` executes selected **actual production functions**, compares JavaScript and Ruby against ten analytic cases at relative tolerance `1e-9`, and samples the existing snap search with synthetic dense geometry. It does not test the DOM, PDF operator decoding, saves, export or user gestures.

Existing Rails controller tests cover stored measured dimensions, viewer state, extraction/export, working document navigation and source-page remapping in `test/controllers/projects_controller_test.rb` and `test/controllers/project_documents_controller_test.rb`. They were inspected, not run as part of this isolated research pass. The PDF data used by many controller tests is only a `%PDF` stub; those tests do not establish PDF rendering fidelity.

# ADR 0001: PDF measurement research boundaries

Date: 17 October 2026. Status: accepted for the isolated research experiment; production technology selection remains proposed.

## Context

Jobs already stores calibrated 2D geometry in viewer-state JSON and recomputes quantities in Ruby. The new module must support reliable PDF interaction and later construction objects/3D without losing measurement provenance or coupling quantities to a rendering engine.

## Decision

- Keep the experiment under `research/pdf_workspace`, with its own Cargo workspace and lockfile. Keep research/specifications/ADRs in this repository so findings are versionable beside their source. Preserve the original sibling roadmap unchanged.
- Prototype PDFium with eframe/wgpu first. This establishes a working candidate, not the final UI/PDF engine choice. Retain PDF.js/Tauri and Hayro as evaluation alternatives.
- Own a document model with stable IDs independent of PDF engine, UI, meshes and Rails attachment IDs. Retain explicit source attachment/blob checksum, page identity and imported legacy ID mapping.
- Separate PDF services, document state, viewport composition, interaction and quantity rules. PDF rasters and render meshes are disposable derived data. Editing an overlay must not rerasterize the PDF.
- M1 PDF work must have explicit request identities, bounded queues/caches, prioritized visible tiles and stale-result rejection. Native crash containment and hard cancellation should be proved with a worker process.
- Use double precision for authoritative geometry and explicit transforms: PDF user space → unrotated canonical page coordinates → view. World units/calibration are separate. GPU coordinates can be relative/single precision. Preserve crop origin, rotation and `/UserUnit` metadata; validate them rather than assuming all PDFs use a simple 72-point, zero-origin frame.
- Legacy import must preserve normalized geometry, effective dimensions, calibration selection semantics and source orientation before any conversion. Import cannot silently reinterpret points or recalculate quantities under new rules.
- Quantity outputs are a deterministic projection of geometry, calibration revision and measurement rule version. Store gross/deductions/net separately when introduced. 3D tessellation must not become a second measurement source.
- Generated 3D from measured 2D is the roadmap's initial 3D scope. Measuring imported IFC or arbitrary 3D meshes is a separate capability requiring its own units, placement, topology and provenance specification.

## Proposed production workspace

Create it only after the experiment passes its gates:

```text
apps/desktop
crates/iqs-document       # IDs, page metadata, calibration, command schema
crates/iqs-pdf            # adapter protocol; engine worker owns native handles
crates/iqs-render         # tiles, GPU composition, overlay display
crates/iqs-geometry       # transforms, predicates, precision policy
crates/iqs-interaction    # tools, indexed snapping, hit-testing, commands
crates/iqs-measure        # versioned quantity rules, legacy compatibility
crates/iqs-storage        # transactional persistence and migration
crates/iqs-sync           # authenticated versioned API, revisions/conflicts
crates/iqs-pio            # introduced with the first wall vertical slice
```

Do not create empty crates as a substitute for a working vertical slice. The PDF adapter cannot depend on BOQ or PIO logic; measurement code cannot depend on UI/GPU types. Rails remains the business platform.

## Alternatives and consequences

A full Jobs rewrite would delay evidence and risk existing workflows, so it is excluded from this experiment. A renderer-owned annotation model would couple project data to one engine. A separate 3D quantity model would create reconciliation problems. A pure Rust PDF renderer could simplify runtime distribution, but fidelity remains an empirical gate. The costs of the selected experiment are a native dependency, worker lifecycle management and a second UI environment to evaluate.

## Follow-up gates

ADR 0002 should select a provisional M1 engine/UI only after corpus/platform evidence and dependency review. ADRs for schema/legacy migration, worker protocol/cache budgets and sync conflict handling follow before those capabilities become production data paths.

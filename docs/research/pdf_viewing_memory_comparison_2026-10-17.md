# PDF viewing memory comparison — 17 October 2026

Same viewing workload on the current Mac: open, fit width, visit pages 1/2/3/4/last, zoom 2× and 4× with pan endpoints, revisit sheets twice, then idle eight seconds. Two fresh runs per engine/file; engine order reversed on run two. No overlays or snapping. All figures below are MiB (1,048,576 bytes).

Memory is macOS `footprint` combined `total footprint` for the viewer process tree. Native includes the PDFium worker; browser includes a fresh dedicated headed Chrome and all descendants. Python/Node controllers and the user’s existing browser/app sessions are excluded. Peak means the largest sampled simultaneous total, not the sum of each process’s separate historical peak.

| File | Engine | Completed | Empty baseline | Workflow peak | After idle |
|---|---|---|---:|---:|---:|
| Structural sketches | Rust / PDFium | 2/2 | 484–485 | 612–613 | 600–601 |
| Structural sketches | Jobs / PDF.js 5.6.205 | 2/2 | 886–928 | 1951–2099 | 1078–1202 |
| Grove Street architectural | Rust / PDFium | 2/2 | 494–496 | 760–776 | 702–709 |
| Grove Street architectural | Current Jobs / PDF.js 3.11.174 | 2/2 | 821–906 | 1590–1609 | 1020–1071 |
| Grove Street architectural | Jobs / PDF.js 5.6.205 | 2/2 | 874–889 | 1735–1736 | 1069–1206 |

Across these two runs, Rust used about **52% less peak footprint than current Jobs on Grove Street**, and about **70% less than the upgraded PDF.js workflow on the structural set**. Grove Street idle footprint was about 33% lower. The fixed empty-viewer baseline contributes to the total difference; the per-phase figures and baseline make that visible. These are results for these workloads and implementations, not an intrinsic language comparison.

## Memory by phase

Ranges across both runs. Opening and zoom/pan are phase peaks; idle is the median footprint after returning to sheet 1.

| File | Engine | Opening peak | Zoom/pan peak | After idle |
|---|---|---:|---:|---:|
| Structural sketches | Rust / PDFium | 534–534 | 612–613 | 600–601 |
| Structural sketches | Jobs / PDF.js 5.6.205 | 1951–2099 | 1626–1714 | 1078–1202 |
| Grove Street architectural | Rust / PDFium | 620–624 | 698–701 | 702–709 |
| Grove Street architectural | Current Jobs / PDF.js 3.11.174 | 1358–1370 | 1502–1511 | 1020–1071 |
| Grove Street architectural | Jobs / PDF.js 5.6.205 | 1396–1449 | 1730–1733 | 1069–1206 |

Current Jobs uses PDF.js 3.11.174. PDF.js 5.6.205 is a separate diagnostic substitution using the Jobs viewer code, not a deployed upgrade. Current Jobs again failed to finish structural-PDF opening in both preliminary trials; no completed-workflow memory ratio is assigned to that failure.

The empty baseline is reported to expose the dedicated browser’s fixed overhead. Peak minus baseline can help compare the workload’s additional cost, but is not an isolated PDF-engine allocation measurement.

## Interpretation limits

- The same 900 × 650 logical-pixel drawing viewport, page dimensions and fit-relative zoom positions are used. Normal native high-DPI tiled backing and normal browser canvas backing remain different; bitmap allocation is an architectural difference under test. Browser scrollbars are hidden in the final footprint runs so client dimensions remain exactly 900 × 650.
- Jobs retains its normal thumbnail loading and initial default-DPI render before the requested fit-width view. Rust opens directly at fit width. This compares the application workflows, not just parsers or equal-sized bitmap renders.
- Native phases wait for visible tile work to settle; browser phases await Jobs rendering. Pan-only browser steps scroll its existing canvas. Subsequent phases wait two seconds (three on open and baseline; eight on final idle).
- `footprint` accounts for memory differently from summed RSS and includes allocations that RSS does not describe well. It is still an OS accounting snapshot, not a complete inventory of every graphics-driver allocation. Samples can miss short peaks.
- Two short runs and eight seconds of idle do not establish leak-free long-term operation. No 2D/3D measurement load, text extraction, OCR, splitting or multi-document session is included.
- The browser harness uses actual Jobs HTML/JS with local pinned assets, imported-document configuration, no Rails write endpoints and nonlocal page requests blocked. This does not measure a live authenticated Rails session or an ordinary multi-tab browser session.

## Reproduce

Use `research/pdf_workspace/run_memory_comparison.py SOURCE --pages N --trials 2 --engines rust pdfjs3 pdfjs5` with the environment described in its README. Structural completed-workflow footprint runs select `rust pdfjs5`; the current-engine failure is retained from the earlier two trials. The opt-in native workflow does not change normal interactive behavior.

Raw samples and per-run logs stay under ignored `research/pdf_workspace/results`. Source PDFs remain at the user-supplied Dropbox paths and were not copied into the repository or uploaded externally. Compact per-phase results, engine versions and source checksums are in [the recorded results](pdf_viewing_memory_results_2026-10-17.json).

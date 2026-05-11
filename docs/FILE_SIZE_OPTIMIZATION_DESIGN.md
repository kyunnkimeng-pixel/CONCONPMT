# PMTCONCON Studio File Size Optimization Design

Stage: `15_REAL_QA_STABILIZATION`
Date: 2026-05-10

## Decision

GIF/file-size optimization should be designed before adding a user-facing optimizer button. The current stabilization patch adds the export grid and per-piece problem reporting first, so oversized GIFs can be identified by exact export index and piece. Actual optimization remains a follow-up implementation task.

## Goals

- Reduce oversized GIF exports without replacing or damaging imported originals.
- Keep animation, frame timing, transparency, and loop settings predictable.
- Let users compare quality/size tradeoffs before applying an optimized derivative.
- Record which export index failed size validation and provide a direct path back to the icon editor.

## Non-Goals

- No automatic destructive overwrite of source files.
- No external upload, cloud processing, scraping, login, or network posting.
- No fake optimizer menu before a working local pipeline exists.

## Candidate Strategies

1. Frame resize/downscale: reduce output cell size only in custom profiles, or warn for DCInside if this would violate 200x200.
2. Palette reduction: lower GIF palette color count in controlled steps.
3. Frame deduplication: remove duplicate or near-duplicate frames while preserving visible timing.
4. Delay normalization: merge very short frame delays where visually acceptable.
5. Lossy GIF quantization: optional, previewed, and never applied to originals.

## Proposed MVP

- Add an optimizer action only for exported GIF pieces over the configured byte limit.
- Run local optimization attempts in a temp folder.
- Show before/after size, frame count, and a visual preview.
- Save optimized output as a generated derivative, not as a replacement original.
- Keep export manifest entries for original rendered bytes and optimized bytes.

## Current Stabilization Coverage

- Export now continues after non-blocking post-render size issues when files can be produced.
- The export grid marks problematic pieces and exposes their export index, filename, alt value, status, and edit button.
- `export-manifest.json` includes validation errors and warnings for diagnostics.

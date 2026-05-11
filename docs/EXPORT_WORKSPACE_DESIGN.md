# Export Workspace Design

Current stage: `EXPORT_WORKSPACE_UI_REFERENCE_AND_DESIGN`

This document describes the public product design for the PMTCONCON Studio export workspace. Generated UI images are layout references only; the product contract remains in `docs/PRODUCT_SPEC.md`, `docs/FEATURE_INVENTORY.md`, and `docs/ARCHITECTURE.md`.

## Reference Images

| Reference | File | Purpose |
| --- | --- | --- |
| Preflight | `docs/ui-references/export-workspace/01-export-workspace-preflight-before-after.png` | Review planned output before starting export. |
| In progress | `docs/ui-references/export-workspace/02-export-workspace-in-progress-before-after.png` | Show one global progress indicator plus per-item status. |
| Completion | `docs/ui-references/export-workspace/03-export-workspace-completion-before-after.png` | Summarize written files, warnings, and failed items. |
| GIF optimization | `docs/ui-references/export-workspace/04-gif-optimization-before-after.png` | Compare a GIF baseline with measured optimization candidates. |

## Goals

- Make the exported images visible before and after generation.
- Keep source icons and generated export pieces in a clear before/after layout.
- Separate file generation success from upload-readiness validation.
- Continue exporting renderable items even when other items have warnings or upload-readiness problems.
- Keep one global progress indicator for each export job.
- Preserve original imported files and store all generated output as derived files.

## Non-Goals

- No DCInside upload, login, posting, scraping, or browser automation.
- No cloud sync, accounts, marketplace, premium, community upload, online sharing, or remote storage.
- No UI action should appear unless it maps to implemented behavior or an explicitly planned MVP command.
- Generated image details such as sample thumbnails, colors, shadows, and exact labels do not create product requirements.

## Workspace Structure

### Toolbar

The toolbar should stay compact and task-focused:

- Profile selector for DCInside or a custom export profile.
- Output folder picker.
- Filename mode selector for sequence or alt-value filenames.
- Preflight action.
- Start export action.
- Cancel action while a job is running.

### Summary

The workspace should expose summary counts for:

- Selected items.
- Excluded items.
- Upload-ready items.
- Warnings.
- Not-upload-ready items.
- Render or write failures after a job runs.

Useful filters are all items, problem items, excluded items, and upload-ready items.

### Before/After Area

The main workspace uses two high-visibility panes:

- Source pane: imported icons or planned export items, including display name, shape, readiness, and include/exclude state.
- Output pane: generated pieces with export number, thumbnail, filename, alt value, format, byte size, limit, and status.

Multi-piece icons should remain visually grouped while still exposing each exported piece.

### Item Details

Selecting an item should show:

- Original icon identity and source preview.
- Piece-level alt values.
- Current validation errors and warnings.
- Export status and output path after export.
- Available actions such as edit, reveal original, reveal output, include/exclude, retry, or optimize when those actions are implemented.

## Export Semantics

- Preflight validates the planned output but does not write final files.
- Export writes renderable items and records per-item results.
- A validation warning should not block export unless strict validation is enabled.
- Upload-readiness errors should mark the item as not upload-ready, but should not stop unrelated renderable items.
- Render/write failures affect only the failed item and must not crash the whole job.
- Final output should be measured and revalidated after writing.

## Reports

The export workspace should produce predictable output:

- `files/` for generated images.
- `alts.txt` when enabled.
- `export_report.txt` for a readable summary.
- `export_report.json` for structured diagnostics.
- `export_issues.csv` for spreadsheet review.

Report rows should include export index, icon ID, piece ID, display name, alt, filename, status, byte size, limit, warnings, errors, and suggested fix.

## Reliability Rules

- Use item-level errors instead of aborting the whole export job.
- Write temporary files before moving them into the final output path.
- Keep GIF export concurrency conservative.
- Avoid holding decoded image batches in memory longer than needed.
- Throttle progress events so the UI remains responsive.

## GIF Optimization Hook

Oversized GIF output should be treated as an optimization candidate. Optimization must generate measured derivative files, show the tradeoff before applying, preserve the original source GIF, and re-run validation after a candidate is applied.

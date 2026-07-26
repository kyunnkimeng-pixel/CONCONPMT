# PMTCONCON Studio 0.2.0 Release Notes

PMTCONCON Studio 0.2.0 is a major local editing-workflow update. It expands the
non-destructive editor, sprite-sheet tools, animation effects, and collection cloning
while preserving imported originals and the existing Explorer-style workflow.

## Highlights

- Added non-destructive horizontal/vertical flip and 90-degree rotation for static
  images, GIFs, and multi-piece icons.
- Added arbitrary frame-sheet to GIF creation with grid review, frame ordering,
  duration editing, realtime playback, and forward/reverse/ping-pong output.
- Added deterministic built-in image effects including pixelate, color adjustment,
  grayscale, sepia, blur, sharpen, outline, and shadow.
- Added 16 motion presets across spatial motion, procedural displacement,
  color/opacity animation, and overlays such as focus lines, sparkles, and rings.
- Completed collection duplication for all durable visual metadata, animated
  multi-piece output, frame-sheet provenance, presets, and active optimized variants.

## Editor And Motion

- Transform, effect, and motion recipes are stored as versioned metadata; imported
  originals are never overwritten.
- The same native Rust rendering pipeline is used by preview, export, optimization,
  static work sheets, and GIF frame sheets.
- Motion rendering uses bounded duration, FPS, cycles, parameters, and deterministic
  seeds. Static sources can become animated GIFs, while existing GIF timing is
  evaluated by timestamps.
- Multi-piece effects are applied to the combined viewport before splitting so
  horizontal and vertical double icons remain visually continuous.
- Reset and restore actions now state their exact scope, and the advanced editor keeps
  image effects and motion in separate discoverable tabs.

## Sprite And Frame Sheets

- Existing static multi-emoticon sheet import/export now uses clearer offset, cell,
  padding, row/column, and reading-order concepts informed by sprite-editor workflows.
- Frame strips support Ctrl/Shift selection, drag and keyboard reorder, reverse,
  duplicate, delete, per-frame duration, and FPS convenience input.
- Generated GIFs preserve the original source sheet and store versioned provenance for
  later inspection.
- Static and GIF sheet presets can be scoped to a collection and survive collection
  duplication.

## Collection Duplication

- Collection, profile, icon, piece, crop, alt, note, cover, placeholder, text,
  transform, effect, motion, loop, ping-pong, and sheet settings are remapped to new
  stable IDs.
- Current previews and effective active optimization variants are copied to independent
  owned paths. Target hashes are recalculated for the cloned IDs and profiles.
- Stale or missing optimization artifacts fall back to the saved render recipe.
  Optimization jobs and previous export paths are intentionally reset.
- Duplicate requests are single-flight in both the toolbar and context menu.

## Safety, Compatibility, And Validation

- Existing libraries are migrated through additive SQLite migrations for transforms,
  frame-sheet GIF provenance, effects, and motion recipes.
- GIF frame timing and loop behavior remain animated in preview and export.
- Input dimensions, frame workloads, animation parameters, sheet layouts, and generated
  artifacts retain bounded validation.
- No new runtime dependency was added for transforms, effects, motion, or collection
  duplication. PMTCONCON Studio remains MIT licensed and local-only.

## Distribution Notes

- The selected Windows release artifact is the NSIS setup for version 0.2.0.
- `SHA256SUMS.txt` contains the SHA-256 checksum for the published NSIS setup.
- The installer is unsigned, so Windows may display an unknown-publisher warning.
- The MSI package is built but withheld until a clean Windows VM install/uninstall pass
  is completed.

## Known Notes

- The production web bundle reports a large JavaScript chunk warning. This does not
  block the desktop package.
- Optional `cargo-deny` and `cargo-about` checks are skipped when those tools are not
  installed; the repository's dependency and forbidden-license guardrails still run.

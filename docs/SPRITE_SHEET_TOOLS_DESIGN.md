# Sprite Sheet / Work Sheet Tools Design

Current stage: `PROFESSIONAL_SPRITE_SHEET_TOOLS_MVP`

## 1. Product Goal

PMTCONCON Studio provides professional non-AI sheet tooling for emoticon production. The workflow is reversible and manual: users import a large fixed-grid sheet, review and select cells, export an editable work sheet, and reimport edited cells through a manifest without overwriting originals.

This is not a compact game-atlas packer. MVP sheets are fixed-grid edit sheets with no trim, no rotate, and no packing. Generated guide sheets and manifests are reference material; the clean sheet is the editable artifact.

## 2. Static Sprite Sheet Import

Static import accepts PNG/JPG/JPEG. GIF is excluded from this flow because animated sources belong to GIF Frame Sheet tools.

Supported MVP modes:

- Grid mode: rows, columns, borders, gaps, and read order.
- Cell-size mode: cell width/height with inferred rows/columns.
- Manifest mode: reimport edited clean sheets with `pmtcon-sheet-v1`.

The backend analyzes the sheet image, computes cell rectangles, detects transparent empty-cell candidates, and returns cell metadata before import. The user chooses cells explicitly in the UI; PMTCONCON Studio never auto-imports detected cells without review.

On import:

- The original sheet is copied into `sheet_imports/original_sheets/`.
- Selected non-empty cells are cropped into PNG files under `sheet_imports/extracted_cells/`.
- Cropped cells are registered as new imported assets/icons.
- Alt values remain empty unless the user edits them later.
- PNG alpha is preserved.

## 3. Static Edit Sheet Export

Static export creates a fixed-grid work sheet from the current collection. MVP uses current collection items. Selected-item and problem-item source modes can be added once the surrounding selection/review state is exposed to the sheet dialog.

Outputs:

- Clean sheet PNG: editable, no grid, no labels, no overlays.
- Guide sheet PNG: reference-only, checker/grid/numeric labels.
- Manifest JSON: `pmtcon-sheet-v1`.

The clean sheet uses the selected background. With `transparent`, gaps and borders remain alpha-transparent. Guide sheets can use checkerboard visualization so transparent artwork is visible.

## 4. Manifest-Based Reimport

Reimport reads `pmtcon-sheet-v1`, validates required fields, maps edited sheet pages by clean sheet filename, crops each manifest cell, and imports results as new icons by default. Processed-variant file output is supported by the backend mode but is not yet promoted as the primary UI path because export-active variant semantics need a follow-up data model decision.

Reimport never overwrites original source files. Mismatched dimensions, missing pages, and out-of-bounds cells become item-level warnings/errors.

## 5. GIF Frame Sheet Export

GIF Frame Sheet is designed for the next stage. The manifest schema and page-planning helper are implemented, but frame decoding/export UI and Tauri commands are intentionally not exposed as clickable UI in this MVP.

Target behavior:

- Decode one GIF icon at a time.
- Export every frame into one or more PNG frame sheets.
- Preserve frame order, duration, loop mode, disposal where available, and page mapping.
- Write `pmtcon-gif-frame-sheet-v1`.

## 6. GIF Frame Sheet Reimport

Planned behavior:

- Read `pmtcon-gif-frame-sheet-v1`.
- Validate edited frame sheet pages, frame count, dimensions, and page mapping.
- Reassemble a new animated GIF processed variant.
- Preserve duration and loop metadata.
- Never overwrite the original GIF.

## 7. Manual Slice Mode

Manual Slice Mode is designed as future work. Data model fields are documented and a validation model exists, but no visible menu action is exposed in the MVP.

Future UX:

- Add, move, resize, duplicate, delete named slices.
- Type exact X/Y/W/H.
- Snap to grid.
- Import/export selected slices.

## 8. Auto-Detect Mode

Auto-detect remains future/experimental. It may infer separator rows/columns from transparent backgrounds or solid background colors, but must only propose settings. It must never auto-import without user review.

## 9. PNG Transparency Handling

Rules implemented in MVP:

- PNG sheet import decodes to RGBA and crops cells as PNG.
- Alpha is preserved during extraction and clean-sheet export.
- Empty-cell detection uses alpha when present.
- Transparent clean sheets keep transparent borders/gaps.
- Guide sheets may flatten to checkerboard for visibility.
- JPG import is allowed, but it has no alpha channel.

## 10. GIF Animation Handling

Existing PMTCONCON Studio GIF behavior remains intact: GIF preview animation, GIF crop/resize, and loop settings are handled by the existing pipeline.

Sheet MVP adds static contact-sheet export behavior: animated GIF icons render their first frame into a static PNG work sheet and warnings state that this is not GIF reconstruction.

Full GIF frame sheet export/reimport is future.

## 11. Page Splitting

Page splitting is implemented for static edit sheets and GIF frame manifest planning.

Defaults:

- Max sheet size: 2048×2048.
- Cell size: 200×200.
- Gap: 8.
- Border: 16.

If requested columns exceed max width, the backend caps columns per page and reports a warning. Rows per page are calculated from max height.

## 12. Backend Modules

- `src-tauri/src/sheet/grid.rs`: grid math, ordering, bounds, empty alpha detection, page splitting.
- `src-tauri/src/sheet/importer.rs`: static sheet import, original preservation, PNG cell extraction, icon creation.
- `src-tauri/src/sheet/exporter.rs`: static edit sheet export, clean/guide/manifest generation, GIF first-frame contact sheet.
- `src-tauri/src/sheet/manifest.rs`: `pmtcon-sheet-v1` and `pmtcon-gif-frame-sheet-v1` structs and validation.
- `src-tauri/src/sheet/reimport.rs`: manifest-based reimport into new icons or processed variant files.
- `src-tauri/src/sheet/gif_frames.rs`: GIF frame manifest planning, future command DTOs.
- `src-tauri/src/sheet/slices.rs`: manual slice metadata model and validation.
- `src-tauri/src/sheet/preview.rs`: preview metadata command.

## 13. Frontend Components

- `SheetImportWizard`: static sheet import flow and manifest reimport entry.
- `SheetImagePicker`: PNG/JPG/JPEG source selection.
- `SheetGridSettingsPanel`: numeric grid controls.
- `SheetGridOverlay`: grid/cell selection overlay.
- `SheetCellReviewGrid`: include/exclude review.
- `SheetCellTile`: per-cell status row.
- `SheetExportDialog`: work sheet export settings and action.
- `SheetExportPreview`: page/count summary.
- `SheetReimportDialog`: manifest reimport.
- `GifFrameSheetDialog`: future placeholder component, not wired to a visible action.
- `ManualSliceCanvas`: future model component, not wired to a visible action.

## 14. Tests

Rust tests cover:

- Grid coordinates and read order.
- Cell-size inference.
- Alpha empty-cell detection.
- Page splitting.
- PNG alpha preservation during sheet import.
- Static edit sheet output size and clean/guide split behavior.
- Manifest validation.
- GIF frame manifest duration/loop/page planning.

Frontend tests cover:

- Include/exclude selection model.
- Empty/out-of-bounds exclusion.
- Edit-sheet page count estimate.
- Render of export preview summary.

## 15. MVP Scope

Implemented:

- Static grid/cell-size import.
- Grid overlay and review UI.
- PNG alpha-preserving cell extraction.
- Static edit sheet export.
- Clean sheet, guide sheet, and `pmtcon-sheet-v1`.
- Manifest-based static reimport into new icons.
- Page splitting.
- GIF first-frame static contact sheet warning.

Scoped as future:

- Full GIF frame sheet export/reimport.
- Manual Slice Mode UI.
- Auto-detect slicing.
- Selected/problem/export-included sheet source modes.
- Text labels beyond numeric guide labels.

## 16. Future Enhancements

- Connect collection/export workspace selection to sheet source modes.
- Add progress events for large sheet operations.
- Promote processed reimport variants to active export variants after a data-model decision.
- Implement full GIF frame sheet export/reimport.
- Add manual slice canvas with snap and exact coordinate editing.
- Add conservative auto-detect proposals with confidence scoring.

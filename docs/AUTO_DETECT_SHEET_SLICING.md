# Auto-Detect Sheet Slicing

Stage: `AUTO_DETECT_SHEET_SLICING_EXPERIMENTAL`

PMTCONCON Studio auto-detect is an experimental proposal feature for sheet import. It helps users find likely grid settings, but it does not import cells automatically and does not replace manual review.

## User Flow

1. Open `시트 가져오기`.
2. Select a PNG/JPG/JPEG sheet.
3. Choose `자동 감지 (실험)`.
4. Run detection.
5. Review proposal cards:
   - method: `alpha` or `solid_background`
   - confidence: `high`, `medium`, or `low`
   - rows/columns
   - cell width/height
   - gap X/Y
   - border left/top/right/bottom
6. Apply one proposal.
7. PMTCONCON Studio opens the normal grid overlay with the proposed settings.
8. User checks alignment, edits numeric settings if needed, selects/excludes cells, and imports through the existing review step.

## Detection Methods

### Alpha Separator

For PNG sheets with alpha, the backend marks rows/columns as separators when almost all pixels are transparent. Content bands between separators become proposed rows/columns.

### Solid Background Separator

For opaque sheets, the backend estimates the background color from image corners. Rows/columns that mostly match that color are treated as separators.

## Safety Rules

- Auto-detect never imports without review.
- Auto-detect never overwrites source files.
- Auto-detect only proposes fixed-grid settings.
- No trim, rotate, packing, or atlas-style extraction is performed.
- Irregular sheets should use `직접 Slice 지정`.
- Low-confidence proposals must be treated as hints, not validated results.

## Backend

- `src-tauri/src/sheet/auto_detect.rs`
- Tauri command: `auto_detect_sheet_grid`

## Frontend

- `SheetImportWizard` exposes `자동 감지 (실험)`.
- `SheetAutoDetectPanel` shows proposals and applies a proposal into the existing grid preview flow.

## Verification

- Rust tests cover transparent separator detection, solid background separator detection, and no-proposal flat images.
- Frontend tests cover proposal rendering.

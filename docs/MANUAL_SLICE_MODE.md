# Manual Slice Mode MVP

Stage: `MANUAL_SLICE_MODE_MVP`

Manual Slice Mode adds a professional, reversible non-grid import path for sheets that cannot be sliced cleanly by rows/columns or cell size.

## User Flow

1. Open `시트 가져오기`.
2. Select a PNG/JPG/JPEG sheet.
3. Choose `직접 Slice 지정`.
4. Create rectangular slices:
   - drag on the sheet preview,
   - or click `Slice 추가` to create a default cell-sized rectangle.
5. Select a slice to move it on the overlay or resize it from the bottom-right handle.
6. Use the right panel to type exact `X`, `Y`, `W`, and `H` values.
7. Name slices, duplicate or delete them, and toggle `포함`.
8. Click `포함 Slice 가져오기`.

## Import Rules

- Included in-bounds slices become new icons in the current collection.
- The original sheet is copied into `sheet_imports/original_sheets/`.
- Cropped slices are written as PNG under `sheet_imports/extracted_cells/`.
- PNG alpha is preserved.
- Empty alpha slices are not automatically excluded; this is a manual workflow.
- Out-of-bounds slices are skipped and reported.
- Original source files are not overwritten.

## Metadata

`metadata 저장` writes the current slice list as JSON under:

```text
sheet_imports/manifests/manual_slices/
```

The backend also has a load path for saved metadata. The first MVP exposes save from the UI; richer external metadata selection/import can be added after real QA.

## Current Constraints

- Rectangular slices only.
- No polygon masks.
- No auto-detect dependency.
- No trim, rotate, packing, or atlas behavior.
- Snap-to-grid is future work.

## Verification

- Rust:
  - manual slice bounds analysis
  - metadata save/load roundtrip
  - alpha-preserving slice import with source preservation and order
- Frontend:
  - manual slice surface renders in sheet UI test coverage

# Sprite Sheet Manifest Spec

PMTCONCON Studio sheet manifests are UTF-8 JSON files. Unknown future fields are tolerated by readers, but required MVP fields must be present and valid.

## `pmtcon-sheet-v1`

Static work sheet manifest for clean-sheet roundtrip.

```json
{
  "schema": "pmtcon-sheet-v1",
  "app": "PMTCONCON Studio",
  "created_at": "2026-05-12T00:00:00Z",
  "collection_id": "collection_uuid",
  "sheet_type": "static_edit_sheet",
  "profile": {
    "cell_width": 200,
    "cell_height": 200,
    "columns": 5,
    "gap_x": 8,
    "gap_y": 8,
    "border_x": 16,
    "border_y": 16,
    "background": "transparent",
    "read_order": "row_major"
  },
  "pages": [
    {
      "page_index": 0,
      "clean_sheet_file": "sheet_001.png",
      "guide_sheet_file": "sheet_guide_001.png",
      "width": 1048,
      "height": 848
    }
  ],
  "items": [
    {
      "icon_id": "icon_uuid",
      "piece_id": null,
      "page_index": 0,
      "row": 0,
      "col": 0,
      "index": 0,
      "export_number": 1,
      "x": 16,
      "y": 16,
      "w": 200,
      "h": 200,
      "display_name": "icon name",
      "alt": "가",
      "icon_type": "single",
      "format": "png",
      "source_hash": "sha256",
      "render_hash": "sha256"
    }
  ]
}
```

Validation rules:

- `schema` must be `pmtcon-sheet-v1`.
- `app` must be `PMTCONCON Studio`.
- `profile.cell_width`, `profile.cell_height`, and `profile.columns` must be positive.
- At least one page is required.
- Every item cell must have positive `w` and `h`.
- Reimport validates page image dimensions and cell bounds before cropping.

Roundtrip rules:

- `clean_sheet_file` identifies the edited PNG page.
- `guide_sheet_file` is reference-only.
- Reimport maps by `page_index` plus the cell rectangle.
- Reimport creates new imported assets or processed variants and never overwrites original sources.

## `pmtcon-gif-frame-sheet-v1`

GIF frame sheet manifest for future frame export/reimport.

```json
{
  "schema": "pmtcon-gif-frame-sheet-v1",
  "app": "PMTCONCON Studio",
  "created_at": "2026-05-12T00:00:00Z",
  "icon_id": "icon_uuid",
  "source_hash": "sha256",
  "loop_mode": "infinite",
  "frame_cell_width": 200,
  "frame_cell_height": 200,
  "columns": 8,
  "gap_x": 8,
  "gap_y": 8,
  "border_x": 16,
  "border_y": 16,
  "pages": [
    {
      "page_index": 0,
      "sheet_file": "frames_sheet_001.png",
      "guide_sheet_file": "frames_guide_001.png",
      "width": 1680,
      "height": 1680
    }
  ],
  "frames": [
    {
      "frame_index": 0,
      "sheet_file": "frames_sheet_001.png",
      "page_index": 0,
      "row": 0,
      "col": 0,
      "x": 16,
      "y": 16,
      "w": 200,
      "h": 200,
      "duration_ms": 80,
      "disposal_method": "background",
      "source_frame_hash": "sha256"
    }
  ]
}
```

Validation rules:

- `schema` must be `pmtcon-gif-frame-sheet-v1`.
- `frame_cell_width` and `frame_cell_height` must be positive.
- At least one frame is required.
- Reimport must detect missing pages, frame count mismatch, changed dimensions, and out-of-bounds cells before reassembly.

GIF reassembly requirements for the future implementation:

- Preserve `frame_index` order.
- Preserve `duration_ms`.
- Preserve `loop_mode`.
- Preserve disposal where the encoder supports it.
- Create a new GIF processed variant and never overwrite the original GIF.

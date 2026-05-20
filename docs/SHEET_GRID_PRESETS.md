# Sheet Grid Presets

Stage: `CONTEXT_MENU_SHEET_WORKFLOW_AND_PRESETS_MVP`

## Goal

Sheet grid presets store repeatable sheet settings for professional round-trip editing. A preset can be saved from export and applied during import, so externally edited sheets can be sliced with the same grid geometry.

## Data Model

Presets are stored in `sheet_grid_presets`:

- `scope`: `global` or `collection`
- `kind`: `static_import_export`, `static_import`, `static_export`, or `gif_frame_export`
- cell size, rows, columns, mode
- gap and border values
- read order
- background
- max sheet width and height
- `frames_per_page` for GIF frame sheets
- clean/guide/manifest output options
- guide label JSON
- default flags for import, export, and GIF frame export
- `is_builtin` to protect built-in presets from deletion

Icon notes are stored separately in `icon_notes` and are not part of preset behavior.

## Built-In Presets

Two built-in presets are created by migration:

- `DCInside 200x200 / 5 columns`
- `GIF Frames 200x200 / 8 columns / 64 frames`

Built-in presets can be applied, set as default, or duplicated. They cannot be deleted or edited in place.

## Import Usage

`시트 가져오기` shows a preset selector above the precise grid settings. Applying a preset updates:

- mode
- rows/columns
- cell width/height
- margins
- gaps
- read order

Fields that do not apply to import, such as output background or guide labels, are ignored safely.

## Static Export Usage

`작업 시트로 내보내기` can save the current export settings as a shared import/export preset. Applying a preset updates:

- cell width/height
- columns
- gap and border
- background
- max sheet size
- clean/guide/manifest options
- guide label options

The preset does not change the export scope. If the dialog was opened for selected icons, it remains selected-only.

## GIF Frame Export Usage

`GIF 프레임 작업시트로 내보내기` can apply compatible grid presets and saves GIF-specific presets with `frames_per_page`. Static import/export presets can still apply compatible cell/gap/border/background fields.

## Defaults

Users can set a default preset for import, export, or GIF frame export. Dialogs load the default preset for their target when opened. Collection-specific defaults take precedence over global defaults; otherwise the built-in preset remains the fallback.

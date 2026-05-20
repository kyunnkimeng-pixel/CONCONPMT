# PMTCONCON Studio 0.1.1 Release Notes

PMTCONCON Studio 0.1.1 focuses on a full local production workflow for building DCInside-style 디시콘 and custom emoticon packs on Windows.

## Highlights

- Windows Explorer-like collection workspace with collection cards, icon grids, persistent ordering, multi-select, context menus, drag ordering, and cover image management.
- Image and animated GIF import with original file preservation.
- Single, horizontal double, and vertical double icon editing with crop boxes, split previews, and export-ready pieces.
- DCInside-style usage preview for checking how icons appear before export.
- Export workspace with validation, file-size checks, selected-item rerun, optimization candidates, and optional `alts.txt`.

## New Production Workflows

### Sprite Sheet / Work Sheet Tools

- Import PNG/JPG sprite sheets by grid or cell size.
- Review cells before import, include or exclude individual cells, and skip empty transparent cells.
- Export selected icons or a full collection as a clean editable Work Sheet, guide sheet, and manifest JSON.
- Reimport an edited Work Sheet through the manifest without overwriting original source files.
- Preserve PNG alpha through sheet import, export, and reimport.
- Split large sheets into pages to avoid oversized single images.

### GIF Frame Sheet Editing

- Export every frame of a GIF icon into editable PNG frame sheets.
- Generate a GIF frame manifest with frame order, duration, loop mode, page, and cell mapping.
- Reimport edited frame sheets to create a new animated GIF variant.
- Keep the original GIF source file intact.
- Use static contact sheets for GIF first-frame overview editing when full animation editing is not needed.

### Context Menus, Notes, And Presets

- GIF icon right-click actions for frame Work Sheet export and non-destructive replacement through a new variant.
- Collection right-click duplication with safe copy naming.
- Icon memo notes with persistent storage, memo edit/delete actions, and hover preview.
- Multi-selected icon export to a Work Sheet containing only the selected icons.
- Shared sheet grid presets for import, static export, and GIF frame export.
- Built-in presets for DCInside 200x200 sheets and GIF frame sheets.

### Manual And Experimental Sheet Slicing

- Manual slice mode for drawing and editing named rectangular slices.
- Exact X/Y/W/H controls, include/exclude state, duplicate/delete, and metadata save/load.
- Experimental auto-detect proposals for transparent separator sheets and solid-background separator sheets.
- Auto-detect never imports automatically; users still review the grid and selected cells before import.

### GIF And Advanced Editing

- Animated GIF preview remains animated in grid, editor, usage preview, and export paths.
- GIF crop/resize processes frame sequences while preserving timing where possible.
- GIF loop settings include preserved source loop, infinite, once, custom repeat, and ping-pong behavior.
- GIF playback FPS changes are visible in the live edit preview before apply.
- Applying GIF playback FPS creates a real variant used by preview and export.
- Text overlays support position, size, color, outline, and user-selected fonts.
- GIF color and playback controls plus JPG quality controls create measured file-size candidates.

## Validation And Safety

- DCInside profile validation covers count, cell size, allowed formats, max bytes, alt length, alt uniqueness, and filename safety.
- Working/placeholder icons can be tracked separately from complete icons.
- Export results can be filtered, selected, regenerated, and inspected before final use.
- Original imported files are preserved. Editing, sheet reimport, GIF frame reimport, and optimization write generated outputs or variants instead of replacing originals.
- The app remains local-only. It does not include upload, login, posting, scraping, browser automation, cloud sync, account, marketplace, or premium features.

## Distribution Notes

- The selected Windows release artifact is the NSIS setup for version 0.1.1.
- `SHA256SUMS.txt` is generated from the current NSIS setup artifact.
- The Windows installer files are currently unsigned, so Windows may show a publisher warning.
- PMTCONCON Studio is distributed under the MIT License.
- Third-party notices are provided in `THIRD_PARTY_LICENSES.md`.

## Known Notes

- The production web bundle currently reports a large JavaScript chunk warning during build. This does not block the desktop app package, but future code splitting may improve load performance.
- The MSI package is withheld from the selected release assets until clean Windows VM install/uninstall QA is completed.

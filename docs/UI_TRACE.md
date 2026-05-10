# UI_TRACE.md - PMTCONCON Studio UI Reference Trace

Stage: `06_UI_REFERENCE_IMAGES`  
Date: 2026-05-09  
Mode: visual reference generation only, no product source implementation.

Generated UI images are subordinate to the written specification. Use
`docs/PRODUCT_SPEC.md`, `docs/FEATURE_INVENTORY.md`, and `docs/ARCHITECTURE.md`
for required behavior. A generated image must not add a menu or feature, and an
omitted visual element must not remove a required feature.

## Reference Images

| Ref | File | Purpose |
|---|---|---|
| R1 | `docs/ui-references/01-main-explorer.png` | Main collection explorer |
| R2 | `docs/ui-references/02-collection-detail.png` | Collection detail and icon grid |
| R3 | `docs/ui-references/03-editor-preview.png` | Crop editor and DCInside-style preview |
| R4 | `docs/ui-references/04-export-validation.png` | Export profile and validation results |

## Generated-Only Elements To Ignore

- Decorative thumbnail artwork, sample icon characters, exact color treatment,
  glass/mica intensity, shadows, gradients, and placeholder filenames.
- Any accidental UI glyph, chevron, tab, badge, row action, or decorative panel
  not backed by `FEATURE_INVENTORY.md`.
- Any cloud, login, account, sync, marketplace, premium, upload-to-community,
  publish, or sharing feature implied by generated pixels.
- Generated text inconsistencies. The product name must remain
  `PMTCONCON Studio`, and user-facing app strings should be Korean.
- Missing generated visuals for required behavior such as reveal original,
  export result reveal, folder import, persistence, SQLite, and soft delete.
  Those remain required because they are in the written spec/inventory.

## Feature-To-UI Mapping

| ID | UI location / planned component | Ref | Trace notes |
|---|---|---|---|
| F001 | Main explorer collection area, `features/collections/components/CollectionGrid.tsx` | R1 | Collection cards are the visible entry point. |
| F002 | Collection card double-click handler in the home route/router | R1 | Behavior is required even though a static image cannot show double-click. |
| F003 | `app/AppShell.tsx`, shared `Toolbar`, shared `Breadcrumb` | R1, R2, R3, R4 | Explorer-like shell is shared across screens. |
| F004 | Main `+` menu action: `새 모음` | R1 | No other generated `+` items are valid unless listed here or below. |
| F005 | Main `+` menu action: `파일 가져오기`; later Tauri import dialog | R1 | Current UI may be mock-backed until native import is implemented. |
| F006 | `DropImportZone.tsx` over main/detail explorer canvases | R1, R2 | Drop affordance may be subtle; behavior remains required. |
| F007 | Collection card cover preview, first imported icon as default cover | R1 | Backing persistence arrives with DB/import stages. |
| F008 | Icon context action or cover edit dialog: `대표 이미지로 설정` | R2 | Also needs 200x200 cover import path from F062. |
| F009 | Collection card label using `InlineNameEditor.tsx` | R1 | Inline rename is part of the card, not a separate fake menu. |
| F010 | Collection route icon area, planned `features/icons/components/IconGrid.tsx` | R2 | Grid/list support belongs in the collection detail screen. |
| F011 | Icon tile label using `InlineNameEditor.tsx` plus `rename_icon` command | R2 | Display name rename is separate from piece alt editing. |
| F012 | Icon context action `썸네일 바꾸기` plus thumbnail override preview handling | R3 | Override changes representative preview without replacing export source. |
| F013 | Icon tile reorder handle with planned `dnd-kit` sortable grid | R2 | Reorder handle is visual only until persistence is wired. |
| F014 | Reorder save through DB repository after drag end | R2 | Persistence is a backend contract; no extra UI control needed. |
| F015 | Icon tile alt label, planned `AltInlineEditor.tsx` | R2 | Alt text should behave like a filename label under the preview. |
| F016 | Alt editor backed by `icon_pieces` repository | R2 | Static image cannot show restart persistence. |
| F017 | Alt inline validation and export validation view | R2, R4 | Length/character feedback should appear before export. |
| F018 | Duplicate alt indicator on grid and validation table | R2, R4 | Duplicate alt is both immediate feedback and export validation. |
| F019 | Icon context menu delete and multi-delete confirmation | R2 | Keyboard Delete shares the same confirmation path. |
| F020 | Selection model for Ctrl multi-select with selected tile states | R2 | Multi-select is visible through selected tiles. |
| F021 | Selection model for Shift range select with selected tile states | R2 | Static image cannot show range gesture, but selected states anchor it. |
| F022 | Planned `IconContextMenu.tsx`: edit, rename, duplicate, delete, set cover | R2 | Do not add context actions beyond implemented/spec items. |
| F023 | Icon duplicate action in `IconContextMenu.tsx` plus clone command | R2 | Duplicate must clone durable icon/piece data. |
| F024 | Home toolbar `선택 복제` plus clone collection command | R1 | Collection duplicate is discoverable without adding fake card menus. |
| F025 | `features/editor/components/EditorPanel.tsx` shape selector: `단일콘` | R3 | Single icon editor mode must affect preview/export. |
| F026 | Editor shape selector: `가로 이중콘` | R3 | Horizontal double requires 2W x H crop viewport and two pieces. |
| F027 | Editor shape selector: `세로 이중콘` | R3 | Vertical double is required even if the visual reference emphasizes horizontal. |
| F028 | `CropCanvas.tsx` free-mode crop rectangle move/resize | R3 | Use react-konva planned canvas behavior. |
| F029 | `CropModeControl.tsx` fixed-mode locked viewport | R3 | Fixed mode allows move/presets, not arbitrary resize. |
| F030 | `CropCanvas.tsx` split-line overlay for double icons | R3 | Split line derives from selected shape. |
| F031 | `PresetPositionGrid.tsx` 3x3 fixed-mode presets | R3 | Preset names must match architecture crop positions. |
| F032 | Editor apply action plus Rust imaging/DB metadata | R3 | Apply must never delete or overwrite original source files. |
| F033 | Editor reset/restore from persisted crop metadata | R3 | Generated image cannot prove persistence; DB stage must implement it. |
| F034 | Import/editor default crop/downsize behavior for varied resolutions | R3 | UI anchors source preview and crop canvas; math lives in imaging. |
| F035 | Collection settings panel and editor cell-size controls | R3, R4 | Custom sizes persist separately from DCInside export validation defaults. |
| F036 | Animated GIF rendering in icon grid/preview | R2, R3 | Static PNG reference may show only an animation placeholder. |
| F037 | Rust GIF crop/resize pipeline behind editor apply/export | R3 | No separate decorative GIF tool should be added. |
| F038 | `GifLoopControl.tsx` in editor side panel | R3 | Loop options are preserve, infinite, once, and count. |
| F039 | `features/preview/components/DcinsidePreview.tsx` | R2, R3 | Preview should resemble a DCInside comment input area. |
| F040 | Preview simulator output at 100x100 display size | R3 | Display size is distinct from export cell size. |
| F041 | Export validation row for DCInside count 10-200 | R4 | Hard error blocks export. |
| F042 | Export validation row for output file <= 2MB | R4 | Validate final generated output, not only source file size. |
| F043 | Export validation row for jpg/png/gif formats | R4 | Format controls belong to profile/export UI. |
| F044 | Export filename mode control: `sequence` | R4 | Sequence filenames use zero-padded order. |
| F045 | Export filename mode control: `alt` | R4 | Alt filename collisions must block export. |
| F046 | Export checkbox/control for `alts.txt 생성` | R4 | Generated text file includes export index, filename, piece ID, display name, alt. |
| F047 | Export completion actions to open folder and/or `alts.txt` | R4 | Not necessarily visible before export; no fake button until implemented. |
| F048 | Export warning list for transparency and 5px margin recommendations | R4 | Warnings do not block unless strict validation is selected. |
| F049 | Import entry points plus `commands/import.rs` original-copy behavior | R1, R2 | UI starts import; source preservation is backend behavior. |
| F050 | No standalone UI; SQLite migrations support all durable UI state | R1-R4 | Required backend foundation, not a menu item. |
| F051 | This trace plus component review for dead/generated-only menus | R1-R4 | Generated-only controls must be ignored or removed before implementation. |
| F052 | local-only UI image prompt, `docs/ui-references/`, and this trace | R1-R4 | Stage 06 documentation workflow output. |
| F053 | Main `+` menu and collection toolbar action: `폴더 가져오기` | R1 | Folder picker and dropped-folder traversal import valid images in deterministic path order and report skips. |
| F054 | Collection toolbar `파일 추가` action for current collection | R2 | Must add files to the selected collection, not always create a new one. |
| F055 | Import result/validation feedback plus `commands/import.rs` hashing | R1, R2 | SHA-256 duplicate handling is backend-led; show clear Korean feedback. |
| F056 | Export profile selector/settings in `ExportDialog.tsx` | R4 | Profile settings must persist through `export_profiles`. |
| F057 | Output folder picker in export UI | R4 | Uses native folder selection through Tauri. |
| F058 | `ValidationResultList.tsx` separating hard errors and warnings | R4 | Hard errors disable export; warnings can be included unless strict. |
| F059 | App startup route/view restore via `app_settings` | R1, R2 | Restore runs once per app session and falls back to home if the collection is stale. |
| F060 | Home toolbar `라이브러리 정리` plus explicit confirmation and cleanup command | R2 | Physical deletion only happens after user confirmation. |
| F061 | Conditional context actions: reveal original / reveal export result | R2 | Export result action is disabled with a clear state until an export path exists. |
| F062 | Collection toolbar `대표 이미지` import for exact 200×200 JPG/PNG | R1, R2 | Cover-only images are stored as source files and are not export icons. |
| F063 | `PreviewComposer.tsx` showing export actual size and 100x100 exposure size | R3 | Generated image may show one preview; implementation must support both checks. |
| F064 | Export dimension validation for default 200x200 DCInside pieces | R4 | Hard error for DCInside profile when dimensions are invalid. |
| F065 | Custom profile validation controls for size/format/bytes | R4 | Generated image focuses DCInside; custom profile remains required. |
| F066 | Piece-level alt editor for multi-piece icons | R2, R3 | Double icons need separate alt values for each exported piece. |

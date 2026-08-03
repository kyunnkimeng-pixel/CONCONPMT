# UI_TRACE.md - PMTCONCON Studio UI Reference Trace

Stage: `06_UI_REFERENCE_IMAGES`
Date: 2026-05-09
Latest implementation trace update: 2026-08-02 (F157 fake-transparency guard)
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
| F024 | Home toolbar `모음 복제` plus clone collection command | R1 | Collection duplicate is discoverable without adding fake card menus. |
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
| F130 | `EditorPanel.tsx` transform controls plus shared Rust render recipe | R3 | Flip and quarter-turn commands are non-destructive and apply identically to static/GIF preview, sheets, optimization, and export. |
| F131 | Collection toolbar `시트로 GIF 만들기` and `FrameSheetToGifDialog.tsx` | R2, R3 | Reuses static grid settings/overlay and adds reviewed frame strip, duration/FPS convenience, loop/direction, sticky playback, actual byte measurement, stale-hash guard, preserved source sheet, and new animated icon commit. |
| F132 | `EditorPanel.tsx` advanced section and `EffectRecipeEditor.tsx`; native `preview_icon_effects` / `update_icon_effects` | R3 | Completed: seven ordered, revisioned static effect kinds render on the combined viewport for static/GIF preview and export. Optimizer and sheet hashes include the recipe; request-scoped artifacts, clone independence, and preview/export/sheet regressions passed the Stage Gate. |
| F133 | `EditorPanel.tsx` 모션 탭, lazy `MotionEditorSection.tsx`, `MotionRecipeEditor.tsx`, `MotionPreviewPanel.tsx`; native `preview_icon_motion` / `update_icon_motion` | R3 | Implemented: 16 presets in four bounded categories, fixed spatial/displacement/color/overlay composition, revisioned `pmtcon-motion-v1`, actual GIF and per-piece byte measurement, clipping/loop/timing details, stale-measurement guards, play/pause and reduced-motion behavior. Static and existing GIF sources share the export/optimizer/sheet renderer; static sheets intentionally use the 0ms poster while GIF frame sheets preserve all frames. |
| F134 | Collection duplicate action and native clone repository | R1, R2 | Complete durable visual metadata, AI source lineage, active variants, presets, and independently owned preview paths are remapped to new stable IDs. |
| F135-F137 | `EditorPanel.tsx` source summary, `AiReviewSection.tsx` history view, native AI repositories | R3 | Provider-neutral non-destructive source/version foundation. Originals remain immutable; activation and rollback are restart-safe and fail closed. |
| F144 | `AiReviewSection.tsx` normalization inspector and comparison views | R3 | Local JPG/PNG candidates support contain-pad or cover-crop normalization with deterministic native preview before commit. |
| F145 | AI outcome panel plus grid reveal/focus integration | R2, R3 | New-icon creation, current-source activation, restore, repeat creation count, open/reveal/continue actions, and one document-wide live region are implemented. |
| F146 | `AiReviewSection.tsx` in-app AI workspace dialog | R3 | Implemented three-tab workspace for local result import, candidate review, and source history. Provider execution and safe web handoff are added only through the separately gated F138–F140 panel below. |
| F138 | `AiProviderPanel.tsx`, `AiWebHandoffPanel.tsx`, history dialog, native `ai_handoff` commands/repository | R1, R3 | Static-single manual round trip includes verified Windows file drag, Explorer fallback, result validation/correction, inactive candidate, restart restore and explicit close. Global quota/history is F151. No provider DOM/session automation. |
| F139 | `AiProviderPanel.tsx` NovelAI form, `ai-provider-model.ts`, `api.ts`, native `ai_provider.rs` and `ai_provider_runtime/` | R3 | Session PAT connect/status/clear, exact experimental action/model fields, source/prompt/cost/rights confirmation and one-image execute feed the existing inactive-candidate review. Token input clears at invoke and is never echoed or persisted; only static-image edit and JSON response are in this gate. |
| F140 | `AiProviderPanel.tsx` `Gemini API (실험실)` form, `ai-provider-model.ts`, native provider adapter | R3 | Mock-tested private static-edit pilot only. Full adult/professional-business/supported-region/paid-service/request-cost confirmations gate execution; the session key is non-persistent. This row does not authorize or claim general consumer release, and Gemini web handoff remains available. |
| F138-F140 security boundary | production `tauri.conf.json`, `capabilities/default.json`, native official-resource enum/constant mapping | R1-R4 | WebView `connect-src` is IPC-only, general `opener:default` is absent, and frontend URLs cannot select arbitrary external destinations. Official links and provider HTTP use separate Rust-owned constant allowlists. |
| F147 | migration `018`, `ai_grid.rs`, `sheet/composer.rs`, `sheet/splitter.rs` | R2-R3 | Durable request items/artifacts, deterministic 2–16 one-page composition, reviewed all-or-none splitting and source-free roots back the visible GRID-2 workflow. |
| F148 | `IconGrid.tsx`/`IconContextMenu.tsx` and `AiGridWorkspaceDialog.tsx` | R2-R3 | Eligible 2–16 static square single icons expose `선택 N개 AI로 수정`; disabled cases show an exact reason. Five steps cover prompt, verified drag/Explorer, result drop, overlay mapping and all-or-none inactive candidates. |
| F149 | Collection toolbar `AI 아이콘 만들기` and `AiGridWorkspaceDialog.tsx` | R2-R3 | Source-free 1–16 generation creates no placeholder. Reviewed cells become source/icon/piece/crop/provenance/order/cover atomically. |
| F150 | Grid/single handoff `파일 끌기` buttons plus `native_drag.rs` | R3 | Mouse starts OS-native drag only after request-ID lookup and managed-file integrity validation; keyboard/non-Windows use Explorer. No arbitrary path or DOM automation. |
| F151 | AppShell sidebar `최근 AI 전달` and `AiHandoffHistoryDialog.tsx` | R1-R3 | Shows unified single/grid 256MiB usage, recent 30 records, lifecycle/cleanup-pending state and request-type-safe drag/reveal/close/manual cleanup; backend protects active work and repeats maintenance every 15 minutes while open. |
| F152 | `GifFrameSheetDialog.tsx` and native GIF manifest/reimport pipeline | R2-R3 | Export/reimport uses manifest page filenames, restores exact frame timing and loop metadata, bounds output, previews rebuilt variant and preserves the original. |
| F153 | native `ai_provider_runtime/provider.rs` and `AiProviderPanel.tsx` | R3 | Gemini 2.5/3.1 use model-specific Interactions payloads; the last inline JPEG is selected, while safe 400 handling distinguishes invalid keys, paid-tier/free-tier preconditions and request fields without exposing raw responses or keys. |
| F154 | `AiGridWorkspaceDialog.tsx`, `AiHandoffHistoryDialog.tsx`, migration `020`, `ai_grid.rs`, `sheet/composer.rs`, and `AiWebHandoffPanel.tsx` | R2-R3 | Source-free generation builds a managed reference board from 1–16 selected icons/external files with 16MiB/128M-pixel guards, GIF poster disclosure, non-square contain, output-template separation and recent-handoff reuse. Proportional web results remain raw candidates and show a local-normalization warning rather than a false hard failure. |
| F155 | `AiProviderPanel.tsx` and `GifFrameSheetDialog.tsx` | R3 | GIF AI entry reuses the manifest frame-sheet roundtrip, copies a geometry/timing/alpha prompt, opens only allowlisted official sites, and returns to explicit manifest page slots that accept browser-renamed PNGs without relying on picker order; direct GIF provider calls remain disabled. |
| F156 | `NovelAiWebGuide.tsx`, provider-specific prompt models, single/grid/GIF web panels and built-in sheet preset | R2-R3 | NovelAI shows a Prompt-success-gated, revision-synchronized Prompt → Undesired Content sequence; current Add a Base Img selection plus direct-base/standalone-reference variants; 200→192 single normalization; arbitrary download-name handling; PNG/JPG/static-WebP detection and alpha-preserving PNG conversion with animated-WebP rejection; one-page-at-a-time GIF Image2Image, bounded explicit page slots and exact canvas warnings, without automating login or provider DOM. |
| F157 | AiGridWorkspaceDialog.tsx, AI prompt models, ai_grid.rs, and AiProviderPanel.tsx | R2-R3 | Source-free single/grid import requires meaningful per-canvas/per-cell real alpha and transparent gap/unused areas, rejecting opaque, one-alpha and thin-border fake transparency before persistence. Generation accepts PNG/WebP rather than JPG, keeps missing-alpha correction at step 3, and keeps stored analysis failures at step 4 with retry. Provider prompts prohibit painted checkerboards; transparent sources cannot enter the current JPEG-only Gemini direct API path. |

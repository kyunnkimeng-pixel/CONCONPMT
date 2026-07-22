# FEATURE_INVENTORY.md - PMTCONCON Studio 구현 누락 방지 체크리스트

이 표는 PMTCONCON Studio의 구현 상태를 추적하기 위한 기능 인벤토리다. `Status`는 `todo | doing | done | blocked | future` 중 하나를 사용한다.

| ID | Feature | Status | Component / Rust module | Test |
|---|---|---:|---|---|
| F001 | 메인 화면의 디시콘 모음 grid 표시 | done | `features/collections/components/CollectionGrid.tsx` + `list_collections` | `npm.cmd run lint` / Rust check / manual |
| F002 | 모음 더블클릭 진입 | done | router + collection grid | `npm.cmd run lint` / manual |
| F003 | Windows Explorer-like toolbar/breadcrumb | done | `app/AppShell.tsx` + routes | `npm.cmd run lint` / manual |
| F004 | 상단 `+` 메뉴: 새 모음 | done | `app/routes/home-route.tsx` + `create_collection` | `npm.cmd run lint` / Rust check |
| F005 | 상단 `+` 메뉴: 여러 이미지 파일 가져오기 | done | `home-route.tsx` file input + `import_image_files` | `npm.cmd run lint` / `cargo test` |
| F006 | Drag and drop import | done | `DropImportZone.tsx` + `import_image_files` | `npm.cmd run lint` / manual |
| F007 | 대표 이미지 자동 설정: 첫 아이콘 | done | `db/repositories/imports.rs` cover update | `cargo test` |
| F008 | 대표 이미지 수동 변경 | done | icon context menu cover action + `set_collection_cover_icon` | `npm.cmd run lint` / `cargo check` |
| F009 | 모음 이름 inline rename | done | `InlineNameEditor.tsx` + `rename_collection` | `npm.cmd run lint` / Rust check |
| F010 | 아이콘 grid/list 표시 | done | `features/icons/components/IconGrid.tsx` + `list_icons` | `npm.cmd run lint` / `cargo check` |
| F011 | 아이콘명 rename | done | `IconTile.tsx` inline display-name editor + `rename_icon` command; alt 값과 분리 저장 | `cargo test` / `npm.cmd run lint` |
| F012 | 아이콘별 대표 썸네일 override | done | icon context menu `썸네일 바꾸기` + `set_icon_thumbnail_override`; original/export source preserved | `cargo test` |
| F013 | 아이콘 drag reorder | done | `IconGrid.tsx` + dnd-kit sortable + `reorder_icons` | `npm.cmd run lint` / `cargo test` |
| F014 | order persistence across restart | done | `icons.order_index` updated by `reorder_icons` | `cargo test` |
| F015 | alt inline edit | done | `AltInlineEditor.tsx` + `update_icon_piece_alt` | `npm.cmd run test` / `cargo test` |
| F016 | alt persistence | done | `icon_pieces.alt_text` repository update | `cargo test` |
| F017 | alt validation: length/characters | done | `lib/validation.ts` + Rust mirror in `repositories/icons.rs` | `npm.cmd run test` / `cargo test` |
| F018 | alt duplicate 표시 | done | `findDuplicateAltPieceIds` + backend duplicate rejection | `npm.cmd run test` / `cargo test` |
| F019 | 삭제 / 다중 삭제 | done | `IconContextMenu.tsx` + keyboard Delete + soft delete | `cargo test` |
| F020 | Ctrl multi-select | done | `selection-model.ts` | `npm.cmd run test` |
| F021 | Shift range select | done | `selection-model.ts` | `npm.cmd run test` |
| F022 | 우클릭 메뉴: delete/duplicate/edit/set cover | done | `IconContextMenu.tsx`; rename, thumbnail override, reveal original/export result are implemented where applicable | `npm.cmd run lint` |
| F023 | 아이콘 복제 | done | `duplicate_icon` command clones source/crop data and assigns unique alt | `cargo test` |
| F024 | 디시콘 모음 복제 | done | home toolbar `선택 복제` + `duplicate_collection`; copied collection keeps separate metadata/icons | `cargo test` / `npm.cmd run lint` |
| F025 | 단일콘 edit mode | done | `features/editor/components/EditorPanel.tsx` + `apply_icon_crop` | `npm.cmd run lint` / Rust check |
| F026 | 가로 이중콘 edit mode | done | `CropCanvas.tsx` + piece role reconciliation | `crop-math.test.ts` / Rust test |
| F027 | 세로 이중콘 edit mode | done | `CropCanvas.tsx` + piece role reconciliation | `crop-math.test.ts` / Rust test |
| F028 | 자유모드 crop box move/resize | done | `CropCanvas.tsx` React-Konva drag handles | `npm.cmd run lint` / `crop-math.test.ts` |
| F029 | 고정모드 crop box fixed size | done | `EditorPanel.tsx` + `fixedCropForPreset` | `crop-math.test.ts` |
| F030 | 이중콘 split line 표시 | done | `CropCanvas.tsx` split-line overlay | `npm.cmd run lint` |
| F031 | fixed mode 9-position presets | done | `EditorPanel.tsx` preset grid + `fixedCropForPreset` | `crop-math.test.ts` |
| F032 | apply crop without deleting source | done | `commands/editor.rs` + `imaging/preview.rs` | Rust test |
| F033 | edit crop after apply | done | `get_icon_editor_state` + `crop_settings` upsert | Rust test |
| F034 | 다양한 input resolution center crop/downsize | done | `imaging/preview.rs` padded crop + resize | Rust unit |
| F035 | collection/icon 기준 사이즈 변경 | done | collection settings panel updates default/export/preview sizes; editor retains icon-level cell overrides | `cargo test` / `npm.cmd run lint` |
| F036 | GIF preview continuous animation | done | grid/editor `<img>` previews use Tauri asset URLs; usage preview refreshes GIF asset URLs so local preview playback keeps repeating | `npm.cmd run lint` / `cargo test` |
| F037 | GIF frame crop/resize | done | `imaging/preview.rs` decodes frames, applies saved crop, resizes viewport/pieces, and writes animated editor preview GIFs | `cargo test` |
| F038 | GIF loop settings | done | editor loop UI + import metadata + preview/export encode via `imaging/gif_pipeline.rs` | `npm.cmd run lint` / `cargo test` |
| F039 | 미리 사용해보기 DC comment-like UI | done | `features/preview/components/DcinsidePreview.tsx` + `PreviewComposer.tsx` | `preview-model.test.ts` / manual |
| F040 | preview at 100x100 display size | done | `DCINSIDE_USAGE_DISPLAY_SIZE` preview model + usage preview renderer | `preview-model.test.ts` / manual |
| F041 | export validation DC count 10-200 | done | `src-tauri/src/export/mod.rs` DCInside validator | `cargo test` |
| F042 | export validation file <= 2MB | done | post-render export validator + profile `max_bytes` | `cargo test` |
| F043 | export validation format jpg/png/gif | done | export planner allowed-format check | `cargo test` |
| F044 | export sequence filename 001~ | done | export filename planner | `cargo test` |
| F045 | export alt filename mode | done | export filename planner with sanitized collision checks | `cargo test` |
| F046 | export `alts.txt` | done | `write_alts_txt` in export command | `cargo test` |
| F047 | open export folder / alt txt | done | `open_export_path` + export completion options | `cargo check` / manual UI path |
| F048 | transparent background / 5px margin warnings | done | export validator warns for JPG transparency risk and upscaling/padding; 5px margin heuristic is user-deprioritized/non-blocking | `cargo test` |
| F049 | import original copied into app library | done | `db/repositories/imports.rs` | `cargo test` |
| F050 | SQLite migrations | done | `src-tauri/migrations/001_app_data.sql` + `db/migrations.rs` | Rust test |
| F051 | no generated UI-only dead menus | done | implemented import, folder import, cleanup, duplicate, reveal, cover, and settings actions; no `준비 중` app menu remains | review + source search |
| F052 | UI image generation reference workflow | done | local-only UI reference brief + `docs/UI_TRACE.md` + `docs/ui-references/` | manual |
| F053 | 상단 `+` 메뉴: 폴더 가져오기 | done | home/collection folder inputs and dropped folder traversal import sorted jpg/jpeg/png/gif files and skip unsupported files with status | `npm.cmd run test` / `npm.cmd run lint` |
| F054 | 현재 선택한 모음에 파일 추가 | done | main selected collection import + collection route import | `npm.cmd run lint` |
| F055 | 가져오기 SHA-256 중복 판정 | done | `db/repositories/imports.rs` reuses `source_files` by hash | `cargo test` |
| F056 | Export profile 선택 및 설정 persistence | done | `features/export/components/ExportDialog.tsx` + `export_profiles` commands | `npm.cmd run lint` / `cargo test` |
| F057 | Export 출력 폴더 선택 | done | export dialog output folder path/default exports root + export command | `npm.cmd run lint` / `cargo test` |
| F058 | Export 검증 결과: 오류/경고 분리 및 경고 포함 export | done | `ValidationResultList.tsx` + Rust validation result | `npm.cmd run lint` / `cargo test` |
| F059 | 마지막으로 연 모음과 보기 모드 복구 | done | `app_settings` commands persist last collection/view; home restores once per app session and falls back if stale | `cargo test` / `npm.cmd run lint` |
| F060 | soft delete 및 명시적 library cleanup | done | home `라이브러리 정리` previews and confirms orphan/temp cleanup before physical deletion | `cargo test` / `npm.cmd run lint` |
| F061 | 원본 보기 및 개별 export 결과 보기 | done | icon context menu reveals original source or first saved per-icon export result with missing-path handling | `npm.cmd run lint` / source review |
| F062 | 200x200 JPG/PNG 대표 이미지 가져오기 | done | collection `대표 이미지` import validates exact 200×200 JPG/PNG and stores cover-only source outside export icons | `cargo test` / `npm.cmd run lint` |
| F063 | 미리보기에서 export 실제 크기와 100x100 노출 크기 모두 확인 | done | usage preview shows 100×100 exposure size and effective collection/icon cell size metadata; export uses the same persisted effective sizes | `preview-model.test.ts` / manual |
| F064 | DCInside export dimension validation: 기본 200x200 | done | Rust export validator checks DCInside profile and per-piece effective size | `cargo test` |
| F065 | Custom profile별 크기/포맷/용량 validation 설정 | done | Custom export profile supports format, size reference, filename mode, max bytes, strict warnings | `npm.cmd run lint` / `cargo test` |
| F066 | 다중콘 조각별 별도 alt 값 편집 | done | `IconTile.tsx` renders all `icon_pieces` with `AltInlineEditor` | `npm.cmd run test` / `cargo test` |
| F067 | Editor panel live draft preview and resizable width | done | `EditorPanel.tsx` adds source-driven live preview, persisted resize handle, and fixed-height route layout | native regression / `npm.cmd run lint` |
| F068 | Explorer grid accidental text selection and drag responsiveness stabilization | done | `IconGrid.tsx` / `IconTile.tsx` suppress native selection and remove active drag transform transition | native regression |
| F069 | Sidebar collection list navigation | done | `AppShell.tsx` lists collections and highlights the active route | native smoke / `npm.cmd run lint` |
| F070 | Export output folder picker | done | `ExportDialog.tsx` + `pick_export_directory` Tauri command via `rfd::FileDialog` | `npm.cmd run tauri -- build` / `cargo test` |
| F071 | DCInside count/alt warnings do not block sequence export per latest user direction | done | `export/mod.rs` classifies count/alt issues as warnings while keeping hard blockers for invalid export mechanics | `cargo test` |
| F072 | Multi-piece icons render as connected grouped cells in grid | done | `IconTile.tsx` renders horizontal/vertical double icons as linked two-cell previews with shared tile selection/drag | native smoke / `npm.cmd run lint` |
| F073 | Import/export visible progress states | done | `collection-route.tsx` import progress overlay; `ExportDialog.tsx` export progress overlay | `npm.cmd run lint` / `npm.cmd run test` |
| F074 | Export-only grid with per-piece include/exclude and status | done | `ExportDialog.tsx` export grid; `ExportRequestPayload.excludedPieceIds`; `export/mod.rs` filtered plan | `cargo test` / `npm.cmd run lint` |
| F075 | Post-render export issues do not discard generated output | done | `export/mod.rs` non-blocking post-render max-byte issue classification, manifest issues, per-piece status update | `cargo test` |
| F076 | Blank placeholder DCInside icons | done | `create_placeholder_icon` command/repository, placeholder UI tile rendering, migration `003_icon_readiness_placeholders.sql` | `cargo test` / `npm.cmd run lint` |
| F077 | Icon readiness tags: complete/working | done | `icons.readiness`, context-menu actions, working tile styling, export skips non-complete icons | `cargo test` / `npm.cmd run lint` |
| F078 | Replace icon image from context menu | done | `replace_icon_source` command/repository and collection route file picker | `cargo test` / `npm.cmd run lint` |
| F079 | GIF file-size optimization design | done | `docs/FILE_SIZE_OPTIMIZATION_DESIGN.md` captured the design gate and has been reconciled after F084/F087/F100 shipped measured local optimization candidates and advanced controls without forbidden external binaries | design doc / F084-F088 / F100 verification |
| F080 | Duplicate icon inserts beside source icon | done | `duplicate_icon` shifts later `order_index` values and inserts the copy immediately after the source icon | `cargo test` / native duplicate adjacency regression |
| F081 | MIT license/dependency guardrails | done | `docs/LICENSE_POLICY.md`, `deny.toml`, `scripts/check-forbidden-dependencies.ps1`, package/Cargo MIT metadata | `npm.cmd run license:forbidden` / `npm.cmd run license:check` |
| F082 | Third-party license notices | done | `THIRD_PARTY_LICENSES.md`, `docs/THIRD_PARTY_LICENSES_GUIDE.md`, and `scripts/generate-third-party-licenses.ps1`; manual dependency review notes resolved and image/GIF/resize license coverage documented | `npm.cmd run license:generate` / `npm.cmd run license:check` |
| F083 | Processed asset variants for optimized exports | done | `processed_asset_variants` migration + `db/repositories/optimization.rs` | `cargo test` |
| F084 | GIF file-size optimization MVP | done | `src-tauri/src/optimization/gif_optimizer.rs` creates measured GIF candidates without external optimizer binaries | `optimization::tests::gif_candidates_are_actual_measured_files_and_original_is_preserved` |
| F085 | Static PNG/JPG resize/size optimization MVP | done | `src-tauri/src/optimization/static_optimizer.rs` re-encodes measured PNG/JPG candidates using existing permissive pipeline | `optimization::tests::static_jpg_candidate_can_be_applied_and_used_by_export` |
| F086 | Active optimized variant used by export | done | `export/mod.rs` resolves non-stale active variants by source/crop/profile hash and copies them into final export output | `cargo test` |
| F087 | Optimization UI candidate comparison | done | `ExportDialog.tsx` oversized item action and `OptimizationPanel` candidate cards with apply/clear flow | `npm.cmd run lint` / `npm.cmd run test` |
| F088 | Editor advanced optimization entry and GIF pingpong loop | done | `EditorPanel.tsx` pencil action opens advanced optimization panel; `gif_pingpong` migration/repositories/export/preview preserve pingpong behavior | `npm.cmd run lint` / `cargo test` |
| F089 | Export Workspace edit continuity and rerun actions | done | `ExportDialog.tsx` opens `EditorPanel` inside the export workspace and exposes file/rerun actions without leaving the export flow | `npm.cmd run lint` / `npm.cmd run test` |
| F090 | Same-size unchanged GIF export passthrough | done | `export_render.rs` copies unchanged single GIF exports directly when crop/size/loop are unchanged, avoiding re-encode size drift | `unchanged_single_gif_export_copies_original_without_reencoding` |
| F091 | Text overlay editor with user/OFL default fonts | done | `EditorPanel.tsx` text controls plus `update_icon_text_overlay`; `imaging/text_overlay.rs` renders real text into PNG/GIF preview/export using `fontdue`; defaults search installed OFL-friendly Korean fonts and otherwise require user-selected ttf/otf | `npm.cmd run lint` / `cargo test` |
| F092 | Export Workspace range/multi selection controls | done | `ExportDialog.tsx` supports Shift range selection, Ctrl toggle selection, selected-count toolbar, visible-row select, clear selection, include/exclude selected, and include/exclude all | `npm.cmd run lint` / `npm.cmd run test` |
| F093 | Collection icon sorting | done | `collection-route.tsx` adds `정렬하기` panel for name/alt sorting with ascending/descending order and persists via `reorder_icons` | `npm.cmd run lint` |
| F094 | Batch alt numeric suffix duplicate prevention | done | `batch-alt.ts` generates unique numeric suffixes for multi-target alt edits and leaves single-target edits unchanged | `npm.cmd run test` |
| F095 | Batch alt comma-separated assignment dialog | done | `IconGrid.tsx` replaces browser prompt with movable batch alt dialog and applies comma-separated values in selection order | `npm.cmd run test` |
| F096 | Export resize filter selection | done | `ExportDialog.tsx`, `export/mod.rs`, and `export_render.rs` support Nearest/Bilinear/Bicubic/Gaussian/Lanczos resize filters | `npm.cmd run lint` / `cargo test` |
| F097 | MB-based max byte inputs | done | `byte-size.ts`, collection settings, and export workspace convert editable MB values to byte limits | `npm.cmd run test` |
| F098 | Selected-item re-export into an existing export folder | done | `export_selected_collection_items` and `ExportDialog.tsx` replace only selected output files while preserving non-dirty session rows | `cargo test` / `npm.cmd run test` |
| F099 | Batch size optimization from Export Workspace selection | done | `ExportDialog.tsx` generates and applies optimization candidates for selected oversized items | `npm.cmd run lint` |
| F100 | Advanced optimization controls for GIF/JPG output | done | `EditorPanel.tsx` exposes GIF FPS limit, playback FPS, color limit, and JPG quality candidate controls | `cargo test` / `npm.cmd run lint` |
| F101 | Text overlay preview/export alignment | done | `CropCanvas.tsx`, `preview.rs`, and `export_render.rs` render the same text overlay in editor preview, generated preview, and final export | `cargo test` |
| F102 | Export workspace completed/pending filters and session merge | done | `export-workspace-model.ts` adds completed/pending filters and preserves written rows after targeted revalidation | `npm.cmd run test` |
| F103 | Static sprite sheet import | done | `sheet/importer.rs`, `SheetImportWizard.tsx`, `SheetGridOverlay.tsx`, `SheetCellReviewGrid.tsx` | `sheet::real_qa::qa_static_sheet_import_grid_cell_size_empty_alpha_and_jpg_warning` / native `native-sprite-sheet-qa-report.json` |
| F104 | Static edit sheet export | done | `sheet/exporter.rs`, `SheetExportDialog.tsx`, `SheetExportPreview.tsx` | `sheet::real_qa::qa_static_edit_sheet_export_clean_guide_manifest_and_page_split` / native `native-sprite-sheet-qa-report.json` |
| F105 | Clean sheet + guide sheet split output | done | `sheet/exporter.rs` writes separate `clean/` and `guide/` PNG pages; clean sheet has no grid/labels | `sheet::real_qa::qa_static_edit_sheet_export_clean_guide_manifest_and_page_split` / native screenshot review |
| F106 | `pmtcon-sheet-v1` manifest generation | done | `sheet/manifest.rs` + `export_edit_sheet` | `sheet::real_qa::qa_static_edit_sheet_export_clean_guide_manifest_and_page_split` / native manifest artifact |
| F107 | Manifest-based static sheet reimport | done | `sheet/reimport.rs`, `SheetReimportDialog.tsx` creates new icons or processed variant files without overwriting originals | `sheet::real_qa::qa_static_manifest_reimport_maps_cells_preserves_originals_and_reports_bad_inputs` / native reimport pass |
| F108 | PNG alpha preservation for sheet import/export/reimport | done | `sheet/importer.rs`, `sheet/exporter.rs`, `sheet/reimport.rs` keep RGBA PNG cells and transparent clean sheet backgrounds | `sheet::real_qa` import/export/reimport QA tests / native clean sheet review |
| F109 | GIF first-frame contact sheet mode | done | `sheet/exporter.rs` renders GIF first frame into static work sheets and returns warnings; `SheetExportDialog.tsx` labels this as `GIF 첫 프레임만 포함` behavior | `cargo test`; static sheet export tests |
| F110 | GIF frame sheet export | done | `sheet/gif_frames.rs` analyzes GIF icons, exports clean/guide PNG frame sheets with page splitting, and writes `pmtcon-gif-frame-sheet-v1`; `GifFrameSheetDialog.tsx` is wired from GIF-only context-menu actions | `cargo test --manifest-path src-tauri\Cargo.toml gif_frame --lib` |
| F111 | GIF frame sheet reimport | done | `sheet/gif_frames.rs` validates edited pages, detects missing/wrong-size pages, crops manifest cells, reassembles animated GIF variants, and preserves originals; `GifFrameSheetDialog.tsx` supports manifest/PNG drag-drop reimport | `cargo test --manifest-path src-tauri\Cargo.toml gif_frame --lib` |
| F112 | Manual slice mode | done | `sheet/slices.rs` analyzes/imports/saves named manual rectangles; `ManualSliceCanvas.tsx` is wired into `SheetImportWizard` as `직접 Slice 지정` with drag/create/move/resize, exact X/Y/W/H fields, include/exclude, duplicate/delete, metadata save, and alpha-preserving PNG import | `cargo test --manifest-path src-tauri\Cargo.toml manual_slice --lib` / `npm.cmd run test -- sheet-ui-model` |
| F113 | Auto-detect sheet slicing proposals | done | `sheet/auto_detect.rs` computes experimental alpha/solid-background separator proposals; `SheetAutoDetectPanel.tsx` lets users run detection, review confidence, and apply a proposal into the normal grid overlay/review workflow without auto-importing | `cargo test --manifest-path src-tauri\Cargo.toml auto_detect --lib` / `npm.cmd run test -- sheet-ui-model` |
| F114 | Context menu GIF frame sheet export | done | `IconContextMenu.tsx` exposes `GIF 프레임 작업시트로 내보내기` only for GIF icons and opens `GifFrameSheetDialog` export mode | `npm.cmd run test` / `cargo test` |
| F115 | Context menu GIF frame sheet replace/reimport | done | `IconContextMenu.tsx` exposes `GIF 프레임 작업시트로 교체하기` only for GIF icons and opens `GifFrameSheetDialog` reimport mode; original GIF remains preserved | `npm.cmd run test` / `cargo test` |
| F116 | Collection duplicate context menu UI | done | `CollectionGrid.tsx`/`CollectionCard.tsx` right-click menu calls `duplicate_collection`; backend copy names use numbered `복사본` conflicts | `cargo test duplicate_collection` / `npm.cmd run lint` |
| F117 | Icon memo/note persistence | done | `icon_notes` migration, `get/update/clear_icon_note` commands, icon DTO note field, `IconGrid` memo dialog | `cargo test icon_note` / `npm.cmd run test` |
| F118 | Icon memo hover indicator | done | `IconTile.tsx` shows notepad indicator beside icon name and hover multi-line memo tooltip | `npm.cmd run lint` |
| F119 | Selected-icons work sheet export | done | `IconContextMenu.tsx` selected export action passes selected IDs into `SheetExportDialog`; `export_edit_sheet` uses `source: selected_icons` | `edit_sheet_export_selected_icons_only_in_grid_order` / `npm.cmd run test` |
| F120 | Sheet grid presets | done | `sheet_grid_presets` migration, preset commands, `SheetGridPresetSelect` shared component | `presets::tests` / `npm.cmd run test` |
| F121 | Built-in sheet presets | done | migration creates protected `DCInside 200x200 / 5 columns` and `GIF Frames 200x200 / 8 columns / 64 frames` presets | `built_in_presets_are_listed_and_not_deleted` |
| F122 | Import/export shared preset application | done | import, static export, and GIF frame export dialogs apply compatible shared preset fields and preserve irrelevant fields safely | `sheet-ui-model.test.tsx` |
| F123 | Public user manual and release readiness | done | `docs/index.html` covers core editing, work sheets, GIF frame sheets, context menus, memo, presets, auto-detect, file preservation, and release links; `docs/RELEASE_READINESS.md`, `docs/RELEASE_NOTES_0.1.1.md`, and `docs/INSTALLER_DISTRIBUTION_QA.md` document release verification, installer QA, and user-facing release notes | docs link validation / release verification commands |
| F124 | Bounded image and sheet import resources | done | `lib/import-file.ts`, `imaging/import_limits.rs`, normal/source/sheet decode paths; ordinary images use sequential IPC and oversized files are individually rejected | `import-file.test.ts` / `imaging::import_limits::tests` / `cargo test` |
| F125 | Collection delete UI and sidebar invalidation | done | home toolbar, collection context menu, Delete key, `collections/events.ts`, existing soft-delete command | `npm.cmd run lint` / manual confirmation path |
| F126 | Responsive collection command bar | done | collection header stacks below 2xl and wraps all actions instead of clipping at the default 1200px window | `npm.cmd run build` / manual resize |
| F127 | Editor request ordering and modal focus accessibility | done | `EditorPanel.tsx` request token/remount; `use-modal-focus.ts`; sheet and icon dialogs | `npm.cmd run lint` / keyboard manual verification |
| F128 | npm lockfile and localized error repair | done | verified `package-lock.json`, removed pnpm lock and unused shadcn CLI, repaired GIF error text | clean `npm.cmd ci` / `npm.cmd audit` / source search |

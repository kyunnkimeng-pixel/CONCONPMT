# FEATURE_INVENTORY.md - PMTCONCON Studio 구현 누락 방지 체크리스트

Codex는 파일을 구현하는 동안 이 표를 계속 갱신해야 한다. `Status`는 `todo | doing | done | blocked | future` 중 하나를 사용한다.

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
| F052 | UI image generation reference workflow | done | local-only UI prompt + `docs/UI_TRACE.md` + `docs/ui-references/` | manual |
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
| F067 | Editor panel live draft preview and resizable width | done | `EditorPanel.tsx` adds source-driven live preview, persisted resize handle, and fixed-height route layout | `qa-artifacts/tauri-user-feedback-regression.mjs` / `npm.cmd run lint` |
| F068 | Explorer grid accidental text selection and drag responsiveness stabilization | done | `IconGrid.tsx` / `IconTile.tsx` suppress native selection and remove active drag transform transition | `qa-artifacts/tauri-user-feedback-regression.mjs` |
| F069 | Sidebar collection list navigation | done | `AppShell.tsx` lists collections and highlights the active route | native smoke / `npm.cmd run lint` |
| F070 | Export output folder picker | done | `ExportDialog.tsx` + `pick_export_directory` Tauri command via `rfd::FileDialog` | `npm.cmd run tauri -- build` / `cargo test` |
| F071 | DCInside count/alt warnings do not block sequence export per latest user direction | done | `export/mod.rs` classifies count/alt issues as warnings while keeping hard blockers for invalid export mechanics | `cargo test` |
| F072 | Multi-piece icons render as connected grouped cells in grid | done | `IconTile.tsx` renders horizontal/vertical double icons as linked two-cell previews with shared tile selection/drag | native smoke / `npm.cmd run lint` |

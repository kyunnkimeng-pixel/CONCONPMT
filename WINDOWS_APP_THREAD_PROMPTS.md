# WINDOWS_APP_THREAD_PROMPTS.md — Codex Windows App 붙여넣기용 프롬프트

이 파일은 Codex Windows App에서 새 thread를 만들 때 바로 복사해 붙여넣기 위한 프롬프트 모음이다. 자세한 준비 명령은 `CODEX_COMMANDS.md`를 참고한다.

## Thread A — 문서 동기화

```text
Read AGENTS.md, INITIAL_CODEX_PROMPT.md, docs/PRODUCT_SPEC.md, docs/FEATURE_INVENTORY.md, docs/IMPLEMENTATION_PLAN.md, docs/DECISIONS.md, and docs/UI_IMAGE_PROMPT.md.

Before coding, verify that the product name is PMTCONCON Studio everywhere.
Update docs/IMPLEMENTATION_PLAN.md and docs/FEATURE_INVENTORY.md if anything is inconsistent or missing.
Do not implement code yet. Summarize risks and the exact next implementation slice.
```

## Thread B — UI reference generation

```text
Read docs/UI_IMAGE_PROMPT.md and docs/FEATURE_INVENTORY.md.
Use $imagegen to create the four UI reference images.
Save them to docs/ui-references/.
Create docs/UI_TRACE.md mapping every feature ID to a UI component, route, context menu, command, or validation module.
Generated images are visual references only and cannot add or remove product features.
```

## Thread C — Phase 0 scaffold

```text
Read INITIAL_CODEX_PROMPT.md and implement Phase 0 only.
Use Tauri 2 + React + TypeScript + Vite + Tailwind CSS v4 + shadcn/ui-style editable components.
Create the PMTCONCON Studio app shell and configure scripts.
No dead menus. No fake features. Run checks and summarize.
```

## Thread D — Phase 1-2 persistence/import

```text
Implement Phase 1 and Phase 2 from docs/IMPLEMENTATION_PLAN.md.
Focus on SQLite persistence, app data library layout, collection CRUD, multiple-file/folder import, drag/drop import, original file copying, hash metadata, first-icon cover image, and restart persistence.
Update FEATURE_INVENTORY and tests.
```

## Thread E — Phase 3 icon explorer

```text
Implement Phase 3 from docs/IMPLEMENTATION_PLAN.md.
Focus on Explorer-like icon grid/list, inline alt edit, rename, duplicate, delete, set cover, Ctrl/Shift multi-select, keyboard Delete, context menu, and persistent drag reorder.
Update FEATURE_INVENTORY and tests.
```

## Thread F — Phase 4 editor/imaging

```text
Implement Phase 4 from docs/IMPLEMENTATION_PLAN.md.
Focus on right editor panel, shape selector, custom cell sizes, free/fixed crop modes, crop overlay, split lines, preset positions, GIF loop settings, crop metadata persistence, original preservation, PNG/JPEG/GIF processing, and tests.
Update FEATURE_INVENTORY.
```

## Thread G — Phase 5-6 preview/export

```text
Implement Phase 5 and Phase 6 from docs/IMPLEMENTATION_PLAN.md.
Focus on DCInside-like preview simulator, animated GIF preview, multi-piece display order, export profiles, DCInside validation, custom profile validation, sequence/alt filenames, alts.txt, and opening export outputs.
Update FEATURE_INVENTORY and tests.
```

## Thread H — review/fix pass

```text
Run a full review against AGENTS.md, PRODUCT_SPEC.md, FEATURE_INVENTORY.md, and UI_TRACE.md.
Find missing required features, dead menus, persistence issues, crop/export math bugs, GIF handling bugs, and validation gaps.
Fix high-confidence issues only, then run lint/test/build.
```

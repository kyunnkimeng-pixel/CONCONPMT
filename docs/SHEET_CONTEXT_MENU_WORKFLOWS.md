# Sheet Context Menu Workflows

Stage: `CONTEXT_MENU_SHEET_WORKFLOW_AND_PRESETS_MVP`

## GIF Icon Work Sheet Actions

GIF icon context menus expose GIF-only actions:

- `GIF 프레임 작업시트로 내보내기...`
- `GIF 프레임 작업시트로 교체하기...`

These actions are not shown for non-GIF icons. Export opens the existing GIF frame sheet dialog and writes clean frame sheets, guide sheets, and a `pmtcon-gif-frame-sheet-v1` manifest. Replace opens the same dialog in reimport mode. The UI uses `교체하기`, but the backend preserves the original GIF source and creates a processed GIF variant from the edited frame sheets.

Static `작업시트로 내보내기` for GIF or motion-enabled icons is a 0ms-poster contact-sheet workflow. It warns that animation is omitted and cannot reconstruct animation. Its `render_recipe_hash` includes crop, transform, text, static effects, and motion, so processed-output modes skip stale cells instead of writing or applying them.

## Selected Icon Work Sheet Export

When one or more icons are selected, the icon context menu exposes:

- `선택 항목 N개 작업시트로 내보내기`

The export dialog receives `selectedIconIds` and uses `source: selected_icons`, so only selected icons are exported. Ordering follows the collection grid order stored by `icons.order_index`, not right-click order.

If selected items include GIF or motion-enabled icons, the static work sheet includes only the 0ms poster frame and shows an animation-loss warning. Full frame, duration, and loop editing is handled by the GIF frame work sheet flow.

## Icon Memo

Icon context menus expose:

- `메모 추가` when no memo exists
- `메모 수정` when a memo exists
- `메모 삭제` when a memo exists

Memos are persisted in `icon_notes`. Empty or whitespace-only text clears the memo. Memos do not affect alt text, export filenames, export validation, or generated sheets.

Icon tiles always show a small memo add/edit button beside the icon name. Hovering or focusing an existing memo displays the multi-line memo near the button.

## Collection Duplicate

Collection cards expose a right-click action:

- `모음 복제`

The action calls the existing `duplicate_collection` backend command. The duplicate receives durable collection, export-profile, icon, piece, crop, note, placeholder, text, transform, effect, motion, frame-sheet provenance, and collection-preset metadata while preserving immutable source files. Effective active variants are copied to independently owned paths with remapped IDs and recalculated hashes; stale or missing variants fall back to the saved render recipe. Names use `<original> 복사본`; conflicts use `<original> 복사본 2`, and so on.

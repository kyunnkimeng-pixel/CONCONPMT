# Sheet Context Menu Workflows

Stage: `CONTEXT_MENU_SHEET_WORKFLOW_AND_PRESETS_MVP`

## GIF Icon Work Sheet Actions

GIF icon context menus expose GIF-only actions:

- `GIF 프레임 작업시트로 내보내기...`
- `GIF 프레임 작업시트로 교체하기...`

These actions are not shown for non-GIF icons. Export opens the existing GIF frame sheet dialog and writes clean frame sheets, guide sheets, and a `pmtcon-gif-frame-sheet-v1` manifest. Replace opens the same dialog in reimport mode. The UI uses `교체하기`, but the backend preserves the original GIF source and creates a processed GIF variant from the edited frame sheets.

Static `작업시트로 내보내기` for GIF icons remains a first-frame contact sheet workflow. It does not reconstruct animation.

## Selected Icon Work Sheet Export

When one or more icons are selected, the icon context menu exposes:

- `선택 항목 N개 작업시트로 내보내기`

The export dialog receives `selectedIconIds` and uses `source: selected_icons`, so only selected icons are exported. Ordering follows the collection grid order stored by `icons.order_index`, not right-click order.

If selected items include GIF icons, the static work sheet includes only the first frame and shows a warning. Full animation editing is handled by the GIF frame work sheet flow.

## Icon Memo

Icon context menus expose:

- `메모하기` when no memo exists
- `메모 수정` when a memo exists
- `메모 삭제` when a memo exists

Memos are persisted in `icon_notes`. Empty or whitespace-only text clears the memo. Memos do not affect alt text, export filenames, export validation, or generated sheets.

Icon tiles show a small notepad indicator beside the icon name when a memo exists. Hovering the indicator displays the multi-line memo near the indicator.

## Collection Duplicate

Collection cards expose a right-click action:

- `모음 복제하기`

The action calls the existing `duplicate_collection` backend command. The duplicate receives copied collection metadata, export profiles, icon rows, piece rows, crop settings, and icon notes while preserving source files. Names use `<original> 복사본`; conflicts use `<original> 복사본 2`, and so on.

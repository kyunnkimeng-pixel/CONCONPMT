# UI_IMAGE_PROMPT.md — Codex 이미지 생성용 UI 참조 프롬프트

이 프롬프트는 Codex Windows App의 이미지 생성 기능 또는 별도 이미지 생성 도구로 UI reference를 만들 때 사용한다. 생성된 이미지는 절대 기능 명세를 대체하지 않는다.

## Important guardrails
- Generate **visual references only**. Do not treat the image as a functional spec.
- Include **only** controls listed in `docs/FEATURE_INVENTORY.md` and `docs/PRODUCT_SPEC.md`.
- Do **not** invent inactive menus, decorative tabs, fake premium buttons, fake cloud sync, account login, marketplace, or settings that are not in the spec.
- If the image generator omits a required feature, the implementation must still include that feature.
- After generating references, create `docs/UI_TRACE.md` mapping every feature ID to a visible UI location/component.

## Prompt 1 — Main explorer screen
Use `$imagegen` if available:

```text
$imagegen Create a polished Windows 11 Explorer-inspired desktop app UI reference for a Korean app named “PMTCONCON Studio”. Screen: main collection explorer. Show a left navigation rail, top breadcrumb “홈”, toolbar with a working + button only, search box, grid of “디시콘 모음” cards. Each card has a 200x200 representative icon preview and an inline-editable Korean name label. No extra menus beyond: 새 모음, 파일 가져오기, 폴더 가져오기. Modern subtle glass/mica feel, compact spacing, Korean labels, no fake cloud/account/premium buttons.
```

## Prompt 2 — Collection detail screen
```text
$imagegen Create a UI reference for PMTCONCON Studio collection detail screen. Windows Explorer-like grid of icon tiles, breadcrumb “홈 > 모음명”, top toolbar with back, + add files, export, preview simulator. Icon tiles show animated-image placeholder, alt text label below like a filename, selected multi-select state, drag reorder handle. Right-click context menu visible with only: 수정, 이름 변경, 복제, 삭제, 대표 이미지로 설정. Korean labels only. No unused buttons.
```

## Prompt 3 — Editor + preview simulator
```text
$imagegen Create a UI reference for PMTCONCON Studio icon editor. Left/main area shows source image with a crop rectangle overlay. Right side panel has Korean controls: 모양 선택(단일콘, 가로 이중콘, 세로 이중콘), 기준 크기, 자유모드/고정모드, 위치 프리셋 3x3, GIF 반복, 적용, 원본으로 초기화. Show split line for horizontal double icon. Bottom or side includes a DCInside comment-like preview where the output appears at 100x100. Korean labels only. No fake features outside the spec.
```

## Prompt 4 — Export validation screen
```text
$imagegen Create a UI reference for PMTCONCON Studio export screen. Show selected collection, DCInside profile, output folder picker, filename mode sequence/alt, alts.txt checkbox, validation table with hard errors and warnings, and a primary “Export” button disabled until hard errors are resolved. Include Korean validation examples: 알트값 중복, 파일 용량 2MB 초과, 10개 미만. No cloud upload, no marketplace, no login.
```

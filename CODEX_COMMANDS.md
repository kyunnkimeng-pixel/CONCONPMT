# CODEX_COMMANDS.md — PMTCONCON Studio / Codex Windows App 진행 가이드

이 문서는 **Codex Windows App**으로 PMTCONCON Studio를 만들 때 쓰는 준비 명령어와 thread별 붙여넣기 프롬프트다. Codex CLI 설치는 필요하지 않다. 다만 Tauri 프로젝트 생성과 검증은 Codex App의 통합 터미널 또는 Windows PowerShell에서 실행한다.

## 0. Codex Windows App 작업 원칙

- Codex App에서 이 프로젝트 폴더를 열고 **Local** 또는 **Worktree** thread로 작업한다.
- 처음에는 **Local**을 추천한다. 큰 기능을 여러 갈래로 시도할 때는 Worktree thread를 사용한다.
- 권한은 가능하면 기본 sandbox/approval 설정을 유지한다. 전체 디스크 접근이나 관리자 권한은 피한다.
- Codex가 작업하기 전 프로젝트 루트의 `AGENTS.md`와 `docs/` 문서를 읽게 만든다.
- 생성 UI 이미지는 시각 참고용이다. 기능 원본은 `docs/PRODUCT_SPEC.md`와 `docs/FEATURE_INVENTORY.md`다.

## 1. Windows 개발 환경 준비

Codex App은 Microsoft Store에서 설치한다. 명령형 설치가 편하면 PowerShell에서 다음을 실행한다.

```powershell
winget install Codex -s msstore
```

Tauri 개발용 기본 도구:

```powershell
winget install --id Git.Git -e
winget install --id OpenJS.NodeJS.LTS -e
winget install --id Rustlang.Rustup -e
npm install -g pnpm
```

추가로 Windows에서 Tauri 빌드를 위해 **Microsoft C++ Build Tools**의 `Desktop development with C++` 워크로드와 **Microsoft Edge WebView2 Runtime**이 필요하다. 설치 후 새 PowerShell을 열고 확인한다.

```powershell
git --version
node --version
pnpm --version
rustc --version
cargo --version
```

## 2. 프로젝트 생성

PowerShell 또는 Codex App의 통합 터미널에서 실행한다.

```powershell
mkdir PMTCONCONStudio
cd PMTCONCONStudio
npm create tauri-app@latest
```

Tauri scaffold 프롬프트가 나오면 다음처럼 선택한다.

```text
Project name: pmtconcon-studio
Identifier: com.pmtconcon.studio
Frontend language: TypeScript / JavaScript
Package manager: pnpm
UI template/framework: React
UI flavor: TypeScript
```

생성 후:

```powershell
cd pmtconcon-studio
pnpm install
git init
git add .
git commit -m "chore: scaffold tauri react app"
```

## 3. 이 패키지의 문서 복사

이 답변에 첨부된 `pmtconcon_studio_codex_pack` 폴더의 파일들을 프로젝트 루트에 복사한다.

```powershell
# 예시: 다운로드한 폴더가 Downloads에 있다고 가정
Copy-Item "$env:USERPROFILE\Downloads\pmtconcon_studio_codex_pack\AGENTS.md" .\AGENTS.md
Copy-Item "$env:USERPROFILE\Downloads\pmtconcon_studio_codex_pack\CODEX_COMMANDS.md" .\CODEX_COMMANDS.md
Copy-Item "$env:USERPROFILE\Downloads\pmtconcon_studio_codex_pack\INITIAL_CODEX_PROMPT.md" .\INITIAL_CODEX_PROMPT.md
Copy-Item "$env:USERPROFILE\Downloads\pmtconcon_studio_codex_pack\WINDOWS_APP_THREAD_PROMPTS.md" .\WINDOWS_APP_THREAD_PROMPTS.md
Copy-Item "$env:USERPROFILE\Downloads\pmtconcon_studio_codex_pack\docs" .\docs -Recurse -Force

git add AGENTS.md CODEX_COMMANDS.md INITIAL_CODEX_PROMPT.md WINDOWS_APP_THREAD_PROMPTS.md docs
git commit -m "docs: add PMTCONCON Studio product spec and agent instructions"
```

## 4. Codex Windows App에서 프로젝트 열기

1. Codex App 실행.
2. `Add project` 또는 프로젝트 선택에서 `pmtconcon-studio` 폴더 열기.
3. 새 thread 생성.
4. 모드는 처음에는 **Local** 선택.
5. 첫 메시지에는 아래 Thread 1 프롬프트를 붙여넣는다.

## 5. Thread 1 — 문서 읽기와 구현 계획 정리

```text
Read AGENTS.md, INITIAL_CODEX_PROMPT.md, and every markdown file under docs/. Do not implement code yet.

Output and update a concrete plan for PMTCONCON Studio:
1. Confirm the app name is PMTCONCON Studio everywhere.
2. Update docs/IMPLEMENTATION_PLAN.md with project-specific risks and phase boundaries.
3. Review docs/FEATURE_INVENTORY.md and add any missing feature IDs required by PRODUCT_SPEC.md.
4. Identify likely risky areas: GIF processing, crop math, multi-piece export order, alt validation, persistence, Windows filesystem safety.
5. Do not add any generated-image-only menus or fake features.

After updating docs, summarize changed files and stop.
```

## 6. Thread 2 — UI 이미지 reference 생성

Codex Windows App은 thread 안에서 이미지 생성 기능을 사용할 수 있다. 이 프롬프트는 UI reference만 만들기 위한 것이다.

```text
Read docs/UI_IMAGE_PROMPT.md and docs/FEATURE_INVENTORY.md.
Use $imagegen to generate the four UI reference images described in docs/UI_IMAGE_PROMPT.md.
Save references under docs/ui-references/.
Then create docs/UI_TRACE.md mapping every feature ID in docs/FEATURE_INVENTORY.md to a planned visible UI component, route, command, or validation module.

Important:
- Generated images are visual references only.
- Do not implement fake menus that only appear in generated images.
- Do not remove required features that are absent from generated images.
- The app name in every reference must be PMTCONCON Studio.
```

## 7. Thread 3 — MVP 구현 시작

Codex App의 composer에 아래를 붙여넣는다. 파일 내용을 직접 읽게 만들기 위해 `INITIAL_CODEX_PROMPT.md`도 함께 언급한다.

```text
Read INITIAL_CODEX_PROMPT.md and follow it. Start implementing PMTCONCON Studio from Phase 0.

Scope for this thread:
1. Install/configure the frontend stack: React + TypeScript + Vite, Tailwind CSS v4, shadcn/ui-compatible components, TanStack Router, Zustand, dnd-kit, react-konva.
2. Configure the Tauri 2 + Rust structure and app title/identifier for PMTCONCON Studio.
3. Create an Explorer-like empty shell: left nav, toolbar, breadcrumb, collection grid placeholder, right-side editor placeholder, preview/export entry points only if wired or clearly disabled.
4. Add scripts for lint/test/build/tauri dev/build where practical.
5. Update docs/FEATURE_INVENTORY.md statuses for completed scaffolding items.
6. Run pnpm lint/test/build if scripts exist. Fix failures caused by your changes.

Do not implement dead menus. Do not create fake cloud/account/premium features.
Stop after Phase 0 is stable and summarize exact commands run.
```

## 8. Phase별 안정 진행 프롬프트

한 thread에서 모두 시키지 말고 아래 프롬프트를 새 thread 또는 같은 thread의 후속 메시지로 순서대로 사용한다.

### Phase 1 — DB와 라이브러리 저장소

```text
Implement Phase 1 from docs/IMPLEMENTATION_PLAN.md for PMTCONCON Studio.
Add SQLite schema/migrations for collections, assets, icons, icon_pieces, export_profiles.
Add Tauri commands for collection CRUD, icon listing, and order updates.
Store the DB and generated/original asset paths under the app data directory.
Add Rust tests for migrations and repository basics.
Update docs/FEATURE_INVENTORY.md statuses.
Run relevant checks and summarize results.
```

### Phase 2 — 탐색기 UI와 import

```text
Implement Phase 2 from docs/IMPLEMENTATION_PLAN.md.
Build the main collection grid, breadcrumb, + menu, multiple file import, folder import, and drag/drop import.
Copy jpg/jpeg/png/gif originals into the app library, hash them, create icon records, and auto-set the first icon as collection cover.
Persist state across restart.
No dead menus. Update FEATURE_INVENTORY and tests.
```

### Phase 3 — 아이콘 관리

```text
Implement Phase 3 icon management.
Inside a collection, add icon grid/list, inline alt editing, icon rename, Ctrl multi-select, Shift range select, keyboard Delete, context menu actions, duplicate/delete/set cover, and drag reorder with persistent order_index.
Validate alt duplicate/length/characters in the UI and shared validation code.
Update FEATURE_INVENTORY and tests.
```

### Phase 4 — 편집기와 이미지 처리

```text
Implement Phase 4 editor and imaging pipeline.
Add the right-side editor panel with source preview, output preview, shape selector(single/horizontal_double/vertical_double), cell size settings, free/fixed crop modes, React-Konva crop box, split lines, 9-position fixed presets, GIF loop settings, apply/reset/revert.
Implement Rust crop/resize for PNG/JPEG and GIF frame crop/resize while preserving original files and crop metadata.
Add crop math and GIF tests.
Update FEATURE_INVENTORY.
```

### Phase 5 — 실사용 미리보기

```text
Implement Phase 5 preview simulator.
Create a DCInside-comment-like preview where users can insert icons between text, see default 100x100 display size, keep GIFs animated, and render multi-piece icons in piece order.
Add tests/manual verification and update FEATURE_INVENTORY.
```

### Phase 6 — export와 검증

```text
Implement Phase 6 export and validation.
Support DCInside and custom profiles, sequence filenames 001~, alt filename mode, alts.txt generation, output folder picker, export folder/txt opening, hard errors vs soft warnings, and persisted order/piece order.
DCInside validation must enforce or warn exactly as docs/PRODUCT_SPEC.md says: 10-200 output images, 200x200 default pieces, jpg/png/gif, 2MB max per file, unique alt values, Korean grapheme length 1-3, allowed specials * ^ ! ~ +, transparent background and 5px margin as warnings.
Add tests and update FEATURE_INVENTORY.
```

## 9. 리뷰 프롬프트

Codex App의 Review/diff 기능과 함께 아래 프롬프트를 사용한다.

```text
Review the current implementation against AGENTS.md, docs/PRODUCT_SPEC.md, docs/FEATURE_INVENTORY.md, and docs/UI_TRACE.md.
Focus on:
1. Required features omitted because UI images did not show them.
2. Dead menus or fake features invented by generated images.
3. Persistence bugs for names, alts, order, crop boxes, covers, GIF loop settings, and export profiles.
4. Crop math errors for single/horizontal_double/vertical_double.
5. GIF animation/loop/export correctness.
6. DCInside validation correctness.
7. Windows filesystem safety and original-file preservation.

Produce a prioritized bug list, then fix only high-confidence issues. Run checks and update FEATURE_INVENTORY.
```

## 10. 검증 명령어

Codex App 통합 터미널에서 실행한다.

```powershell
pnpm lint
pnpm test
pnpm build
pnpm tauri dev
pnpm tauri build
```

실패하면 Codex에게 터미널 출력과 함께 다음처럼 요청한다.

```text
The validation command failed. Read the terminal output, identify the root cause, and fix the smallest safe set of files. Do not bypass tests or remove required functionality. Re-run the failed command.
```

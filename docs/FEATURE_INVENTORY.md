# FEATURE_INVENTORY.md — 구현 누락 방지 체크리스트

Codex는 이 파일을 구현 중 계속 갱신해야 한다. `Status`는 `todo | doing | done | blocked | future` 중 하나를 사용한다.

| ID | Feature | Status | Component / Rust module | Test |
|---|---|---:|---|---|
| F001 | 메인 화면에 디시콘 모음 grid 표시 | todo | `features/collections` | UI test |
| F002 | 모음 더블클릭 진입 | todo | router + collection grid | UI test |
| F003 | Windows Explorer-like toolbar/breadcrumb | todo | `app/AppShell.tsx` | UI snapshot |
| F004 | 우상단 `+` 메뉴: 새 모음 | todo | collection actions | UI test |
| F005 | 우상단 `+` 메뉴: 여러 파일 가져오기 | todo | Tauri dialog/import command | integration/manual |
| F006 | Drag and drop import | todo | dropzone + Tauri command | UI/manual |
| F007 | 대표 이미지 자동 설정: 첫 아이콘 | todo | DB + collection card | unit |
| F008 | 대표 이미지 수동 변경 | todo | context menu/edit dialog | UI test |
| F009 | 모음 이름 inline rename | todo | collection card | UI test |
| F010 | 아이콘 grid/list 표시 | todo | `features/icons` | UI test |
| F011 | 아이콘명 rename | todo | icon actions | UI test |
| F012 | 아이콘별 대표/썸네일 override | todo | editor metadata | unit/manual |
| F013 | 아이콘 drag reorder | todo | dnd-kit sortable | UI test |
| F014 | order persistence across restart | todo | DB repository | integration |
| F015 | 알트값 inline edit | todo | alt label editor | UI test |
| F016 | 알트값 persistence | todo | DB repository | integration |
| F017 | 알트값 validation: length/characters | todo | `lib/validation.ts` + Rust mirror | unit |
| F018 | 알트값 중복 표시 | todo | collection validation | unit/UI |
| F019 | 삭제 / 다중 삭제 | todo | context menu + soft delete | UI/DB test |
| F020 | Ctrl multi-select | todo | selection model | UI test |
| F021 | Shift range select | todo | selection model | UI test |
| F022 | 우클릭 메뉴: delete/rename/duplicate/edit/set cover | todo | context menu | UI test |
| F023 | 디시콘 복제 | todo | clone icon command | DB test |
| F024 | 디시콘 모음 복제 | todo | clone collection command | DB test |
| F025 | 단일콘 edit mode | todo | editor + imaging | unit/UI |
| F026 | 가로 이중콘 edit mode | todo | editor + imaging | crop unit |
| F027 | 세로 이중콘 edit mode | todo | editor + imaging | crop unit |
| F028 | 자유모드 crop box move/resize | todo | react-konva editor | UI/manual |
| F029 | 고정모드 crop box fixed size | todo | react-konva editor | UI/unit |
| F030 | 이중콘 split line 표시 | todo | react-konva editor | UI/manual |
| F031 | fixed mode 9-position presets | todo | editor controls | unit/UI |
| F032 | apply crop without deleting source | todo | Rust imaging + DB metadata | unit |
| F033 | edit crop after apply | todo | DB crop metadata restore | integration |
| F034 | 다양한 input resolution center crop/downsize | todo | imaging pipeline | unit |
| F035 | collection/icon 기준 사이즈 변경 | todo | settings/editor | unit/UI |
| F036 | GIF preview continuous animation | todo | image renderer | UI/manual |
| F037 | GIF frame crop/resize | todo | Rust gif pipeline | unit/manual |
| F038 | GIF loop settings | todo | editor + gif encode | unit/manual |
| F039 | 미리 사용해보기: DC comment-like UI | todo | `features/preview` | UI test |
| F040 | preview at 100×100 display size | todo | preview simulator | UI test |
| F041 | export validation DC count 10–200 | todo | validation/export | unit |
| F042 | export validation file <= 2MB | todo | export command | integration |
| F043 | export validation format jpg/png/gif | todo | export command | unit |
| F044 | export sequence filename 001~ | todo | export naming | unit |
| F045 | export alt filename mode | todo | export naming | unit |
| F046 | export `alts.txt` | todo | export command | integration |
| F047 | open export folder / alt txt | todo | Tauri command | manual |
| F048 | transparent background / 5px margin warnings | todo | validator | unit/manual |
| F049 | import original copied into app library | todo | import command | integration |
| F050 | SQLite migrations | todo | `src-tauri/migrations` | Rust test |
| F051 | no generated UI-only dead menus | todo | UI trace review | review |
| F052 | UI image generation reference workflow | todo | `docs/UI_IMAGE_PROMPT.md` | manual |

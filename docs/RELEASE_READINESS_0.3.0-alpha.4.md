# PMTCONCON Studio 0.3.0-alpha.4 Release Readiness

검증일: 2026-07-29 (KST)

## 릴리스 범위

- Gemini 2.5/3.1 이미지 요청 계약 수정과 400 오류의 키·결제·요청 형식 분류
- 같은 비율의 큰 AI 결과 보존·경고·로컬 목표 크기 정규화
- 모음 내부 아이콘과 외부 PNG/JPG/GIF를 사용한 생성 참고 이미지 board
- GIF 프레임 시트의 Gemini AI Studio/NovelAI 수동 웹 AI 왕복과 timing·loop 복원
- Windows 관리 경로의 raw/canonical 별칭을 허용하는 native drag-out 보강
- 참고 이미지 생성 전달을 최근 AI 전달의 끌기·탐색기·취소 흐름에 통합

## 자동 검증

| 검사 | 결과 |
| --- | --- |
| `npm.cmd run lint` | PASS |
| `npm.cmd run test` | PASS — 56 files, 336 tests |
| `npm.cmd run build` | PASS |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` | PASS — 342 tests |
| `npm.cmd run license:generate` | PASS |
| `npm.cmd run license:check` | PASS |
| `npm.cmd run license:forbidden` | PASS |
| `npm.cmd run tauri -- build --bundles nsis` | PASS |
| `npm.cmd run release:checksums` | PASS |

`cargo-deny`와 `cargo-about`은 로컬에 설치되어 있지 않아 선택 검사만 SKIPPED로
기록했습니다. 이번 변경은 새 의존성을 추가하지 않았고, 현재 의존성 그래프에서
`THIRD_PARTY_LICENSES.md`를 다시 생성했습니다.

Vite는 기존 단일 메인 chunk가 500kB보다 크다는 성능 경고를 표시합니다. Rust
release 빌드는 미사용 메서드·필드 경고 12개를 표시하지만 빌드와 기능 검증을
막지는 않습니다.

## 브라우저 UI 검증

최신 production bundle을 headed Chromium에서 Tauri 명령 모의와 함께 확인했습니다.

- `AI 아이콘 만들기`에서 모음 내부 아이콘 2개 선택 및 `2/16` 카운트
- 외부 참고 이미지의 합계 16MB 안내와 PNG/JPG/GIF 입력 경로 노출
- `prepare_ai_generation_workspace` 호출에 `referenceIconIds: ["i1", "i2"]` 전달
- 준비된 참고 board 미리보기와 출력 배치가 아닌 참고 자료임을 명시하는 프롬프트
- 최근 AI 전달의 `파일 끌기`가 `requestId`만으로 `start_ai_web_handoff_drag` 호출
- 1200×760과 800×760에서 body/dialog 가로 넘침 없음
- 예상 밖 Tauri 명령 0, 앱 JavaScript 오류 0

브라우저에는 기능과 무관한 `/favicon.ico` 404 한 건만 기록됐습니다. GIF AI 왕복은
전용 프런트 컴포넌트 테스트와 Rust frame-sheet 왕복 테스트로 검증했습니다.

실제 외부 웹사이트의 로그인, DOM, 쿠키, 업로드 완료와 생성 결과는 자동화하지
않습니다. 실제 유료 Gemini/NovelAI API 호출도 하지 않아 계정 비용·키 노출 없이
요청 직렬화, 응답 분류와 오류 redaction을 검증했습니다.

## 설치 산출물

- NSIS:
  `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.3.0-alpha.4_x64-setup.exe`
- 크기: `7,015,871` bytes
- SHA-256:
  `9f65e173a979acf56df1ddeaa535dc64b07117735c0e8b052bf9a50592577815`
- 체크섬:
  `src-tauri/target/release/bundle/SHA256SUMS.txt`
- Product name: `PMTCONCON Studio`
- Product/File version: `0.3.0-alpha.4`
- Authenticode: `NotSigned`

문자가 포함된 prerelease identifier를 MSI가 허용하지 않는 기존 제약 때문에,
버전을 바꾸지 않고 프리릴리스를 지원하는 NSIS 설치 파일을 릴리스 산출물로
확정했습니다. 깨끗한 별도 Windows PC 설치 검증과 코드 서명은 아직 수행하지
않았습니다.

## 보존·안전성 확인

- Gemini 오류 본문과 API 키는 UI·직렬화 오류에 그대로 노출하지 않습니다.
- 참고 board는 별도 관리 파일로 만들며 내부 아이콘·외부 원본을 변경하지 않습니다.
- 외부 참고 파일은 JS IPC 변환 전에 합계 16MiB를 제한하고, 전체 참고 원본은
  128,000,000 픽셀을 초과하면 거부합니다.
- GIF 참고는 첫 프레임 포스터만 쓰며, 비정사각형 입력은 늘이지 않고 투명 여백에
  contain 배치합니다.
- 같은 비율의 AI 결과는 원본을 보존한 채 후보 검토에서 목표 크기로 정규화하고,
  다른 비율 또는 필요한 알파 손실은 계속 차단합니다.
- GIF AI 결과는 새 처리 버전으로 가져오며 원본 GIF, 프레임 순서·delay·loop 설정을
  보존합니다.
- 네이티브 드래그는 raw/canonical 관리 루트 별칭을 허용하되, 관리 경로 이탈,
  누락 파일, 디렉터리와 symlink/reparse를 계속 거부합니다.

# PMTCONCON Studio 0.3.0-alpha.3 Release Readiness

검증일: 2026-07-29 (KST)

## 릴리스 범위

- 선택한 정적 단일 아이콘 2–16개 그리드 일괄 편집과 all-or-none 후보 저장
- 원본 없는 단일/최대 16개 그리드 아이콘의 원자적 생성
- GIF 프레임 스프라이트 manifest 왕복과 프레임별 delay·loop 복원
- 검증된 Windows 파일의 native drag-out과 Explorer 대체 경로
- 단일·그리드 전달을 합친 256MiB 제한, 7일 만료, 15분 주기 정리와 최근 기록

## 자동 검증

| 검사 | 결과 |
| --- | --- |
| `npm.cmd run lint` | PASS |
| `npm.cmd run test` | PASS — 54 files, 326 tests |
| `npm.cmd run build` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --all-targets` | PASS — 322 tests |
| `npm.cmd run license:generate` | PASS |
| `npm.cmd run license:check` | PASS |
| `npm.cmd run license:forbidden` | PASS |
| `npm.cmd run tauri -- build --bundles nsis` | PASS |

`cargo-deny`와 `cargo-about`은 로컬에 설치되어 있지 않아 선택 검사만
SKIPPED로 기록했습니다. 저장소의 Windows 실제 의존성 그래프에서 고지를 다시
생성했고, 새 `drag 2.1.1` 의존성은 `Apache-2.0 OR MIT`입니다.

Vite는 기존 단일 메인 chunk가 500kB보다 크다는 성능 경고를 표시하지만 빌드와
기능 검증을 막지는 않습니다. Rust의 기존 미사용 코드 경고도 오류가 아닙니다.

## 브라우저 UI 검증

최신 production bundle을 headed Chromium에서 Tauri 명령 모의와 함께 확인했습니다.

- 2개 Ctrl 다중 선택 → `선택 2개 AI로 수정` → 1×2 배치 → 웹 전달
- 전달 단계의 `요청 취소 후 새 작업`, `입력 파일 끌기`, `탐색기에서 선택`
- native drag가 임의 경로가 아닌 `requestId`만으로
  `start_ai_grid_input_drag`를 호출
- 최근 목록에서 단일 전달과 AI 그리드 전달을 구분하고 각 전용 명령으로 분기
- 800×760에서 body/dialog 가로 넘침 없음
- 브라우저 console error 0, 예상 밖 Tauri 명령 0

실제 외부 웹사이트의 로그인, DOM, 쿠키, 업로드 완료와 생성 결과는 자동화하지
않습니다. 실제 유료 Gemini/NovelAI API 호출도 이 로컬 기능 릴리스의 검증 범위가
아닙니다.

## 설치 산출물

- NSIS:
  `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.3.0-alpha.3_x64-setup.exe`
- 크기: `6,983,325` bytes
- SHA-256:
  `2abc39fff83884977054a4f1df6e9b9e4ec39563de8cf0d2734de313a8e8b0cd`
- 체크섬:
  `src-tauri/target/release/bundle/SHA256SUMS.txt`
- Product name: `PMTCONCON Studio`
- Product/File version: `0.3.0-alpha.3`
- Authenticode: `NotSigned`

기본 `targets: all` 빌드는 release 실행 파일 컴파일까지 성공했으나 MSI가
문자가 포함된 optional prerelease identifier를 허용하지 않아 번들 단계에서
중단됐습니다. 버전을 바꾸지 않고 프리릴리스를 지원하는 NSIS 설치 파일을
릴리스 산출물로 확정했습니다.

## 보존·안전성 확인

- 준비·검토 중에는 원본, crop과 활성 AI 소스를 변경하지 않습니다.
- 그리드 편집은 필수 셀 하나라도 비거나 구조가 다르면 저장하지 않습니다.
- 원본 없는 생성의 최종 목록과 검토된 포함 셀이 다르면 전체 트랜잭션을
  롤백합니다.
- 네이티브 드래그는 앱 관리 루트와 파일 무결성을 다시 확인하고
  symlink/reparse와 관리 경로 이탈을 거부합니다.
- 총량 정리는 완료·취소·만료 payload부터 처리하고 활성
  `prepared/awaiting_result` 요청을 공간 확보 목적으로 삭제하지 않습니다.
- 다른 아이콘이나 후보가 참조하는 source bytes는 삭제하지 않습니다.
- 잠긴 파일은 `cleanup_pending`으로 남겨 다음 주기에 재시도합니다.

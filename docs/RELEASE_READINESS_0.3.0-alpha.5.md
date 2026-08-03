# PMTCONCON Studio 0.3.0-alpha.5 Release Readiness

검증일: 2026-08-03 (KST)

## 릴리스 범위

- GIF AI 왕복에서 `frames_sheet_*.png`만 업로드하도록 파일 역할과 안내를 단순화
- 같은 작업 창의 manifest 자동 재사용과 세션이 끝난 경우의 수동 복구 경로
- GIF 시트 페이지별 안전한 네이티브 드래그와 Explorer 선택 대체 흐름
- PNG/JPG/JPEG/정적 WebP 결과 판별·정규화, 페이지 슬롯 매핑과 animated WebP 차단
- 원본 GIF를 보존한 프레임 순서·delay·loop 복원과 최종 결과 미리보기
- 실제 알파와 그려진 체커보드 배경을 구분하는 투명도 정책 및 명시적 불투명 결과 동의
- Gemini의 원본 그림체 유지 프롬프트와 NovelAI Image2Image/Vibe Transfer/Precise Reference 안내

## 자동 검증

| 검사 | 결과 |
| --- | --- |
| `npm.cmd run lint` | PASS |
| `npm.cmd run test` | PASS — 60 files, 394 tests |
| `npm.cmd run build` | PASS |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` | PASS — 371 tests |
| `npm.cmd run license:generate` | PASS |
| `npm.cmd run license:check` | PASS |
| `npm.cmd run license:forbidden` | PASS |
| `npm.cmd run tauri -- build --bundles nsis` | PASS |
| `npm.cmd run release:checksums` | PASS |

`cargo-deny`와 `cargo-about`은 로컬에 설치되어 있지 않아 선택 검사만 SKIPPED로
기록했습니다. 이번 변경은 새 의존성을 추가하지 않았고 현재 의존성 그래프에서
`THIRD_PARTY_LICENSES.md`를 다시 생성했습니다.

Vite는 기존 메인 chunk가 500kB보다 크다는 성능 경고를 표시합니다. Rust release
빌드는 미사용 함수·필드 경고 13개를 표시하지만 빌드와 기능 검증을 막지는 않습니다.

## 기능 검증 범위

- 프런트엔드 테스트로 GIF manifest 자동 재사용, 복구용 manifest 선택, 파일 역할 표시,
  페이지별 드래그 준비, 결과 페이지 슬롯 연결과 사용자 오류 메시지를 검증했습니다.
- Rust 테스트로 실제 파일 형식 판별, 정적 WebP/JPEG 변환, 알파 정책, 체커보드 감지,
  정확한 시트 형상과 GIF 프레임 timing·loop 복원을 검증했습니다.
- 실제 Gemini/NovelAI 유료 요청과 외부 웹사이트의 로그인·DOM·다운로드 자동화는
  실행하지 않았습니다. 공급자 웹 흐름은 사용자가 파일과 프롬프트를 직접 전달하는
  구조를 유지합니다.

## 설치 산출물

- NSIS 원본:
  `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.3.0-alpha.5_x64-setup.exe`
- GitHub 업로드 파일:
  `src-tauri/target/release/bundle/PMTCONCON.Studio_0.3.0-alpha.5_x64-setup.exe`
- 크기: `7,125,377` bytes
- SHA-256:
  `ccb083e9d0b20b831de1b592b4aa05ebb28332a9904727f733f9ffa99a7afde1`
- 체크섬:
  `src-tauri/target/release/bundle/SHA256SUMS.txt`
- Product name: `PMTCONCON Studio`
- Product/File version: `0.3.0-alpha.5`
- Authenticode: `NotSigned`

NSIS 원본과 GitHub 업로드 파일의 SHA-256은 동일하며 `SHA256SUMS.txt`를
`certutil`의 독립 계산 결과와 대조했습니다. 문자가 포함된 prerelease identifier를
MSI가 허용하지 않는 제약 때문에 NSIS만 배포합니다. 깨끗한 별도 Windows PC 설치
검증과 코드 서명은 아직 수행하지 않았습니다.

## 보존·안전성 확인

- AI 결과를 적용하기 전까지 원본 아이콘과 원본 GIF는 변경하지 않습니다.
- 같은 대화 상자의 manifest는 앱 내부에서 유지하며 브라우저에 업로드하지 않습니다.
- 페이지 드래그는 앱이 검증해 만든 관리 경로의 복사본만 사용합니다.
- 투명도 필수 모드에서 불투명 결과와 고신뢰도 가짜 체커보드는 차단합니다.
- 배경 포함 모드는 사용자의 명시적 동의와 최종 미리보기를 요구합니다.
- 다운로드 확장자와 실제 디코딩 형식이 달라도 실제 형식을 기준으로 처리하고,
  animated WebP처럼 지원하지 않는 입력은 구체적인 오류로 중단합니다.

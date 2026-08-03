# AI_WEB_HANDOFF_UX.md — 빠른 웹 AI 왕복

## 목적

PMTCONCON Studio의 웹 AI 전달은 브라우저 확장이나 로그인 세션 자동화를 요구하지
않는다. 사용자가 작업 흐름을 끊지 않고 다음 한 줄을 완료하도록 돕는다.

`요청 입력 → 웹 AI로 바로 준비 → 파일 드래그 + 프롬프트 붙여넣기 → 결과 드롭 → 검사 → 후보 검토`

앱은 로그인, cookie/session, 웹 DOM, 업로드 성공 여부, 생성 버튼, 결과 polling과
다운로드를 제어하지 않는다. Windows에서는 앱이 다시 검증한 관리형 파일을
운영체제 native drag-out으로 브라우저 업로드 영역까지 직접 끌 수 있다.
키보드·비Windows·호환성 문제에는 Explorer 선택을 사용하며, 어느 경로도
웹사이트의 업로드 성공을 뜻하지 않는다.

## 빠른 경로

1. 사용자는 공식 웹사이트와 `원하는 수정`만 선택한다.
2. `웹 AI로 바로 준비` 한 번으로 다음을 수행한다.
   - 현재 저장된 작업 종류를 판정한다.
   - 외부에 전달할 clean PNG와 내부 manifest를 만든다.
   - 구조 보호 기본 프롬프트와 사용자 요청을 합친다.
   - 최종 프롬프트를 복사한다.
   - 선택한 공식 웹사이트를 연다.
3. 정적 전달 도우미는 upload PNG 하나만 보여주고, GIF 전달 도우미는 각
   `frames_sheet_*.png` clean page를 개별 카드로 보여준다.
   - Windows 마우스 사용자는 `파일 끌기`로 웹 업로드 영역까지 직접 끈다.
   - 키보드·비Windows·호환성 문제에는 `탐색기에서 파일 선택`을 사용한다.
4. 사용자가 웹에서 저장한 로컬 PNG/JPG/JPEG/static WebP를 결과 drop zone에 놓는다.
5. 앱은 즉시 검사한다.
   - 정상 정적 단일 결과는 비활성 AI 후보로 보관하고 기존 후보 검토로 이동한다.
   - 현재 아이콘과 원본은 자동으로 바꾸지 않는다.
   - 구조 오류는 후보를 만들지 않고 수정 프롬프트를 제공한다.

내부 manifest, request ID, 관리 경로와 package 구조는 일반 UI와 파일 선택기에
노출하지 않는다. 단, GIF export session을 더 이상 자동 복원할 수 없는 수동 복구
흐름에서는 사용자가 보관한 `frames_manifest.json`을 명시적으로 고를 수 있다.

## 기본 프롬프트 종류

기본 구조 프롬프트는 정확히 두 종류다.

- `static_icon_sheet`
  - `single | grid`
  - `edit | generate`
  - 정적 단일과 정적 아이콘 grid의 개수, 행 우선 순서, 셀 경계와 투명 배경을 보호한다.
- `gif_temporal_sprite`
  - frame count, 열/행, 캔버스와 셀 크기, 행 우선 순서, 프레임 추가·삭제·복제 금지,
    인접 프레임의 시각적 연속성을 설명한다.

정적 single edit과 F148–F149의 선택 아이콘 2–16개 grid edit, 원본 없는 single/grid
생성은 모두 수동 웹 전달로 연결되어 있다. GIF의 의미상 프레임 순서·정체성·깜빡임은
픽셀 구조 검사만으로 확정할 수 없으므로 provider 자동 생성 범위가 아니다. 대신
`pmtcon-gif-frame-sheet-v2` 로컬 왕복은 manifest 파일명, frame delay와 loop metadata를
정확히 복원하며 사람이 최종 동작을 검토한다.

### GIF 파일 역할과 manifest 재사용

- 웹에 올림: `frames_sheet_001.png`, `frames_sheet_002.png` 등 clean page만 해당한다.
- 웹에 올리지 않음: `frames_guide_*.png`는 셀 번호·시간을 보는 사람용 참고 파일이다.
- 앱 전용: `frames_manifest.json`은 page slot, frame timing/order/loop 복원 자료다.
- 같은 export dialog/session에서는 앱이 manifest를 유지하므로 사용자는 결과 page만
  놓는다. 앱 재시작, 별도 교체 진입 또는 관리 기록을 검증할 수 없을 때만 수동
  manifest picker/drop을 recovery fallback으로 연다.
- 여러 page는 한 장씩 같은 prompt와 provider settings로 처리한다. 각 clean page에는
  검증된 native `파일 끌기`와 Explorer fallback을 제공하고 guide/manifest에는 웹 전달
  drag를 제공하지 않는다.
- PNG는 권장 형식이다. JPG/JPEG/static WebP 또는 완전 불투명 PNG는 exact canvas
  검사 뒤 `배경 포함으로 계속`을 고른 경우에만 내부 PNG 중간 결과가 된다. animated
  WebP는 지원하지 않는 애니메이션 결과로 알린다.
- painted checker는 실제 alpha가 아니며 경고한다. 앱은 checker를 자동 제거하지 않는다.
  구조 보존 prompt를 사용해도 provider의 원본 그림체·캐릭터 비율·frame 간 일관성은
  보장되지 않으므로 재조립 전후 animation을 사람이 검토한다.

## 오류와 수정 프롬프트

각 진단은 다음 필드를 가진다.

- `code`
- `severity`: `blocking | warning | manual_review`
- `문제`
- `영향`
- `expected / actual`
- `해결`
- `suggestedPrompt | null`

프롬프트 수정으로 해결할 수 있는 구조 문제에만 결정적인 문장을 제공한다.

- 캔버스 불일치: 정확한 폭·높이 유지
- 정적 grid 수/경계 불일치: 셀 추가·삭제·병합·재배열 금지
- GIF frame 수/geometry 불일치: frame 추가·삭제·복제 금지
- 투명 필수 요청의 alpha 손실: 투명 배경 PNG와 alpha 유지
- 투명 권장 요청의 불투명 결과: 자동 수정하지 않고 배경 포함 여부를 사용자가 선택

다음 오류에는 프롬프트를 제안하지 않는다.

- 손상되거나 다운로드가 끝나지 않은 파일
- 로그인·인증
- 구독·이용 등급
- quota/rate limit
- 네트워크
- 만료되거나 현재 소스와 달라진 handoff

여러 수정 문장은 안정적인 code 순서로 중복 제거한 뒤 `[구조 수정 요청]` 아래에
추가한다. 원래 사용자 요청을 덮어쓰지 않으며 자동 재시도하지 않는다.

## 보존과 정리

- 단일 package 위치: app data의 `ai/handoffs/<request-id>`
- 그리드 input/output: app-managed source와 `ai_grid_payload_retention` 장부로 추적
- 사용자 전달 파일: `upload.png`
- 내부 파일: `manifest.json`, `prompt.txt`
- 기본 보존: 7일, 명시적 연장: 한 번만 30일
- 진행 중 최신 세션은 AI 패널을 다시 열거나 앱을 재시작한 뒤 복원한다.
- 성공 commit, 사용자 닫기 또는 만료 cleanup은 전달 payload만 삭제하고 request,
  candidate, version, 원본과 롤백 이력은 보존한다.
- cleanup은 앱 시작·직접 접근·새 전달 준비뿐 아니라 앱을 켜 둔 동안 15분마다 실행한다.
  잠긴 파일은 `cleanup_pending`으로 남겨 다음 주기에 다시 시도한다.
- 단일 package와 grid input/output를 합친 관리형 전달 payload 한도는 256MiB다.
  준비 직전 cleanup 후에도 새 artifact가 들어가지 않으면 정확한 용량 오류로
  중단하며, 공간 확보를 위해 활성 payload를 자동 삭제하지 않는다.
- 사이드바 `최근 AI 전달`은 최근 30건(backend 최대 100), 사용량, 만료/닫힘/정리 대기,
  결과 수신 여부와 안전한 파일 끌기·Explorer·닫기·지금 정리 동작을 보여준다.
- DB에는 raw prompt와 임의 절대 경로 대신 hash, 고정 파일명과 구조 metadata만 둔다.
- 파일 드래그 IPC는 request ID만 받고 live managed artifact를 다시 resolve한 뒤
  path·size·SHA·dimension을 검증한다. 임의 경로, symlink/reparse/traversal은 거부한다.

## 완료 경계와 후속

완료:

- 정적 single 및 2–16개 static-single grid의 request-linked manual web package
- 원본 없는 single/grid 생성 프롬프트와 결과 grid 검토/atomic icon 생성
- 원클릭 prompt 복사와 allowlist 공식 사이트 열기
- Windows 검증 파일 native drag와 안정적인 Explorer 선택 fallback
- 결과 drop/picker, typed local validation, 구조 오류 전용 수정 프롬프트
- 같은 request의 비활성 candidate, 원본/활성 source 무변경
- 7일/1회 30일 retention, 15분 주기 cleanup, 256MiB quota, 최근 전달 이력

완료(2026-08-02):

- GIF clean page별 drag/Explorer와 guide/manifest 역할 표시
- same-session manifest 자동 재사용 및 수동 recovery fallback
- JPG/JPEG/static WebP opaque intermediate와 painted-checker 검토

후속 또는 의도적으로 제외:

- provider가 생성한 animated GIF의 AI candidate/version 계보와 시간적 일관성 검토
- 가로/세로 이중콘 grid와 여러 페이지
- 웹사이트 DOM·로그인·cookie·생성 결과 자동 관찰 또는 자동 다운로드

마지막 항목은 확장 프로그램이나 provider DOM 자동화 없이 구현하지 않는다. 인증,
quota, 형식, 네트워크 오류는 사용자가 붙여넣은 문구만 로컬에서 분류하며 자동
재시도하거나 의미 없는 프롬프트를 제안하지 않는다.

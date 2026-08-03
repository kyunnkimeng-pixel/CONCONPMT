# AI_GRID_WORKFLOW_DESIGN.md — 컬렉션 단위 AI 편집·생성

기준일: 2026-07-28
상태: 구현 완료 (0.3.0-alpha.3)

## 1. 결론

사용자가 지적한 두 기능은 현재 구현되어 있지 않다.

1. 선택한 여러 아이콘을 한 장의 grid로 묶어 한 번에 AI 편집
2. 원본 이미지 없이 한 개 또는 여러 개의 새 아이콘 생성

현재 provider 경로는 `icon_id`가 필수인 정적 이미지 한 장 편집이며, 결과도
후보 한 장만 저장한다. 기존 `새 아이콘으로 추가`도 원본 아이콘을 복제한 뒤 AI
후보를 연결하는 흐름이므로 순수 text-to-image의 원본 역할을 대신할 수 없다.

두 기능은 단일 아이콘 편집기 안에 억지로 넣지 않고, 모음 화면에서 여는
`AI 일괄 작업` 작업공간으로 구현한다. 기존 단일 아이콘 `AI로 수정`은 빠른
개별 편집 흐름으로 유지한다.

## 2. 제품 범위

### 2.1 첫 구현 범위

- 정적 단일콘 JPG/PNG
- 선택한 2~16개 아이콘의 provider 입력용 clean grid 조립
- 원본 없는 새 아이콘 1개 생성
- 원본 없는 새 아이콘 2~16개 grid 생성
- 한 사용자 동작당 HTTP 요청 한 번, 자동 재시도와 provider fallback 없음
- 결과 전체 시트와 각 cell을 검토한 뒤에만 비활성 후보 또는 새 아이콘으로 저장
- 원본, 기존 AI version과 현재 활성 source 보존
- 앱 재시작 뒤에도 요청, 시트, cell mapping, 후보와 적용 이력 복구

provider별 검증 전송 제한은 더 작을 수 있다. 현재 Gemini 1K 계약에서는 품질을
위해 기본 최대 9개를 사용하고, 10~16개는 검증된 2K 이상 출력 계약이 있을 때만
활성화한다.

### 2.2 첫 구현에서 제외

- GIF 전체 frame 일괄 AI
- 가로/세로 이중콘을 piece로 분리한 뒤 다시 합치는 AI 작업
- 서로 다른 크기·비율을 한 grid에 섞는 작업
- 결과가 오자마자 현재 아이콘을 자동 교체하는 동작
- background queue, 예약 실행, 자동 반복 생성
- AI가 만든 불투명 배경을 투명 배경이라고 간주하는 동작
- 정확한 공급자 과금액을 알 수 없는데 비용 절감을 보장하는 문구

GIF와 다중콘은 정적 grid의 계보·비용·정합성 검증이 끝난 뒤 별도 Stage Gate로
진행한다.

## 3. 진입점과 화면 흐름

### 3.1 진입점

- 모음 도구막대: `AI 만들기`
  - 원본 없는 한 개/여러 개 생성
- 아이콘 다중 선택 우클릭 메뉴: `선택 N개 AI로 수정`
  - 현재 `IconGrid`가 보유한 Ctrl/Shift 선택 순서와 collection 표시 순서를 사용
- 단일 아이콘 편집기: 기존 `AI로 수정` 유지

첫 구현에서는 선택 상태가 `IconGrid` 내부에 있으므로 다중 편집은 우클릭 메뉴
진입을 사용한다. 추후 도구막대에서 선택 N개를 표시하려면 selection을 route로
올리는 별도 계약을 추가한다.

### 3.2 공통 작업공간

`AiGridWorkspaceDialog`는 다음 다섯 단계로 구성한다.

1. **작업 선택**
   - `선택 아이콘 수정`
   - `새 아이콘 만들기`
2. **대상과 배치**
   - 대상 수, 행·열, cell 크기, 읽기 순서
   - 실제 provider에 보낼 clean grid와 번호 overlay가 있는 로컬 guide 미리보기
3. **공급자와 프롬프트**
   - 공통 프롬프트
   - cell별 의도 목록
   - provider/model, 외부 전송 내용, 요청 횟수와 비용 확인
4. **결과 grid 검토**
   - 전체 결과 시트
   - 예상 grid overlay
   - cell별 포함/제외, 이름, alt, 대상 mapping
   - 필요한 경우 offset/cell/gap을 직접 조정
5. **저장**
   - 편집: `선택한 결과를 비활성 후보로 추가`
   - 생성: `새 아이콘 N개 만들기`

고정 안내 문구:

- `요청 1회 · 결과 시트 1장 · 최대 N칸`
- `AI가 grid 경계를 정확히 지킨다고 보장할 수 없습니다. 저장 전에 각 칸을 확인해 주세요.`
- `결과를 저장해도 원본과 현재 적용 이미지는 바뀌지 않습니다.`
- `요청 횟수가 줄어도 공급자 과금이 같은 비율로 줄어든다는 뜻은 아닙니다.`

### 3.3 여러 아이콘 편집

1. 사용자가 정적 단일콘 2개 이상을 선택한다.
2. 앱이 collection 표시 순서대로 ordered target snapshot을 고정한다.
3. crop, transform, text, effect, motion의 0ms poster와 현재 effective source를
   사용해 각 cell을 렌더한다.
4. 한 장의 clean PNG와 로컬 manifest를 만든다.
5. 사용자가 전송 시트와 prompt를 확인하고 한 번 명시적으로 요청한다.
6. 결과 시트를 검토하고 각 cell의 원래 target mapping을 확인한다.
7. 승인된 cell을 각 target icon의 **비활성 AI 후보**로 저장한다.
8. 후보 작업공간에서 개별 비교 후 현재 아이콘 적용 또는 새 아이콘 생성이 가능하다.

선택이 비어 있으면 전체 모음으로 fallback하지 않고 요청을 차단한다. 기존 정적
시트 exporter의 empty-selection fallback을 과금 작업에 재사용하지 않는다.

### 3.4 원본 없는 생성

#### 한 개 생성

- grid를 만들지 않는다.
- collection 기본 cell 크기와 정사각 출력 canvas를 사용한다.
- provider 결과를 origin icon이 없는 비활성 candidate로 저장한다.
- 검토 후 사용자가 `새 아이콘 만들기`를 눌렀을 때 처음으로 icon root를 만든다.

#### 여러 개 생성

- 사용자가 생성 수와 공통 스타일을 정하고, 각 칸의 감정·동작 문구를 입력한다.
- 기본 배치는 가까운 정사각형 row-major grid다.
  - 2개: 1×2
  - 3~4개: 2×2
  - 5~9개: 3×3
  - 10~16개: 4×4
- provider에는 한 장의 결과 시트를 요청한다.
- 결과 cell을 검토하고 제외·순서·이름·alt를 정한 뒤 한 transaction으로 새
  아이콘을 만든다.
- 요청 전에 투명 placeholder icon을 만들지 않는다.

## 4. Grid 계약

### 4.1 Provider 입력

- schema: `pmtcon-ai-grid-v1`
- 정확히 한 페이지
- row-major
- 최대 4×4, 최대 16 cell
- 동일한 정사각 cell
- packed/trim/rotate 금지
- provider 입력 clean sheet에는 번호, 글자, grid 선을 굽지 않는다.
- 번호와 mapping은 사용자가 보는 local guide overlay와 manifest에만 둔다.
- 입력 PNG bytes, SHA-256, canvas 크기, rows/columns, border/gap, 각 cell 좌표와
  target snapshot을 불변으로 저장한다.

provider canvas가 고정 1024×1024이면 cell과 gap/border가 정수 좌표로 정확히
맞도록 계산한다. 예를 들어 4×4는 244px cell과 16px gap으로 1024px를 정확히
사용할 수 있다. 임의 반올림 좌표는 사용하지 않는다.

### 4.2 Provider 출력

네트워크 응답 단계에서 다음을 먼저 검사한다.

- 허용된 MIME과 확장자
- encoded byte, 한 변, 총 pixel 제한
- 예상한 이미지 결과 수
- decode 성공
- provider별 출력 canvas 계약
- request가 아직 취소되지 않았는지

유효한 이미지지만 grid가 예상 위치를 따르지 않으면 결과를 자동 분할하거나
적용하지 않는다. 전체 시트를 `layout review pending` artifact로 보존하고 기존
grid overlay/직접 Slice UI를 재사용해 사용자가 배치를 확인하도록 한다.

cell candidate 생성은 다음 조건을 모두 만족할 때 한 번에 수행한다.

- 모든 선택 cell이 bounds 안에 있음
- mapping ordinal이 중복되지 않음
- 선택된 cell이 decode 가능한 비어 있지 않은 PNG로 추출됨
- 편집 모드의 모든 target snapshot이 여전히 current
- request와 item 상태가 예상 revision과 일치

하나라도 실패하면 후보를 일부만 만들지 않는다. raw output sheet는 실패 사유와
함께 검토 artifact로 남길 수 있지만 현재 아이콘과 candidate/version은 0개
변경한다.

## 5. 공급자별 현실성

### 5.1 Gemini

- 공식 image generation surface는 text-only 생성과 image+text 편집을 지원한다.
- 여러 reference image 입력도 가능하지만 첫 grid 구현은 target N개를 하나의
  manifest-backed clean sheet로 합쳐 입력 한 장으로 전송한다.
- 공급자가 프롬프트에서 요구한 이미지 개수나 layout을 항상 지킨다고 보장할 수
  없으므로, N개의 독립 출력을 요구하지 않고 결과 시트 한 장을 받은 뒤 로컬에서
  검토·분할한다.
- 현재 앱의 Gemini 계약은 1K JPEG 한 장이다. JPEG 결과는 alpha가 없으므로
  투명 배경 유지 기능으로 설명하지 않는다.
- 1K에서 4×4는 cell당 세부 묘사가 부족할 수 있어 기본 live gate는 3×3 이하로
  둔다. 2K/4K는 exact model/가격/응답 계약을 별도 검증한 뒤 연다.

### 5.2 NovelAI

- 공식 `/ai/generate-image` schema는 prompt, model, parameters와 선택적
  `parameters.image`, `n_samples`를 제공하므로 image를 생략한 text-to-image
  요청 형태를 표현할 수 있다.
- 공식 OpenAPI는 `action`과 `model` 값을 enum으로 정의하지 않는다. 현재처럼
  versioned experimental contract string과 요청별 확인을 유지한다.
- 다중 편집은 입력 PNG 한 장에 grid를 합성하고 `n_samples=1`로 요청한다.
- 여러 새 아이콘도 첫 grid 단계에서는 결과 시트 한 장을 요청한다.
  `n_samples=N`은 N개의 무료 결과를 뜻하지 않으며 grid mapping과 다른 과금
  의미를 가지므로 별도 기능으로 섞지 않는다.
- PNG 응답이라도 실제 투명 배경과 cell 경계 보존은 결과 검토 전까지 신뢰하지 않는다.

### 5.3 웹 수동 handoff

웹사이트 자동 로그인, DOM 제어, 업로드, 다운로드 감시는 계속 하지 않는다.
request-linked handoff package가 구현되기 전에는 사용자가 clean sheet를 직접
저장·업로드·다운로드하고, 결과 시트를 grid 작업공간으로 다시 가져오는 수동
흐름만 제공한다.

### 5.4 비용 표시

한 시트로 묶으면 HTTP 호출 수와 결과 이미지 수를 줄일 가능성은 있지만,
공급자는 해상도, output token, sample 또는 별도 unit으로 과금할 수 있다.
따라서 UI는 다음만 표시한다.

- 실제 요청 횟수
- 요청한 결과 이미지 수와 해상도
- provider가 응답한 usage가 있으면 그 값
- 기준일이 있는 로컬 예상치는 `예상`으로 분리

`API N배 절약`과 같은 보장 문구는 사용하지 않는다.

## 6. 데이터 모델

기존 단일 아이콘 request/candidate/version/state를 유지하면서 migration에서
다음 계약을 추가한다.

### 6.1 `ai_requests`

- `request_scope`
  - `icon_edit`
  - `grid_edit`
  - `single_generate`
  - `grid_generate`
- `retry_of_request_id` nullable self-FK. 재시도는 원 요청을 수정하지 않고 새
  request로만 기록한다.
- origin icon 전용 lineage/source/revision snapshot 컬럼은 scope에 따라 nullable
  하도록 table rebuild와 CHECK를 사용한다.
- 무원본 생성에 빈 문자열, 가짜 icon ID 또는 placeholder source를 넣지 않는다.

### 6.2 `ai_request_items`

- `id`
- `request_id`
- `item_index`
- `origin_icon_id` nullable
- `origin_icon_id_snapshot` nullable; soft-delete 뒤에도 당시 대상을 감사할 수 있는 불변 ID
- `target_name_snapshot`
- `shape`
- `cell_width`, `cell_height`
- `original_lineage_id`, generation nullable
- `original_source_sha256`, `effective_source_sha256` nullable
- `activation_revision`, `native_recipe_signature` nullable
- `input_render_sha256` nullable
- `output_candidate_id` nullable
- `review_status`
- `UNIQUE(request_id, item_index)`

`grid_edit` item은 모든 target snapshot이 필수이고, generate item은 origin 관련
필드가 모두 NULL이어야 한다.

request/item의 scope, request ID, item index, cell 좌표와 원본 snapshot은 INSERT
뒤 수정할 수 없도록 trigger로 보호한다.

### 6.3 `ai_request_artifacts`

- `request_id`
- `role`: `input_sheet | output_sheet`
- `source_file_id`
- `sha256`
- `manifest_json`
- `UNIQUE(request_id, role)`

input/output sheet와 cell 후보의 source file을 library cleanup reference 계산에
포함한다.

artifact의 request, role, source와 manifest도 INSERT 뒤 수정하지 않는다.

### 6.4 `ai_candidates`

- nullable `request_item_id` FK 추가
- trigger로 candidate와 item의 `request_id`가 같음을 강제
- grid 결과의 candidate index는 item index와 안정적으로 연결
- 기존 candidate table이 immutable이므로 `request_item_id`는 candidate INSERT
  시점에 함께 기록

### 6.5 새 아이콘 provenance

- source-free candidate cell 자체를 새 icon의 immutable original source로 사용
- `ai_icon_root_creations.source_icon_id = NULL`
- `candidate_id`와 생성 request/item을 기록
- collection default size, 새 stable ID와 새 lineage를 부여
- 여러 아이콘 생성은 order/cover/pieces/source/provenance를 한 transaction으로 생성
- source-free candidate는 현재 아이콘 적용 대상이 없으므로 `새 아이콘 만들기`만 허용

기존 candidate normalization, stale 판정과 후보 목록 쿼리는
`candidate.request_item_id → ai_request_items` snapshot을 우선 사용하고 legacy
단일 후보만 `ai_requests.origin_icon_id` snapshot으로 fallback해야 한다.
source-free로 만든 icon의 후보/이력은
`ai_icon_root_creations.icon_id` ownership 경로에서도 조회되어야 한다.

## 7. 요청 수명주기와 롤백

1. **draft**: 대상과 grid 설정 편집, HTTP 0
2. **prepared**: 모든 target snapshot과 input sheet hash를 한 IMMEDIATE
   transaction에서 기록
3. **awaiting_result**: dispatch 직전 atomic claim
4. **layout_review_pending**: 유효한 결과 sheet를 받았으나 cell mapping 미확정
5. **completed**: 모든 cell candidate를 불변으로 생성
6. **failed/cancelled**: 후보·현재 source 변경 0

규칙:

- pre-dispatch 취소는 HTTP 0
- post-dispatch 취소는 응답이 와도 candidate로 저장하지 않음
- 자동 재시도 금지
- `다시 시도`는 새 request ID와 새 snapshot을 만드는 명시적 사용자 동작
- 편집 target은 결과 cell 확정 전에 모두 CAS 재검증
- stale target이 하나라도 있으면 일괄 후보 생성 0
- 모든 후보는 비활성으로 시작
- `모두 적용`은 대상 revision 전체를 먼저 확인한 뒤 all-or-none transaction
- 원본/이전 version 복귀는 기존 provider-free rollback 경로 재사용
- 앱 재시작 시 dispatch 중이던 작업을 자동 재전송하지 않음

## 8. 기존 코드 재사용과 필요한 분리

### 그대로 재사용

- Ctrl/Shift selection과 collection order
- `SheetGridSettings`, preset, overlay, cell review UI
- `sheet/grid.rs`의 grid 계산, bounds, page 계획
- 정적 sheet exporter의 effective-source 및 native recipe 렌더 순서
- manifest의 source/render recipe provenance
- reimport의 manifest 좌표 crop
- PNG cell batch icon 생성의 transaction/ordering/cover 골격
- AI candidate immutability, normalization, version/state CAS와 rollback
- 기존 transport의 exact origin, redirect 금지, timeout, response bound와 단일
  `send()` 계약

### 공용 helper로 분리

- `sheet::composer`
  - 선택 icon을 clean PNG bytes와 immutable item map으로 메모리 조립
  - 디스크 출력과 UI guide 생성에 종속되지 않음
- `sheet::splitter`
  - 검증된 manifest/수동 grid로 output bytes를 PNG cell로 분할
- `ai::batch_commit`
  - cell candidates all-or-none 저장
  - source-free icon roots all-or-none 생성

기존 exporter의 private `load_icons`, `render_icon_items`, `render_sheet_page`를
복사하지 않고 위 helper로 추출해 정적 작업 시트와 AI grid가 동일 렌더 계약을
사용하게 한다.

## 9. 실패·혼동 방지 UX

- GIF, 이중콘, 17개 이상 선택 시 메뉴는 실행하지 않고 정확한 비활성 사유 표시
- grid가 2페이지가 되면 요청을 보내지 않고 cell/열/대상 수 조정을 안내
- AI가 cell을 합치거나 순서를 바꾸면 자동 mapping하지 않음
- 결과 cell마다 `원본 #N → 결과 #N`과 대상 아이콘 이름을 함께 표시
- 불투명 JPEG 결과에는 `투명 배경 아님` 표시
- 빈 alt는 저장 가능하되 `작업중`과 export 경고 표시
- provider 오류, 401/429/timeout/schema drift에서 자동 재시도하지 않음
- API key/PAT는 기존 session-only, non-echo, non-persistence 계약 유지
- 전체 요청의 usage를 N개 후보 각각에 중복 합산하지 않음

## 10. 구현 Stage Gate

### GRID-1 — Provider-free foundation

Status: complete on 2026-07-29. Provider/UI/network 연결 없이 persistence, deterministic
composition/splitting, reviewed inactive candidates, recovery와 provenance 계약을 검증했다.
Source-free atomic new-icon commit과 사용자 진입점은 GRID-2 범위다.

- migration과 repository
- in-memory one-page composer/splitter
- 2~16 static single target snapshot
- source-free item 계약
- exact hash, stale, cleanup, all-or-none tests
- 네트워크와 live 메뉴 없음

### GRID-2 — Collection workspace + local/manual-web result

Status: complete on 2026-07-29.

- 모음 도구막대 `AI 아이콘 만들기`와 다중 선택 `선택 N개 AI로 수정`
- restart-safe five-step dialog, target/layout/prompt/web/result/review/save 흐름
- 선택 2–16개 static square single input composer와 request-ID-only native drag/Explorer
- 원본 없는 1–16개 생성 항목, 결과 PNG/JPG drop/picker와 overlay/manual mapping
- grid edit all-or-none inactive candidates, source-free atomic new-icon roots/order/cover
- exact disabled/error reasons, 구조 문제 전용 correction prompt, 자동 retry/network 0

GRID-2의 공식 웹 열기는 사람이 파일·프롬프트를 전달하는 수동 흐름이다. provider API
실행, 로그인/DOM/cookie 제어와 자동 결과 다운로드는 GRID-2 완료 주장에 포함하지 않는다.

### GRID-3 — Provider adapters

- Gemini text-to-image single/grid and selected-grid contract
- NovelAI text-to-image single/grid and selected-grid contract
- provider별 count/resolution gate
- exact consent/cost/payload confirmation
- one click/one HTTP/no retry
- mock transport tests; 일반 test는 실제 provider를 호출하지 않음

### GRID-4 — Explicit paid pilot and packaging

- 사용자 제공 session credential
- 공급자별 한 번의 소액 live pilot
- actual result layout/quality/usage 측정
- 원본 보존·rollback·cleanup 재검증
- lint/test/build/Rust/license guard/browser QA/NSIS package

각 Gate는 별도 `STAGE_GATE_RESULT`로 닫는다. GRID-1과 GRID-2는 통과했다.
GRID-3 mock과 별도 사용자 동의가 끝나기 전 live provider 요청은 열지 않는다.

## 11. 수용 테스트

- 선택 순서와 manifest item 순서가 항상 같음
- 빈 선택이 전체 collection으로 바뀌지 않음
- double/GIF/oversized/multi-page 입력이 HTTP 전에 거부됨
- 같은 snapshot과 설정이 byte-identical input sheet/hash를 만듦
- output size/grid/cell mismatch에서 candidate/icon 변경 0
- target 하나가 stale이면 batch candidate 변경 0
- 결과 후보 생성 뒤 현재 icon source 변경 0
- source-free 생성 전에 placeholder icon 0
- 새 아이콘 N개 생성의 source/provenance/order/cover가 atomic
- request usage가 후보 수만큼 중복 집계되지 않음
- 앱 재시작 후 layout review와 candidate 이력 복구
- cancel/retry/recovery가 자동 HTTP를 만들지 않음
- 모든 기존 원본과 AI version으로 provider 없이 복귀 가능

## 12. 공식 참고 자료

- [Gemini image generation](https://ai.google.dev/gemini-api/docs/image-generation)
- [Gemini Interactions API](https://ai.google.dev/api/interactions-api?hl=en)
- [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing)
- [NovelAI Image API OpenAPI](https://image.novelai.net/docs/doc.json)
- [NovelAI Image API Swagger](https://image.novelai.net/docs/index.html)

공급자 model, schema, 가격과 약관은 release와 live pilot 직전에 다시 확인한다.

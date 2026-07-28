# PMTCONCON Studio AI 통합 설계

검토 기준일: 2026-07-26

## 1. 결론

PMTCONCON Studio의 AI 지원은 한 공급자에 종속된 즉시 적용 기능이 아니라,
공급자 중립적인 `요청 → 후보 → 검토 → 활성화 → 복귀` 흐름으로 구현한다.

권장 순서는 다음과 같다.

1. API가 없는 비파괴 AI 이력·활성 소스 기반을 먼저 만든다.
2. 첫 자동 adapter는 NovelAI Image API로 구현한다. PMTCONCON Studio가 계정이나
   중계 서버를 운영하지 않고, 각 사용자가 공식 사이트에서 발급한 Persistent API
   Token을 쓰는 BYOT 방식이다. 공식 지원 확인과 소액 live pilot을 통과하기 전에는
   `조건부 주 provider`로 표시한다.
3. 웹 AI 수동 handoff는 token 없이 쓸 수 있는 공급자 중립 fallback으로 남긴다.
4. Gemini API와 OpenAI Image API는 각각 배포 적합성·비용 검토를 통과한 경우에만,
   사용자 실행 literal loopback endpoint는 별도 보안 검토 뒤 같은 provider
   contract의 후속 adapter 후보로 다룬다.
5. GIF 프레임별 AI와 sprite-sheet 일괄 생성은 비용·일관성 검증이 끝난 뒤
   실험 기능으로 다룬다.
후보 가져오기·비교·규격화·적용·복귀의 화면 구조와 deterministic pixel 계약은
`docs/AI_WORKSPACE_UX_DESIGN.md`를 따른다. 편집 패널에 공급자·후보·이력을 계속
누적하지 않고 큰 앱 내부 작업공간으로 분리하며, provider adapter는 같은 후보
검토와 rollback 흐름에 결과만 추가한다.


AI 결과는 언제나 불변 후보이다. 생성 성공만으로 현재 아이콘을 바꾸지 않으며,
적용과 복귀는 저장된 파일과 DB 포인터를 전환해 수행한다. 같은 프롬프트를 다시
호출해 이전 상태를 재현하는 방식은 롤백으로 인정하지 않는다.

## 2. 제품 원칙

- `icons.source_file_id`와 그 원본 바이트는 AI 작업으로 변경하거나 삭제하지 않는다.
- AI 결과도 SHA-256으로 식별되는 불변 `source_files`로 등록한다.
- 현재 사용 중인 AI 결과는 별도 active pointer로 선택한다. 포인터가 `NULL`이면
  원본 소스를 사용한다.
- 후보 생성, 적용, 원본 복귀, 이전 AI 버전 선택은 앱 재시작 후에도 유지된다.
- 프로젝트 library가 존재하고 사용자가 해당 AI 이력을 명시적으로 영구 삭제하지 않은
  한, 원본과 저장된 모든 AI version으로 provider 호출 없이 언제든 복귀할 수 있다.
  일반 cache/optimizer/library cleanup은 이 바이트를 삭제하지 않는다. 영구 삭제는
  잃게 되는 rollback 지점과 clone/descendant 참조를 보여주는 별도 확인을 요구한다.
- 생성 중 아이콘 편집 상태가 달라지면 후보는 보존하되 자동 적용하지 않는다.
- 기본 적용 동작은 `새 아이콘으로 추가`이다. 현재 아이콘의 베이스 소스 전환은
  호환 가능한 결과에서만 명시적으로 허용한다.
- 이름, alt, 메모, 정렬 순서와 collection 설정은 AI 버전 전환 대상이 아니다.
- API key, access token, cookie는 SQLite, 설정 JSON, 로그, 오류 보고서, AI 요청
  이력에 기록하지 않는다.
- PMTCONCON Studio 자체는 유료 AI 서비스, 공용 공급자 계정, 공용 key/token,
  과금 중계 proxy를 운영하지 않는다. 공급자 구독·크레딧·과금과 약관 준수는
  각 사용자의 계정에 귀속된다.
- 실제 동작이 구현되기 전에는 AI 탭·메뉴·버튼을 노출하지 않는다.

## 3. 현재 구조와 분리해야 하는 이유

현재 원본은 `source_files`, 아이콘의 선택 원본은 `icons.source_file_id`, 렌더
캐시는 `current_preview_path`와 piece preview path에 저장된다. 크롭, 변형,
텍스트, 정적 효과와 motion은 별도 durable metadata이다.

다음 기존 구조는 AI 이력으로 재사용하지 않는다.

- `replace_icon_source`는 `icons.source_file_id`를 바꾸고 crop/transform을
  초기화하므로 AI 적용·롤백 명령으로 쓰지 않는다.
- `current_preview_path`는 다시 만들 수 있는 캐시이므로 버전 원본이 아니다.
- frontend의 저장값 복원은 메모리 draft 복원이며 재시작 후 AI 롤백이 아니다.
- `processed_asset_variants`는 export profile/piece별 최적화 결과이다. AI 원본
  계보, 부모 버전, 실패 요청과 활성 revision을 표현하지 못하므로 분리한다.

단, 기존 `source_hash` stale 판정은 effective AI source의 SHA-256을 입력으로
사용해 그대로 활용한다.

## 4. 영속 데이터 모델

첫 구현은 네 가지 역할을 분리한다.

또한 `icons`에 non-null `original_lineage_id`와 증가만 하는
`original_lineage_generation`을 추가한다. 기존 icon은 migration에서 서로 다른
stable ID와 generation 0을 받고, 새 import와 새 root icon도 새 ID/generation 0을
받는다. 사용자가 일반 `이미지 교체`를 할 때는 바이트/SHA가 이전과 같더라도
lineage ID를 반드시 새로 발급하고 generation을 증가시킨다. 따라서 `A → B → A`
재교체가 과거 A 계보를 우연히 되살리지 않는다.

### `ai_requests`

한 번의 생성 또는 편집 실행을 나타낸다.
request는 status, 만료, supersede와 metadata scrub이 갱신되는 실행 기록이다.
불변인 것은 candidate, source bytes와 생성 당시 snapshot이며 request row 전체가 아니다.

- stable ID, nullable origin collection/input icon ID와 생성 당시 ID/name snapshot
- provider mode: `manual_web | api | local_endpoint`
- service surface: `novelai_api | novelai_web | openai_api | chatgpt_web |
  gemini_api | gemini_web | loopback_endpoint | other_manual`
- `adapter_id`, `adapter_contract_version`와 provider-qualified/versioned canonical JSON
  schema로 만든 요청·협상 capability, data-tier, retention, consent snapshot. 각
  snapshot은 allowlist field만 허용하고 64 KiB 이하이며 해당 lifecycle point에서
  확정된 뒤 불변이다. 미지원 operation을 다른 operation이나 provider로 조용히
  바꾸지 않는다.
- secret 자체가 아닌 `credential_mode_snapshot`: `none | session | environment |
  os_vault_ref`. AI-1과 첫 cloud adapter는 이 snapshot만 저장하고 `os_vault_ref`를
  거부한다. 이 단계에는 credential binding table/column/FK 또는 vault locator가
  없으며 token도 저장하지 않는다.
- 웹 surface의 account context: `personal | business_workspace | work_school | unknown`
- provider, 사용자가 선언했거나 API가 확인한 model, operation
- `provenance_trust`: `api_verified | manual_declared | manual_unverified`
- typed `policy_refs[]`: `terms | privacy | data_controls | pricing |
  model_terms_or_license | attribution`, URL, 검토 기준일/version
- versioned allowlist-only prompt/options snapshot. prompt와 허용한 scalar/enum/numeric/
  짧은 string/array 값만 canonical JSON으로 합계 64 KiB까지 저장한다. binary/base64
  input·mask·reference, header, Authorization, token, cookie, secret filesystem reference,
  완전한 provider request/response는 구조적으로 거부하고 snapshot에 넣지 않는다.
- 실제 전송 input/mask/reference package의 관리 경로와 SHA-256
- 입력 `original_lineage_id`/`original_lineage_generation`, 원본/effective source SHA-256,
  `payload_input_signature`, 요청 당시 전체 `request_recipe_signature`와 시작 시
  activation revision
- 상태: `prepared | awaiting_result | running | completed | failed | cancelled | expired`
- 별도 `superseded_at/reason`; 실행 완료 여부와 입력 stale 여부를 섞지 않는다.
- provider request ID, `provider_usage`, pricing snapshot으로 계산한
  `estimated_provider_units`, nullable `estimated_cost`, nullable
  `provider_reported_cost`, 정제된 오류
- 생성/시작/완료 시각

provider 변경은 별도 request와 payload 확인·consent를 요구한다. 자동 fallback은 금지한다.

실패·취소·superseded 요청의 metadata는 원인 확인을 위해 남길 수 있지만 key와
중복 payload는 남기지 않는다. 진행 중 원본/recipe가 바뀌어도 provider 결과는
검증 후 비활성 후보로 보존하며 자동 적용하지 않는다.

`payload_input_signature`는 실제 외부 전송 바이트를 판정한다. `base_source`는
effective base와 provider preprocessing/mask/reference만 포함하고 crop, text,
effect, motion 같은 downstream recipe는 제외한다. `rendered_viewport`는 이미
합성된 viewport가 payload이므로 전체 native recipe를 포함한다. 별도의
`request_recipe_signature`는 요청 당시 전체 편집 상태를 provenance로 남긴다.
활성화 prepare에서 다시 계산하는 `activation_recipe_signature`가 최종 CAS의
기준이며 이 세 signature를 하나로 취급하지 않는다.
`activation_recipe_signature`는 매 activation operation의 transient prepare/CAS
snapshot이며 candidate/version identity가 아니다. 기존 version 재선택 때도 현재
recipe에서 다시 계산하며 새 version row를 만들지 않는다.

origin collection/icon ID는 provenance 탐색용 weak reference이며 ownership가 아니다.
`ON DELETE SET NULL`과 immutable snapshot을 사용해 원본 icon/collection의 영구
정리가 clone이 공유하는 request/candidate/version을 cascade 삭제하지 않게 한다.
candidate/version이 참조하는 request row는 최소 provenance가 남는 동안 삭제하지
않되, 사용자는 prompt/options/error 같은 선택 metadata를 별도로 scrub할 수 있다.

웹 account context가 `unknown`이면 consumer 기본 안내만 보여주고 실제 workspace의
정책을 사용자가 확인해야 한다고 명시한다. `other_manual`은 exact official URL과
typed policy reference allowlist가 별도 검토되지 않은 한 `official` 또는
`policy_verified`로 표시하지 않는다.

### `ai_candidates`

한 요청에서 얻은 하나 이상의 공급자 결과를 나타낸다. 아이콘 소유권이나 rollback
parent는 두지 않는다.

- stable ID, request ID, candidate index
- 안전하게 decode한 provider raw output의 `raw_source_file_id`와 SHA-256
- 형식, 크기, animated/alpha 여부와 provider capability snapshot
- 생성 시각

authoritative 후보 바이트는 요청별 폴더가 아니라 기존 content-addressed
`source_files` 저장 규약과 `originals/<sha>` 경로를 사용한다. 같은 바이트는 SHA
unique row를 재사용한다. `ai/inputs/`와 `ai/handoffs/`는 외부 전송을 위한 임시
request artifact일 뿐 source of truth가 아니다.

provider native 출력이 현재 아이콘 canvas와 다르면 raw source를 보존하고, 적용
단계에서 deterministic normalization recipe로 별도의 불변 `source_files` 결과를
만든다. raw source와 normalized source 어느 쪽도 덮어쓰지 않는다.

### `icon_ai_versions`

공급자 후보를 특정 아이콘과 원본 계보에 적용한 로컬 버전을 나타낸다.

- stable ID, icon ID, candidate ID
- `base_original_source_file_id`, `base_original_lineage_id`,
  `base_original_lineage_generation`과 같은 아이콘의 nullable `parent_version_id`
- 실제 renderer가 사용할 `effective_source_file_id`
- input stage: `base_source | rendered_viewport | gif_poster`
- apply kind: `active_source | new_icon_root`
- provider-native size, target canvas, fit/alignment/pad/resize filter/alpha 처리의
  versioned normalization recipe와 hash
- canvas, animation kind, `payload_input_signature`와 normalization compatibility
  metadata
- 생성 시각

`base_source` 버전만 호환 검사를 통과한 기존 아이콘의 active source가 될 수 있다.
`base_source` 후보의 기본 `새 아이콘으로 추가`는 source icon을 복제하되 새 icon/
piece/lineage/version ID를 발급하고 alt를 비우며 readiness를 `working`으로 둔다.
원본 source와 현재 crop/shape/cell/loop/transform/text/effect/motion 및 AI version
chain은 clone 규칙으로 복사하고 새 candidate version을 child로 활성화한다.
이 작업은 일반 clone을 commit한 뒤 두 번째 activation을 수행하지 않는다. target
DAG/state에 candidate child와 active pointer까지 먼저 포함한 다음 최종
`EffectiveVisualSource`로 variant/preview를 만들고, 새 icon 전체를 하나의
DB/file-compensation protocol로 commit한다. 실패하면 반쪽 icon을 남기지 않는다.
request/candidate/usage/cost row는 복사하지 않으므로 새 아이콘에서도 원본과 이전
AI version으로 독립 복귀할 수 있다.

`rendered_viewport`와 `gif_poster`는 첫 버전에서 `new_icon_root`만 허용한다. 이 경우
선택한 effective source를 새 아이콘의 `icons.source_file_id`로 삼고 full-canvas
crop, 새 piece/lineage ID, 빈 alt와 `작업중` 상태를 만든다. 이미 bake된 기존
crop/transform/text/effect/motion은 복사하지 않는다. 무입력 생성도 candidate를
원본으로 하는 collection 기본 cell의 single working icon으로 만든다.

### `icon_ai_state`

아이콘별 현재 AI 버전 선택만 나타낸다.

- `icon_id`
- nullable `active_version_id`
- 증가만 하는 activation `revision`
- `updated_at`

`active_version_id = NULL`은 `icons.source_file_id` 원본이다. active/parent version과
base original source 및 lineage가 같은 아이콘 계보에 속하도록 composite foreign
key와 commit 전 검사를 사용한다. 완료·검증된 candidate에서 만든 version만
활성화할 수 있다.

### 외래키, migration과 소스 metadata

- `ai_requests.origin_collection_id`와 `origin_icon_id`는 nullable
  `ON DELETE SET NULL`이고 생성 당시 snapshot은 남긴다.
- `ai_candidates.request_id`는 `ai_requests`를 `RESTRICT/NO ACTION`으로 참조하며
  `(request_id, candidate_index)`는 unique이다.
- `icon_ai_versions.icon_id`는 icon 삭제에 `CASCADE`, candidate와
  `base_original_source_file_id`/`effective_source_file_id`는 각각 candidate와
  `source_files`를 `RESTRICT/NO ACTION`으로 참조한다.
- version parent는 `(icon_id, base_original_lineage_id,
  base_original_lineage_generation, parent_version_id)`가 같은 네 필드의 parent
  candidate key를 참조하는 lineage-scoped composite FK여서 같은 icon의 다른 lineage도
  parent가 될 수 없다. state의 `(icon_id, active_version_id)`는
  `UNIQUE(icon_id, id)`를 참조하고 current lineage 일치는 activation CAS/DB guard로
  검사한다. self-parent/state 관계는 deferred constraint 또는 영구 정리 transaction의
  state/child-first 삭제 순서로 처리한다.
- `base_original_source_file_id`는 mutable `icons.source_file_id`와 FK로 묶지 않는다.
  `source_files`에 대한 FK와 activation의 lineage/source CAS가 계보 일치를 검증한다.
- migration은 모든 기존 icon에 서로 다른 `original_lineage_id`, generation 0과
  `icon_ai_state(active_version_id = NULL, revision = 0)`를 backfill한다. 이후 state
  row가 없는 icon은 원본 상태로 추측하지 않고 migration/data 오류로 처리한다.
- migration은 `icons.original_lineage_id`의 DB-generated stable default와 generation
  0 default를 확정하고, 같은 INSERT transaction에서 original-only `icon_ai_state`를
  만드는 DB trigger를 설치한다. 중앙 `insert_icon_with_visual_state` repository helper는
  import, placeholder, duplicate, static/GIF sheet commit과 collection clone이 이 기본
  경계를 통과하게 한다. clone처럼 명시적 lineage map이 필요한 경로만 helper의
  override를 사용한다. source-search gate와 DB invariant test는 trigger/helper를
  우회한 icon INSERT와 state 없는 icon을 검출한다.
- `source_files`에 nullable `has_alpha`를 추가한다. 새 import와 AI candidate는
  bounded safe decode 결과를 기록하고, 기존 `NULL` row는 실제 사용 전 같은
  decoder로 lazy backfill한다. `has_alpha`의 고정 `pmtcon-alpha-v1` 의미는 채널 존재가
  아니라 decoded/display-composited pixel 중 alpha가 255 미만인 픽셀이 하나라도
  있는지이다. animated source는 bounded workload 안에서 모든 표시 frame을 검사한다.
  JPEG 또는 모든 frame/pixel이 완전 불투명이면 false이다. 확장자로 추측하지 않으며
  activation에 필요한 metadata가 decode/backfill되지 않으면 적용을 거부한다.
  animation/frame/loop metadata도 `source_files`의 검증된 값을 권위값으로 쓴다.
- migration 등록과 동시에 library cleanup/reference query를 확장해 candidate,
  version, soft-deleted history가 참조하는 source를 보존한다.

## 5. effective visual source

모든 native 소비자는 직접 `icons.source_file_id`를 조인하는 대신 공통
`EffectiveVisualSource` repository/resolver를 사용해야 한다.

```text
active AI version이 있음 → icon_ai_versions.effective_source_file_id
active AI version이 없음 → icons.source_file_id
```

resolver는 original/effective source ID, active version/candidate ID, path, extension,
SHA-256, 크기, alpha, animation/loop metadata와 normalization recipe hash를 함께
반환한다.

`icon_ai_state` 누락, 다른 icon의 active version, 존재하지 않는 source row/file,
SHA 불일치 또는 decode/metadata 불일치는 원본으로 조용히 fallback하지 않는다.
UI는 `AI 소스 복구 필요` 상태와 원본 복귀/이력 정리 선택을 보여주고 editor render,
preview 저장과 export를 막는다. `active_version_id = NULL`인 정상 상태만 원본을
선택한다.

editor DTO는 `EditorVisualSources { original_source, effective_render_source,
active_version }`로 분리한다. 원본 정보와 원본 보기만 `original_source`, canvas,
crop 측정, preview와 저장은 `effective_render_source`를 사용한다.

다음 경로가 같은 resolver와 effective source hash를 사용해야 한다.

- crop/editor preview와 저장 preview
- text/static effect 및 motion preview·commit
- 최종 export
- optimizer 분석과 GIF FPS preview
- 정적 작업 시트 export/reimport stale guard
- GIF frame 작업 시트 export/reimport identity와 stale guard
- collection cover preview fallback

`processed_asset_variants.source_file_id`와 `source_hash`는 항상 effective render
source ID/SHA를 뜻한다. AI가 비활성일 때만 두 값이 원본 ID/SHA와 같다.

기존 nullable/no-FK variant는 AI migration 직후 reconciliation한다. `source_file_id`
가 `NULL`이고 `source_hash`가 owning icon의 현재 original source SHA와 정확히 같으면
그 source ID를 backfill한다. non-null ID도 FK row/file 존재와 row SHA가
`source_hash`와 같은지 검증한다. NULL을 해소할 수 없거나 ID/SHA/file이 불일치하면
`is_active_for_export = 0`으로 만들고 legacy-stale로 취급한다. current/piece preview가
그 variant path를 가리키면 final effective source의 native preview를 먼저 재생성해
pointer를 교체하며, 재생성 실패는 repair-required로 fail closed한다.

reconciliation 뒤 table을 nullable `source_files` FK와 nullable `output_sha256`으로
rebuild한다. legacy artifact는 file 존재, 저장 byte size, format/dimension의 bounded
safe decode를 통과한 경우에만 파일 SHA-256을 계산해 `output_sha256`을 backfill한다.
missing/decode/size 불일치는 variant를 inactive legacy-stale로 두고 digest를 만들지
않는다. NULL source/digest는 과거 stale row에만 허용한다. 새 write/activation은
non-null effective source ID, ID+SHA와 encoded artifact `output_sha256`을 함께 저장한다.
active lookup은 source ID/SHA, crop, output-profile compatibility와 실제 output file
SHA를 모두 확인하며 stale bytes를 사용하지 않는다.

sheet manifest는 AI foundation에서 static `pmtcon-sheet-v2`와 GIF
`pmtcon-gif-frame-sheet-v2`로 version을 올린다.

- static v2의 각 item은 기존 `source_hash`를 original SHA-256 의미로 유지하면서
  `source_file_id`, `original_lineage_id`, `original_lineage_generation`,
  `render_source_file_id`, `render_source_sha256`을 추가한다. 기존 `render_hash`와
  `render_recipe_hash` 의미는 유지한다.
- GIF v2는 top level의 기존 `source_file_id`/`source_hash`를 original identity로
  유지하고 `original_lineage_id`, `original_lineage_generation`,
  `render_source_file_id`, `render_source_sha256`을 추가한다.
- v2 reimport는 original ID/hash/lineage/generation, effective render source/hash와
  전체 recipe hash가 모두 일치해야 한다.
- legacy v1은 AI가 비활성이고 `original_lineage_generation = 0`인 icon에만 기존
  original hash 검사로 허용한다. AI가 활성인 icon 또는 generation이 1 이상인
  icon에 v1을 가져오면 추측하지 않고 해당 item/job을 stale로 건너뛴다.

직접 `icons.source_file_id`를 쓰는 allowlist는 원본 정보/파일 보기, import와 일반
원본 교체, migration/backfill, cleanup/reference count, clone/provenance ownership
query로 한정한다. 이 query들은 render source를 결정하지 않는다. 새 직접 join이
추가되지 않았는지 source-search regression으로 검사한다. AI는 renderer의 맨
앞단인 base source만 바꾸며 기존 crop, transform, text, static effects, motion과
piece split 순서는 그대로 유지한다.

## 6. 후보 생성·활성화·복귀

1. 요청 생성 시 original/effective source hash, 원본 lineage ID/generation,
   activation revision, mode별 `payload_input_signature`, 전체
   `request_recipe_signature`와 실제 외부 전송 payload를 snapshot한다.
2. SQLite/shared render lock을 잡은 채 네트워크나 사용자 웹 작업을 기다리지 않는다.
   결과는 형식, decode, dimensions, alpha, animation, byte/pixel workload를 검사해
   불변 `source_files`와 비활성 candidate로만 저장한다.
3. 사용자가 후보를 비교하고 `새 아이콘으로 추가` 또는 호환 가능한 경우에만
   `현재 아이콘에 사용`을 선택한다. 생성 완료만으로 적용하지 않는다.
4. 현재 아이콘 activation prepare는 새 operation ID와 목표 activation revision,
   icon/activation 소유의 final preview path를 먼저 발급한다. candidate를 이 icon에
   처음 materialize할 때만 새 version ID도 발급한다. 최신 original/effective source,
   원본 lineage ID/generation, active version, activation revision 및 crop/shape/cell/loop/
   transform/text/effect/motion/piece를 포함한 전체 `activation_recipe_signature`를
   snapshot한다.
5. candidate input stage, 원본 계보, animation kind와 normalization recipe를
   검증하고 shared DB lock 밖에서 request-owned staging preview/piece를 렌더한다.
6. 짧은 immediate transaction을 열고 original/effective source,
   원본 lineage ID/generation, active version, activation revision과
   `activation_recipe_signature`가 prepare snapshot과 같은지 final CAS한다.
   하나라도 다르면 rollback하고 staging을 지운다.
7. CAS 통과 후 같은 volume에서 staging 파일을 미리 발급한 icon/activation-owned
   durable final path로 atomic rename하고 DB path를 final path로 rebase한다. 최초
   candidate materialization에만 `icon_ai_versions` row를 insert하고, active pointer, 증가한
   revision과 final current/piece preview path를 같은 transaction에서 update한 뒤
   commit한다. rename/DB/commit 실패 시 DB를 rollback하고 staging 및 승격된 final
   artifact를 보상 정리한다. rename 뒤 commit 전 crash가 남긴 미참조 final 파일은
   startup/library sweep가 durable reference를 확인한 뒤 회수한다.
8. 원본 복귀나 이전 AI version 선택은 새 version을 만들지 않고 기존 version 또는
   `NULL`을 선택한다. provider를 호출하지 않으며 같은 prepare → staging render →
   final CAS → durable rename/path rebase → DB commit/compensation 경로를 사용한다.
일반 `이미지 교체`는 새로운 원본 계보를 시작한다. 새 source decode와 preview를
먼저 staging한다. 같은 protocol로 final CAS 후 preview를 final icon-owned path로
rename하고 하나의 transaction에서 `icons.source_file_id`, 새 `original_lineage_id`,
증가한 `original_lineage_generation`, crop/transform/thumbnail/preview,
`icon_ai_state.active_version_id = NULL`, 증가한 activation revision을 함께 commit한다.
같은 transaction에서 옛 original/effective signature로 실행 중인 request에
`superseded_at/reason`을 기록한다. 늦게 등록되는 result도 snapshot 불일치를 다시
검사해 request를 superseded로 만들고 비활성 candidate로만 저장한다.

이전 원본에 묶인 version/candidate는 이력으로 보존하지만 새 원본의 현재 아이콘에는
다시 활성화할 수 없고 별도 새 아이콘으로만 추가할 수 있다. version의
base original source, lineage ID 또는 generation 중 하나라도 현재 값과 다르면
활성화를 거부한다. superseded 요청의 provider 결과가 나중에 도착해도 비활성
candidate로만 등록한다.

stale 요청의 결과는 버리지 않는다. `편집 상태가 바뀌어 자동 적용하지 않았습니다`
상태의 비활성 후보로 남겨 사용자가 새 아이콘으로 추가하거나 다시 생성할 수 있게
한다.

활성 AI source가 바뀌면 기존 optimization variant는 effective source hash가
다르므로 자동으로 stale 처리된다. 원본 또는 예전 AI source로 돌아왔을 때도
현재 hash와 일치하는 artifact만 재사용한다. preview path 자체는 이력이 아니므로
복귀 시 native renderer로 다시 만든다.

## 7. 두 가지 입력·적용 모드

### 베이스 소스 편집

현재 effective base canvas를 입력으로 만들되 공급자에는 그 provider가 지원하는
native resolution/aspect로 보낸다. 200×200 결과를 provider가 직접 반환한다고
가정하지 않는다. crop·text·effect·motion은 보내지 않고 PMTCONCON Studio의 별도
편집 metadata로 유지한다.

후보 비교는 provider-native raw 결과와 target canvas용 normalized 결과를 함께
보여준다. normalization은 `contain + pad` 또는 `cover + crop`, alignment, resize
filter, pad RGBA/alpha 처리의 bounded deterministic recipe로 저장하고 hash한다.
비용 추정은 작은 target cell이 아니라 실제 provider-native 입력·출력 크기를
기준으로 한다.

현재 아이콘에는 normalized source가 원래 canvas와 animation kind에 정확히
호환될 때만 활성화한다. opaque-only provider 결과는 투명 배경이 사라질 수 있음을
경고하고 alpha를 임의로 복원했다고 표시하지 않는다. 호환되지 않거나 사용자가
정규화를 원치 않으면 새 아이콘으로 추가한다. 이 모드는 현재 아이콘에서 AI만
즉시 제거할 수 있는 기본 source-replacement 방식이다.

### 현재 보이는 결과 편집

결합 viewport의 정적 렌더를 보내므로 사용자가 보는 구성과 가장 가깝다. 다만
이미 적용된 crop/effect/text를 다시 적용하면 이중 처리가 생길 수 있으므로 첫
버전에서는 `새 아이콘으로 추가`만 허용한다. 새 아이콘은 full-canvas crop,
빈 alt와 `작업중` 상태로 만들고 원본 아이콘은 그대로 둔다.

UI는 요청 전에 공급자에 전송되는 정확한 이미지, mask, reference와 prompt를
보여준다.

## 8. 공급자 선택

픽셀화, 반전·회전, 색조·밝기 변화, displacement와 motion처럼 이미 native
renderer가 결정적으로 처리하는 작업은 AI로 보내지 않는다. AI는 그림체·캐릭터·
배경·구성처럼 의미를 바꾸는 편집과 새 이미지 생성에 집중한다.

| 경로 | 역할 | 장점 | 제약 | 결정 |
|---|---|---|---|---|
| NovelAI Image API | 첫 자동 adapter | 공식 제3자 API, anime/character 특화, img2img·inpaint·reference, 구독 ImageAnlas 활용 | 18세 이상, 광범위한 PAT 보안, 사람 동작 필수, 편집은 Anlas 소모 가능, alpha 보장 없음 | 첫 구현·조건부 주 provider |
| 웹 AI 수동 handoff | API 없는 fallback | 기존 웹 구독 활용, 공급자 중립 | 자동 회수·모델·비용 검증 불가 | 보조 경로 |
| Gemini API | 선택형 cloud 후보 | 한국어, 멀티턴, 복수 참조, 다양한 해상도 | 현재 이미지 모델 무료 tier 없음, professional/business·18세·지역/유료 조건 | 기본 비선택 |
| OpenAI Image API | 선택형 cloud 후보 | 단일 생성·편집, mask, 복수 참조, high-fidelity 입력 | desktop BYOK 잔여 위험·과금, 출력 capability 변동 | 고급 후속 |
| literal loopback endpoint | 고급 사용자 | 사용자 실행 서버·GPU 활용 가능 | 설치·VRAM·workflow·모델 license, workflow의 외부 유료 호출 가능 | 고급 후속 |

NovelAI는 공식 이미지 API가 제3자 user-facing 앱에 사용자의 Persistent API
Token을 요구하는 방식을 문서화했고, 현재 구독은 ImageAnlas를 제공한다. 따라서
무료·MIT 배포인 PMTCONCON Studio가 공급자 계정이나 과금 서버를 운영하지 않는
첫 자동 경로에 가장 잘 맞는다. 앱은 NovelAI 로그인, 이메일, 비밀번호를 받지
않는다. 사용자가 NovelAI Account 화면에서 발급한 PAT만 직접 입력한다.
공식 Account UI에서 새 PAT를 만들면 기존 PAT는 무효화되고 popup을 닫은 뒤 다시
볼 수 없다. 앱은 `Authorization: Bearer pst-<token>` 형태만 Rust adapter가 만들고,
frontend로 token을 되돌려 보내거나 로그에 남기지 않는다. invoke 경계로 전달한 뒤
frontend 입력 state를 즉시 비우고 clipboard를 자동으로 읽지 않는다. clipboard에
token이 남을 수 있다는 경고와 사용자가 직접 수행할 clipboard 정리·PAT rotation
안내를 제공한다. `401`은 자동 재시도하지 않고 새 PAT 발급/재입력을 안내한다.
이번 실행의 token을 지워도 DB binding 삭제는 없으며 request/candidate/version/
source와 active pointer는 그대로 보존한다.

초기 NovelAI adapter는 한 번의 명시적 버튼 동작당 한 장만 생성한다. background
queue, 무인 batch, 연쇄 생성과 자동 retry를 금지한다. 공식 API 문서의 모든
generation은 사람의 동작으로 시작해야 한다는 조건을 capability에 고정한다.
첫 범위는 text-to-image, img2img와 mask 기반 inpaint이다. Vibe/Precise Reference,
Director Tools와 streaming은 별도 capability fixture와 사용량 검토 뒤 추가한다.

일반 생성에서 Opus의 0 ImageAnlas 조건은 한 장, 28 steps 이하, normal-size 범위와
함께 다른 이미지를 base로 쓰지 않는 경우이다. 따라서 새 이모티콘 text-to-image는
조건부 0 Anlas일 수 있지만 기존 이모티콘을 보내는 img2img/style edit는 ImageAnlas를
쓸 수 있다. 공식 웹 Inpaint 문서는 별도로 Opus의 Focused Inpainting이 큰 이미지의
영역을 0 Anlas로 inpaint할 수 있다고 설명한다. 그러나 공개 Image API 문서는 이
Focused action/payload 지원과 웹 구독 혜택의 API 차감 동일성을 명시하지 않는다.
따라서 inpaint는 공식 지원 확인과 user-approved small live pilot 전까지 항상
`공급자에서 확인`으로 표시하며 무료라고 약속하지 않는다.

PMTCONCON Studio는 provider unit을 USD actual/billed로 환산하지 않는다. 요청 전에
settings와 공식 조건, 추정 ImageAnlas 또는 `공급자에서 확인` 상태를 보여주며 결제
가능 요청은 자동 재시도하지 않는다.

Gemini는 기본 provider로 선택하지 않는다. 2026-07-26 현재 최신 Gemini image
generation 모델은 API pricing에서 free tier가 `Not available`이고, adapter
Stage Gate 전에 공식
Additional Terms 기준으로 18세 이상, 미성년자 대상/접근 가능성 없음,
professional/business 목적, 지원 지역, EEA·스위스·영국 사용자 제공 시 Paid
Services 조건이 PMTCONCON Studio의 실제 배포 대상과 맞는지 제품·법무 적합성
검토를 통과해야 한다.

OpenAI Image API는 사용자가 별도 project key와 과금을 원하는 경우의 고급
후속 adapter로 남긴다. 어느 cloud adapter도 provider-neutral candidate,
activation, rollback schema를 바꾸지 않는다. adapter를 hard-disable하거나
credential을 지워도 기존 candidate/version/source bytes와 active pointer는
보존하고 로컬 복귀를 계속 지원한다.

가격, 모델명, 투명 배경과 데이터 정책은 앱 release와 무관하게 바뀔 수 있다.
adapter capability와 공식 링크를 표시하고 비용은 출처·가격 snapshot 기준일,
provider-native 입력/출력 크기, 포함 항목과 오차 범위를 적은 `추정치`로만 보여준다.
provider가 반환한 usage와 nullable provider-reported cost를 추정치와 분리한다.
로컬에서 계산한 금액은 `actual` 또는 `billed` cost라고 부르지 않으며 실시간
pricing/청구 API를 약속하지 않는다.

## 9. 웹 AI 수동 handoff

PMTCONCON Studio가 지원할 수 있는 안전한 범위는 다음과 같다.

1. 사용자가 선택한 웹 service surface와 account context를 명시하고, 해당 조합의
   공식 약관·개인정보·데이터 제어 링크와 검토 기준일을 request snapshot에 기록한다.
2. `ai/handoffs/<request-id>/`에 전송용 이미지, 선택적 mask/reference,
   `prompt.txt`와 비밀값 없는 manifest를 만든다.
3. 실제 전송 예정 payload를 미리 보여준 뒤 prompt 복사와 공식 홈페이지 열기를
   제공한다.
4. 사용자가 직접 로그인, 업로드, 생성, 다운로드한다.
5. `AI 결과 가져오기`가 파일을 검증하고 동일한 candidate 이력에 등록한다.

DOM 조작, cookie/session 접근, 자동 업로드, 결과 scraping과 자동 다운로드는
제품 기능으로 만들지 않는다. 웹 UI 변경에 취약하고 소비자 서비스 약관과 충돌할
수 있으며, PMTCONCON Studio가 결과의 모델·비용·provenance를 검증할 수도 없다.

웹 handoff에는 API 데이터 정책을 대신 표시하지 않는다. ChatGPT 웹과 Gemini 웹은
각 account context에 맞는 개인정보·데이터 제어 surface로 안내한다. 사용자가 결과를
가져올 때 model은 `사용자 선택` 또는 `검증되지 않음`, 비용은 `계산 불가`일 수
있으며 이는 오류가 아닌 정상 상태이다. request의 `service_surface`,
`account_context`, `provenance_trust`, typed `policy_refs[]`가 이 불확실성을 보존한다.

## 10. API key, 개인정보와 비용

- 개발자 공용 key를 앱이나 설치 파일에 포함하지 않는다. 사용자별 BYOK만 쓴다.
- Tauri의 Rust backend도 사용자의 PC에서 실행되는 client이다. OS credential
  store는 저장 중 key를 보호할 뿐, local malware, debugger 또는 취약한 frontend/
  invoke 경로로부터 런타임 key를 완전히 보호하지 못한다. 따라서 API 연결 화면은
  이 잔여 위험을 설명하고 수동 handoff를 token 없는 fallback으로 제공한다.
- cloud 호출은 provider별 좁은 Rust command에서 수행한다. adapter 코드가 소유한
  exact HTTPS origin allowlist(scheme + host + port + path prefix), 엄격한 CSP,
  임의 URL/scheme/port/path override 금지, redirect 금지, frontend로 key 반환
  금지와 Authorization/secret 로그 금지를 적용한다. NovelAI MVP는
  `https://image.novelai.net:443/ai/generate-image`만 허용하고 추가 endpoint는
  capability별 별도 검토 후 allowlist에 넣는다. 앱 안에서 account login이나
  token 발급을 위해 primary account API를 호출하지 않는다.
- 영속 key는 별도 향후 Stage Gate 전까지 구현하지 않는다. 개발·초기 검증은 환경
  변수 또는 이번 실행 전용 key로 제한하고, foundation/첫 NovelAI adapter에는
  credential binding schema를 만들지 않는다.
- 향후 영속 credential Stage Gate는 먼저 비밀값을 담지 않는
  `ai_credential_bindings(id, adapter_id, provider, state, created_at, updated_at)` 부모와
  adapter/provider 일치 guard를 추가하고, 그때만 nullable
  `ai_requests.credential_binding_id REFERENCES ai_credential_bindings(id) ON DELETE SET
  NULL`을 추가한다. token은 OS vault에만 둔다. 삭제는 DB에서 binding을 `deleting`으로
  표시해 새 호출을 막고 → vault entry 삭제 → DB row 삭제 순서로 진행하며, 중간 실패/
  crash는 retryable repair state로 마무리한다. 이 migration과 회귀 테스트는 그 향후
  Stage Gate의 범위이다.
- 사용자에게 제한된 project key, 최소 권한, 지출 한도/알림과 정기 rotation을
  권장한다. 개발자 운영 proxy는 별도 서버·계정·비용·개인정보 책임이 필요하므로
  현재 제품 범위에 포함하지 않는다.
- HTTP client와 credential dependency는 MIT 정책 및 transitive license 검사를
  통과한 뒤에만 추가한다.
- 첫 외부 전송 전 service surface, 공급자, 실제 전송 파일, prompt,
  mask/reference, account context와 적용되는 terms/privacy/data controls/model
  terms·attribution 링크를 보여주고 사용자의 명시적 확인을 받는다. API는 model과
  출처/기준일/가정을 포함한 예상 비용도 표시한다.
  수동 웹 handoff의 model·비용은 검증 불가 또는 계산 불가일 수 있음을 그대로
  표시한다.
- 결제 가능 요청은 자동 재시도하지 않는다. 취소가 이미 발생한 비용을 되돌린다고
  약속하지 않는다.
- base64 이미지, key, Authorization header, 전체 provider 응답을 로그에 남기지
  않는다.
- PMTCONCON Studio의 MIT license는 앱 코드에만 적용된다. 앱은 input 권리,
  provider/model/workflow 조건, attribution 의무 또는 생성물의 독점성·사용 권리를
  보증하지 않으며 consent와 export 이력에서 관련 typed reference를 다시 볼 수 있게
  한다.

사용자 설치 로컬 adapter는 hostname 문자열이 아닌 URL parsing 결과가 literal
`127.0.0.1` 또는 `[::1]`인 HTTP endpoint만 허용한다. DNS hostname, wildcard
address, LAN/private IP, userinfo와 redirect를 거부하고 모든 연결 단계에서
loopback을 다시 검사한다. timeout·응답 byte·decode dimension·총 pixel workload
제한도 적용한다. 원격 HTTPS endpoint는 이 adapter에 섞지 않고 별도 provider/
보안 검토 대상으로 둔다.

ComfyUI 등 GPL 프로그램의 binary, source, 설치 자동화, workflow와 custom node를
앱에 번들하지 않는다. 독립적으로 작성한 generic HTTP adapter로 사용자가 별도
실행한 endpoint에 연결하는 경계만 후속 검토한다. 일부 local workflow와 Partner
Node는 외부 유료 API를 호출할 수 있으므로 UI에는 “PMTCONCON Studio에서 endpoint
까지의 전송만 로컬이며 workflow 내부 처리는 별도입니다”라고 표시한다. 실행 전
사용자가 workflow와 비용을 확인하게 한다.

## 11. GIF와 sprite sheet

첫 AI MVP는 정적 PNG/JPG와 GIF의 명시적 poster-frame 후보만 지원한다.

- GIF 전체를 모르게 poster 한 장으로 교체하지 않는다.
- GIF poster-frame AI 결과는 첫 버전에서 새 정적 아이콘으로만 추가하며 현재
  animated icon의 base source로 활성화하지 않는다.
- 정적 AI 후보에 기존 native motion 효과를 적용해 애니메이션을 만드는 흐름을
  먼저 제공한다.
- 전체 프레임 변경은 기존 GIF frame 작업 시트의 export/manifest/reimport
  경로를 수동 handoff에 연결하는 실험부터 시작한다.
- 프레임별 API 호출은 요청 전 총 frame 수와 예상 비용을 보여주고 opt-in으로만
  허용한다.
- sprite sheet 한 장이 한 API 호출이라는 이유만으로 저렴하다고 간주하지 않는다.
  공급자는 보통 입력·출력 pixel/token을 과금하고, grid 경계 오염, 셀 누락과
  캐릭터 불일치가 생길 수 있다.
- n-up 생성은 manifest, cell count/dimension 검증과 실패 시 전체 후보 비활성화를
  갖춘 실험 기능으로만 다룬다.

## 12. 복제·정리·테스트

아이콘 복제와 collection 복제는 provider 실행 사실인 `ai_requests`와
`ai_candidates`를 복사하지 않는다. clone은 동일한 immutable candidate와
content-addressed source 바이트를 참조한다. 원본 icon의 distinct 과거 lineage마다
서로 다른 clone lineage ID를 일대일 발급하고 generation 값은 보존한다. 현재
lineage는 새 `icons.original_lineage_id`에 매핑하고 현재 generation도 복사한다.
각 `icon_ai_versions.base_original_lineage_id/generation`, parent map과
`icon_ai_state` pointer를 새 icon/version/lineage ID로 remap하므로 과거 lineage가
현재 clone lineage로 합쳐져 활성화되는 일이 없다. `base_source`의 `새 아이콘으로
추가`도 이 전체-lineage map을 사용한다. provider request ID, usage와 cost는 distinct
`ai_requests` 실행 1회로만 계산된다.

복제 순서는 다음으로 고정하고 DB/file compensation을 하나의 clone protocol로
다룬다.

1. 새 icon/piece/profile과 모든 durable crop/transform/text/effect/motion/sheet
   recipe ID map을 만든다.
2. distinct lineage를 일대일 매핑한 뒤 complete version DAG/state를 만든다.
   `새 아이콘으로 추가`이면 candidate child와 그 active pointer도 이 단계에 포함한다.
3. transaction 안의 target state로 `EffectiveVisualSource`를 resolve한다.
4. source-side active variant가 유효한지 먼저 검사하고, 그 variant의 `source_hash`,
   `crop_hash`, output format과 ID/path를 제외한 output-affecting profile fields의
   compatibility hash가 final target effective source/crop/profile과 모두 같은 경우에만
   새 ID와 target-owned path로 bytes/row를 복제한다. 다른 source의 bytes를 target
   source hash로 재라벨링하지 않는다.
5. 하나라도 다르면 variant file/row 복제와 promoted-preview remap을 모두 건너뛰고
   final target effective source의 native recipe에서 preview/export를 재생성한다.
   모두 같고 source preview가 promoted active variant를 가리킬 때만 새 target
   variant path로 remap한다. 그 외 current/piece preview는 F134 규칙대로 native
   render 또는 독립 복사한다.
6. version/state/variant와 durable preview path를 함께 commit한다. version insert,
   pointer, preview 또는 variant copy 어느 단계가 실패해도 DB를 rollback하고
   staging/promoted 파일을 보상 정리한다.

clone별 preview, activation revision과 이후 version 계보는 서로 독립적이다.
pending request 중 clone해도 늦게 도착한 candidate는 원래 request에 비활성으로만
붙고 clone에 자동 version을 만들지 않는다. clone에서 쓰려면 사용자가 candidate를
명시적으로 추가해야 한다.

library cleanup은 candidate, version, active/parent 이력 또는 soft-deleted
아이콘/collection 이력에서 참조하는 `source_files`를 고아로 판단하지 않는다.
AI 결과와 metadata의 영구 삭제는 descendant와 clone 참조를 검사하는 명시적
`AI 이력 정리` 또는 soft-delete 영구 정리에서만 허용한다.

`ai/inputs/`, `ai/handoffs/`와 activation staging은 source of truth가 아닌 민감한
중복 사본이다. staging은 commit/abort 직후 지우고, input/handoff payload는 API
완료·실패·취소 또는 수동 결과 import 뒤 즉시 지운다. 수동 `awaiting_result`
package의 기본 만료는 7일이며 사용자가 명시한 `30일 보관`만 한 번의 bounded
연장으로 허용한다. 만료 시 payload를 지우고 request를 `expired`로 전환한다.
crash가 남긴 staging/final-path 고아는 startup에서 durable reference를 검사한 뒤
24시간 이상 된 것만 회수한다. UI는 `요청 및 외부전송 사본 삭제`를 제공하고
prompt/options/error scrub과 source 후보 삭제를 분리한다. 정리 후에도 request에는
payload hash, surface/policy snapshot과 정제된 최소 provenance만 남기며 민감한
payload를 무기한 중복 보관하지 않는다.

첫 foundation stage의 필수 자동 검증:

- provider-qualified/versioned snapshot의 canonical encoding, 64 KiB 제한과 allowlist;
  full request/response, header, token, cookie, base64/binary payload 삽입 거부
- foundation schema에 credential binding table/column/FK가 없고 session/environment
  token clear 뒤에도 request/candidate/version/source/active pointer가 보존됨
- adapter 비활성화 뒤 새 network call은 차단하지만 기존 active source, clone,
  preview/export, cleanup 보호와 original/previous-version rollback은 유지
- provider 변경 시 새 request/exact-payload consent 생성, 실패 provider에서 fallback
  request가 만들어지지 않음
- NovelAI mock의 `Authorization: Bearer pst-<token>` exact header와 frontend token
  input clear/non-echo, PAT 교체 뒤 `401` 무재시도 안내
- AI 적용·복귀 후 원본 ID, SHA-256과 바이트 불변
- DB 재시작 후 원본과 이전 AI 후보로 복귀
- stale activation revision 거부
- activation의 전체 recipe/lineage CAS 실패 시 pointer/preview 원자성, staging/
  promoted-final 보상 정리와 crash orphan sweep
- 일반 이미지 교체 시 active pointer 해제와 이전 원본 계보 재활성화 거부
- 동일 바이트 `A → B → A` 재교체 후 과거 lineage version 활성화 거부
- generation 1 이상인 `A → B → A` icon의 legacy v1 manifest stale 거부
- 모든 import/placeholder/duplicate/sheet/clone icon INSERT의 lineage default와 atomic
  state 생성, source-search 우회 방지
- 생성 중 편집된 요청의 후보 보존 및 자동 적용 금지
- 잘못된 형식, 과대 파일, pixel workload와 호환되지 않는 크기 거부
- `pmtcon-alpha-v1`의 opaque-channel false, 실제 투명 pixel true, GIF 모든 표시 frame
  scan과 legacy NULL lazy backfill
- provider raw와 deterministic normalized source 및 recipe hash 보존
- preview/export/optimizer/static sheet/GIF sheet의 effective source 일치
- static/GIF v2 manifest의 original ID/hash/lineage/generation과 effective render
  source/hash 일치, AI-active/generation-1+ legacy v1 stale skip
- allowlist 외 직접 `icons.source_file_id` render join 방지
- editor의 original metadata와 effective canvas source 분리 및 깨진 state/source의
  fail-closed render/export 차단
- `processed_asset_variants.source_file_id/source_hash`의 effective source 의미
- AI source 변경 시 optimization stale 판정
- legacy variant의 unambiguous source ID와 bounded artifact `output_sha256` backfill,
  NULL/ID/SHA/file/decode/size mismatch inactive stale 처리, promoted preview native
  재생성과 새 variant non-null source ID/hash/output digest 검증
- cleanup의 AI 이력 source 보존과 input/handoff/staging retention
- 아이콘 복제 및 collection 복제 후 request/cost 비중복, version/state/preview 독립성
- active AI + promoted optimized GIF + multi-piece clone의 고정 순서, preview
  variant remap과 단계별 DB/file rollback
- 과거 lineage가 여러 개인 clone의 lineage 일대일 remap과 현재-lineage activation
  격리
- pending request clone 뒤 late result 비부착
- base-source 새 아이콘 candidate/preview/variant 실패 시 반쪽 icon·artifact 부재
- cross-icon parent/active와 same-icon cross-lineage parent 거부, active-lineage CAS,
  migration state/lineage backfill과 `(request_id, candidate_index)` uniqueness
- active/promoted source variant에서 다른 base-source candidate child를 새 icon으로
  추가할 때 old variant bytes/row와 promoted-preview remap 부재, final target native
  preview/export 사용
- 원본 collection/icon 영구 정리 후 clone의 공유 provenance와 rollback 생존
- `cleanup_library(apply=true)` 뒤 inactive candidate와 soft-deleted AI 이력 source 보존
- API가 없는 fake/manual provider fixture로 모든 검증 실행

## 13. 단계

### AI-0 — 설계

이 문서, 제품 명세, 기능 인벤토리, ADR과 구현 계획을 일치시킨다. API 호출과
UI 노출은 하지 않는다.

### AI-1 — 비파괴 foundation

provider-neutral mutable request/immutable candidate/icon-version/state migration,
effective source resolver와 editor original/effective DTO, fail-closed repair state,
full-recipe CAS를 쓰는 atomic activation/rollback, managed temporary paths와 bounded
cleanup/고정순서 clone, effective variant, manifest·direct-source regression 및
mock/manual import 테스트를 구현한다. 네트워크 호출은 없다.

구현 완료(2026-07-27). 로컬 JPG/PNG 결과를 비활성 후보로 가져와 새 아이콘으로
추가하거나 고급 동작으로 현재 아이콘에 적용할 수 있고, 원본/이전 version 복귀와
재시작 후 복구가 provider 호출 없이 동작한다. 자동 provider 단계 전에
unmaterialized 후보에도 적용되는 immutable `input_stage`와 강제 종료가 남긴
검증된 빈 preview parent 정리를 추가한다.
### AI-1.5 — 후보 정규화와 작업공간

임의 크기 static JPG/PNG raw candidate를 보존하고 `contain + pad` 또는
`cover + crop`으로 현재 effective base-source canvas에 맞춘 deterministic
normalized source와 적용 전 native preview를 만든다. compact editor entry, 큰 후보
비교 dialog, 새 아이콘 생성 후 reveal/open, 한 개의 status region과 combined
review/editor mutation result를 구현한다. 이 단계는 provider, network, token 또는
새 dependency를 추가하지 않는다.

세부 구현 순서와 수용 기준은 `docs/AI_WORKSPACE_UX_DESIGN.md`의 AI-UX-1~3을
따른다.


### AI-2 — NovelAI Image API

공식 사이트에서 사용자가 발급한 Persistent API Token을 session-only로 받아
`https://image.novelai.net:443/ai/generate-image`의 text-to-image, img2img와
inpaint를 common candidate contract에 연결한다. 사람의 버튼 동작당 한 장,
redirect/자동 retry/background batch 금지, bounded ZIP/JSON decode, exact payload,
ImageAnlas 조건/추정치, privacy/rights 확인과 mock HTTP 검증을 포함한다. 공식 지원
확인과 사용자가 명시적으로 허용한 소액 live pilot 전에는 조건부 상태로 유지한다.

### AI-3 — 웹 수동 handoff

전송 package, prompt 복사, 공식 사이트 열기, 결과 가져오기와 A/B 후보 비교를
구현한다. service-surface별 정책과 검증되지 않은 model/cost 상태, payload
retention/삭제 UX를 포함하며 브라우저 자동조작은 없다.

### AI-4 — 공급자 확장

Gemini adapter는 유료 image tier와 나이·대상 사용자·professional/business·지원
지역·유료 서비스 조건의 배포 적합성 gate를 통과한 경우에만 검토한다. OpenAI
Image API와 literal loopback adapter도 별도 stage에서 각각 비용/secret 또는
redirect/SSRF와 workflow 외부 호출 disclosure를 검증한다. 모두 독립된
license/privacy/security Stage Gate를 통과해야 한다.

### AI-5 — GIF/sprite 실험

poster-frame + native motion, frame-sheet handoff, opt-in frame batch와 n-up
sprite 후보를 측정된 `provider_usage`/`estimated_provider_units`, 기준일이 있는
nullable `estimated_cost`와 nullable `provider_reported_cost` 및 일관성 fixture로
검증한다.

## 14. 공식 참고 자료

- [NovelAI Image Generation API](https://image.novelai.net/docs/index.html)
- [NovelAI Image Generation API schema](https://image.novelai.net/docs/doc.json)
- [NovelAI Persistent API Token](https://docs.novelai.net/en/text/usersettings/account/)
- [NovelAI subscriptions and ImageAnlas](https://docs.novelai.net/en/subscription/)
- [NovelAI image generation](https://docs.novelai.net/en/image/)
- [NovelAI image generation models](https://docs.novelai.net/en/image/models/)
- [NovelAI inpaint](https://docs.novelai.net/en/image/inpaint/)
- [NovelAI Terms of Service](https://novelai.net/terms)
- [OpenAI Image generation guide](https://developers.openai.com/api/docs/guides/image-generation)
- [OpenAI GPT Image 2 model](https://developers.openai.com/api/docs/models/gpt-image-2)
- [OpenAI API data controls](https://developers.openai.com/api/docs/guides/your-data)
- [OpenAI Terms of Use](https://openai.com/policies/terms-of-use/)
- [OpenAI Services Agreement](https://openai.com/policies/services-agreement/)
- [OpenAI API key safety](https://help.openai.com/en/articles/5112595-best-practices-for-api-key-safety)
- [ChatGPT Data Controls FAQ](https://help.openai.com/en/articles/7730893-data-controls-faq)
- [ChatGPT Business data and privacy](https://help.openai.com/en/articles/8798634)
- [ChatGPT personal and business training controls](https://help.openai.com/en/articles/8983130-what-is-the-chatgpt-enterprise-and-team-data-policy)
- [Gemini image generation](https://ai.google.dev/gemini-api/docs/image-generation)
- [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing)
- [Gemini API Additional Terms](https://ai.google.dev/gemini-api/terms)
- [Gemini API key security](https://ai.google.dev/gemini-api/docs/api-key)
- [Gemini Apps Privacy Hub](https://support.google.com/gemini/answer/13594961)
- [Gemini Apps work or school account privacy](https://support.google.com/gemini/answer/14620100)
- [ComfyUI repository and GPL-3.0 license](https://github.com/Comfy-Org/ComfyUI)
- [ComfyUI server routes](https://docs.comfy.org/development/comfyui-server/comms_routes)
- [ComfyUI API key and Partner Node integration](https://docs.comfy.org/development/comfyui-server/api-key-integration)

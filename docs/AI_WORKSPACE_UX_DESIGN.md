# PMTCONCON Studio AI 작업공간 UX·후보 정규화 설계

검토 기준일: 2026-07-27

## 1. 결정 요약

현재 편집 패널의 접힌 `AI 후보 및 소스 이력` 영역은 안전한 비파괴 기반을
검증하는 데에는 충분하지만, 프롬프트 작성과 공급자 실행까지 같은 영역에 더하면
패널이 지나치게 길어지고 기존 `적용` 동작과 혼동된다.

따라서 AI 기능은 다음 구조로 정리한다.

- 우측 편집 패널에는 현재 소스 상태, `AI로 수정` 진입 버튼과 AI 활성 시 빠른
  `원본으로 돌아가기`만 남긴다.
- 실제 작업은 앱 내부의 큰 `AI 작업공간` modal dialog에서 수행한다. 별도 Tauri
  window는 만들지 않는다.
- 작업공간은 `결과 가져오기`, `후보 검토`, `소스 이력` 세 탭으로 역할을
  분리한다. 아직 구현되지 않은 공급자 기능은 탭이나 비활성 메뉴로 미리
  노출하지 않는다.
- 후보 검토의 중심은 `원본 → AI 원본(raw) → 규격화 결과 → 최종 적용 모습`의
  큰 A/B 비교이다.
- AI 공급자가 반환한 임의 크기 JPG/PNG를 거부하지 않는다. raw 파일은 그대로
  보존하고 `전체 보이기(contain + pad)` 또는 `빈틈 없이 채우기(cover + crop)`로
  현재 base-source canvas에 맞춘 별도 불변 source를 만든다.
- 기본 완료 동작은 계속 `새 아이콘으로 추가`이다. `현재 아이콘에 사용`도
  숨기지는 않지만 보조 동작으로 표시하고, 원본과 이전 AI source가 보존됨을
  버튼 가까이에 설명한다.
- 생성 성공 후에는 `새 아이콘 열기`, `목록에서 보기`, `계속 후보 비교` 중 다음
  행동을 즉시 선택할 수 있어야 한다.

이 설계는 `docs/AI_INTEGRATION_DESIGN.md`의 request/candidate/version/state와
`EffectiveVisualSource` 계약을 바꾸지 않는다. 현재
`icon_ai_versions`에 이미 있는 provider-native 크기, target canvas,
normalization recipe/hash와 effective source 필드를 사용하므로 첫 정규화 MVP에는
새 migration이 필요하지 않다.

## 2. 목표와 제외 범위

### 목표

- 처음 쓰는 사용자도 “AI 결과를 가져오는 것”과 “현재 아이콘을 바꾸는 것”이
  별도 단계임을 이해한다.
- 1200×760 기본 창에서 후보, 비교, 설정, 완료 동작을 스크롤에 묻히지 않고
  확인한다.
- 서로 다른 해상도와 비율의 AI 결과를 적용 전에 검토하고 결정적으로 맞춘다.
- raw 결과, 규격화 source, 원본과 이전 AI version을 앱 재시작 뒤에도 구분한다.
- 현재 아이콘 적용과 새 아이콘 생성을 안전한 기본값·명확한 결과 상태로 제공한다.
- NovelAI API와 수동 웹 handoff가 나중에 추가되어도 후보 검토와 rollback UI를
  다시 만들지 않는다.

### 첫 구현에서 제외

- NovelAI 네트워크 호출, token 입력과 공급자 설정
- 웹 로그인, DOM upload, scraping 또는 자동 download
- GIF 전체 frame AI 편집과 sprite-sheet n-up 생성
- AI 배경 제거를 가장한 임의 alpha 복원
- layer/brush/liquify를 갖춘 범용 이미지 편집기
- AI 후보나 rollback source의 영구 삭제 UI

## 3. 사용자에게 보여 줄 개념

내부 데이터 이름을 그대로 노출하지 않고 다음 네 용어를 사용한다.

| UI 용어 | 내부 의미 | 설명 |
|---|---|---|
| 원본 | `icons.source_file_id` | AI로 덮어쓰거나 삭제하지 않는 최초/교체 원본 |
| AI 원본 | `ai_candidates.raw_source_file_id` | 공급자 또는 수동 도구가 반환한 그대로의 결과 |
| 규격화 결과 | version의 `effective_source_file_id` | raw를 현재 canvas에 결정적으로 맞춘 source |
| 최종 적용 모습 | effective source + native recipe render | crop, transform, text, effect, motion까지 반영한 모습 |

`현재 편집 소스`는 원본 또는 활성 AI version 중 하나다. 원본으로 돌아가도 후보와
AI source 이력은 삭제되지 않는다.

## 4. 진입과 정보 구조

### 편집 패널의 간단한 진입부

source preview 가까이에 다음 compact section을 둔다.

```text
이미지 소스                         [원본 사용 중]
원본: character.png
[AI로 수정]
```

AI version이 활성화된 경우:

```text
이미지 소스                     [AI 소스 사용 중]
원본은 보존되어 있습니다.
[AI 작업공간 열기] [원본으로 돌아가기]
```

- 기존 `AiReviewSection`의 후보·이력 본문은 dialog로 이동한다.
- 편집 패널 하단의 일반 `적용`은 `크롭·변형 적용`으로 바꿔 AI source 적용과
  구분한다.
- 저장하지 않은 편집 draft가 있어도 후보와 이력은 볼 수 있다. 외부 전송,
  규격화 확정, 새 아이콘 생성과 현재 source 전환만 잠그고, 잠금 이유와
  `크롭·변형 적용 후 계속` 안내를 해당 동작 옆에 표시한다.

### AI 작업공간

1200×760 창에서는 바깥 여백 16px를 남기는 최대 1168×728 dialog를 사용한다.
header, 탭, 한 개의 상태 영역과 footer action bar는 고정하고 본문 column만
독립적으로 스크롤한다.

```text
┌ AI로 이미지 수정 · 아이콘명 · 원본 사용 중                    [닫기] ┐
├ [결과 가져오기] [후보 검토] [소스 이력]                              ┤
│ 후보 rail 190px │ 큰 비교 영역 minmax(0, 1fr) │ 맞춤 설정 280px       │
│ 독립 스크롤     │ 원본/AI 원본/규격화/최종      │ 독립 스크롤            │
├ 상태 또는 오류 — 이 dialog 안에서 한 번만 알림                         ┤
│ [취소]       [현재 아이콘에 사용] [새 아이콘으로 추가 · 권장]          │
└───────────────────────────────────────────────────────────────────────┘
```

- 첫 MVP의 실제 기능명은 `결과 가져오기`이다. NovelAI adapter 또는 수동 handoff가
  구현된 뒤에만 첫 탭 이름을 `새 작업`으로 넓힌다.
- 폭 1024px 미만에서는 후보 rail을 상단 가로 thumbnail strip으로 바꾸고 맞춤
  설정은 접을 수 있는 하단 panel로 이동한다.
- dialog body에 page 전체 horizontal scroll이 생기지 않아야 한다.

## 5. 단계별 사용자 흐름

### 5.1 직접 만든 결과 가져오기

1. 사용자가 `AI로 수정`을 누른다.
2. 현재 단계 안내는 “API나 네트워크를 사용하지 않고 이미 저장한 결과를
   가져옵니다”라고 정확히 표시한다.
3. 출처와 JPG/PNG 파일을 선택하거나 drop한다.
4. frontend는 확장자와 16MB 제한을 빠르게 안내하고, backend는 실제 decode,
   최대 변·pixel workload와 정적 이미지 여부를 검증한다.
5. 임의 해상도와 비율은 오류가 아니다. 후보를 raw source로 보존한 뒤 자동으로
   `후보 검토`로 이동하고 가져온 후보를 선택한다.
6. 현재 아이콘은 이 시점에 바뀌지 않는다.

출처는 provenance 표시용이며, 사이트를 열거나 파일을 자동 전송했다는 의미로
표현하지 않는다. model과 비용을 확인할 수 없는 수동 결과는 각각
`검증되지 않음`, `계산 불가`로 표시할 수 있다.

### 5.2 후보 선택과 큰 비교

후보 rail의 각 선택 항목에는 후보 번호, 파일명, 출처, 생성/가져오기 시각과
상태를 표시한다. 접근성 이름도 이를 포함한다.

예:

```text
후보 2, result.png, NovelAI 웹 수동 결과, 7월 27일 14시 20분
```

중앙 비교 toolbar:

- `원본`
- `AI 원본`
- `규격화 결과`
- `최종 적용 모습`
- `겹쳐 보기`
- `화면 맞춤`, `100%`
- checkerboard 켜기/끄기

마우스용 전후 slider는 추가할 수 있지만 유일한 비교 방법으로 사용하지 않는다.
각 명시적 view button과 keyboard 조작만으로 같은 비교가 가능해야 한다.

비교 아래에는 다음 metadata를 같은 순서로 보여준다.

- AI 원본 크기와 파일 용량
- target base-source canvas 크기
- 최종 output preview 크기
- 실제 투명 pixel 사용 여부
- 잘림, padding, opaque 결과와 animation 호환 경고

### 5.3 크기 맞춤

기본값은 `전체 보이기`다.

| UI | recipe mode | 동작 | 기본 용도 |
|---|---|---|---|
| 전체 보이기 · 권장 | `contain_pad` | 전부 보이도록 축소/확대하고 남는 곳을 padding | 캐릭터나 말풍선 잘림 방지 |
| 빈틈 없이 채우기 | `cover_crop` | canvas를 가득 채운 뒤 넘치는 부분 crop | 배경과 꽉 찬 구도 |

공통 설정:

- 3×3 정렬
- 일반 일러스트용 `Lanczos3` resize
- pixel art용 `Nearest` resize
- padding RGBA. 기본은 `(0, 0, 0, 0)` transparent

고급 설정은 기본값으로 접는다. 정렬은 contain에서는 padding 위치, cover에서는
crop 기준점을 뜻한다.

오른쪽 inspector 예:

```text
AI 원본              1024×768 JPG · 투명 pixel 없음
현재 소스 canvas      640×640
규격화 결과           640×640 PNG
최종 출력 preview     200×200

주의: 위·아래에 투명 여백이 생깁니다.
주의: 원본 그림 내부의 불투명 배경은 제거하지 않습니다.
현재 아이콘에 사용 가능
```

### 5.4 새 아이콘으로 추가

footer의 강조 동작은 `새 아이콘으로 추가 · 권장`이다.

- base-source 후보는 현재 저장된 icon/piece/shape/crop/transform/text/effect/motion
  recipe를 복제하고, 규격화 결과를 child AI version으로 활성화한다.
- 새 icon은 빈 alt, `작업중` readiness로 만든다.
- stale 후보도 raw를 버리지 않고 현재 저장 상태를 기준으로 규격화 preview를
  다시 만든 뒤 새 아이콘으로 추가할 수 있다.
- static/animated 또는 input-stage가 안전하게 복제되지 않는 경우에는 이유를
  표시하고 첫 MVP에서 차단한다. 조용히 다른 모드로 바꾸지 않는다.

성공 뒤 dialog는 닫히지 않고 outcome panel을 보여준다.

```text
새 아이콘을 추가했습니다.
작업중 상태이며 export 전에 alt를 입력해야 합니다.
[새 아이콘 열기] [목록에서 보기] [계속 후보 비교]
```

- `새 아이콘 열기`: dialog를 닫고 새 tile을 선택·scroll한 뒤 편집 패널을 연다.
- `목록에서 보기`: dialog를 닫고 새 tile을 선택·scroll·focus한다.
- `계속 후보 비교`: source icon의 작업공간에 남는다.
- 같은 후보로 이미 만든 icon이 있으면 `이 후보로 하나 더 추가`라고 표현하고
  기존 생성 icon link/count를 보여 의도하지 않은 중복 생성을 줄인다.

### 5.5 현재 아이콘에 사용

`현재 아이콘에 사용`은 footer에 보이는 secondary action으로 둔다. `고급 동작`
안에 숨기지 않는다.

허용 조건:

- candidate input stage가 `base_source`
- raw와 현재 effective source가 모두 정적이거나 명시적으로 지원되는 같은
  animation kind
- 규격화 결과가 target base-source canvas와 정확히 일치
- original lineage/generation, activation revision과 전체 native recipe가
  preview 시점과 동일
- candidate가 현재 icon 적용 관점에서 stale하지 않음

버튼 가까이에 “현재 crop·효과는 유지되며 원본과 이전 AI source로 언제든
돌아갈 수 있습니다”를 표시한다.

적용 성공 뒤 outcome:

```text
현재 아이콘이 AI 소스를 사용 중입니다.
[편집기로 돌아가기] [원본으로 돌아가기] [소스 이력 보기]
```

현재 아이콘 적용이 불가능해도 `새 아이콘으로 추가`까지 함께 잠그지 않는다.
각 동작의 호환성을 별도로 계산하고 가시적 이유를 표시한다.

### 5.6 소스 이력과 복귀

- 원본을 timeline 첫 항목으로 고정한다.
- AI version은 최신순으로 보여 주되 `현재 소스`를 text와 icon으로 함께 표시한다.
- 각 항목에는 raw 출처, 규격화 mode, canvas, 날짜를 표시한다.
- `이 소스로 전환`과 `원본으로 돌아가기`는 공급자를 호출하지 않는다고 명시한다.
- 전환 성공 후 dialog의 review state와 편집기 state를 같은 mutation 결과로
  갱신한다.

## 6. 결정적 후보 정규화 계약

### 6.1 target canvas

첫 MVP의 `base_source` 후보 target은 적용 동작 시점의
`EffectiveVisualSource.effective_render_source` width/height다. 이는 export cell
크기가 아니라 crop metadata가 참조하는 base-source canvas다.

- current icon 적용: 현재 effective canvas에 맞춘다.
- base-source 새 icon: source icon의 현재 저장 recipe를 복제하므로 같은 canvas에
  맞춘다.
- 무입력 text-to-image와 `rendered_viewport`/`gif_poster` root의 target 규칙은
  provider 단계에서 별도 추가한다.

### 6.2 UI options와 canonical recipe

frontend가 편집하는 값과 backend가 저장하는 canonical recipe를 분리한다.

```ts
type AiNormalizationOptions = {
  mode: "contain_pad" | "cover_crop";
  alignment:
    | "top_left" | "top" | "top_right"
    | "left" | "center" | "right"
    | "bottom_left" | "bottom" | "bottom_right";
  resizeFilter: "lanczos3" | "nearest";
  padRgba: [number, number, number, number];
};
```

backend가 source와 target을 조회해 만드는 저장 형식:

```json
{
  "schema": "pmtcon-ai-normalization-v1",
  "kind": "contain_pad",
  "rawSourceFileId": "source-...",
  "rawSourceSha256": "...",
  "providerNativeWidth": 1024,
  "providerNativeHeight": 768,
  "targetCanvasWidth": 640,
  "targetCanvasHeight": 640,
  "alignment": "center",
  "resizeFilter": "lanczos3",
  "padRgba": [0, 0, 0, 0],
  "outputFormat": "png"
}
```

source와 target 크기가 같고 실제 pixel 변환이 필요 없으면 backend가 canonical
`identity` recipe로 축약하고 raw source를 effective source로 재사용할 수 있다.
client가 source ID/SHA, target 또는 output path를 결정하지 않는다.

### 6.3 pixel math

동일 입력이 platform과 실행 시점에 관계없이 같은 geometry를 만들도록 정수
계산 규칙을 고정한다.

- contain: 비율을 유지하며 target 안에 들어오는 가장 큰 크기를 사용한다.
  제한 축은 target과 정확히 맞추고 다른 축은 round-half-up 후 target을 넘지
  않게 clamp한다.
- cover: 비율을 유지하며 target을 모두 덮는 가장 작은 크기를 사용한다.
  비제한 축은 ceil해 1px 빈틈이 생기지 않게 한다.
- center의 홀수 잔여 pixel은 오른쪽/아래쪽에 하나 더 둔다.
- alignment는 남는 padding 또는 crop offset에 동일한 0/0.5/1 의미로 적용한다.
- resize와 RGBA 합성은 기존 native image pipeline에서 수행한다. CSS preview가
  저장 결과의 기준이 되어서는 안 된다.

규격화 static output은 PNG로 저장한다. 이는 transparent padding과 alpha를
손실 없이 보존하기 위한 source 형식이며, 최종 export format/용량 최적화와는
별개다.

### 6.4 alpha와 경고

- raw의 `has_alpha`는 실제 decoded pixel 기준이다.
- transparent padding은 바깥 여백만 투명하게 만들 뿐, AI가 그린 불투명 배경을
  제거하지 않는다.
- opaque raw에서 “배경 투명화됨”이라고 표시하지 않는다.
- cover crop에서 중요한 content가 잘릴 수 있음을 overlay와 경고로 표시한다.
- alpha가 없는 target profile이나 JPG export의 flatten은 이 normalization
  단계가 아니라 기존 export pipeline에서 처리·검증한다.

### 6.5 preview와 commit

preview는 선택 후보에 대해서만 지연 생성한다.

```ts
type AiNormalizationPreview = {
  candidateId: string;
  rawSource: SourceFileSummary;
  targetCanvasWidth: number;
  targetCanvasHeight: number;
  normalizedPreviewUrl: string;
  finalRenderPreviewUrl: string | null;
  recipe: AiNormalizationRecipeV1;
  recipeHash: string;
  previewSignature: string;
  currentIconCompatibility: {
    allowed: boolean;
    reasonCode: string | null;
    reasonText: string | null;
  };
  newIconCompatibility: {
    allowed: boolean;
    reasonCode: string | null;
    reasonText: string | null;
  };
  warnings: Array<{
    code: string;
    severity: "info" | "warning";
    message: string;
  }>;
};
```

`previewSignature`에는 candidate raw ID/SHA, canonical recipe hash, target canvas,
lineage/generation, activation revision과 전체 native recipe signature를 포함한다.
이는 authorization token이 아니라 “사용자가 본 결과와 commit 입력이 같은가”를
검사하는 stale token이다.

apply/create command는 client가 보낸 path나 크기를 신뢰하지 않는다. options와
expected preview signature를 받아 source/target/recipe/signature를 backend에서
다시 계산한다. 다르면 `미리보기가 오래되었습니다`로 거부하고 최신 preview를
다시 만들게 한다.

preview artifact는 source of truth가 아니며 bounded temporary path에 둔다. commit
때 같은 recipe로 normalized PNG bytes를 다시 만들고 SHA-256을 계산해
content-addressed `source_files`에 등록한다. raw source는 candidate에 그대로
남고 normalized source는 `icon_ai_versions.effective_source_file_id`가 참조한다.
정규화와 native preview render는 shared SQLite lock과 final transaction 밖의
request-owned staging에서 먼저 수행한다. 짧은 final CAS transaction만 pointer,
version, durable path를 commit하며 transaction 또는 file promotion이 실패하면
기존 AI activation compensation protocol을 따른다.

### 6.6 recipe별 version identity

같은 candidate를 한 번 사용했다는 이유만으로 다른 맞춤 방법까지 막지 않는다.
current icon lineage 안에서 다음 조합을 materialization identity로 사용한다.

```text
icon ID
+ original lineage ID/generation
+ candidate ID
+ normalization recipe hash
```

- 같은 조합이 이미 있으면 새 version을 만들지 않고 저장된 version을 다시 선택한다.
- 같은 candidate라도 `contain_pad`와 `cover_crop` 또는 alignment가 다르면 서로
  다른 version을 만들 수 있다.
- candidate 단위 `isMaterialized` 하나로 모든 현재 적용을 막는 기존 파생값은
  recipe별 existing version/count로 바꾼다.
- 새 icon 생성 이력은 현재 icon version materialization과 구분한다. 후보 card에
  이 후보로 만든 같은 collection의 icon count와 최신 non-deleted icon link를
  제공한다.
- 손상되거나 파일이 사라진 비활성 candidate/version은 목록에서 조용히 빼지 않고
  `사용할 수 없는 이력`으로 표시한다. active source 손상은 기존 fail-closed repair
  흐름을 유지한다.

## 7. frontend 구성과 상태

```text
AiSourceSummary
└─ AiWorkspaceDialog
   ├─ AiWorkspaceHeader
   ├─ AiWorkspaceTabs
   ├─ AiImportResultPanel
   ├─ AiCandidateRail
   ├─ AiCandidateCompareStage
   │  ├─ AiCompareToolbar
   │  ├─ CheckerboardPreview
   │  └─ NormalizationOverlay
   ├─ AiNormalizationInspector
   ├─ AiVersionHistory
   ├─ AiOutcomePanel
   ├─ AiWorkspaceActionBar
   └─ AiWorkspaceStatusRegion
```

server state와 dialog draft를 분리한다.

```ts
type AiWorkspaceState = {
  view: "import" | "review" | "history";
  phase: "idle" | "loading" | "previewing" | "mutating";
  selectedCandidateId: string | null;
  compareView: "original" | "raw" | "normalized" | "final" | "overlay";
  normalizationOptions: AiNormalizationOptions;
  normalizationPreview: AiNormalizationPreview | null;
  outcome:
    | null
    | { type: "icon_created"; icon: IconSummary }
    | { type: "source_changed"; message: string };
};
```

`useAiWorkspaceController` + `useReducer`로 충분하며 durable state를 Zustand에
복제하지 않는다. candidate/version의 권위값은 backend `AiReviewState`다.

현재 `createAiIconRoot` 응답의 새 icon `reviewState`를 source icon state로 오해해
버리지 않는다. source icon은 생성으로 바뀌지 않으므로 불필요한 재조회 대신
mutation 결과와 새 icon DTO를 outcome에 보존한다.

현재 icon activation/rollback command는 다음처럼 한 commit 결과를 반환하도록
확장한다.

```ts
type AiSourceMutationResult = {
  reviewState: AiReviewState;
  editorState: IconEditorState;
};
```

이렇게 하면 child AI UI는 성공했지만 `EditorPanel` 재조회가 실패해 서로 다른
상태를 보여 주는 문제를 없앨 수 있다. route list update가 별도로 실패한 경우에는
`저장은 완료됐지만 목록 표시를 새로 고치지 못했습니다`와 `목록 새로고침`을
보이고 mutation 자체를 실패로 표현하지 않는다.

collection route와 `IconGrid`에는 명시적인 reveal request를 추가한다.

```ts
type IconRevealRequest = {
  iconId: string;
  action: "focus_tile" | "open_editor";
  requestId: number;
};
```

grid는 대상 tile을 select하고 `scrollIntoView({ block: "nearest" })` 후 focus한다.
`open_editor`는 같은 작업 뒤 해당 `EditorPanel`을 연다.

## 8. provider 확장 시 유지할 흐름

후보 검토 이후의 UI와 normalization/activation contract는 provider와 무관하다.
공급자 단계에서는 첫 탭 앞부분만 확장한다.

### NovelAI adapter가 구현된 뒤

1. 작업 종류: 새 이미지, 현재 base 이미지 수정, mask 영역 수정
2. prompt와 지원 옵션
3. 전송할 실제 이미지/mask/reference, model, 해상도, 예상 provider unit,
   약관·개인정보·권리 확인
4. 사용자의 한 번의 `1장 생성` 동작
5. 비활성 candidate 생성 후 같은 `후보 검토`로 이동

Persistent API Token은 별도 session 연결 sheet에서 입력하고 invoke 전달 즉시
frontend input state를 비운다. token을 화면에 다시 보여주거나 DB에 저장하지
않는다. 연결 상태에는 `이번 실행 동안만`, `세션 연결 해제`와 runtime client
위험을 표시한다. `401`, `429`, cancel과 retry는 각각 독립 상태이며 자동 retry나
provider fallback을 하지 않는다.

### 수동 웹 handoff가 구현된 뒤

1. 공식 service surface와 account context 선택
2. exact-send package와 prompt/정책 link 확인
3. prompt 복사와 공식 사이트 열기
4. 사용자가 직접 로그인·upload·생성·download
5. 결과 가져오기 후 같은 candidate review

브라우저 자동화가 없다는 점과 model/cost가 검증되지 않을 수 있음을 정상 상태로
표현한다.

## 9. 접근성과 상태 알림

- dialog에 `role="dialog"`, `aria-modal="true"`, 제목과 설명 연결
- 기존 `use-modal-focus` 경계를 재사용해 focus trap, Escape close와 닫힌 뒤
  `AI로 수정` 버튼 focus 복원
- 탭은 tab/tabpanel keyboard pattern을 따른다.
- 후보 rail은 radiogroup 또는 single-select listbox로 구현한다.
- 후보 accessible name에 번호, 파일명, 출처, 날짜를 포함한다.
- 상태 성공 알림은 dialog 전체에 단 하나의 `aria-live="polite"` 영역을 둔다.
  동일 문장을 AI section, EditorPanel과 collection route에서 동시에 알리지 않는다.
- 오류도 dialog 안의 한 alert 영역에 합치고, field 오류는 해당 입력과
  `aria-describedby`로 연결한다.
- 비활성 이유를 `title`에만 두지 않고 버튼 가까운 visible text로 표시한다.
- 선택, stale, 활성 상태는 색상 외 text/icon을 함께 사용한다.
- 최소 40px pointer target과 명확한 focus ring을 사용한다.
- overlay/zoom/alignment는 keyboard 대체 조작을 제공한다.
- `prefers-reduced-motion`에서는 dialog transition과 비교 animation을 제거한다.

## 10. 오류와 복구 문구

공용 cover import 문구를 재사용하지 않고 AI 작업 맥락의 error code/message를
사용한다.

| 상황 | 사용자 문구 |
|---|---|
| decode 실패 | `AI 후보 이미지를 읽을 수 없습니다. 손상되지 않은 JPG/PNG인지 확인해 주세요.` |
| 용량 초과 | `AI 후보 이미지는 최대 16MB까지 가져올 수 있습니다.` |
| pixel workload 초과 | `AI 후보 이미지 해상도가 안전 처리 한도를 넘었습니다.` |
| GIF 선택 | `현재 AI 후보는 정적 JPG/PNG만 지원합니다. GIF는 프레임 작업 단계에서 지원할 예정입니다.` |
| stale preview | `아이콘의 원본 또는 편집 설정이 바뀌어 미리보기가 오래되었습니다.` |
| incompatible current apply | `이 후보는 현재 아이콘에 바로 사용할 수 없습니다. 새 아이콘으로 추가할 수 있습니다.` |
| mutation saved/list refresh failed | `변경은 저장됐지만 아이콘 목록 표시를 새로 고치지 못했습니다.` |

사용자가 해결할 수 없는 `AI 소스 복구 필요`는 일반 후보 dialog 대신 별도 repair
state를 우선 표시하고, 원본 복구나 이력 정리 선택 전에는 render/export를 막는
기존 fail-closed 원칙을 유지한다.

## 11. 구현 순서

### AI-UX-1 — 후보 정규화 MVP

상태: 구현 완료 (2026-07-27). 후보 정규화·적용 기반은 AI-UX-1 범위이며,
전용 작업공간 이동은 아래 AI-UX-2에서 구현했다.

- local JPG/PNG candidate를 raw 크기 그대로 받아 보존
- `contain_pad`/`cover_crop`, 3×3 alignment, Lanczos3/Nearest, transparent pad
- native normalized preview와 canonical recipe/hash/signature
- raw/normalized source 분리, current/new-icon compatibility와 AI 전용 오류
- 현재 activation/new-icon transaction에 normalized immutable source 연결
- 실제 final/piece 크기와 commit-parity 용량 검증, recipe별 이력 요약
- 손상된 비활성 이력의 사용 불가 표시와 no-follow/보상 정리

### AI-UX-2 — 전용 작업공간

상태: 구현 완료 (2026-07-27).

- compact `AiSourceSummary`와 큰 앱 내부 `AiWorkspaceDialog`
- `결과 가져오기`, `후보 검토`, `소스 이력`의 세 구현 탭
- 후보 rail과 원본/raw/normalized/final/overlay 비교, 화면 맞춤/100%,
  checkerboard
- 고정 header·탭·status·footer와 1200×760 및 1024px 미만 반응형 배치
- `role="dialog"`, Escape 닫기와 진입 버튼 focus 복원의 기본 dialog 경계
- 일반 적용 동작을 `크롭·변형 적용`으로 구분

### AI-UX-3 — 완료 연속성과 접근성

상태: 구현 완료 (2026-07-28).

- 새 icon outcome에서 `새 아이콘 열기`, `목록에서 보기`, `계속 후보 비교`를
  제공하고, 직접 생성 provenance에 근거한 반복 생성 수와 최신 icon을 표시한다.
- activation/rollback은 review/editor state를 같은 transaction 결과로 반환하며
  성공 뒤 별도 GET으로 상태를 재구성하지 않는다.
- typed reveal request가 tile 선택·스크롤·focus와 선택적 editor 열기를 연결하고,
  미저장 변경 승인 또는 busy 차단이 실패하면 modal과 focus를 그대로 유지한다.
- 중첩 modal은 최상위 하나만 Escape/Tab을 처리한다. AI modal이 열리면
  EditorPanel·ExportDialog·route·alt 경고·dnd-kit 안내를 포함한 배경 live
  region을 억제해 문서 전체 status/alert를 정확히 하나로 유지한다.
- reduced-motion, 1200×760/800×760 overflow, keyboard/focus, 생성 3회,
  반복 수 1→2→3, reveal/open, activation/restore 후속 GET 0, 중첩 Export
  Escape와 예상 밖 command/network 0을 headed browser QA 13/13으로 검증했다.

NovelAI 생성과 token/prompt/consent UI는 AI-UX-3에 포함하지 않는다. 수동 웹
handoff와 NovelAI API는 각각 F138/F139의 별도 provider Stage Gate에서 구현하며,
그 전에는 작업공간에 생성 UI를 노출하지 않는다.

## 12. 수용 기준

- 1024×768 또는 1024×1024 JPG/PNG를 200×200이 아닌 source icon에 가져와도
  동일 크기 오류 없이 raw 후보가 보존된다.
- contain+pad와 cover+crop 결과를 적용 전에 큰 화면에서 원본/AI 원본과 비교한다.
- raw와 normalized source SHA가 구분되고 저장된 version recipe/hash로 결과를
  재현한다.
- transparent padding과 실제 background removal을 혼동시키지 않는다.
- preview 뒤 source/recipe/revision이 바뀌면 commit을 막고 다시 preview하게 한다.
- 1200×760에서 footer action이 잘리거나 기존 편집 footer와 겹치지 않는다.
- stale 후보는 현재 icon 적용만 명확한 이유로 막고, 재규격화 후 새 icon 생성은
  유지한다.
- 새 icon 생성 성공 뒤 한 번의 동작으로 새 tile을 열거나 목록에서 찾는다.
- 동일 후보로 이미 만든 icon이 있으면 반복 생성임을 명확히 알린다.
- 현재 source 전환 뒤 review/editor state가 한 mutation 결과와 일치한다.
- keyboard만으로 dialog 진입, 후보 선택, 비교, 규격화 선택, 새 icon 생성,
  원본 복귀와 닫기가 가능하다.
- 자동 adapter가 구현되기 전에는 생성, token 연결 또는 비용 menu가 노출되지 않는다.
- 기존 원본, AI candidate/version, rollback, clone, cleanup과 effective-source
  회귀 테스트가 계속 통과한다.

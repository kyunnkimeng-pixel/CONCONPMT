# PMTCONCON Studio 편집 완성도 확장 설계

작성일: 2026-07-24

## 1. 범위

이 문서는 다음 사용자 승인 범위를 단계별로 구현하기 위한 설계 기준이다.

1. 초기화 의미와 기존 기능 발견성 개선
2. 좌우/상하 반전과 90도 회전
3. 임의 프레임 시트에서 새 GIF 만들기
4. 픽셀화와 검증된 내장 효과
5. 컬렉션 복제 완전성 보강

각 항목은 독립 Stage Gate를 통과한 뒤 다음 항목으로 진행한다. 아직 구현되지 않은
항목은 메뉴에 먼저 노출하지 않는다.

## 2. 현재 기반

이미 구현되어 재사용할 기반은 다음과 같다.

- 고정 grid, 직접 Slice, 자동 감지를 포함한 정적 시트 가져오기
- GIF 전체 프레임 작업 시트 내보내기와 manifest 기반 재조립
- GIF 프레임 timing/loop 보존과 ping-pong export
- React Konva crop canvas와 Rust `image`/`gif` 렌더 파이프라인
- 아이콘별 텍스트 overlay와 용량 최적화 variant
- 컬렉션 복제와 아이콘 메모

새 기능은 이 기반 위에 좁게 추가하며 범용 페인팅·레이어 편집기를 별도로 만들지
않는다.

## 3. 외부 편집기 참고 원칙

### Aseprite

Aseprite에서 참고할 것은 사용자 흐름과 정보 구조뿐이다.

- Sprite Sheet Import는 시작 X/Y, sprite 크기, padding, sheet type을 분리한다.
- sheet type이 셀 읽기 순서를 결정하며 이 원칙은 정적 이모티콘 묶음과 animation
  frame 양쪽에 통용된다.
- sheet export는 대상과 배치 방향을 명시하며 CLI 문서에는 rows, columns,
  horizontal, vertical, packed 배치와 sheet 크기, 행/열 수, border/shape/inner
  padding, 빈 셀 제외, JSON metadata가 서로 다른 옵션으로 정리되어 있다.
- 가져온 프레임은 timeline에서 순서와 frame duration을 편집한다.
- 편집 중 animation preview를 계속 볼 수 있다.
- 여러 frame을 선택해 duration이나 transform을 함께 적용할 수 있다.
- export 방향은 forward, reverse, ping-pong을 구분한다.

PMTCONCON Studio는 Aseprite의 layer×frame×cel 전체 모델을 복제하지 않는다.
이미 구현된 정적 시트 흐름에는 offset/cell/padding/order 검토 방식을 적용하고,
프레임 시트에서 GIF 하나를 만드는 흐름에는 한 줄 frame strip만 추가한다.

정적 이모티콘 시트에 적용하는 참고 범위:

- 가져오기: 시작 X/Y, cell 너비/높이, 가로/세로 간격, 행·열 수, 행/열 우선
  읽기 순서, 번호 overlay, 빈 셀 후보 제외
- 내보내기: 선택한 아이콘과 piece를 영속화된 순서로 배치하고 행·열/page 크기를
  명시
- 재가져오기: 편집용 clean sheet와 사람이 확인하는 guide sheet를 분리하며
  manifest로 원래 icon/piece ID를 안전하게 연결
- packed 자동 배치는 원위치 재가져오기와 충돌하기 쉬우므로 첫 흐름에는 사용하지
  않고, 향후 추가하더라도 manifest를 필수로 한다.

GIF 프레임 시트에 적용하는 참고 범위:

- 같은 분할 입력을 재사용한 뒤 각 셀을 frame으로 해석
- frame strip에서 포함 여부, 순서, duration과 반복을 편집
- 실제 생성 결과의 frame 수, 총 재생시간과 byte 크기를 확인한 뒤 새 GIF로 등록

Aseprite 본체 source와 공식 binary는 Aseprite EULA 대상이다. 본체 코드, binary,
CLI, icon, cursor, theme, screenshot, sample asset을 복사·번들·port하지 않는다.
공식 사용자 문서에서 동작 원칙만 확인하고 PMTCONCON Studio의 데이터 모델,
한국어 문구, 레이아웃, 아이콘, 테스트를 독자 구현한다.

공식 참고:

- https://www.aseprite.org/docs/sprite-sheet/
- https://www.aseprite.org/docs/cli/
- https://www.aseprite.org/docs/timeline/
- https://www.aseprite.org/docs/frame-duration/
- https://www.aseprite.org/docs/preview-window/
- https://www.aseprite.org/docs/transformations/
- https://www.aseprite.org/docs/exporting/
- https://github.com/aseprite/aseprite#license
- https://raw.githubusercontent.com/aseprite/aseprite/main/EULA.txt

### 허용 라이선스 참고 프로젝트

기능 분류와 UX 참고 대상으로만 사용한다. dependency 추가는 별도 license gate를
거친다.

- Pixelorama: MIT, frame/layer timeline과 non-destructive outline/gradient
  map/drop shadow
- Piskel: Apache-2.0, 단순한 sprite animation workflow
- miniPaint: MIT, 일반적인 색상 조정과 filter 분류
- TOAST UI Image Editor: MIT, crop/flip/rotate 및 일반 filter 분류
- Filerobot Image Editor: MIT, original comparison과 undo/redo/reset 구조

기존 `image`, `gif`, `react-konva`로 구현 가능한 효과는 외부 편집기 전체를
의존성으로 추가하지 않고 독자 구현한다.

## 4. 공통 제품 원칙

1. 원본 파일은 절대로 덮어쓰지 않는다.
2. 편집 설정은 재시작 후에도 유지되며 preview와 export가 같은 recipe를 사용한다.
3. Konva/CSS는 미리보기 전용 기준이 아니다. 최종 기준은 Rust renderer다.
4. GIF 효과는 모든 frame에 같은 recipe를 적용하고 기존 frame delay와 disposal을
   가능한 범위에서 보존한다.
5. GIF를 새로 생성하거나 효과를 적용할 때 frame 수, 총 재생시간, 실제 측정 용량을
   보여준다.
6. 기본 편집에는 자주 쓰는 기능만 보이고 효과와 animation 기능은 맥락에 맞게
   펼친다.
7. 실행 코드를 포함하는 외부 효과 plugin은 지원하지 않는다.
8. frame strip과 effect panel은 필요할 때 lazy-load해 초기 explorer bundle을
   키우지 않으며, 범용 외부 편집기 전체를 두 번째 app shell로 포함하지 않는다.

## 5. Stage 1 — 초기화 의미와 발견성

현재 `초기화`는 전체 편집값이 아니라 draft crop을 기본 위치와 크기로 되돌린다. 다음처럼
동작 범위를 이름에 표시한다.

| 동작 | 문구 | 확인 |
|---|---|---|
| crop draft를 기본 위치와 크기로 | `크롭 기본값` | 없음 |
| 이번 편집 draft를 저장값으로 복원 | `저장값으로 되돌리기` | 없음 |
| 댓글 미리보기 내용을 비움 | `미리보기 비우기` | 없음 |
| 시트 분할 draft를 앱 초기값으로 | `분할 설정 초기값` | 없음, 분석 결과는 무효화 |
| 미래의 시각 편집 recipe 전체 초기화 | `이 아이콘의 시각 편집 초기화…` | 무엇을 유지/삭제하는지 명시 후 확인 |
| 미래의 앱 데이터 전체 초기화 | 설정의 별도 위험 구역 | 강한 확인과 백업 안내 |

아이콘명, alt, 메모, 순서와 원본 파일은 시각 편집 초기화 대상이 아니다.

발견성은 새 도구막대를 더 쌓는 대신 다음으로 개선한다.

- 앱 sidebar에 공개 사용 설명서 진입점 제공
- 시트에서 가져오기/시트로 내보내기 버튼에 결과가 무엇인지 설명하는 tooltip 제공
- 컬렉션 복제 버튼에 복제 범위를 설명하는 tooltip 제공
- 상세 아이콘 tile에 메모 추가/수정 버튼을 항상 제공하고 기존 메모는 hover/focus로 확인
- 좁은 command bar에서도 버튼 문구가 세로로 쪼개지지 않게 줄 단위로 wrap

수용 기준:

- 네 종류의 초기화/복원 문구가 서로 다른 범위를 정확히 표현한다.
- 사용 설명서를 시스템 기본 browser로 열 수 있고 실패 시 한국어 오류가 보인다.
- tooltip만 읽어도 시트에서 가져오기와 시트로 내보내기의 차이를 알 수 있다.
- 메모가 없는 아이콘에서도 tile에서 바로 메모 추가 dialog를 열 수 있다.
- frontend lint, test, build가 통과한다.

## 6. Stage 2 — 비파괴 기본 변형

첫 MVP:

- 좌우 반전
- 상하 반전
- 왼쪽 90도
- 오른쪽 90도

데이터는 icon별 비파괴 transform metadata로 저장한다. preview/export hash에
transform을 포함해 stale variant를 재사용하지 않는다.

내부 recipe는 90도 회전 수 `0..3`과 canonical 좌우 반전으로 8개의 서로 다른
시각 상태만 저장한다. 사용자는 좌우/상하 반전 버튼을 모두 사용할 수 있지만
동등한 조합은 같은 canonical 상태와 같은 variant hash가 된다.

렌더 순서는 `텍스트 overlay → source crop → 변형 전 viewport 크기로 resize →
viewport 전체 회전·반전 → piece 분할`로 고정한다. 따라서 다중콘의 경계에서
각 piece가 따로 뒤집히지 않는다.

GIF에서는 모든 frame에 같은 recipe를 적용하고 frame timing과 loop를 보존한다.
변형이 하나라도 있으면 원본 GIF passthrough를 사용하지 않는다. 미래 frame
strip에서 일부 frame만 선택하는 기능을 추가할 때는
`현재 프레임 / 선택 N개 / 전체 프레임` scope를 버튼 근처에 표시한다.

90도 회전 정책:

- 단일 정사각 cell은 그대로 회전한다.
- 비정사각 custom cell은 width/height를 교환한다.
- 가로 2칸과 세로 2칸은 회전 시 shape을 함께 교환한다.
- piece ID와 alt는 화면에서 움직인 시각 내용에 따라 이동하고, `piece_index`와
  piece role은 회전 후 출력 위치를 나타낸다.
- crop 좌표는 원본 기준으로 유지하고 홀수 quarter-turn에서는 역변환한 viewport
  너비/높이를 사용한다.
- 원본 교체는 기존 crop과 transform을 기본값으로 되돌린다는 사실을 적용 직후
  명시하고 새 current/piece preview를 즉시 재생성해 이전 원본 조각이 남지 않게
  한다.
- 고급 텍스트 편집은 변형 전 원본 좌표를 사용한다는 안내를 표시하고, 저장 결과
  및 출력 후보에는 현재 transform을 적용한다.
- GIF 프레임 시트 manifest는 source hash와 당시 render-recipe hash를 함께
  기록한다. 시트 내보내기 뒤 원본 또는 recipe가 바뀌면 재가져온 파일은 보존하되
  현재 export의 active variant로 연결하지 않는다.
- 임의 각도 회전은 투명 여백과 보간 정책이 필요하므로 MVP에서 제외한다.

## 7. Stage 3 — 프레임 시트로 GIF 만들기

Aseprite에서 참고한 흐름을 PMTCONCON Studio 목적에 맞게 줄인다.

### 1단계: 프레임 나누기

- 파일 선택
- 시작 X/Y, cell width/height, gap/padding, 행/열
- 읽기 순서: 행 우선, 열 우선, 역순
- 번호 overlay와 빈 셀 후보 제외
- 포함/제외 검토

현재 Sheet Import 분석과 cell review UI를 재사용한다.

### 2단계: 애니메이션 조정

- 한 줄 frame strip
- Ctrl/Shift 선택과 drag reorder
- 선택 frame 복제/삭제/순서 뒤집기
- frame별 duration(ms)
- 선택 frame duration 일괄 변경
- 전체 동일 FPS 입력은 duration 일괄 입력의 편의 기능으로만 제공
- 반복: 한 번, 무한, 지정 횟수
- 생성 방향: 정방향, 역방향, ping-pong. 첫 MVP는 역방향/핑퐁 순서를 생성
  frame sequence에 반영하며 독립적인 재사용 `direction` 필드를 약속하지 않는다.
- sticky realtime preview

### 3단계: GIF 만들기

- frame 수, 총 재생시간, 출력 크기 표시
- 실제 GIF를 생성해 byte size 측정
- DCInside 2MB 초과 경고
- 원본 sheet를 library에 보존
- 생성 GIF를 새 source file과 새 icon으로 등록

Layer, cel, onion skin, animation tag, 직접 pixel painting은 MVP에서 제외한다.

## 8. Stage 4A — 정적 내장 효과

여러 허용 라이선스 편집기에서 반복적으로 보이는 효과와 이모티콘 실사용 가치를
함께 기준으로 첫 효과군을 정한다.

현재 상태는 구현 및 Stage Gate 검증 완료다. 7종 효과의 정적/GIF/다중콘 공용
렌더 경로, revision 충돌, 요청별 preview/save artifact, 복제 독립성, 작업 시트
일치성을 Rust 131개와 frontend 106개 테스트로 확인했다.

| 효과 kind | 사용자 기능 | 첫 단계 parameter |
|---|---|---|
| `pixelate` | 픽셀화 | block size |
| `color_adjust` | 밝기·대비·채도·색조 | brightness, contrast, saturation, hue |
| `tone` | 흑백/세피아 | mode, amount |
| `blur` | 블러 | radius |
| `sharpen` | 선명화 | amount |
| `outline` | 윤곽선 | radius, RGBA color |
| `shadow` | 그림자 | X/Y offset, blur radius, RGBA color |

아이콘마다 `pmtcon-effects-v1` ordered JSON recipe와 revision을 저장한다. 각
step은 stable ID, enabled 상태와 bounded parameter를 가지며, enabled step의
순서는 실제 렌더 순서다. 마지막으로 읽은 revision과 다른 저장은 stale edit로
거부하고 사용자가 최신 저장값을 다시 불러오게 한다.

authoritative render order에서 정적 효과는 crop/resize와 whole-viewport transform
뒤, piece split 전에 결합 viewport 전체에 적용한다. 정적 이미지와 GIF의 모든
frame에 같은 recipe를 적용하며 GIF frame delay와 loop 설정을 가능한 범위에서
보존한다. 윤곽선과 그림자를 piece별로 적용해 가운데 이음새가 생기게 해서는 안
된다.

Rust native renderer가 exact preview, generated preview와 export의 기준이다.
optimizer 후보 분석, 정적 작업 시트, GIF frame 작업 시트의 source/render recipe
hash에도 정규화된 effect recipe를 포함해 stale artifact를 활성화하지 않는다.
이 단계는 기존 `image`, `gif`, `react-konva` 기반을 재사용하고 새 effect
dependency나 외부 editor runtime을 추가하지 않는다.

## 9. Stage 4B — 필수 motion 효과

상태: 구현 완료. 모션은 선택적인 장기 후보가 아니라 내장 효과 완성 범위의
필수 기능이며, 아이콘별 revision이 있는 `pmtcon-motion-v1` recipe로 저장된다.
정적 효과와 별도 Stage로 검증해 timing, loop, clipping, palette와 실제 용량의
실패 범위를 분리했다. 전체 검증 명령과 최종 개수는 Stage Gate 결과에 기록한다.

사용자에게는 16개 preset으로 보이되 네 개의 bounded renderer 범주를 공유한다.

| 범주 | 첫 preset | 주요 parameter |
|---|---|---|
| 공간 변형 | 흔들기, 통통 튀기, 두근/호흡, 까딱 회전, 회전 | X/Y 진폭, scale, angle/turns, pivot, cycles |
| procedural displacement | 가로/세로 사인파 물결, 2축 젤리 일렁임, 방사형 리플, 제한된 글리치 밴드 | axis/center, amplitude, spatial waves/bands, phase, seed, interpolation, edge mode |
| 색상·불투명도 | 색조 순환, 지정색 박동, 밝기·채도 박동, 번쩍임 | hue turns, target color, mix, brightness/saturation, opacity |
| overlay | 집중선, 반짝이, 확산 링 | center, count, size/width, color, opacity, seed |

MVP는 범주별 최대 한 효과만 활성화한다. 합성 순서는 `저장된 정적 효과 결과 →
공간 변형 → procedural displacement → 색상·불투명도 → overlay`로 고정해 임의
stack 순서가 만드는 조합 폭발과 preview/export 차이를 피한다. 전체 motion은
다중콘 결합 viewport에 적용한 뒤 piece를 분할한다.

반복 출력은 normalized phase와 정수 cycles-per-loop를 사용하고 반복 sequence는
끝점 frame을 중복하지 않는다. 1회 출력은 endpoint를 포함해 기본 자세로 끝나게
한다. 정적 입력은 duration과 FPS로 새 GIF를 만들며, 기존 GIF는 frame index가
아니라 누적 frame delay timestamp를 기준으로 phase를 계산한다. 원본 GIF의 frame
delay와 effective loop를 보존한다.

흔들림과 particle overlay는 recipe에 저장한 seed와 frame phase/particle ID로
계산하는 stateless deterministic random을 사용한다. displacement는 forward
pixel scattering이 아닌 inverse sampling으로 구현하며 bilinear interpolation은
premultiplied alpha에서 수행한다. pixel art용 nearest와 transparent/clamp/mirror
중 bounded edge mode를 제공한다.

편집기의 lazy-loaded 모션 탭은 duration, FPS, frame 수, cycles, 진폭, effective
loop, 잘린 frame, 재생/일시정지와 OS reduced-motion 상태를 보여준다. 사용자는
현재 recipe로 `GIF 미리보기·용량 측정`을 실행해 native GIF와 전체/piece별 실제
byte를 확인한 다음 같은 render signature로 저장한다. 입력·revision이 바뀌면
측정값을 즉시 무효화한다. 표시 용량은 편집 preview 기준이며 최종 export에서는
profile과 optimizer를 적용한 뒤 다시 검증한다.

editor preview, export, optimizer, GIF frame 작업 시트는 같은 motion recipe를
사용한다. 정적 작업 시트는 의도적으로 0ms poster frame 한 장만 내보내고 animation
손실 warning을 반환한다. static manifest의 `render_recipe_hash`에 motion을 포함해
가공본 적용/교체 모드의 stale cell을 건너뛰고 적용하지 않는다. 모든 frame, duration과
loop를 왕복하려면 GIF frame 작업 시트를 사용한다.

무거운 render는 DB snapshot 뒤 shared SQLite lock을 해제하고, encoding 완료 후
revision과 input signature를 다시 확인해 commit한다. 요청 preview와 이전 motion
artifact는 bounded/reference-safe cleanup을 적용한다. 아이콘과 컬렉션 복제는
motion recipe를 보존하되 mutable preview ownership을 공유하지 않는다.

구현 근거는 recipe/hash/migration, timestamp/seed/alpha/loop seam, static-to-GIF,
existing-GIF timing, measured save/recheck, multi-piece split, export/optimizer/sheet,
clone/artifact, frontend motion editor/preview 회귀다. 새 dependency는 추가하지 않았다.

사용자 제공 displacement map, freeform liquify/warp, 임의 shader, 실행형 effect
plugin과 무제한 motion stack은 MVP에서 제외한다.

## 10. Stage 5 — 컬렉션 복제 완전성

2026-07-26 Stage Gate에서 컬렉션 복제는 기본 collection/profile/icon/piece/crop/note뿐
아니라 이후 추가된 모든 지속 시각 상태를 새 ID 관계로 보존하도록 완성했다.

복제 후 원본과 같아야 하는 상태:

- collection 설정과 export profile
- icon kind/readiness/placeholder
- crop, shape, size override, GIF loop/ping-pong
- text overlay, transform, effect, motion recipe와 revision
- icon piece alt와 note
- cover mapping
- collection 전용 정적/GIF 시트 프리셋
- frame-sheet GIF 생성 provenance

current/piece preview와 유효한 active optimization variant는 복제본 전용 경로에
복사한다. active variant는 현재 source/crop/profile hash 및 output format과 일치할
때만 복사하고, 새 profile/icon/piece ID를 기준으로 target hash를 다시 계산한다.
오래됐거나 파일이 사라진 variant는 건너뛰어 저장된 비파괴 recipe 렌더로 fallback한다.
optimization job과 `last_export_path`는 실행 이력이므로 초기화한다.
향후 AI foundation이 들어오면 F134의 복제 원칙을 다음 순서로 확장한다. 먼저
icon/piece와 durable recipe를 새 ID로 복제한다. 모든 distinct 과거 lineage는 서로
다른 새 ID로 일대일 map하고 generation을 보존한 뒤 complete AI version DAG/state를
만든다. base-source `새 아이콘으로 추가`이면 candidate child와 active pointer도 이
state에 포함하고 나서 target effective source를 resolve한다. source/final-target의
source hash, crop hash, output format과 ID/path를 제외한 output-affecting profile
compatibility가 모두 같은 variant만 복제한다. 불일치 variant의 bytes/row를 target
hash로 재라벨링하지 않고 promoted-preview remap도 건너뛴 뒤 final effective source의
native recipe에서 재생성한다. 모두 같은 경우에만 promoted variant를 새 target path로
remap한다. 이 순서와 path commit은 하나의 DB transaction/file-compensation
protocol이어야 하며 실패 시 반쪽 icon을 남기지 않는다.
`processed_asset_variants.source_file_id/source_hash`는 effective source를 뜻한다.
pending request의 late candidate는 복제본에 자동 부착하지 않는다.

도구막대와 context menu는 import 또는 복제가 진행 중일 때 복제를 비활성화하고,
요청 ref guard로 같은 tick의 중복 실행도 차단한다. tooltip은 아이콘·편집 상태·내보내기
설정이 독립적인 새 모음으로 복사된다는 범위를 설명한다.

수용 검증은 active GIF variant의 새 ID/경로/hash, 원본 artifact 제거 후 복제본 생존,
frame-sheet recipe와 시트 preset의 독립 수정, horizontal/vertical double 아이콘의 실제
원본·복제본 export 파일명/alt/role/byte 동일성, icon 단독 복제의 shared-profile 경로를
포함한다. 기존 text GIF, placeholder, effect/motion, preview ownership 회귀와 함께
복제 직후 동일성과 이후 편집 독립성을 보장한다.
## 11. 명시적 제외

- Aseprite/Pixelorama/miniPaint 등 외부 앱의 binary 또는 source embedding
- 범용 layer/brush/paint editor
- Aseprite UI의 pixel-perfect 복제
- 임의 JavaScript/shader effect plugin
- AI 편집·생성(이 문서의 v0.2 편집 완성도 범위에서는 제외하며,
  `docs/AI_INTEGRATION_DESIGN.md`의 별도 비파괴 설계를 따른다)
- 임의 각도 rotation, 자유형 liquify와 고비용 범용 warp 편집기

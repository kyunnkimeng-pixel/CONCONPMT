# PRODUCT_SPEC.md — PMTCONCON Studio 제품 명세

## 1. 목표
PMTCONCON Studio는 디시인사이드 디시콘 제작 규칙을 기본 프로필로 제공하면서, 다른 커뮤니티/메신저용 이모티콘 제작에도 쓸 수 있는 데스크톱 제작 보조 프로그램이다. 사용자는 이미지·GIF를 가져와 모음 단위로 관리하고, 순서·알트값·자르기·다중콘 분할·미리보기·export를 한 앱에서 처리한다.

## 2. 기본 DCInside 프로필
사용자가 제공한 고객센터 규칙을 기본값으로 사용한다.

- 업로드 이미지 크기: 기본 200×200 px.
- 실제 노출 미리보기: 기본 100×100 px.
- 등록 가능 이미지 수: 최소 10개, 최대 200개.
- 파일 1개 용량 제한: 2MB.
- 허용 확장자/포맷: jpg, png, gif.
- 알트값: 각 이미지마다 숫자나 단어를 붙여 순서/의미를 부여하는 값.
- 알트값 길이: 한글 기준 1~3자.
- 허용 특수문자: `*`, `^`, `!`, `~`, `+`.
- 알트값 중복 불가.
- 캐릭터 배경은 투명 이미지 권장: png/gif.
- 전체 사이즈에는 아이콘과 상하좌우 5px 여백 포함 권장.
- 일상 대화에서 사용하기 어려운 이미지, 사물/인물/풍경 사진은 사용 지양.

이 규칙 중 업로드 가능 수, 크기, 포맷, 용량, 알트값 길이/문자/중복은 export 전 검증한다. 투명 배경, 여백, 사용성 권장은 경고로 표시한다.

## 3. 커스텀 프로필
DCInside 외 이모티콘 제작을 위해 collection 또는 icon 단위로 기준 크기를 변경할 수 있어야 한다.

- Collection 기본값: `cellWidth`, `cellHeight`, `displayWidth`, `displayHeight`, `maxBytes`, `allowedFormats`.
- Icon override: 특정 아이콘만 다른 cell 크기를 가질 수 있다.
- Preview scale: 기본은 `displayWidth/displayHeight`로 보여주되, 편집 패널에서는 원본 크기와 export 크기를 모두 확인 가능해야 한다.

## 4. 주요 화면

### 4.1 메인 화면 — 디시콘 모음 탐색기
- Windows 파일 탐색기와 유사한 레이아웃.
- 중앙에는 “디시콘 모음” 카드/블록이 grid로 표시된다.
- 각 카드에는 대표 이미지와 모음 이름이 표시된다.
- 모음 이름은 inline rename 가능.
- 모음 대표 이미지는 최초에는 첫 번째 아이콘으로 자동 설정된다.
- 사용자는 우클릭 메뉴 또는 편집 버튼으로 대표 이미지를 변경할 수 있다.
- 우상단 `+` 버튼:
  - 새 모음 만들기.
  - 파일 여러 개 가져와 새 모음 만들기.
  - 폴더 가져와 새 모음 만들기.
  - 현재 선택한 모음에 파일 추가.
- Drag and drop:
  - 메인 화면에 여러 이미지 파일/폴더를 놓으면 새 모음 생성 플로우가 열린다.
  - 특정 모음 위에 놓으면 해당 모음에 추가한다.

### 4.2 모음 내부 화면
- Breadcrumb: `홈 > 모음명`.
- 아이콘들이 파일 탐색기처럼 grid/list로 표시된다.
- 아이콘 타일에는 미리보기 이미지와 알트값이 파일명처럼 아래쪽에 표시된다.
- GIF는 grid에서도 계속 애니메이션이 재생되어야 한다.
- 알트값은 클릭 즉시 inline 수정 가능.
- 아이콘명은 별도로 rename 가능.
- 아이콘들은 drag reorder 가능하며 저장 후 앱 재시작에도 유지되어야 한다.
- Shift range select, Ctrl multi select, keyboard Delete, context menu를 지원한다.
- 우클릭 메뉴: 편집, 이름 변경, 복제, 삭제, 대표 이미지로 설정, 원본 보기, export 결과 보기.

### 4.3 우측 편집 패널
아이콘 선택 후 “수정”을 누르거나 더블클릭/단축키로 열린다.

필수 구성:
- 원본 이미지/GIF 표시.
- 출력 미리보기.
- 모양 선택: 단일콘, 가로 이중콘, 세로 이중콘.
- 기준 cell 크기 입력: 기본 200×200, collection 기본값에서 상속 가능.
- 자유모드/고정모드 토글.
- Crop rectangle overlay.
- 이중콘일 때 split line 표시.
- 자유모드: 박스 이동/리사이즈 가능, 선택 모양의 aspect ratio 유지.
- 고정모드: 박스 크기 고정, 위치만 이동 가능.
- 고정모드 위치 프리셋: 중앙, 좌상단, 상단, 우상단, 좌측, 우측, 좌하단, 하단, 우하단.
- GIF 반복 설정: 원본 유지, 무한 반복, 1회, 사용자 지정 반복 횟수.
- 적용, `크롭 기본값`, `저장값으로 되돌리기`.

적용을 눌러도 원본과 crop metadata를 유지해서 나중에 박스 위치/크기를 다시 수정할 수 있어야 한다.

### 4.4 실사용 미리보기
- “미리 사용해보기” 기능.
- DCInside 댓글 창과 유사한 UI를 만든다.
- export될 실제 크기와 100×100 노출 크기 모두 확인 가능.
- GIF는 계속 재생되어야 한다.
- 사용자는 임시 댓글 입력창에서 텍스트 사이에 디시콘을 삽입해 배치감을 확인할 수 있다.
- 다중콘은 조각들이 실제 사용 순서대로 이어져 보이도록 시뮬레이션한다.

### 4.5 Export 화면
- Export 프로필 선택: DCInside / Custom.
- 출력 폴더 선택.
- 파일명 방식: `001~0nn` sequence 또는 alt 값.
- alt txt 생성 여부.
- 검증 결과 표시: 오류/경고 분리.
- 오류가 있으면 export 차단. 경고는 “경고 포함 export” 가능.
- Export 후 폴더 열기 및 `alts.txt` 열기.

## 5. 이미지 가져오기
- 허용 입력: jpg, jpeg, png, gif.
- 다양한 해상도 입력 가능.
- 입력 시 원본을 app library에 복사하고 SHA-256으로 중복 여부를 판정한다.
- 원본 파일 경로가 나중에 사라져도 편집 가능해야 한다.
- 200×200이 아닌 이미지는 기본적으로 crop/downsize를 통해 output cell에 맞춘다.
- 최초 crop box는 중앙 cover 방식으로 자동 생성한다.
- 투명 배경 권장 여부를 감지할 수 있으면 경고로 표시한다.
- 앱 안정성을 위해 원본 한 파일은 최대 64MB, 한 변은 최대 12,000px, 전체는 최대 3,200만 픽셀로 제한한다.
- GIF는 최대 500프레임이며 전체 프레임 면적이 1억 2,800만 픽셀을 넘지 않아야 한다.
- 여러 일반 이미지는 파일별로 순차 처리하고, 제한을 넘은 파일만 명확한 사유와 함께 건너뛴다. 매니페스트와 여러 시트처럼 한 요청에 함께 전송해야 하는 파일은 합계 64MB로 제한한다.

## 6. 다중콘 처리
- 단일콘: 하나의 icon이 하나의 export image를 만든다.
- 가로 이중콘: 하나의 icon이 좌/우 두 조각의 export image를 만든다.
- 세로 이중콘: 하나의 icon이 상/하 두 조각의 export image를 만든다.
- 각 조각은 별도 alt 값을 가져야 한다. DCInside export에서는 모든 조각의 alt 값도 중복 불가다.
- UI에서는 다중콘을 하나의 그룹 타일로 보여주되, export validation에서는 조각 단위로 센다.
- 순서는 icon order → piece order 순으로 결정한다.

## 7. 알트값 validation
DCInside profile 기준:

- 빈 값 불가.
- 중복 불가.
- 한글 기준 1~3자. 구현에서는 `Intl.Segmenter` 또는 grapheme-aware 라이브러리/함수를 사용해 사람이 보는 글자 단위로 세는 것을 목표로 한다.
- 허용 문자: 한글, 영문, 숫자, 공백 없는 일반 단어 문자, 특수문자 `* ^ ! ~ +`.
- 금지: 줄바꿈, 탭, 경로 구분자, 파일명 위험 문자, 이모지(DC profile에서는 기본 금지), 제어문자.

Custom profile에서는 별도 규칙을 설정할 수 있다.

## 8. 저장 및 복구
- SQLite를 사용한다.
- 모든 변경은 낙관적으로 UI에 반영하되 실패 시 되돌린다.
- 앱 시작 시 마지막으로 열었던 모음과 보기 모드를 복구한다.
- 정렬 순서, selection은 durable state가 아니어도 되지만 icon order는 durable state다.
- 삭제는 soft delete 후 사용자가 “라이브러리 정리”를 할 때 물리 삭제하도록 한다.

## 9. Export output
- Export는 현재 표시 순서와 같은 순서여야 한다.
- Sequence filename mode 예시: 총 9개면 `001.png`~`009.png`, 총 120개면 `001.png`~`120.png`.
- Alt filename mode는 파일명 안전화가 필요하며, 중복/빈 값/위험 문자가 있으면 차단한다.
- `alts.txt` 예시:

```text
# PMTCONCON Studio export
# Collection: 예시 모음
001.png	001	아이콘명	알트값
002.png	002	가로이중콘 오른쪽	알트값2
```

- GIF export에서는 loop 설정을 반영한다.
- 결과 파일이 2MB를 초과하면 DC profile에서는 오류로 표시한다. 가능하면 압축/품질 조정 제안을 표시한다.

## 10. 품질 기준
- 주요 기능마다 최소 하나의 자동 테스트 또는 수동 검증 체크리스트 필요.
- UI는 Windows 11 Explorer 감성을 따른다: 좌측 navigation, 상단 toolbar, breadcrumb, grid cards, context menu, inline rename.
- 기능을 숨기지 말고, 비활성화 상태에는 이유를 표시한다.
- 오류 메시지는 한국어로 명확하게 제공한다.
## 2026-05-10 QA stabilization policy note

- Latest user direction for export UX: DCInside output count and alt-text validity/duplication should be shown as warnings in sequence filename export, not as hard blockers. Truly mechanical export blockers still block: empty collection, missing source files, unsupported output mechanics, unsafe alt filename mode collisions/empty stems, invalid dimensions where output cannot be produced safely, and encoded file size over the configured hard byte limit.

## 11. 편집 완성도 확장 범위

2026-07-24 사용자 승인 범위는 다음 다섯 단계다. 각 단계는 구현·검증 후 별도
Stage Gate를 통과해야 하며 미래 기능을 동작하지 않는 메뉴로 먼저 노출하지 않는다.

1. 초기화 의미와 기존 기능 발견성 개선
2. 비파괴 좌우/상하 반전과 90도 회전
3. 임의 프레임 시트에서 새 GIF 만들기
4. 픽셀화와 검증된 내장 효과
5. 컬렉션 복제 완전성 보강

### 초기화 의미

- `크롭 기본값`은 현재 편집 draft의 crop만 기본 위치와 크기로 되돌린다.
- `저장값으로 되돌리기`는 이번 편집 draft를 마지막 저장 상태로 복원한다.
- `미리보기 비우기`는 실사용 미리보기의 임시 댓글과 삽입 아이콘만 비운다.
- `분할 설정 초기값`은 시트 분할 draft와 기존 셀 분석/선택만 앱 초기값으로
  되돌리며 저장된 기본 프리셋을 뜻하지 않는다.
- 향후 `이 아이콘의 시각 편집 초기화`는 crop, transform, effect, text, active
  visual variant만 대상으로 하고 아이콘명, alt, 메모, 순서, 원본은 유지한다.
- 앱 데이터 전체 초기화는 편집기 초기화와 분리하고 위험 구역과 강한 확인을
  사용한다.

### 정적 이모티콘 시트

- Aseprite의 sprite sheet UX는 GIF 전용 참고가 아니다. 기존 다중 이모티콘
  가져오기에도 시작 X/Y, cell 크기, padding/gap, 행·열 수와 읽기 순서를
  독립적으로 검토하는 원칙을 적용한다.
- 정적 시트 가져오기는 번호 overlay, 빈 셀 후보 제외와 셀별 포함/제외 검토를
  거친다.
- 정적 작업 시트 내보내기는 영속화된 icon/piece 순서를 유지하고, 편집용 clean
  sheet와 확인용 guide sheet 및 ID mapping manifest를 분리한다.
- packed layout은 manifest 없이 원위치 재가져오기에 사용하지 않는다.

### 프레임 시트 → GIF

- grid offset, cell size, gap/padding, 행/열과 읽기 순서를 먼저 검토한다.
- 선택 프레임은 한 줄 frame strip에서 순서, 포함 여부와 frame duration(ms)을
  편집한다.
- FPS는 frame duration 일괄 입력의 편의 기능이며 persisted timing의 기준은
  frame별 duration이다.
- 실시간 preview, 반복(한 번/무한/지정), 생성 방향(정방향/역방향/핑퐁),
  총 재생시간, 실제 측정 용량을 GIF 생성 전에 확인한다.
- 첫 MVP의 역방향/핑퐁은 생성 frame sequence에 반영하며 별도 영속 direction
  필드는 수용 기준으로 약속하지 않는다.
- 원본 sheet를 library에 보존하고 생성 GIF를 새 animated icon으로 등록한다.

### 변형과 효과

- 변형/effect recipe는 비파괴 metadata로 영속화하고 preview/export에 동일하게
  적용한다.
- 90도 회전은 비정사각 cell의 너비/높이와 가로/세로 다중콘 shape을 교환한다.
  piece ID와 alt는 시각 내용에 따라 이동하고 piece index/role은 변형 후 출력
  위치를 나타낸다.
- transform은 결합 viewport 전체에 적용한 뒤 piece를 분할하며 GIF의 모든
  frame에 동일하게 적용한다. 원본 교체 시 crop과 transform은 기본값으로
  초기화한다.
- 첫 정적 효과군은 픽셀화, 색상 조정, 흑백/세피아 tone, 블러, 선명화,
  윤곽선, 그림자의 7종이다.
- 정적 효과는 아이콘별 `pmtcon-effects-v1` ordered JSON recipe로 저장한다.
  recipe revision을 함께 저장하고 마지막으로 읽은 revision과 다른 저장은
  충돌로 거부한다. 각 step은 안정적인 ID, enabled 상태와 bounded parameter를
  가지며 step 순서가 렌더 결과와 hash에 영향을 준다.
- 정적 효과는 crop/resize와 whole-viewport transform 뒤, piece split 전에
  결합 viewport 전체에 적용한다. GIF는 같은 순서와 parameter를 모든 frame에
  적용하고 기존 frame delay와 loop 설정을 가능한 범위에서 보존한다.
- 브라우저 CSS filter를 결과 기준으로 사용하지 않는다. Rust native renderer가
  exact preview와 저장·export 결과의 기준이며 optimizer 분석, 정적 작업 시트,
  GIF frame 작업 시트의 source/render recipe hash에도 같은 effect recipe를
  포함한다.
- 정적 효과 MVP는 기존 `image`/`gif` 처리 기반으로 구현하며 새 effect
  dependency나 외부 editor runtime을 추가하지 않는다.

### Motion 효과

- motion 효과는 구현 완료된 필수 편집 기능이다. 아이콘별 revision과 함께
  `pmtcon-motion-v1` 정규화 recipe를 영속화하며 stale revision 또는 render
  signature로 저장된 측정값이 만료되면 저장을 거부한다.
- 모션 탭은 다음 네 범주의 16개 preset을 제공한다.
  - 공간 변형: 흔들기, 통통 튀기, 두근/호흡, 까딱 회전, 회전
  - procedural displacement: 가로/세로 사인파 물결, 2축 젤리 일렁임,
    방사형 리플, 제한된 글리치 밴드
  - 색상·불투명도: 색조 순환, 지정색 박동, 밝기·채도 박동, 번쩍임
  - overlay: 집중선, 반짝이, 확산 링
- 범주별 최대 한 효과만 활성화하고 `저장된 정적 효과 결과 → 공간 변형 →
  displacement → 색상·불투명도 → overlay`의 고정 합성 순서를 사용한다.
  효과는 다중콘 결합 viewport 전체에 적용한 뒤 piece를 분할한다.
- 반복 출력은 normalized phase와 정수 cycles-per-loop를 사용해 결정적으로
  이어져야 한다. 정적 입력은 duration과 FPS로 새 GIF가 되며, 기존 GIF는 frame
  index가 아니라 누적 frame timestamp로 phase를 계산하고 원본 frame timing과
  effective loop를 보존한다. 흔들림·반짝이 같은 변동 효과는 persisted seed로
  재현한다.
- displacement는 output pixel에서 source pixel을 찾는 inverse sampling으로
  구현한다. bilinear 보간은 premultiplied alpha에서 수행하고 pixel art를 위한
  nearest 옵션과 transparent/clamp/mirror bounded edge handling을 제공한다.
- motion UI는 play/pause, OS reduced-motion 상태, duration, FPS, frame 수, loop,
  잘림 frame, 실제 인코딩 전체 및 piece별 용량을 함께 보여준다. 이 수치는 현재
  편집 native preview 기준이며 최종 용량은 export profile과 optimizer를 적용한 뒤
  다시 검증한다. measure와 commit은 같은 renderer와 recipe hash를 사용한다.
- 활성 motion은 editor preview, saved preview, export, optimizer baseline/cache,
  GIF frame 작업 시트에서 동일한 recipe를 사용한다. 출력 profile이 GIF를 허용하지
  않으면 motion export를 검증 단계에서 막고 사용자가 profile을 고치게 한다.
- 정적 작업 시트에는 0ms poster frame 하나만 포함하고 애니메이션 손실 warning을
  반환한다. `pmtcon-sheet-v1`의 `render_recipe_hash`에 motion을 포함해, 내보낸 뒤
  recipe가 바뀌면 가공본 적용/교체 모드에서 해당 셀을 건너뛰고 적용하지 않는다.
  새 아이콘 생성 모드는 원본을 보존하며, 모든 frame, duration과 loop를 왕복하려면
  GIF frame 작업 시트를 사용한다.
- native GIF 인코딩 전에는 DB snapshot을 만들고 shared SQLite lock을 해제한다.
  저장 직전 revision/input signature를 다시 검사하며 요청 preview와 최종 artifact는
  bounded cleanup 정책으로 관리한다. 아이콘/컬렉션 복제는 motion recipe를 독립적으로
  보존하고 mutable preview path를 공유하지 않는다.
- 기존 `image`/`gif` 처리 기반을 재사용하며 새 motion dependency를 추가하지 않는다.
  사용자 제공 displacement map, freeform liquify, 임의 shader, 실행형 effect plugin과
  범용 layer/brush editor는 현재 범위에서 제외한다.

### 컬렉션 복제 완전성

- 컬렉션 복제는 collection/export profile/icon/piece와 crop, 크기, GIF loop,
  ping-pong, text, transform, effect, motion, alt, note, cover를 새 ID로 복사한다.
- 컬렉션 전용 시트 프리셋과 frame-sheet GIF 생성 provenance도 복사하되, 복제본의
  수정이 원본을 바꾸지 않도록 새 ID를 사용한다.
- 현재 source/crop/profile hash 및 형식과 일치하는 유효한 활성 최적화 결과만
  복제본 소유 경로로 복사하고, 새 profile/icon/piece ID 기준 hash를 다시 계산한다.
  오래됐거나 파일이 사라진 variant는 복사하지 않고 저장된 편집 recipe로 되돌아간다.
- optimization job과 `last_export_path`는 실행 이력이므로 복사하지 않는다.
  복제 중에는 도구막대와 context menu의 추가 복제 요청을 막는다.

세부 UX, 라이선스 경계와 단계별 수용 기준은
`docs/EDITOR_COMPLETENESS_DESIGN.md`를 따른다.

## 12. AI 지원 원칙

- AI 기능은 optional이며 공급자와 무관하게 `요청 → 후보 → 검토 → 적용 → 복귀`
  순서를 따른다. 생성 성공만으로 현재 아이콘을 바꾸지 않는다.
- `icons.source_file_id`와 원본 바이트는 AI 작업으로 변경하거나 삭제하지 않는다.
  공급자 결과는 불변 candidate, 특정 아이콘에 적용한 결과는 별도 AI version으로
  저장하며, 정상 `icon_ai_state.active_version_id = NULL`일 때만 원본을 사용한다.
- 원본 계보는 SHA/source ID와 별도인 `original_lineage_id` 및 증가만 하는
  `original_lineage_generation`으로 식별한다. 일반 이미지 교체는 같은 바이트를
  다시 선택해도 새 lineage ID를 발급하고 generation을 증가시킨다.
- AI request/candidate/version·활성 상태·부모 계보·provenance는 앱 재시작 후에도
  유지한다.
  원본 또는 이전 AI 버전으로의 복귀는 저장된 파일을 사용하며 provider를 다시
  호출하지 않는다.
- 프로젝트 library가 남아 있고 사용자가 해당 AI 이력을 명시적으로 영구 삭제하지
  않은 한 원본과 모든 저장 AI version으로 provider 호출 없이 언제든 복귀한다.
  일반 cleanup은 rollback source를 보호하고, 영구 삭제는 잃는 지점과 공유 참조를
  보여주는 별도 확인을 요구한다.
- 모든 import, placeholder, duplicate, sheet commit과 clone은 icon·lineage·원본-only
  AI state를 같은 transaction에서 만들며 state 없는 icon을 허용하지 않는다.
- 일반 이미지 교체는 새 원본 계보를 시작하고 active AI 상태를 해제한다. 이전
  원본의 후보는 이력으로 남지만 새 원본 위에 다시 활성화하지 않는다.
- AI는 첫 단계에서 renderer의 base source만 바꾼다. crop, transform, text,
  static effects, motion, shape, alt, 메모와 순서는 그대로 유지한다.
- 기본 적용은 `새 아이콘으로 추가`이다. 현재 아이콘 적용은 canvas와 input
  stage, animation kind, original lineage, activation revision과 전체 native recipe가
  호환·최신일 때만 명시적으로 수행한다.
  base-source 새 아이콘은 alt를 비우고 readiness를 `working`으로 설정한다.
  candidate child/state를 effective source resolve와 variant/preview보다 먼저 포함해
  새 icon 전체를 한 번에 commit하며 실패 시 반쪽 icon을 남기지 않는다.
- editor는 `original_source`와 `effective_render_source`를 분리한다. 원본 정보/보기는
  전자, canvas·preview·측정·저장·export는 후자를 사용한다. AI state pointer,
  effective source 파일·SHA·decode metadata가 깨졌으면 원본으로 조용히 fallback하지
  않고 복구 안내와 함께 render/export를 막는다.
- `has_alpha`는 alpha 채널 존재가 아니라 실제 decoded/display-composited pixel의
  투명도 사용을 뜻하며 animated source의 모든 표시 frame에서 검사한다.
- `processed_asset_variants`의 source ID/hash는 effective render source를 뜻한다.
  기존 nullable source ID는 original ID/SHA가 명확히 일치할 때만 backfill한다.
  legacy artifact의 file/size/format/dimension을 bounded 검증해 `output_sha256`도
  backfill한다. NULL·ID·SHA·file/decode/size 불일치는 inactive stale로 전환하고
  promoted preview를 final effective source의 native preview로 교체한다. 새 variant는
  non-null source ID/SHA와 output digest 일치를 요구한다.
  static/GIF 작업 시트 v2도 original identity와 effective render identity를 분리하며,
  AI 활성 또는 `original_lineage_generation >= 1` icon에 legacy v1 결과를 추측
  적용하지 않는다.
- 픽셀화, 색상 조정, 변형과 motion처럼 기존 native renderer가 처리하는 작업은
  AI로 보내지 않고, AI는 그림체·캐릭터·배경·구성 변경과 새 이미지에 집중한다.
- 요청 전 외부로 전송할 정확한 이미지·mask·reference·prompt, service surface와
  account context, 그 조합에 적용되는 typed terms/privacy/data-controls/model-terms
  참조를 보여주고 사용자의 확인을 받는다. API는 model과 근거 있는 예상 비용을
  표시한다. 수동 웹 결과의 model·비용은 `검증되지 않음`·`계산 불가`일 수 있으며
  이는 정상 상태이다.
- PMTCONCON Studio 자체는 유료 AI 서비스, 공용 공급자 계정/key/token 또는 과금
  중계 proxy를 운영하지 않는다. 앱은 무료·MIT로 배포하고 공급자 구독·크레딧·
  과금은 각 사용자의 계정에 귀속한다.
- API key/token/cookie는 DB, 설정 파일, AI 이력과 로그에 저장하지 않는다.
  영속 BYOK는 OS 비밀 저장소를 사용하기 전까지 지원하지 않는다. OS 저장소도
  desktop client의 런타임 key 탈취를 막는 완전한 해결책은 아니다.
- foundation과 첫 NovelAI adapter는 `credential_mode_snapshot`만 기록하고 credential
  binding table/column/FK를 만들지 않으며 `os_vault_ref`를 거부한다.
- 영속 credential은 별도 향후 Stage Gate에서만 추가한다. 그때 비밀값 없는
  `ai_credential_bindings` 부모, adapter/provider 일치 guard와 nullable request FK
  `ON DELETE SET NULL`을 함께 만들고 token은 OS vault에만 둔다. 삭제는 DB `deleting`
  표시 → vault entry 삭제 → DB row 삭제 순서와 retryable repair로 처리하며 기존 AI
  이력·active pointer·로컬 rollback을 지우지 않는다.
- PMTCONCON Studio의 MIT license는 앱 코드에만 적용된다. 모델·workflow·공급자
  약관·attribution과 생성물의 이용 권리는 별도이며 앱이 이를 보증하지 않는다.
- 홈페이지 연계는 비밀값이 없는 prompt/source 안내 화면, 검토된
  공식 사이트 열기, 사용자가 직접 수행하는 업로드·생성·다운로드·결과 가져오기로
  제한한다. 로그인, DOM, cookie/session, 자동 업로드, scraping과 결과 다운로드를
  자동화하지 않는다.
- production WebView의 `connect-src`는 Tauri IPC만 허용하고 일반
  `opener:default` 권한을 두지 않는다. frontend는 임의 URL을 전달하지 않고 검토된
  resource enum만 Rust에 보내며, Rust가 compile-time official HTTPS constant로
  변환해 연다. provider API HTTP도 별도의 Rust exact-endpoint allowlist와 redirect
  금지를 적용한다.
- API key/PAT는 현재 앱 실행 중에만 Rust 쪽 session credential로 보관한다. DB,
  설정, local storage, AI 이력과 로그에 기록하거나 frontend로 다시 보내지 않으며,
  사용자가 연결 해제하거나 앱을 종료하면 폐기한다. 한 번의 명시적 사용자 클릭은
  정확히 한 요청만 만들고 background batch, 연쇄 생성, 자동 retry와 provider
  fallback을 금지한다. immutable request와 실제 provider-ready 입력 SHA를 먼저
  `running`으로 저장하고 HTTP 직전 `awaiting_result`를 원자 claim한다. 취소가 먼저
  이기면 HTTP는 0회다. 앱은 single-instance이며 재시작은 진행 요청을 재전송하지
  않는다. 파일 생성 직후 강제종료 고아는 24시간·DB 참조·관리 경로를 확인한 뒤
  fail-closed startup sweep으로만 정리한다.
- 2026-07-28 첫 자동 provider pilot은 NovelAI Image API의 **기존 정적 이미지
  편집 한 장**으로 축소한다. text-to-image, mask inpaint, GIF, sprite와 multi-frame
  처리는 이번 gate에 포함하지 않는다. 사용자가 NovelAI Account 화면에서 직접
  발급한 Persistent API Token만 session-only로 받고 로그인·이메일·비밀번호나
  primary account API는 다루지 않는다. exact endpoint는
  `https://image.novelai.net:443/ai/generate-image`이며 초기 응답 계약은 bounded
  JSON만 허용하고 ZIP은 거부한다.
- NovelAI 공개 OpenAPI는 request의 `action`과 `model`에 enum을 제공하지 않는다.
  따라서 앱이 사용하는 두 값은 공식 enum이라고 부르지 않고 versioned experimental
  contract string으로 취급한다. 요청마다 정확한 값과 전송 source/prompt를 사용자에게
  보여주고 확인받으며, 알 수 없는 값이나 응답 schema drift는 추측하지 않고
  fail closed 한다. 새 PAT 발급은 이전 PAT를 무효화하고 token은 한 번만 표시되므로
  입력 전달 뒤 frontend state를 비우며 `401`을 재시도하지 않고 PAT 교체를 안내한다.
- Gemini API는 일반 소비자용 기본 기능이 아니다. 2026-07-28 기준 image model은
  free tier가 없으므로, 비공개 정적 이미지 편집 pilot도 18세 이상, 미성년자 대상이
  아님, 지원 지역, professional/business 목적, 사용자의 유료 key와 비용 확인,
  해당 surface의 data-policy 검토를 모두 통과한 경우에만 명시적으로 열 수 있다.
  현재 Interactions allowlist는 `gemini-2.5-flash-image`와
  `gemini-3.1-flash-image`다. 2.5 요청에는 지원하지 않는 `image_size`를 넣지 않고,
  3.1에만 `1K`를 요청한다. 응답의 모든 `model_output`을 순회해 마지막 inline
  `image/jpeg`를 후보로 저장한다. 400 응답은 잘못된 key, 무료 등급/결제 미설정의
  `FAILED_PRECONDITION`, 실제 요청 필드를 고정된 안내로 구분하고 provider body와
  비밀값은 버린다. 이 조건을 확인하지 않은 일반 배포에서는 Gemini API key 입력과
  실행을 숨기거나 비활성화하고 Gemini 공식 웹 수동 handoff만 제공한다.
- 기준일과 제한의 근거는
  [NovelAI Image API schema](https://image.novelai.net/docs/doc.json),
  [NovelAI Account/PAT 안내](https://docs.novelai.net/en/text/usersettings/account/),
  [Gemini Interactions API](https://ai.google.dev/api/interactions-api?hl=en),
  [Gemini image generation](https://ai.google.dev/gemini-api/docs/image-generation),
  [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing),
  [Gemini API Additional Terms](https://ai.google.dev/gemini-api/terms)이다. 모델,
  가격, schema와 약관은 바뀔 수 있으므로 live pilot과 release마다 다시 확인한다.
- provider 변경은 새 request, exact payload 확인과 consent를 요구한다. adapter
  비활성화, session token clear 또는 향후 persistent binding 삭제 뒤에도 기존
  candidate/version/source와 active pointer 및 provider 없는 로컬 복귀를 보존한다.
- 사용자 실행 로컬 adapter는 literal `127.0.0.1`/`[::1]`만 허용하고 redirect를
  금지한다. 외부 runtime, model, workflow는 번들하지 않으며 local workflow가
  외부 유료 API를 호출할 수 있음을 실행 전에 알린다.
- cloud adapter는 코드 소유 exact HTTPS origin(scheme/host/port/path-prefix)만
  허용하고 사용자의 URL override와 redirect를 거부한다.
- 첫 automated provider pilot은 기존 정적 JPG/PNG 한 장 편집만 다룬다. 직접 API의
  GIF 전체 frame batch, animated output, text-to-image와 inpaint는 별도 opt-in
  실험 Stage Gate로 분리한다. 수동 웹 GIF 편집은 direct provider 호출이 아니라
  `pmtcon-gif-frame-sheet-v2` clean page 왕복으로 제공한다. manifest는 앱 복원용으로
  유지하고 웹에 업로드하지 않으며, 원본 GIF와 frame timing/order/loop를 그대로
  보존·복원한다.
- 실제 flow가 구현되기 전에는 AI 메뉴나 탭을 노출하지 않는다.
- 아이콘/collection 복제는 request·candidate·source bytes를 공유하되 새 lineage와
  version/state/preview ownership을 만든다. 모든 과거 lineage를 서로 다른 새 lineage로
  일대일 remap하고 generation을 보존해 과거 이력이 현재 clone 계보로 합쳐지지 않게
  한다. durable recipe와 complete AI state를 먼저 매핑한다. source/crop hash와
  ID/path를 제외한 output-affecting profile compatibility가 final target과 모두 같은
  optimization variant만 복제하며, 불일치 bytes를 새 source hash로 재라벨링하지
  않는다. 부적합 variant/promoted preview는 건너뛰고 final effective source에서
  native 재생성한다. 전체를 하나의 transaction/file-compensation protocol로 commit한다.
- pending request 중 복제한 경우 늦게 도착한 candidate를 복제본에 자동 부착하지
  않으며 비용·usage는 distinct provider request 한 번으로만 계산한다.
- 편집 패널에는 현재 AI source 상태와 `AI로 수정` 진입만 간결하게 두고, 후보
  가져오기·큰 비교·규격화·적용·복귀는 앱 내부의 전용 AI 작업공간에서 수행한다.
  기본 1200×760 창에서 header/tabs/status/action bar는 고정하고 후보·비교·설정
  영역만 독립적으로 스크롤한다.
- 공급자 또는 수동 작업이 반환한 임의 크기 JPG/PNG는 raw candidate로 그대로
  보존한다. 단일 수동 웹 결과가 요청 크기와 달라도 목표와 가로세로 비율이 같으면
  차단하지 않고 로컬 정규화 경고를 표시한다. 비율이 다르면 계속 차단하고, 요청에서
  투명을 필수로 고른 경우 alpha 손실도 차단한다. 투명이 권장인 요청의 불투명 결과는
  자동 적용하지 않고 사용자가 `배경 포함으로 계속`을 명시적으로 선택해야 한다.
  적용 전 `전체 보이기(contain + pad)` 또는
  `빈틈 없이 채우기(cover + crop)`, 3×3 정렬과 bounded resize filter로 현재
  base-source canvas에 맞춘 별도 불변 source를 만들고 normalization recipe/hash를
  AI version에 저장한다. 기본은 transparent pad를 쓰는 `전체 보이기`다.
- 후보 검토는 `원본`, `AI 원본`, `규격화 결과`, `최종 적용 모습`을 큰 화면,
  checkerboard와 실제 크기·alpha·잘림 경고로 비교한다. transparent padding은
  AI 결과 내부의 불투명 배경 제거로 표현하지 않는다.
- AI 완료 기본 동작은 `새 아이콘으로 추가`이며, 현재 아이콘 적용은 보이는
  secondary action으로 제공한다. 새 아이콘 생성 뒤 `새 아이콘 열기`,
  `목록에서 보기`, `계속 후보 비교`를 제공하고 빈 alt·`작업중` 상태를 알린다.
- AI 작업공간은 한 개의 status/alert 영역, 고유한 후보 accessible name,
  가시적인 비활성 이유, keyboard focus 복원과 reduced-motion을 지원한다.

### AI-UX-1 구현 상태 (2026-07-27)

- 로컬에서 가져온 임의 크기 정적 JPG/PNG를 불변 raw candidate로 보존하며,
  가져오기만으로 현재 아이콘이나 원본 바이트를 바꾸지 않는다.
- Rust native renderer가 `pmtcon-ai-normalization-v1` recipe에 따라
  `전체 보이기(contain + transparent pad)`와
  `빈틈 없이 채우기(cover + crop)`, 3×3 정렬, Lanczos3/Nearest filter를
  결정적으로 적용한다. 규격화가 필요한 결과는 별도 불변 PNG source로 저장한다.
- `원본`, `AI 원본`, `규격화 결과`, `최종 적용 모습`을 checkerboard와 함께
  비교하고 실제 정규화 캔버스·최종 렌더·조각 크기, alpha,
  padding/crop geometry 및 경고를 표시한다. 미리보기에서 실제 적용과 같은
  조각 용량 제한을 검사하므로 준비 완료 뒤 같은 입력이 용량 때문에 새로
  실패하지 않는다.
- 미리보기 signature는 candidate SHA, target canvas, normalization recipe,
  lineage/generation, activation revision과 native visual recipe를 묶는다. 현재
  아이콘 적용과 새 아이콘 생성 시 Rust가 이를 다시 계산하여 오래된 미리보기를
  거부한다.
- 기본 `새 아이콘으로 추가`와 호환되는 `현재 아이콘에 적용` 모두 같은
  normalization 계약을 사용한다. 적용 후에도 원본 및 이전 AI version으로의
  로컬 복귀가 가능하며 provider 재호출은 필요하지 않다.
- AI 소스 이력은 맞춤 방식·정렬·필터·원본→캔버스를 구분해 표시한다. 손상된
  비활성 후보나 version은 숨기지 않고 `사용 불가` 사유와 함께 남기며 적용·복귀만
  fail-closed로 차단한다. 저장 성공 뒤 편집기 표시 재조회만 실패한 경우에도
  저장 실패로 오인시키지 않고 표시 새로고침을 별도로 재시도할 수 있다.
- normalized source 또는 preview 승격이 실패하면 새로 만든 원본·썸네일·preview를
  보상 정리한다. 관리 디렉터리는 component별 no-follow 검증과 Windows reparse
  point 차단을 거친다.
- 이 단계는 로컬 candidate 검토·규격화·적용 vertical slice이며, 전용 AI
  작업공간 배치는 아래 AI-UX-2에서 구현했다.

### AI-UX-2 구현 상태 (2026-07-27)

- 편집 패널에는 compact 이미지 소스 요약과 AI 작업공간 진입만 남기고, 후보
  가져오기·검토·소스 이력을 큰 앱 내부 modal dialog로 이동했다.
- 작업공간은 `결과 가져오기`, `후보 검토`, `소스 이력` 세 탭만 제공하며,
  원본·AI 원본·규격화 결과·최종 적용 모습·겹쳐 보기를 화면 맞춤/100%와
  checkerboard로 비교한다.
- header·탭·status·footer는 고정하고 본문만 스크롤한다. 1024px 미만에서는
  후보를 가로 rail로 바꾸고 inspector를 비교 영역 아래에 배치한다.
- dialog role, Escape 닫기와 진입 버튼 focus 복원의 기본 경계를 제공한다.
- AI-UX-3에서 생성 후 새 아이콘 열기·목록 reveal·계속 비교, 직접 생성
  provenance 기반 중복 안내, combined mutation DTO, 완전한 keyboard/focus,
  document-wide 단일 live-region, reduced-motion과 browser continuity를 구현했다.
  1200×760/800×760 headed browser QA 13/13과 후속 GET·예상 밖 network 0을
  통과했다.
- AI-UX-3 checkpoint에는 provider 요청, prompt, token, consent와 외부 전송 UI가
  포함되지 않았다. F138/F150–F151의 정적 단일 JPG/PNG 수동 웹 왕복은
  `ai/handoffs/<request-id>`의 관리형 `upload.png`·manifest·prompt package,
  검증된 Windows 파일 드래그와 Explorer fallback, 7일 보관/1회 30일 연장,
  15분 주기 정리, 256MiB 총량 제한, 최근 전달 이력, 결과 구조 검사와 같은
  request의 비활성 후보 저장을 지원한다. 원본과 활성 소스는 자동 변경하지 않는다.
  로그인·DOM·cookie·자동 생성/다운로드/결과 판정은 계속 금지한다. F148–F149는
  선택한 정적 단일 아이콘 2–16개 수동 웹 그리드 수정과 원본 없는 단일/그리드
  생성을 지원한다. 생성은 모음의 단일 아이콘과 외부 PNG/JPG/GIF를 합쳐 1–16개
  참고 이미지 board로 준비할 수 있다. 외부 파일은 IPC 직렬화 전에 합계 16MiB로,
  내부·외부 전체 참고 source는 누적 128M 픽셀로 제한한다. 비정사각형 source는
  contain으로 비율을 보존하고 GIF 참고는 첫 프레임 poster를 사용한다. 참고 board
  배치와 실제 출력 geometry를 프롬프트에서 분리하며, 살아 있는 reference board는
  최근 전달에서 다시 끌기·Explorer 열기·취소할 수 있다. F152/F155의 GIF manifest
  왕복은 clean PNG page와 구조 보호 프롬프트만 Gemini/NovelAI 웹에 수동 전달하고
  앱이 보관한 manifest로 정확한 frame delay/loop를 복원한다. direct provider animated/GIF batch는 여전히
  별도 Stage Gate다. F139 session-only
  NovelAI 정적 이미지 편집 pilot은 mock·보안·license Stage Gate와 사용자 승인 live
  test 전에는 일반 release 완료 기능으로 표시하지 않는다.

### GIF 웹 전달 UX 계약 (2026-08-02)

- 웹 AI에 올리는 파일은 라벨·격자 없는 `frames_sheet_001.png`와 이후 clean page뿐이다.
  여러 페이지는 화면에 표시된 실제 캔버스대로 한 장씩 같은 prompt와 provider 설정으로
  처리한다. `frames_guide_*.png`는 셀 번호·시간을 사람이 확인하는 파일이므로 웹에
  올리지 않는다. `frames_manifest.json`도 frame timing/order/loop와 page mapping을
  복원하는 앱 전용 파일이며 웹 입력이 아니다.
- 내보내기 직후 같은 dialog/session에서 결과를 가져오면 앱이 생성한 manifest와 page
  slot을 유지해 사용자가 JSON을 다시 선택하지 않는다. 앱 재시작, 별도 `교체하기`
  진입, 관리 기록 만료·손상처럼 자동 연결을 검증할 수 없는 때에만 수동 manifest
  선택/drop을 복구 경로로 제공한다. 자동 연결도 icon/source/recipe hash를 다시
  검증하며 다른 내보내기의 manifest를 추측하지 않는다.
- 각 clean page는 관리형 파일 검증을 통과한 Windows native drag와 Explorer 선택을
  제공한다. guide와 manifest에는 웹 전달 drag를 제공하지 않는다. 브라우저 업로드,
  생성, 다운로드는 사용자가 공식 웹에서 직접 수행한다.
- 결과는 PNG를 권장하지만 JPG/JPEG와 정적 WebP도 실제 byte signature로 판별해
  불투명 중간 결과로 검토할 수 있다. exact page canvas와 명시적 page slot mapping은
  계속 필수다. alpha가 없거나 모든 pixel이 불투명하면 `배경 포함으로 계속`을 명시적으로
  선택해야 내부 PNG로 정규화하며, 선택 전에는 GIF variant를 만들지 않는다. animated
  WebP는 첫 frame으로 조용히 평탄화하지 않고 지원하지 않는 애니메이션 결과로 알린다.
- 이미지에 직접 그린 checkerboard는 투명 alpha가 아니므로 경고한다. 사용자가 배경
  포함을 선택하면 checker도 pixel로 남는다는 점을 미리 보여 주며, 회색 외곽선과
  anti-aliasing 손상을 피하기 위해 자동 제거하지 않는다. 투명 배경은 권장값이지
  모든 생성·편집 결과의 보편적 필수 조건은 아니다.
- 구조 보존 prompt와 같은 reference/settings는 품질을 돕지만 생성 모델이 원본 그림체,
  캐릭터 비율 또는 frame 간 일관성을 정확히 보장한다고 표현하지 않는다. 재조립 전후
  사용자가 전체 animation, 깜빡임과 style drift를 검토해야 한다.

### NovelAI 웹 전달 UX 계약 (2026-07-29)

- NovelAI는 자연어와 태그를 모두 지원하지만 PMTCONCON Studio는 V4+에서 제어하기
  쉬운 짧은 영문 소문자 태그를 기본 안내한다. Prompt와 Undesired Content는 서로
  다른 입력란이므로 별도 표시·복사하고, 정확한 시트 구조 안내를 긴 태그처럼 섞지 않는다.
  Prompt 복사가 성공해야 2단계 Undesired Content 버튼이 열리며, Prompt·공급자·요청을
  다시 바꾸거나 복사하면 완료 상태를 초기화해 클립보드 내용과 화면 단계가 어긋나지 않는다.
- 기존 단일 아이콘, 선택 아이콘 grid, GIF clean frame sheet처럼 배치·실루엣·셀
  순서를 유지하는 편집에는 Image2Image를 우선 안내한다. Strength/Noise의 단일
  정답값은 강제하지 않고 낮게 시작해 결과를 보며 조절하도록 설명한다.
- 원본 없는 생성의 참고 board는 출력 틀이 아니다. 그림체·색감·질감은 Vibe Transfer,
  V4.5의 캐릭터/스타일 일관성은 Precise Reference로 안내한다. 두 기능은 동시에
  사용하지 않으며, 여러 Character Reference가 서로 다른 캐릭터로 분리된다고 약속하지 않는다.
- 실제 NovelAI 화면의 `Add a Base Img (Optional)` 업로드를 안내하고, `What do you want to do with this image?` 선택 창이 표시되면 `Image2Image`를 고르며 바로 base image가 붙는 UI에서는 이어서 나타나는 Strength/Noise를 조절하도록 설명한다. Vibe Transfer/Precise Reference가 별도 패널로 보이면 해당 패널의 Add Image를 사용한다. 결과 형식은 `메뉴(☰) > Account Settings > Image Settings 탭 > Image Generation > Image Format for Generated Images > PNG`를 권장한다. 정적 단일·grid 결과는 Download Image로 받은 PNG/JPG/WebP를 바이트 signature로 판별하고 정적 WebP만 alpha 보존 내부 PNG로 정규화한다. animated WebP는 첫 프레임으로 조용히 평탄화하지 않고 지원하지 않는 애니메이션 결과로 차단한다. 브라우저 다운로드명은 identity나 매핑 키로 신뢰하지 않는다. 투명이 필요한 경우에만 `Director Tools > Remove BG`를 선택적으로 적용하고 alpha를 확인하며 배경 포함 작업에는 강제하지 않는다.
- NovelAI 웹에서 200×200 입력이 192×192처럼 가까운 지원 크기로 바뀔 수 있다.
  정적 단일 결과는 목표와 같은 비율이면 원본 후보를 보존하고 로컬에서 목표 크기로
  정규화한다. grid와 GIF frame sheet는 셀 복원을 위해 정확한 캔버스가 필요하므로
  임의 보정하지 않는다. 다중 페이지 GIF는 각 clean PNG를 한 장씩 같은 Prompt·Strength·Noise·sampler 설정으로 Image2Image 처리하되 해상도는 표시된 페이지별 실제 캔버스로 바꾸고 현재 페이지 하나만 반환하도록 안내한다. 마지막 부분 페이지가 작을 수 있으므로 페이지별 실제 캔버스를 표시하고 exact 검증한다. NovelAI가 다운로드명을 바꾸면 manifest 페이지 슬롯에서 사용자가 결과를 명시적으로 연결하며, Chrome의 `(1)` 접미사와 PNG 확장자 대소문자는 모호하지 않을 때만 자동 연결한다. 다중 페이지에서 마지막 남은 unrelated file을 추측해 자동 배정하지 않는다. JPG/JPEG와 정적 WebP는 exact canvas 검사 뒤 불투명 중간 결과로만 연결하고 사용자의 `배경 포함으로 계속` 확인을 요구한다. renderer는 manifest JSON 4MB, 페이지 500개, 결과 한 장 64MB, manifest+결과 합계 64MB를 IPC 전에 검사한다. GIF에는 200px 셀과 최대 1024px 캔버스를 유지하는 내장
  `NovelAI 웹 호환 GIF / 200x200 / 4x4` 프리셋을 제공한다.
- Vibe Transfer/Precise Reference와 Image2Image는 구독 중에도 Anlas가 들 수 있으므로
  사용자가 NovelAI의 Generate 버튼 비용 표시를 확인하도록 한다. 로그인·업로드 방식
  선택·생성·다운로드는 계속 공식 웹에서 사람이 직접 수행한다.

### 컬렉션 AI grid 요구사항 (2026-07-28)

- 모음 화면에서 원본 없는 새 아이콘 한 개 또는 여러 개를 만들 수 있어야 한다.
  한 개 생성은 grid를 강제하지 않고, 여러 개 생성은 최대 4×4 한 페이지 clean
  grid와 local manifest를 사용한다.
- 정적 단일콘 여러 개를 선택해 `선택 N개 AI로 수정`할 수 있어야 한다. 선택
  순서, 각 아이콘의 lineage, effective source hash, native recipe와 activation
  revision을 요청 전에 고정한다.
- AI 입력 시트에는 번호·라벨·guide 선을 굽지 않고, 사람이 보는 local overlay와
  manifest에만 cell 번호와 target mapping을 둔다.
- 결과 grid는 자동 적용하지 않는다. 전체 시트와 각 cell의 bounds, 빈 cell,
  순서와 target mapping을 사용자가 검토한 뒤에만 비활성 후보 또는 새 아이콘으로
  저장한다. grid가 어긋나면 기존 overlay/직접 Slice 방식으로 수동 보정할 수 있다.
- 선택이 비어 있을 때 전체 모음 요청으로 바꾸지 않는다. 2페이지 이상, GIF,
  가로/세로 이중콘과 provider별 검증 한도를 넘는 요청은 HTTP 전에 정확한 사유로
  차단한다.
- 원본 없는 생성에 투명 placeholder icon이나 가짜 source/lineage를 만들지 않는다.
  승인된 candidate cell이 새 icon의 최초 immutable source가 되고 생성 provenance를
  기록한다.
- 원본 없는 단일/그리드 생성 결과는 파일 확장자나 alpha 픽셀 한 개가 아니라 decoded
  pixel의 의미 있는 alpha와 painted checker를 검사한다. 투명 배경은 기본 권장값이며
  사용자가 `투명 필수`를 고른 경우에만 canvas와 각 target cell 외곽 연결 alpha=0
  영역 5% 이상, border·gap·unused cell 95% 이상 조건을 blocking gate로 적용한다.
  그 밖의 불투명 PNG/JPG/JPEG/static WebP는 자동 적용하지 않고 결과 검토에서
  `배경 포함으로 계속`을 명시적으로 선택한 경우에만 후보/새 아이콘으로 진행한다.
  가짜 checker, alpha 한 픽셀과 1px 투명 테두리는 실제 투명으로 표시하지 않는다.
  고신뢰도 painted checker는 `투명 필수`에서 차단하고, `배경 포함`을 명시적으로 고른
  경우에만 경고 후 유지한다. 배경 포함을 선택하면 그 pixel이 그대로 남음을 보여 주며 회색 외곽선과
  안티앨리어싱을 손상할 수 있는 checker 자동 제거는 하지 않는다. static WebP는
  alpha를 보존해 내부 PNG로 정규화하고 animated WebP는 별도 애니메이션 결과로 차단한다.
- 결과 artifact 저장 뒤 cell 분석이 일시적으로 실패하면 backend의
  layout_review_pending 상태에 맞춰 4단계를 유지하고 파일 재업로드 없이
  결과 다시 분석으로 복구한다.
- 현재 Gemini 직접 API adapter는 JPEG 결과만 받으므로 `투명 필수` 요청은 HTTP와
  비용 발생 전에 차단하고 실제 alpha를 요구하는 수동 웹 전달로 안내한다. 사용자가
  배경 포함 결과를 명시적으로 허용한 요청만 기존 비용·consent gate 아래 진행할 수
  있으며 반환 JPEG를 투명하다고 표시하지 않는다.
- 요청 한 번의 usage/cost를 N개 후보에 중복 합산하지 않는다. 호출 수가 줄어도
  공급자 과금 절감을 보장하지 않는다.
- 한 사용자 동작은 HTTP 요청 한 번만 만들며 자동 retry, fallback과 background
  batch를 금지한다. 다시 시도는 새 request, 새 snapshot과 새 비용 확인을 요구한다.
- 첫 구현은 정적 single JPG/PNG 2~16개를 위한 provider-free grid foundation과
  mock UI부터 시작한다. Gemini 1K live 계약은 품질상 기본 3×3 이하로 제한하고,
  GIF/다중콘/더 큰 grid는 별도 Stage Gate에서 검증한다.

세부 데이터 모델, provider 경계, 롤백 transaction과 단계별 수용 기준은
`docs/AI_INTEGRATION_DESIGN.md`, 화면 흐름과 deterministic normalization 계약은
`docs/AI_WORKSPACE_UX_DESIGN.md`, collection grid 계약은
`docs/AI_GRID_WORKFLOW_DESIGN.md`를 따른다.

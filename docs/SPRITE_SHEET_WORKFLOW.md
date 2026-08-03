# 스프라이트 시트 / 작업 시트 워크플로우

PMTCONCON Studio의 시트 도구는 자동 마법사가 아니라 제작자가 수치를 정하고 검토한 뒤 실행하는 전문 작업 흐름이다.

## 시트에서 가져오기

1. 모음 화면에서 `시트에서 가져오기`를 연다.
2. PNG, JPG, JPEG 시트 이미지를 선택한다.
3. 분할 방식을 선택한다.
   - `Grid로 자르기`: 행/열 기준.
   - `셀 크기로 자르기`: 셀 너비/높이 기준.
   - `Manifest로 복원`: 이전에 내보낸 `pmtcon-sheet-v1`로 다시 가져오기.
   - `직접 Slice 지정`: 사용자가 사각형 Slice를 만들고 좌표를 직접 입력하는 수동 가져오기.
   - `자동 감지 (실험)`: 투명/단색 separator를 분석해 grid 설정 후보를 제안한다.
4. 오른쪽 설정 패널에서 열, 행, 셀 크기, 여백, 간격, 읽기 순서, 빈 셀 alpha 기준을 입력한다.
5. 중앙 미리보기에서 grid, 셀 번호, 선택 상태, 빈 셀 후보를 확인한다.
6. 사용자가 셀을 클릭해 포함/제외를 조정한다.
7. 셀 검토 단계에서 `전체 선택`, `전체 해제`, `선택 반전`, `빈 셀 후보 제외`를 사용한다.
8. `선택한 셀 가져오기`를 실행한다.

가져오기 결과:

- 원본 시트는 앱 데이터의 `sheet_imports/original_sheets/`에 보존된다.
- 가져온 셀은 PNG로 추출되어 새 아이콘으로 등록된다.
- PNG alpha는 유지된다.
- alt 값은 자동 생성하지 않는다.
- 빈 셀 후보와 범위 밖 셀은 건너뛰고 결과에 보고한다.

## 시트로 내보내기

1. 모음 화면에서 `시트로 내보내기`를 연다.
2. 모음 도구에서는 현재 모음 전체를, 아이콘 우클릭의 `선택 항목 N개 작업시트로 내보내기...`에서는 현재 선택만 대상으로 한다.
3. Layout을 설정한다.
   - Cell W/H
   - Columns
   - Gap X/Y
   - Border X/Y
   - Max W/H
   - Background: transparent, checker, white, black
4. Output을 설정한다.
   - Clean sheet PNG
   - Guide sheet PNG
   - Manifest JSON
   - Guide cell/export number
   - 출력 폴더
5. `작업 시트 내보내기`를 실행한다.

내보내기 결과:

- Clean sheet는 라벨, grid, overlay가 없는 편집용 PNG다.
- Transparent 배경을 선택하면 외부 편집에서 alpha를 유지할 수 있다.
- Guide sheet는 사람 확인용이며 grid와 숫자 라벨을 포함할 수 있다.
- Manifest는 reimport 기준이 되는 `pmtcon-sheet-v1` JSON이다.
- 큰 작업은 자동으로 여러 페이지로 나뉜다.
- GIF 또는 모션 적용 아이콘은 정적 contact sheet에 0ms 포스터 프레임 한 장만 포함된다. 애니메이션 손실 경고가 표시되며 GIF 재조립용으로 사용할 수 없다.

### 정적 작업 시트와 GIF 프레임 작업 시트의 차이

- 정적 `작업 시트`는 여러 아이콘을 한꺼번에 보정하는 contact sheet다. 아이콘마다
  0ms 포스터 한 장만 배치하고 `pmtcon-sheet-v1` manifest로 icon/piece 위치를
  왕복한다. crop·변형·텍스트·정적 효과·모션을 포함한 `render_recipe_hash`가
  달라지면 가공본 적용/교체 모드에서 해당 셀을 건너뛰고 적용하지 않는다.
- `GIF 프레임 작업 시트`는 GIF 하나의 모든 유효 프레임을 펼친다. frame duration,
  loop와 page/cell 좌표를 `pmtcon-gif-frame-sheet-v2`에 저장하므로 애니메이션을
  편집하고 다시 조립할 때 사용한다.
- 정적 원본에 모션을 저장하면 preview/export는 GIF가 되지만, 정적 작업 시트는
  여전히 0ms 포스터만 내보낸다. 움직임 전체를 외부에서 편집하려면 GIF 프레임
  작업 시트를 선택한다.

## 수정된 시트 다시 가져오기

1. `시트에서 가져오기`에서 `Manifest로 복원`을 선택한다.
2. `sheet_manifest.json`을 선택한다.
3. 수정한 clean sheet PNG 파일을 선택한다.
4. `매니페스트로 다시 가져오기`를 실행한다.

다시 가져오기 규칙:

- 매니페스트의 `page_index`, `x`, `y`, `w`, `h`를 기준으로 셀을 crop한다.
- 수정된 시트 크기가 매니페스트와 다르면 경고한다.
- 누락 페이지 또는 범위 밖 셀은 건너뛰고 보고한다.
- 기본 결과는 새 아이콘이다.
- 원본 아이콘과 원본 파일은 덮어쓰지 않는다.
- 가공본 생성 또는 기존 가공본 교체 모드에서는 manifest의 `render_recipe_hash`를
  현재 crop·변형·텍스트·정적 효과·모션 recipe와 비교한다. stale이면 해당 셀의
  파일을 쓰거나 적용하지 않고 skipped item으로 보고한다.

## 프레임 시트로 새 GIF 만들기

이 흐름은 매니페스트가 없는 PNG/JPG/JPEG 프레임 시트를 새 애니메이션
아이콘 하나로 만든다. 기존 GIF를 작업 시트로 풀었다가 variant로 되돌리는
흐름과는 별개다.

1. 모음 화면에서 `시트로 GIF 만들기`를 연다.
2. 기존 정적 시트 가져오기와 같은 행/열 또는 셀 크기, border, gap, 읽기
   순서와 공유 프리셋으로 셀을 분석한다.
3. 번호 overlay와 셀 썸네일에서 포함 셀을 검토한다. 빈 셀 후보는 기본
   제외되지만 의도한 투명 프레임이라면 직접 포함할 수 있고, 범위 밖 셀은
   포함할 수 없다.
4. 선택 셀을 한 줄 프레임 스트립으로 만든 뒤 Ctrl/Shift 선택, 드래그 또는
   키보드 이동, 복제, 삭제, 선택 순서 뒤집기와 전체 순서 뒤집기를 사용한다.
5. 프레임별 duration(ms)을 입력하거나 선택 프레임에 시간을 일괄 적용한다.
   FPS 입력은 전체 duration을 계산하는 편의 기능일 뿐 저장 기준은 frame별
   duration이며, 실제 GIF 시간은 10ms 단위로 정규화된다.
6. 생성 방향은 정방향, 역방향, 핑퐁 중에서 고른다. 역방향과 핑퐁은 최종
   프레임 배열에만 반영되고 생성 아이콘에 다시 적용되는 별도 방향 상태를
   만들지 않는다. 핑퐁의 반복 재생은 양 끝 프레임 중복을 피하고, 한 번
   재생은 마지막에 시작 프레임을 덧붙여 왕복을 완결한다.
7. 한 번, 무한 반복, 지정 횟수를 정하고 sticky 동작 미리보기에서 최종 프레임
   수와 총 시간을 확인한다. 시스템의 동작 줄이기가 켜져 있으면 기본 재생은
   멈춘다.
8. `GIF 미리 생성 및 용량 측정`으로 실제 GIF를 인코딩한다. 모음의 byte 제한을
   넘으면 경고하며, 프레임·시간·방향·반복 설정을 바꾸면 측정값은 즉시
   만료된다.
9. 최신 render hash가 확인된 경우에만 `새 GIF 아이콘 만들기`를 실행할 수
   있다. 서버는 같은 입력과 recipe를 다시 렌더링해 hash를 검증한다.

생성 결과:

- 원본 프레임 시트는 `sheet_imports/original_sheets/`에 별도로 보존된다.
  생성 직후 `원본 시트 위치 열기`로 Explorer에서 보존 파일을 확인할 수 있다.
- 원본 시트 hash, grid와 frame-duration recipe, 방향, 반복, 실제 bytes와
  render hash가 `pmtcon-frame-sheet-gif-v1` provenance로 영속화된다.
- 출력 GIF는 animated source, 전체 원본 crop, 단일 piece를 가진 새 아이콘으로
  등록되며 이후 일반 GIF처럼 편집·미리보기·내보내기를 사용할 수 있다.
- GIF의 1비트 alpha 또는 프레임당 256색 한계로 모양이 달라질 가능성이 있으면
  실제 생성 전에 경고한다.

## GIF 프레임 시트로 내보내기

GIF 프레임 시트는 애니메이션 GIF 하나를 외부 편집 가능한 PNG 프레임 시트로 풀어내는 흐름이다.

1. GIF 아이콘 하나를 선택한다.
2. 아이콘 컨텍스트 메뉴에서 `GIF 프레임 시트로 내보내기`를 연다.
3. frame count, duration, loop mode, 예상 page 수를 확인한다.
4. frame cell size, columns, frames per page, max sheet size, gap, border, background를 설정한다.
5. `GIF 프레임 시트 내보내기`를 실행한다.
6. PMTCONCON Studio가 clean `frames_sheet_*.png`, guide `frames_guide_*.png`,
   `frames_manifest.json`을 생성한다.
7. 웹 AI에는 `frames_sheet_*.png`만 올린다. Clean sheet에는 번호, grid, label이
   들어가지 않는다. `frames_guide_*.png`는 사람 확인용이며 절대 편집 결과로
   업로드하지 않는다. `frames_manifest.json`은 앱 복원용이고 웹에 올리지 않는다.
8. frame 수가 많으면 max sheet size와 frames per page 기준으로 자동 page split된다.
9. 웹 AI 전달 화면은 clean page마다 `이 파일 끌기`와 Explorer fallback을 제공한다.
   여러 page는 한 장씩 같은 prompt와 provider settings로 처리한다.

## GIF 프레임 시트 다시 가져오기

수정된 GIF 프레임 시트는 원본 GIF를 덮어쓰지 않고 새 processed variant로 재조립한다.

1. 내보낸 dialog/session을 계속 사용하면 앱이 `pmtcon-gif-frame-sheet-v2` manifest와
   page slot을 유지하므로 JSON을 다시 넣지 않는다. 수정된 결과 page만 놓는다.
2. 앱을 재시작했거나 별도 `교체하기`로 들어온 경우에는 복구 fallback으로
   `frames_manifest.json`을 선택한 뒤 수정 결과를 연결한다.
3. 수정된 PNG/JPG/JPEG/static WebP 결과를 page slot에 놓는다. 이름이 바뀌었거나
   여러 page라면 각 결과를 해당 `frames_sheet_*.png` slot에 직접 연결한다.
4. PNG는 권장 형식이다. JPG/JPEG/static WebP 또는 완전 불투명 PNG는 exact canvas
   검사 뒤 `배경 포함으로 계속`을 명시적으로 선택해야 내부 PNG로 정규화된다.
   animated WebP는 첫 frame으로 바꾸지 않고 오류로 알린다.
5. painted checker는 실제 투명이 아니라는 경고를 표시하고 자동 제거하지 않는다.
   배경 포함을 선택하면 checker pixel도 결과 GIF에 남는다.
6. frame count, missing page, changed dimension, loop mode, duration summary와 전체
   animation의 style drift·깜빡임을 검토한다. AI가 원본 그림체나 frame 일관성을
   보장한다고 가정하지 않는다.
7. mismatch가 있거나 불투명 결과의 명시적 선택이 없으면 자동 진행하지 않는다.
8. 문제가 없으면 `GIF variant 만들기`를 실행한다.
9. PMTCONCON Studio가 frame index 순서대로 셀을 crop하고 duration/loop mode를 유지해 animated GIF를 다시 만든다.
10. 결과는 `processed_variants/gif_frame_reimports` 계열의 새 GIF 파일이며 원본 GIF는 보존한다.
11. single GIF 아이콘이고 선택한 export profile 셀 크기가 프레임 시트 셀 크기와 같으면 export 활성 variant로 설정할 수 있다.
12. 프레임 시트에는 내보낼 당시의 source와 crop·회전·반전·텍스트·정적 효과·모션·반복 recipe hash가 기록된다. 그 뒤 원본이나 recipe가 바뀌었다면 재가져온 GIF는 파일 variant로만 보존하고 현재 export의 활성 variant로 잘못 연결하지 않는다.
13. 선택한 대상 아이콘 ID와 manifest의 icon_id가 다르면 파일을 만들기 전에 중단한다.
14. manifest의 ID·결과 파일명·좌표·페이지/프레임/픽셀 작업량을 먼저 검증하고, 검증한 동일 이미지 스냅샷으로 GIF를 조립한다. DB 저장이 실패하면 생성 중인 파일도 정리한다.

## 우클릭 기반 작업 시트 흐름

- GIF 아이콘 우클릭:
  - `GIF 프레임 작업시트로 내보내기...`는 모든 프레임을 PNG 작업 시트와 guide 시트, manifest로 내보냅니다.
  - `GIF 프레임 작업시트로 교체하기...`는 수정된 프레임 작업 시트를 manifest로 검증한 뒤 새 GIF variant를 만듭니다.
  - UI에는 교체라고 표시되지만 원본 GIF 파일은 덮어쓰지 않습니다.
- 여러 아이콘 선택 후 우클릭:
  - `선택 항목 N개 작업시트로 내보내기...`는 선택된 아이콘만 정적 작업 시트로 내보냅니다.
  - 순서는 현재 모음의 grid/order_index를 따릅니다.
  - GIF 또는 모션 적용 아이콘은 정적 작업 시트에서 0ms 포스터 프레임만 포함되고 애니메이션 손실 경고가 표시됩니다.
- 모음 카드 우클릭:
  - `모음 복제`는 원본 모음을 변경하지 않고 복사본 모음을 생성합니다.
- 아이콘 우클릭:
  - `메모 추가`, `메모 수정`, `메모 삭제`로 아이콘별 작업 메모를 관리합니다.
  - 메모는 alt, export filename, 검증 결과에 영향을 주지 않습니다.

## 시트 프리셋

- `시트에서 가져오기`, `시트로 내보내기`, `GIF 프레임 작업시트로 내보내기`는 같은 프리셋 시스템을 사용합니다.
- 내보내기에서 저장한 cell size, columns, gap, border, background, max size, guide/manifest 옵션을 가져오기에서 다시 적용할 수 있습니다.
- 기본 제공 프리셋:
  - `DCInside 200x200 / 5 columns`
  - `GIF Frames 200x200 / 8 columns / 64 frames`
- 기본 제공 프리셋은 삭제할 수 없고, 복제해서 사용자 프리셋으로 편집합니다.
## 직접 Slice 지정

고정 grid나 cell-size로 맞지 않는 시트는 `시트에서 가져오기`에서 `직접 Slice 지정`을 사용한다.

1. PNG/JPG/JPEG 시트를 선택한다.
2. 방식 단계에서 `직접 Slice 지정`을 선택한다.
3. 시트 미리보기 위에서 드래그해서 사각형 Slice를 만든다.
4. 필요하면 `Slice 추가`로 기본 크기 Slice를 만들고, overlay에서 이동하거나 오른쪽 아래 핸들로 크기를 조정한다.
5. 오른쪽 좌표 패널에서 X/Y/W/H를 정확히 입력한다.
6. Slice 이름을 지정하고 `포함` 여부를 조정한다.
7. 필요한 Slice는 복제하거나 삭제한다.
8. `metadata 저장`으로 현재 Slice 목록을 app data에 JSON으로 저장할 수 있다.
9. `포함 Slice 가져오기`를 실행한다.

가져오기 결과:

- 포함된 정상 범위 Slice만 새 아이콘으로 등록된다.
- 원본 시트는 `sheet_imports/original_sheets/`에 보존된다.
- Slice 결과는 PNG로 추출되어 alpha를 보존한다.
- 범위를 벗어난 Slice는 건너뛰고 사용자에게 보고한다.
- 이 기능은 수동 workflow이므로 빈 투명 Slice를 자동 제외하지 않는다.

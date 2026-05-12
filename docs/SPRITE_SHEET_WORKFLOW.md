# 스프라이트 시트 / 작업 시트 워크플로우

PMTCONCON Studio의 시트 도구는 자동 마법사가 아니라 제작자가 수치를 정하고 검토한 뒤 실행하는 전문 작업 흐름이다.

## 시트 가져오기

1. 모음 화면에서 `시트 가져오기`를 연다.
2. PNG, JPG, JPEG 시트 이미지를 선택한다.
3. 분할 방식을 선택한다.
   - `Grid로 자르기`: 행/열 기준.
   - `셀 크기로 자르기`: 셀 너비/높이 기준.
   - `Manifest로 복원`: 이전에 내보낸 `pmtcon-sheet-v1`로 다시 가져오기.
   - `직접 Slice 지정`, `자동 감지`: 준비 중 상태이며 MVP에서는 실행하지 않는다.
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

## 작업 시트로 내보내기

1. 모음 화면에서 `작업 시트`를 연다.
2. 현재 모음 전체를 대상으로 한다. MVP에서는 선택 아이콘/문제 항목 필터는 아직 노출하지 않는다.
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
- GIF 아이콘은 정적 contact sheet로 첫 프레임만 포함되며, GIF 재조립용이 아니다.

## 수정된 시트 다시 가져오기

1. `시트 가져오기`에서 `Manifest로 복원`을 선택한다.
2. `sheet_manifest.json`을 선택한다.
3. 수정한 clean sheet PNG 파일을 선택한다.
4. `매니페스트로 다시 가져오기`를 실행한다.

다시 가져오기 규칙:

- 매니페스트의 `page_index`, `x`, `y`, `w`, `h`를 기준으로 셀을 crop한다.
- 수정된 시트 크기가 매니페스트와 다르면 경고한다.
- 누락 페이지 또는 범위 밖 셀은 건너뛰고 보고한다.
- 기본 결과는 새 아이콘이다.
- 원본 아이콘과 원본 파일은 덮어쓰지 않는다.

## GIF 프레임 시트로 내보내기

이 기능은 다음 단계의 구현 대상이다. MVP에서는 schema와 page planning만 준비되어 있고 클릭 가능한 메뉴는 없다.

계획된 흐름:

1. GIF 아이콘 하나를 선택한다.
2. `GIF 프레임 시트로 내보내기`를 연다.
3. frame count, duration, loop mode, 예상 page 수를 확인한다.
4. frame cell size, columns, frames per page, max sheet size, gap, border, background를 설정한다.
5. 모든 프레임을 PNG frame sheet로 내보낸다.
6. `pmtcon-gif-frame-sheet-v1`에 duration, loop mode, frame index, page mapping을 저장한다.

## GIF 프레임 시트 다시 가져오기

이 기능은 다음 단계의 구현 대상이다.

계획된 흐름:

1. `pmtcon-gif-frame-sheet-v1` manifest를 선택한다.
2. 수정된 frame sheet PNG 파일을 선택한다.
3. frame count, missing page, changed dimension, loop mode, duration summary를 검토한다.
4. mismatch가 있으면 자동 진행하지 않는다.
5. 문제가 없으면 새 animated GIF processed variant를 만든다.
6. 원본 GIF는 보존한다.

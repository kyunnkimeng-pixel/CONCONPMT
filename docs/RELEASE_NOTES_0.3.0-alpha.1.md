# PMTCONCON Studio 0.3.0-alpha.1 릴리스 노트

PMTCONCON Studio 0.3.0-alpha.1은 AI 이미지 결과를 원본을 훼손하지 않고
가져오고, 비교하고, 규격화하고, 새 아이콘 또는 현재 아이콘에 적용하는 흐름을
검증하기 위한 프리릴리스입니다.

## 주요 변경 사항

- 로컬에 저장된 JPG/PNG AI 결과를 후보로 가져오는 전용 AI 작업공간을
  추가했습니다.
- 원본, AI 원본, 규격화 결과, 최종 적용 모습을 나란히 비교할 수 있습니다.
- 임의 크기 결과를 `전체 보이기` 또는 `빈틈 없이 채우기` 방식으로 현재
  아이콘 canvas에 맞출 수 있습니다.
- 기본 완료 동작은 원본 아이콘을 유지하는 `새 아이콘으로 추가`입니다.
- 호환되는 정적 후보는 현재 아이콘의 소스로 사용할 수 있으며, 원본과 이전
  AI 소스로 언제든 되돌릴 수 있습니다.
- 같은 후보로 만든 아이콘의 횟수와 최근 아이콘을 표시하고, 생성 후 바로
  열기·목록에서 보기·후보 비교 계속하기를 지원합니다.
- 중첩된 편집기와 내보내기 작업공간에서도 키보드 focus, Escape, 상태 알림과
  저장하지 않은 변경 확인이 일관되게 동작합니다.

## 비파괴 저장과 안전

- 가져온 원본 파일은 덮어쓰거나 삭제하지 않습니다.
- AI 요청, 후보, icon version, 활성 source와 원본 lineage를 서로 분리해
  SQLite에 저장합니다.
- 후보 원본과 규격화 결과는 별도 불변 source로 보존합니다.
- 적용과 복귀는 preview·source pointer·DB 변경을 하나의 보상 가능한 작업으로
  처리하며, 오래된 preview나 손상된 비활성 이력은 fail-closed로 거부합니다.
- preview, 내보내기, 용량 최적화, 정적 작업 시트와 GIF frame sheet가 같은
  effective source를 사용합니다.
- collection/icon 복제는 provider 실행이나 비용을 중복 기록하지 않으면서
  source lineage와 복귀 이력을 독립적으로 복제합니다.

## 이번 프리릴리스에 포함되지 않는 기능

- Gemini, NovelAI 또는 다른 공급자의 API 호출
- API key나 로그인 정보 입력·저장
- 웹사이트 자동 로그인, 자동 업로드, DOM 조작 또는 결과 자동 다운로드
- GIF 전체 frame AI 변환과 sprite-sheet 일괄 AI 생성

따라서 현재 `결과 가져오기`는 사용자가 다른 도구에서 직접 생성하고 저장한
JPG/PNG를 선택하는 로컬 기능입니다. 네트워크 전송은 발생하지 않습니다.
공식 사이트 열기와 prompt package를 제공하는 수동 웹 handoff는 다음 단계에서
별도 구현·검증합니다.

## 배포 참고

- 이 버전은 기능 검증용 프리릴리스입니다.
- 공개 설치 자산은 Windows x64 NSIS setup과 `SHA256SUMS.txt`입니다.
- 설치 파일은 코드 서명되지 않았으므로 Windows가 알 수 없는 게시자 경고를
  표시할 수 있습니다.
- MSI는 clean Windows VM 설치·제거 검증 전까지 공개하지 않습니다.
- PMTCONCON Studio 코드는 계속 MIT License로 배포되며 새 runtime dependency는
  추가되지 않았습니다.

# PMTCONCON Studio

PMTCONCON Studio는 DCInside 스타일 디시콘과 커스텀 이모티콘 팩을 가져오고, 정리하고, 편집하고, 검증해서 내보내는 Windows 데스크톱 앱입니다.

이미지와 GIF를 모음 단위로 관리하면서 alt 값, 순서, 자르기 영역, 다중 조각 아이콘, 내보내기 규칙을 한 화면 흐름에서 다룰 수 있도록 만드는 것이 목표입니다.

## 주요 기능

- 이미지와 GIF 파일 또는 폴더를 모음으로 가져오기
- Windows 파일 탐색기처럼 모음과 아이콘을 탐색, 이름 변경, 복제, 삭제, 정렬
- Ctrl/Shift 다중 선택과 드래그 앤 드롭 순서 변경
- 단일 아이콘, 가로 2칸, 세로 2칸 아이콘 편집
- 원본 파일을 보존하고 crop metadata를 따로 저장
- GIF 미리보기와 프레임 기반 crop/export 처리
- DCInside 댓글 영역과 비슷한 사용 미리보기
- DCInside 규칙과 커스텀 프로필 기반 내보내기 검증
- `alts.txt` 생성과 sequence/alt 기반 파일명 내보내기

## 현재 상태

이 저장소는 개발 중인 소스 코드입니다. 아직 일반 사용자용 설치 파일 배포보다는 로컬 빌드와 기능 검증에 초점이 맞춰져 있습니다.

## 개발 환경

필요한 도구:

- Node.js와 npm
- Rust toolchain
- Tauri 2 개발에 필요한 Windows 빌드 도구

의존성 설치:

```powershell
npm install
```

개발 실행:

```powershell
npm run tauri:dev
```

검사:

```powershell
npm run lint
npm run test
npm run build
```

데스크톱 앱 빌드:

```powershell
npm run tauri:build
```

## 기술 스택

- Tauri 2 + Rust
- React 19 + TypeScript + Vite
- Tailwind CSS v4
- TanStack Router
- dnd-kit
- react-konva
- SQLite

## 문서

- [제품 명세](docs/PRODUCT_SPEC.md)
- [기능 구현 현황](docs/FEATURE_INVENTORY.md)
- [구현 계획](docs/IMPLEMENTATION_PLAN.md)
- [아키텍처](docs/ARCHITECTURE.md)
- [기술 결정 기록](docs/DECISIONS.md)
- [UI reference trace](docs/UI_TRACE.md)

## 라이선스

이 프로젝트의 라이선스는 [LICENSE](LICENSE)를 확인하세요.

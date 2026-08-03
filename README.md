# PMTCONCON Studio

PMTCONCON Studio는 DCInside 스타일 디시콘과 커스텀 이모티콘 모음을 제작하기 위한 Windows 데스크톱 앱입니다. 이미지와 GIF를 가져오고, 모음 단위로 정리하고, 아이콘과 작업 시트를 편집한 뒤, 업로드 가능한 파일과 `alts.txt`를 내보내는 작업 흐름을 제공합니다.

최신 Windows 설치파일은 [GitHub Releases](https://github.com/kyunnkimeng-pixel/CONCONPMT/releases)에서 받을 수 있습니다.

상세 사용 설명서는 [PMTCONCON Studio 사용 설명서](https://kyunnkimeng-pixel.github.io/CONCONPMT/)에서 볼 수 있습니다.
릴리스 전 확인 범위는 [Release Readiness](docs/RELEASE_READINESS.md)에 정리되어 있습니다.
AI 프리릴리스 검증 범위는 [Release Readiness 0.3.0-alpha.1](docs/RELEASE_READINESS_0.3.0-alpha.1.md)에 별도로 기록합니다.
최신 안정 버전의 변경 사항은 [Release Notes 0.2.0](docs/RELEASE_NOTES_0.2.0.md)에서 볼 수 있습니다.
AI 후보 작업공간 프리릴리스는 [Release Notes 0.3.0-alpha.1](docs/RELEASE_NOTES_0.3.0-alpha.1.md)에서 확인할 수 있습니다.
이미지 표시 핫픽스는 [Release Notes 0.3.0-alpha.2](docs/RELEASE_NOTES_0.3.0-alpha.2.md)에서 확인할 수 있습니다.
GIF AI 전달 UX·배경 검토·NovelAI 안내 개선 프리릴리스는 [Release Notes 0.3.0-alpha.5](docs/RELEASE_NOTES_0.3.0-alpha.5.md)에서 확인할 수 있습니다.
검증·패키징 결과는 [Release Readiness 0.3.0-alpha.5](docs/RELEASE_READINESS_0.3.0-alpha.5.md)에 기록했습니다.

## 화면

### 모음 탐색과 alt 일괄 변경

아이콘을 카드형 탐색 화면에서 확인하고, alt 값 오류나 중복 상태를 바로 볼 수 있습니다. 여러 아이콘을 선택한 뒤 쉼표로 구분한 alt 값을 순서대로 일괄 적용할 수 있고, 파일/폴더 추가, 정렬, 대표 이미지 설정, 빈 디시콘 생성, 내보내기 작업으로 이어지는 도구 모음을 제공합니다.

![PMTCONCON Studio main collection view](docs/manual-assets/manual-01-explorer-wide-disicon-3.png)

### crop 박스 크기 조절

원본 이미지는 그대로 보존하고, 출력용 crop 박스만 별도로 저장합니다. 기본 DCInside 셀 크기는 200x200이며, crop 박스는 이미지 위에서 원하는 위치로 옮기거나 크기를 조정한 뒤 적용할 수 있습니다.

![PMTCONCON Studio crop editor with resized box](docs/manual-assets/manual-05-single-crop-resized-disicon-3.png)

### 가로 2칸 자르기

한 아이콘을 2개의 200x200 조각으로 내보내야 하는 경우, 가로 2칸 모양을 선택해 400x200 crop 영역을 잡을 수 있습니다. 편집 화면은 분할선을 표시하고, 내보내기 단계에서는 왼쪽과 오른쪽 조각을 export 순서에 맞춰 생성합니다.

![PMTCONCON Studio horizontal double crop editor](docs/manual-assets/manual-06-horizontal-double-crop-disicon-3.png)

### 텍스트 추가와 고급 편집

텍스트 오버레이를 켜고 문구, 크기, 위치, 글자색, 외곽선, 폰트를 조정할 수 있습니다. 같은 고급 편집 화면에서 실제 export 후보를 생성해 용량을 측정하고, GIF 재생 FPS, 색상 수, JPG 품질 같은 압축 옵션도 적용할 수 있습니다.

![PMTCONCON Studio text overlay editor](docs/manual-assets/manual-08-advanced-gif-text-disicon-3.png)

### 내보내기 작업공간

내보낼 원본과 생성 결과를 나란히 보며, 파일명, 형식, 리사이즈 필터, MB 단위 용량 제한, `alts.txt` 생성 여부를 확인합니다. 항목별 업로드 가능 여부, 경고, 용량 초과, 수정 진입점을 분리해서 표시하고, 선택한 항목만 다시 내보내거나 용량 압축 후보를 일괄 적용할 수 있습니다.

![PMTCONCON Studio export workspace](docs/manual-assets/manual-10-export-workspace-disicon-3.png)

### 스프라이트 시트와 작업 시트

큰 PNG/JPG 시트를 격자로 잘라 새 아이콘으로 가져오고, 선택한 아이콘이나 모음 전체를 clean sheet, guide sheet, manifest JSON으로 내보낼 수 있습니다. 수정된 작업 시트는 manifest로 다시 가져와 원본을 덮어쓰지 않고 새 아이콘이나 처리 variant로 저장합니다.

![PMTCONCON Studio work sheet export dialog](docs/manual-assets/manual-14-work-sheet-export-dialog.png)

### GIF 프레임 작업 시트

GIF 아이콘은 우클릭 메뉴에서 모든 프레임을 PNG frame sheet로 내보내고, 수정된 frame sheet를 manifest의 페이지 파일명으로 다시 가져와 새 animated GIF variant를 만들 수 있습니다. 프레임별 시간과 once/infinite/finite 반복 설정을 복원하며 원본 GIF 파일은 보존됩니다.

![PMTCONCON Studio GIF frame sheet export dialog](docs/manual-assets/manual-16-gif-frame-sheet-export.png)
### AI 그리드와 수동 웹 전달

모음 도구의 `AI 아이콘 만들기`에서 원본 없이 한 개 또는 최대 16개 이모티콘을 만들 수 있습니다. 모음에 있는 단일 아이콘과 외부 PNG/JPG/GIF를 최대 16개까지 참고 이미지로 골라 한 장의 별도 reference board로 전달할 수도 있습니다. 외부 파일은 합계 16MB, 전체 참고 원본은 1.28억 픽셀로 제한하며 비정사각형 이미지는 늘이지 않습니다. GIF 참고는 첫 프레임 포스터로 표시되며, 프롬프트가 참고 배치와 실제 출력 그리드를 명확히 구분합니다. 정적 단일 아이콘 2–16개를 Ctrl/Shift로 선택한 뒤 우클릭하면 한 장의 clean grid로 일괄 수정할 수 있습니다. 웹에서 받은 결과 시트는 셀 매핑을 검토한 뒤에만 비활성 후보 또는 새 아이콘으로 모두 함께 저장됩니다.

정적 단일 결과가 200×200 대신 1024×1024처럼 같은 비율로 돌아오면 원본 결과를 보존하고 경고한 뒤 앱의 후보 검토에서 목표 크기로 정규화합니다. 비율이 다르거나 필요한 투명도가 사라진 결과는 계속 차단합니다. GIF는 편집기의 `GIF AI 프레임 시트 작업 시작`에서 clean PNG와 manifest를 내보내고, 구조 보호 프롬프트와 함께 Gemini AI Studio/NovelAI 웹으로 전달한 뒤 timing·순서·loop를 복원해 별도 GIF 처리 버전으로 다시 가져옵니다.

#### NovelAI 웹 사용 팁

NovelAI는 자연어도 이해하지만, 앱은 결과 제어가 쉬운 `lower-case, comma-separated` 영문 태그를 Prompt용으로 준비하고 제외 태그는 `Undesired Content`용으로 따로 복사합니다. 업로드 뒤에는 목적에 맞춰 다음 방식을 고릅니다.

- 기존 아이콘·그리드·GIF 프레임 배치 유지: `Image2Image`
- 그림체·색감·질감 참고: `Vibe Transfer`
- V4.5에서 캐릭터/스타일 일관성 참고: `Precise Reference` (Vibe Transfer와 동시 사용 불가)

전달 이미지는 `Add a Base Img (Optional)`로 올립니다. `What do you want to do with this image?` 창이 뜨면 `Image2Image`를 고르고, 바로 base image가 붙으면 이어서 보이는 Strength/Noise를 낮게 시작하세요. PNG 설정 경로는 `메뉴(☰) → Account Settings → Image Settings 탭 → Image Generation → Image Format for Generated Images → PNG`입니다. 앱에서는 먼저 `1/2 Prompt`를 복사한 뒤에만 `2/2 Undesired Content` 버튼이 열리며, 요청을 바꾸면 다시 1단계로 돌아갑니다. 정적 단일·그리드는 Download Image로 받은 PNG/JPG/WebP를 가져올 수 있고 정적 WebP는 alpha를 보존한 내부 PNG로 자동 변환합니다. animated WebP는 첫 프레임으로 바꾸지 않고 오류로 알립니다. 투명 배경이 채워졌다면 `Director Tools → Remove BG`를 적용하세요. 200×200이 192×192처럼 바뀌어도 단일 아이콘은 정사각 비율과 필요한 투명도가 유지되면 적용 시 목표 크기로 맞춥니다. 그리드와 GIF 시트는 정확한 캔버스가 필요합니다. GIF clean PNG는 한 장씩 같은 Prompt·Strength·Noise·sampler 설정으로 처리하되, 해상도는 앱이 표시한 페이지별 실제 캔버스로 바꾼 뒤 다시 가져오기 슬롯에 연결합니다. NovelAI가 파일명을 바꾸어도 직접 연결할 수 있고 Chrome의 `(1)` 접미사는 모호하지 않을 때만 자동 인식합니다. 다중 페이지의 마지막 남은 파일은 추측하지 않습니다. GIF 결과는 PNG를 권장하지만 JPG/JPEG·정적 WebP도 실제 디코딩 형식과 exact canvas를 검사해 내부 PNG로 변환하며, 불투명 결과는 배경 포함 동의를 요구합니다. GIF·animated WebP는 지원하지 않아 구체적인 안내와 함께 차단합니다. manifest는 4MB, 페이지는 500개, 결과 이미지 한 장과 전체 전달은 각각 64MB 한도에서 앱이 읽기 전에 검사합니다. `NovelAI 웹 호환 GIF / 200x200 / 4x4` 프리셋을 권장하며, 생성 전 Generate 버튼의 Anlas 비용을 확인하세요.
Windows에서는 앱이 다시 검증한 관리형 파일을 공식 웹의 업로드 영역까지 직접 끌 수 있고, 키보드·비Windows·호환성 문제에는 Explorer 선택을 사용합니다. 앱은 로그인, DOM, cookie, 생성 완료 또는 다운로드를 자동 제어하지 않습니다. 사이드바 `최근 AI 전달`에서 256MiB 임시 저장 공간, 만료/정리 상태와 최근 기록을 확인할 수 있으며 앱이 켜져 있으면 15분마다 만료 정리를 다시 시도합니다.

## 주요 기능

- Windows 파일 탐색기와 비슷한 모음/아이콘 관리 화면
- JPG, PNG, 애니메이션 GIF 파일 및 폴더 가져오기
- 원본 파일 보존과 crop, 크기, 순서, 내보내기 설정의 별도 저장
- 200x200 기준 crop 박스 이동, 크기 조정, 자유/고정 모드 편집
- 단일 아이콘, 가로 2칸 아이콘, 세로 2칸 아이콘 편집과 조각별 내보내기
- GIF 미리보기, 프레임 기반 crop/export, 반복 설정 지원
- 아이콘 이름과 조각별 alt 값 인라인 편집
- 선택 아이콘 alt 값 일괄 변경과 쉼표 기반 순차 입력
- Ctrl/Shift 다중 선택과 드래그 앤 드롭 순서 변경
- 텍스트 오버레이 추가, 위치/크기/색상/외곽선/폰트 조정
- GIF FPS/색상 수 조정, JPG 품질 후보 생성
- GIF 재생 FPS 실시간 편집 미리보기와 variant 적용
- DCInside 규칙과 커스텀 프로필 기반 내보내기 검증
- Nearest, Bilinear, Bicubic, Gaussian, Lanczos 리사이즈 필터 선택
- 선택 항목만 다시 내보내기와 선택 항목 용량 압축
- 순서 기반 파일명 또는 alt 기반 파일명 내보내기
- 선택 가능한 `alts.txt` 생성
- 스프라이트 시트 가져오기와 manifest 기반 작업 시트 재가져오기
- 선택 아이콘만 작업 시트로 내보내기
- GIF 프레임 작업 시트 내보내기/교체하기와 Gemini/NovelAI 수동 웹 AI 왕복
- 정적 단일 아이콘 웹 AI 왕복: 검증 파일 직접 끌기/Explorer, 프롬프트, 결과 검사와 비활성 후보 저장
- 선택한 정적 단일 아이콘 2–16개 AI 그리드 일괄 수정과 all-or-none 후보 저장
- 원본 없는 단일/최대 16개 그리드 아이콘 생성, 내부·외부 참고 이미지 board와 atomic 새 아이콘 저장
- 최근 AI 전달 목록, 256MiB 임시 저장 제한, 앱 실행 중 15분 주기 만료 정리
- 아이콘 메모와 hover 표시
- 가져오기/내보내기/GIF frame sheet에 공유되는 grid 프리셋
- 직접 Slice 지정과 자동 감지(실험) proposal 적용

## DCInside 내보내기

기본 DCInside 프로필은 200x200 출력 셀과 JPG, PNG, GIF 형식을 기준으로 동작합니다. 내보내기 전에 출력 개수, 파일 크기, 형식, alt 값 길이, alt 중복, 파일명 안전성을 검증합니다.

커스텀 모음에서는 셀 크기, 미리보기 표시 크기, 출력 형식, 파일 크기 제한, 파일명 방식을 별도로 설정할 수 있습니다.

## 개발 환경

필요한 도구:

- Node.js와 npm
- Rust toolchain
- Tauri 2 개발에 필요한 Windows 빌드 도구

의존성 설치:

```powershell
npm.cmd install
```

개발 실행:

```powershell
npm.cmd run tauri:dev
```

검사:

```powershell
npm.cmd run lint
npm.cmd run test
npm.cmd run build
```

데스크톱 앱 빌드:

```powershell
npm.cmd run tauri:build
```

## 기술 스택

- Tauri 2 + Rust
- React 19 + TypeScript + Vite
- Tailwind CSS
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
- [Release Readiness](docs/RELEASE_READINESS.md)
- [Release Readiness 0.3.0-alpha.1](docs/RELEASE_READINESS_0.3.0-alpha.1.md)
- [Release Notes 0.2.0](docs/RELEASE_NOTES_0.2.0.md)
- [Release Notes 0.3.0-alpha.1](docs/RELEASE_NOTES_0.3.0-alpha.1.md)
- [Installer Distribution QA](docs/INSTALLER_DISTRIBUTION_QA.md)
- [Installer Distribution QA 0.3.0-alpha.1](docs/INSTALLER_DISTRIBUTION_QA_0.3.0-alpha.1.md)
- [라이선스 정책](docs/LICENSE_POLICY.md)
- [서드파티 라이선스 고지 안내](docs/THIRD_PARTY_LICENSES_GUIDE.md)

## 라이선스

PMTCONCON Studio는 [MIT License](LICENSE)로 배포됩니다.

서드파티 의존성 고지는 [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md)에 정리되어 있습니다.

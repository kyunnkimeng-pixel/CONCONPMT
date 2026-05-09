# PMTCONCON Studio Codex Pack

PMTCONCON Studio를 Codex Windows App으로 제작하기 위한 문서 패키지다.

## 파일 구성

- `AGENTS.md`: Codex가 프로젝트에서 반드시 따라야 할 규칙.
- `CODEX_COMMANDS.md`: Windows 개발 환경 준비, Tauri scaffold, Codex Windows App 진행 가이드.
- `WINDOWS_APP_THREAD_PROMPTS.md`: Codex App thread에 바로 붙여넣는 프롬프트 모음.
- `INITIAL_CODEX_PROMPT.md`: MVP 구현 시작용 긴 프롬프트.
- `docs/PRODUCT_SPEC.md`: 제품 명세.
- `docs/FEATURE_INVENTORY.md`: 기능 누락 방지 체크리스트.
- `docs/IMPLEMENTATION_PLAN.md`: phase별 구현 계획.
- `docs/UI_IMAGE_PROMPT.md`: UI reference 이미지 생성 프롬프트.
- `docs/DECISIONS.md`: 기술 선택/아키텍처 결정 기록.

## 핵심 원칙

1. 앱 이름은 **PMTCONCON Studio**다.
2. 기능 원본은 `PRODUCT_SPEC.md`와 `FEATURE_INVENTORY.md`다.
3. 생성 UI 이미지는 참고용이며, 가짜 메뉴를 추가하거나 필수 기능을 삭제하면 안 된다.
4. 원본 이미지/GIF는 보존하고 crop metadata를 따로 저장한다.
5. DCInside export 규칙과 custom emoticon profile을 모두 지원한다.

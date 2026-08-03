export const AI_WORKSPACE_TABS = [
  { value: "import", label: "AI 수정·가져오기" },
  { value: "review", label: "후보 검토" },
  { value: "history", label: "소스 이력" },
] as const;

export type AiWorkspaceView = (typeof AI_WORKSPACE_TABS)[number]["value"];

export const AI_COMPARE_VIEWS = [
  { value: "original", label: "원본" },
  { value: "raw", label: "AI 원본" },
  { value: "normalized", label: "규격화 결과" },
  { value: "final", label: "최종 적용 모습" },
  { value: "overlay", label: "겹쳐 보기" },
] as const;

export type AiCompareView = (typeof AI_COMPARE_VIEWS)[number]["value"];
export type AiCompareZoom = "fit" | "actual";

export type AiWorkspaceUiState = {
  view: AiWorkspaceView;
  compareView: AiCompareView;
  compareZoom: AiCompareZoom;
  checkerboardEnabled: boolean;
};

export type AiWorkspaceUiAction =
  | { type: "set_view"; view: AiWorkspaceView }
  | { type: "set_compare_view"; view: AiCompareView }
  | { type: "set_compare_zoom"; zoom: AiCompareZoom }
  | { type: "set_checkerboard"; enabled: boolean }
  | { type: "toggle_checkerboard" }
  | { type: "reset" };

export type AiWorkspaceLayout = {
  mode: "wide" | "narrow";
  candidateRailOrientation: "vertical" | "horizontal";
  inspectorPlacement: "right" | "bottom";
};

export const AI_WORKSPACE_WIDE_BREAKPOINT = 1024;

export function createInitialAiWorkspaceUiState(): AiWorkspaceUiState {
  return {
    view: "import",
    compareView: "final",
    compareZoom: "fit",
    checkerboardEnabled: true,
  };
}

export function aiWorkspaceUiReducer(
  state: AiWorkspaceUiState,
  action: AiWorkspaceUiAction,
): AiWorkspaceUiState {
  switch (action.type) {
    case "set_view":
      return state.view === action.view
        ? state
        : { ...state, view: action.view };
    case "set_compare_view":
      return state.compareView === action.view
        ? state
        : { ...state, compareView: action.view };
    case "set_compare_zoom":
      return state.compareZoom === action.zoom
        ? state
        : { ...state, compareZoom: action.zoom };
    case "set_checkerboard":
      return state.checkerboardEnabled === action.enabled
        ? state
        : { ...state, checkerboardEnabled: action.enabled };
    case "toggle_checkerboard":
      return { ...state, checkerboardEnabled: !state.checkerboardEnabled };
    case "reset":
      return createInitialAiWorkspaceUiState();
  }
}

export function nextAiWorkspaceTab(
  tab: AiWorkspaceView,
  key: string,
): AiWorkspaceView | null {
  const tabs = AI_WORKSPACE_TABS.map(({ value }) => value);
  const currentIndex = tabs.indexOf(tab);

  switch (key) {
    case "ArrowRight":
      return tabs[(currentIndex + 1) % tabs.length];
    case "ArrowLeft":
      return tabs[(currentIndex - 1 + tabs.length) % tabs.length];
    case "Home":
      return tabs[0];
    case "End":
      return tabs[tabs.length - 1];
    default:
      return null;
  }
}

export function nextAiCandidateIndex(
  index: number,
  count: number,
  key: string,
): number | null {
  const candidateCount = Number.isFinite(count)
    ? Math.max(0, Math.trunc(count))
    : 0;
  if (candidateCount === 0) {
    return null;
  }

  if (key === "Home") {
    return 0;
  }
  if (key === "End") {
    return candidateCount - 1;
  }

  const movesForward = key === "ArrowRight" || key === "ArrowDown";
  const movesBackward = key === "ArrowLeft" || key === "ArrowUp";
  if (!movesForward && !movesBackward) {
    return null;
  }

  const hasCurrentCandidate =
    Number.isInteger(index) && index >= 0 && index < candidateCount;
  if (!hasCurrentCandidate) {
    return movesForward ? 0 : candidateCount - 1;
  }

  return movesForward
    ? (index + 1) % candidateCount
    : (index - 1 + candidateCount) % candidateCount;
}

export function aiWorkspaceLayoutForWidth(
  width: number,
): AiWorkspaceLayout {
  if (width >= AI_WORKSPACE_WIDE_BREAKPOINT) {
    return {
      mode: "wide",
      candidateRailOrientation: "vertical",
      inspectorPlacement: "right",
    };
  }

  return {
    mode: "narrow",
    candidateRailOrientation: "horizontal",
    inspectorPlacement: "bottom",
  };
}

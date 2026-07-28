import { describe, expect, it } from "vitest";

import {
  AI_COMPARE_VIEWS,
  AI_WORKSPACE_TABS,
  aiWorkspaceLayoutForWidth,
  aiWorkspaceUiReducer,
  createInitialAiWorkspaceUiState,
  nextAiCandidateIndex,
  nextAiWorkspaceTab,
} from "@/features/editor/ai-workspace-model";

describe("AI workspace UI model", () => {
  it("keeps the fixed workspace tab order and Korean labels", () => {
    expect(AI_WORKSPACE_TABS).toEqual([
      { value: "import", label: "AI 수정·가져오기" },
      { value: "review", label: "후보 검토" },
      { value: "history", label: "소스 이력" },
    ]);
  });

  it("keeps all explicit compare views and Korean labels", () => {
    expect(AI_COMPARE_VIEWS).toEqual([
      { value: "original", label: "원본" },
      { value: "raw", label: "AI 원본" },
      { value: "normalized", label: "규격화 결과" },
      { value: "final", label: "최종 적용 모습" },
      { value: "overlay", label: "겹쳐 보기" },
    ]);
  });

  it("creates a fresh, safe initial state", () => {
    const first = createInitialAiWorkspaceUiState();
    const second = createInitialAiWorkspaceUiState();

    expect(first).toEqual({
      view: "import",
      compareView: "final",
      compareZoom: "fit",
      checkerboardEnabled: true,
    });
    expect(second).toEqual(first);
    expect(second).not.toBe(first);
  });

  it("reduces tabs, comparison, zoom and checkerboard", () => {
    const initial = createInitialAiWorkspaceUiState();
    const reviewing = aiWorkspaceUiReducer(initial, {
      type: "set_view",
      view: "review",
    });
    const comparing = aiWorkspaceUiReducer(reviewing, {
      type: "set_compare_view",
      view: "overlay",
    });
    const actualSize = aiWorkspaceUiReducer(comparing, {
      type: "set_compare_zoom",
      zoom: "actual",
    });
    const hiddenCheckerboard = aiWorkspaceUiReducer(actualSize, {
      type: "set_checkerboard",
      enabled: false,
    });
    const shownCheckerboard = aiWorkspaceUiReducer(hiddenCheckerboard, {
      type: "toggle_checkerboard",
    });

    expect(initial).toEqual(createInitialAiWorkspaceUiState());
    expect(shownCheckerboard).toEqual({
      view: "review",
      compareView: "overlay",
      compareZoom: "actual",
      checkerboardEnabled: true,
    });
  });

  it("preserves state identity for idempotent actions and resets all UI draft", () => {
    const initial = createInitialAiWorkspaceUiState();

    expect(
      aiWorkspaceUiReducer(initial, { type: "set_view", view: "import" }),
    ).toBe(initial);
    expect(
      aiWorkspaceUiReducer(initial, {
        type: "set_compare_view",
        view: "final",
      }),
    ).toBe(initial);
    expect(
      aiWorkspaceUiReducer(initial, {
        type: "set_compare_zoom",
        zoom: "fit",
      }),
    ).toBe(initial);
    expect(
      aiWorkspaceUiReducer(initial, {
        type: "set_checkerboard",
        enabled: true,
      }),
    ).toBe(initial);

    const changed = {
      ...initial,
      view: "history" as const,
      compareView: "raw" as const,
      compareZoom: "actual" as const,
      checkerboardEnabled: false,
    };
    expect(aiWorkspaceUiReducer(changed, { type: "reset" })).toEqual(initial);
  });

  it("moves workspace tabs with ArrowLeft/Right and wraps at both ends", () => {
    expect(nextAiWorkspaceTab("import", "ArrowRight")).toBe("review");
    expect(nextAiWorkspaceTab("review", "ArrowRight")).toBe("history");
    expect(nextAiWorkspaceTab("history", "ArrowRight")).toBe("import");
    expect(nextAiWorkspaceTab("import", "ArrowLeft")).toBe("history");
    expect(nextAiWorkspaceTab("history", "ArrowLeft")).toBe("review");
  });

  it("supports tab Home/End and ignores unrelated keys", () => {
    expect(nextAiWorkspaceTab("review", "Home")).toBe("import");
    expect(nextAiWorkspaceTab("review", "End")).toBe("history");
    expect(nextAiWorkspaceTab("review", "Enter")).toBeNull();
    expect(nextAiWorkspaceTab("review", "Tab")).toBeNull();
  });

  it("moves candidate selection in either rail orientation and wraps", () => {
    expect(nextAiCandidateIndex(0, 3, "ArrowRight")).toBe(1);
    expect(nextAiCandidateIndex(1, 3, "ArrowDown")).toBe(2);
    expect(nextAiCandidateIndex(2, 3, "ArrowRight")).toBe(0);
    expect(nextAiCandidateIndex(0, 3, "ArrowLeft")).toBe(2);
    expect(nextAiCandidateIndex(2, 3, "ArrowUp")).toBe(1);
  });

  it("supports candidate Home/End and a missing current selection", () => {
    expect(nextAiCandidateIndex(1, 4, "Home")).toBe(0);
    expect(nextAiCandidateIndex(1, 4, "End")).toBe(3);
    expect(nextAiCandidateIndex(-1, 4, "ArrowRight")).toBe(0);
    expect(nextAiCandidateIndex(-1, 4, "ArrowLeft")).toBe(3);
  });

  it("does not move candidates for empty lists or unrelated keys", () => {
    expect(nextAiCandidateIndex(0, 0, "ArrowRight")).toBeNull();
    expect(nextAiCandidateIndex(0, Number.NaN, "ArrowRight")).toBeNull();
    expect(nextAiCandidateIndex(0, 3, "Enter")).toBeNull();
  });

  it("uses the wide three-column arrangement at 1024px and above", () => {
    expect(aiWorkspaceLayoutForWidth(1024)).toEqual({
      mode: "wide",
      candidateRailOrientation: "vertical",
      inspectorPlacement: "right",
    });
    expect(aiWorkspaceLayoutForWidth(1440)).toEqual(
      aiWorkspaceLayoutForWidth(1024),
    );
  });

  it("uses the narrow rail and bottom inspector below 1024px", () => {
    expect(aiWorkspaceLayoutForWidth(1023)).toEqual({
      mode: "narrow",
      candidateRailOrientation: "horizontal",
      inspectorPlacement: "bottom",
    });
    expect(aiWorkspaceLayoutForWidth(720)).toEqual(
      aiWorkspaceLayoutForWidth(1023),
    );
  });
});

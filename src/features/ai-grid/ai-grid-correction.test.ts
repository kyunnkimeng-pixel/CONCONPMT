import { describe, expect, it } from "vitest";

import { buildAiGridCorrectionPrompt } from "@/features/ai-grid/ai-grid-correction";
import type { AiGridWorkspace } from "@/features/ai-grid/types";
import type { SheetGridAnalysis } from "@/features/sheets/types";

const workspace = {
  itemCount: 4,
  requestScope: "grid_edit",
  layout: {
    canvasWidth: 1024,
    canvasHeight: 1024,
    rows: 2,
    columns: 2,
    cellSize: 500,
    gapX: 8,
    gapY: 8,
    borderLeft: 8,
    borderTop: 8,
    borderRight: 8,
    borderBottom: 8,
  },
} as AiGridWorkspace;

function analysis(
  overrides: Partial<SheetGridAnalysis> = {},
): SheetGridAnalysis {
  return {
    sheetWidth: 1024,
    sheetHeight: 1024,
    computedRows: 2,
    computedColumns: 2,
    cellCount: 4,
    outOfBoundsCells: [],
    emptyCellCandidates: [],
    cells: Array.from({ length: 4 }, (_, index) => ({
      index,
      page: 0,
      row: Math.floor(index / 2),
      col: index % 2,
      x: 8 + (index % 2) * 508,
      y: 8 + Math.floor(index / 2) * 508,
      w: 500,
      h: 500,
      outOfBounds: false,
      emptyCandidate: false,
    })),
    warnings: [],
    ...overrides,
  };
}

describe("buildAiGridCorrectionPrompt", () => {
  it("returns no invented prompt when geometry is valid", () => {
    expect(buildAiGridCorrectionPrompt(workspace, analysis())).toBeNull();
  });

  it("asks an edit provider to refill empty target cells", () => {
    const prompt = buildAiGridCorrectionPrompt(
      workspace,
      analysis({ emptyCellCandidates: [2] }),
    );

    expect(prompt).toContain("4개 셀을 모두 채우고");
  });

  it("describes exact canvas and grid repairs for a malformed result", () => {
    const malformed = analysis({
      sheetWidth: 896,
      cells: analysis().cells.slice(0, 3),
      outOfBoundsCells: [3],
    });
    const prompt = buildAiGridCorrectionPrompt(workspace, malformed);

    expect(prompt).toContain("1024×1024");
    expect(prompt).toContain("2행 × 2열");
    expect(prompt).toContain("정확히 4개");
    expect(prompt).toContain("정적 투명 PNG");
  });
});

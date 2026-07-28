import { describe, expect, it } from "vitest";

import {
  aiGridStepForStatus,
  buildAiGridPrompt,
  defaultResultMapping,
  reviewDecisions,
  sheetSettingsFromLayout,
  validateReviewDecisions,
} from "@/features/ai-grid/ai-grid-workspace-model";
import type { AiGridWorkspace } from "@/features/ai-grid/types";
import type { SheetCell, SheetGridAnalysis } from "@/features/sheets/types";

const workspace = (scope: AiGridWorkspace["requestScope"] = "grid_edit"): AiGridWorkspace => ({
  requestId: "request-grid-1",
  collectionId: "collection-1",
  requestScope: scope,
  status: "layout_review_pending",
  retryOfRequestId: null,
  layout: {
    canvasWidth: 1024,
    canvasHeight: 1024,
    rows: 1,
    columns: 2,
    cellSize: 504,
    gapX: 16,
    gapY: 16,
    borderLeft: 0,
    borderTop: 260,
    borderRight: 0,
    borderBottom: 260,
  },
  itemCount: 2,
  candidateCount: 0,
  createdIconCount: 0,
  inputArtifact: null,
  outputArtifact: null,
  items: [0, 1].map((itemIndex) => ({
    id: `item-${itemIndex}`,
    itemIndex,
    originIconId: scope === "grid_edit" ? `icon-${itemIndex}` : null,
    originIconIdSnapshot: scope === "grid_edit" ? `icon-${itemIndex}` : null,
    targetNameSnapshot: `표정 ${itemIndex + 1}`,
    shape: "single" as const,
    rowIndex: 0,
    columnIndex: itemIndex,
    inputRect: { x: itemIndex * 520, y: 260, width: 504, height: 504 },
    reviewStatus: "pending" as const,
    outputCandidateId: null,
    createdIconId: null,
  })),
  createdAt: "2026-07-29T00:00:00Z",
  updatedAt: "2026-07-29T00:00:00Z",
});

const cells: SheetCell[] = [0, 1].map((index) => ({
  index,
  page: 0,
  row: 0,
  col: index,
  x: index * 520,
  y: 260,
  w: 504,
  h: 504,
  outOfBounds: false,
  emptyCandidate: false,
}));

function gridAnalysis(
  overrides: Partial<SheetGridAnalysis> = {},
): SheetGridAnalysis {
  return {
    sheetWidth: 1024,
    sheetHeight: 1024,
    computedRows: 1,
    computedColumns: 2,
    cellCount: 2,
    outOfBoundsCells: [],
    emptyCellCandidates: [],
    cells,
    warnings: [],
    ...overrides,
  };
}

describe("AI grid workspace model", () => {
  it("builds a deterministic geometry prompt with the ordered item list", () => {
    const prompt = buildAiGridPrompt(workspace(), "픽셀 아트로 바꿔줘");
    expect(prompt).toContain("1 rows × 2 columns");
    expect(prompt).toContain("1. 표정 1\n2. 표정 2");
    expect(prompt).toContain("픽셀 아트로 바꿔줘");
    expect(prompt).toContain("Never merge, remove, add, reorder");
  });

  it("derives exact grid settings and a row-major mapping", () => {
    expect(sheetSettingsFromLayout(workspace().layout)).toMatchObject({
      mode: "cell_size",
      rows: 1,
      columns: 2,
      cellWidth: 504,
      borderTop: 260,
    });
    expect([...defaultResultMapping(workspace(), cells)]).toEqual([[0, 0], [1, 1]]);
  });

  it("requires every edit target and rejects duplicate output cells", () => {
    const edit = workspace();
    const mapping = new Map([[0, 0], [1, 0]]);
    const decisions = reviewDecisions(edit, cells, mapping, new Set([0, 1]));
    expect(validateReviewDecisions(edit, decisions, gridAnalysis())).toContain("중복");
    const excluded = decisions.map((decision, index) => index === 1 ? { ...decision, include: false } : decision);
    expect(validateReviewDecisions(edit, excluded, gridAnalysis())).toContain("모든 대상");
  });

  it("allows generation exclusions but never an empty atomic batch", () => {
    const generation = workspace("grid_generate");
    const mapping = defaultResultMapping(generation, cells);
    const one = reviewDecisions(generation, cells, mapping, new Set([1]));
    expect(validateReviewDecisions(generation, one, gridAnalysis())).toBeNull();
    const none = reviewDecisions(generation, cells, mapping, new Set());
    expect(validateReviewDecisions(generation, none, gridAnalysis())).toContain("하나 이상");
  });

  it("blocks structural mismatches and included empty cells", () => {
    const edit = workspace();
    const mapping = defaultResultMapping(edit, cells);
    const editDecisions = reviewDecisions(edit, cells, mapping, new Set([0, 1]));

    expect(
      validateReviewDecisions(
        edit,
        editDecisions,
        gridAnalysis({ sheetWidth: 2048 }),
      ),
    ).toContain("캔버스");
    expect(
      validateReviewDecisions(
        edit,
        editDecisions,
        gridAnalysis({ emptyCellCandidates: [1] }),
      ),
    ).toContain("비어 있는 결과 셀");

    const generation = workspace("grid_generate");
    const generationDecisions = reviewDecisions(
      generation,
      cells,
      mapping,
      new Set([0, 1]),
    );
    expect(
      validateReviewDecisions(
        generation,
        generationDecisions,
        gridAnalysis({ emptyCellCandidates: [1] }),
      ),
    ).toContain("제외");
    const excludedEmpty = reviewDecisions(
      generation,
      cells,
      mapping,
      new Set([0]),
    );
    expect(
      validateReviewDecisions(
        generation,
        excludedEmpty,
        gridAnalysis({ emptyCellCandidates: [1] }),
      ),
    ).toBeNull();
  });

  it("restores the correct step from persisted status", () => {
    expect(aiGridStepForStatus({ ...workspace(), status: "prepared" })).toBe(3);
    expect(aiGridStepForStatus({ ...workspace(), status: "awaiting_result" })).toBe(3);
    expect(aiGridStepForStatus({ ...workspace(), status: "layout_review_pending" })).toBe(4);
    expect(aiGridStepForStatus({ ...workspace(), status: "layout_review_pending", candidateCount: 2 })).toBe(5);
    expect(aiGridStepForStatus({ ...workspace(), status: "completed" })).toBe(5);
  });
});
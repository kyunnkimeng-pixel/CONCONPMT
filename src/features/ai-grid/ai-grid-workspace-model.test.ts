import { describe, expect, it } from "vitest";

import {
  aiGridStepForStatus,
  buildAiGridPrompt,
  defaultResultMapping,
  reviewDecisions,
  selectAiGridResultFile,
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
  it("accepts downloaded PNG/JPG/WebP and leaves alpha policy to explicit review", () => {
    const webp = {
      name: "novelai prompt s-123.webp",
      size: 512,
      type: "image/webp",
    } as File;
    const pngWithUnknownName = {
      name: "download.bin",
      size: 512,
      type: "application/octet-stream",
    } as File;
    const jpeg = {
      name: "result.jpg",
      size: 512,
      type: "image/jpeg",
    } as File;
    const oversized = {
      name: "large.webp",
      size: 16 * 1024 * 1024 + 1,
      type: "image/webp",
    } as File;

    expect(selectAiGridResultFile([webp], "grid_edit")).toEqual({
      file: webp,
      error: null,
    });
    expect(
      selectAiGridResultFile([pngWithUnknownName], "grid_edit").error,
    ).toBeNull();
    expect(selectAiGridResultFile([], "grid_edit").error).toContain(
      "Download Image",
    );
    expect(
      selectAiGridResultFile(
        [webp, pngWithUnknownName],
        "grid_edit",
      ).error,
    ).toContain("2개");
    expect(selectAiGridResultFile([oversized], "grid_edit").error).toContain(
      "16MB",
    );
    expect(selectAiGridResultFile([jpeg], "grid_edit").error).toBeNull();
    for (const scope of ["single_generate", "grid_generate"] as const) {
      expect(selectAiGridResultFile([webp], scope).error).toBeNull();
      expect(selectAiGridResultFile([jpeg], scope)).toEqual({
        file: jpeg,
        error: null,
      });
    }
  });

  it("builds a deterministic geometry prompt with the ordered item list", () => {
    const prompt = buildAiGridPrompt(workspace(), "픽셀 아트로 바꿔줘");
    expect(prompt).toContain("1 rows × 2 columns");
    expect(prompt).toContain("1. 표정 1\n2. 표정 2");
    expect(prompt).toContain("픽셀 아트로 바꿔줘");
    expect(prompt).toContain("Never merge, remove, add, reorder");
    expect(prompt).toContain("every pixel outside each icon");
    expect(prompt).toContain("alpha 0");
    expect(prompt).toContain("Never draw or rasterize a checkerboard");
    expect(prompt).toContain("gray-and-white tiles");
  });

  it("allows an explicit opaque intermediate without ever requesting a painted checkerboard", () => {
    const prompt = buildAiGridPrompt(
      workspace("grid_generate"),
      "같은 캐릭터로 네 가지 표정",
      "gemini_web",
      "allow_opaque",
    );
    const novelPrompt = buildAiGridPrompt(
      workspace("grid_generate"),
      "same character, four expressions",
      "novelai_web",
      "allow_opaque",
    );

    expect(prompt).toContain("Transparent PNG is preferred");
    expect(prompt).toContain("plain uniform background color");
    expect(prompt).toContain("no texture, gradient, shadow, checkerboard");
    expect(prompt).toContain("keep that background");
    expect(novelPrompt).toContain("plain uniform background");
    expect(novelPrompt).not.toContain("transparent background");
  });
  it("requires real alpha instead of a painted checkerboard for source-free generation", () => {
    const prompt = buildAiGridPrompt(
      workspace("grid_generate"),
      "같은 캐릭터로 네 가지 표정",
    );

    expect(prompt).toContain("every gap and unused cell");
    expect(prompt).toContain("must have alpha 0");
    expect(prompt).toContain("transparency grid");
    expect(prompt).toContain("opaque background to imitate transparency");
  });
  it("separates a generation reference board from the required output geometry", () => {
    const generation = workspace("grid_generate");
    generation.inputArtifact = {
      role: "input_sheet",
      sourceFileId: "reference-source",
      originalFilename: "references.png",
      filePath: "C:\\managed\\references.png",
      previewUrl: "asset://references.png",
      extension: "png",
      mimeType: "image/png",
      width: 1024,
      height: 1024,
      byteSize: 100,
      sha256: "reference-sha",
      hasAlpha: true,
      manifestJson: '{"schema":"pmtcon-ai-grid-v1","kind":"generation_reference"}',
      createdAt: "2026-07-29T00:00:00Z",
    };

    const prompt = buildAiGridPrompt(generation, "같은 캐릭터로 만들어줘");

    expect(prompt).toContain("REFERENCE BOARD only");
    expect(prompt).toContain("not the output template or output geometry");
    expect(prompt).toContain("Required geometry: 1024×1024px canvas");
  });

  it("builds a compact NovelAI tag prompt with only a short layout sentence", () => {
    const prompt = buildAiGridPrompt(
      workspace(),
      "Pixel ART;\nBright SMILE",
      "novelai_web",
    );

    expect(prompt.split("\n")).toHaveLength(2);
    expect(prompt).toContain("pixel art, bright smile");
    expect(prompt).toContain("Keep the original 1 by 2 cell layout");
    expect(prompt).not.toContain("Required geometry:");
    expect(prompt).not.toContain("Cell order:");
  });

  it("tells NovelAI that a generation reference is not the output layout", () => {
    const generation = workspace("grid_generate");
    generation.inputArtifact = {
      role: "input_sheet",
      sourceFileId: "reference-source",
      originalFilename: "references.png",
      filePath: "C:\\managed\\references.png",
      previewUrl: "asset://references.png",
      extension: "png",
      mimeType: "image/png",
      width: 1024,
      height: 1024,
      byteSize: 100,
      sha256: "reference-sha",
      hasAlpha: true,
      manifestJson: '{"schema":"pmtcon-ai-grid-v1","kind":"generation_reference"}',
      createdAt: "2026-07-29T00:00:00Z",
    };

    const prompt = buildAiGridPrompt(
      generation,
      "consistent character",
      "novelai_web",
    );
    expect(prompt).toContain("Create exactly 2 icons");
    expect(prompt).toContain("reference only, not the output layout");
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
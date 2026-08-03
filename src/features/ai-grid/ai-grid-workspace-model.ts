import { normalizeNovelAiPromptInput } from "@/features/ai-web/novelai-web-model";
import type { AiGridLayout, AiGridWorkspace, ReviewedAiGridDecision } from "@/features/ai-grid/types";
import type {
  SheetCell,
  SheetGridAnalysis,
  SheetGridSettings,
} from "@/features/sheets/types";

export type AiGridWebService = "gemini_web" | "novelai_web";
export type AiGridResultBackgroundPolicy =
  | "preserve_transparency"
  | "allow_opaque";

export const AI_GRID_RESULT_MAX_BYTES = 16 * 1024 * 1024;

export function selectAiGridResultFile(
  files: Iterable<File> | ArrayLike<File>,
  requestScope: AiGridWorkspace["requestScope"],
) {
  const items = Array.from(files as ArrayLike<File>);
  if (items.length === 0) {
    return {
      file: null,
      error:
        requestScope === "grid_edit"
          ? "웹 미리보기 주소가 아니라 Download Image로 내려받은 PNG·JPG·WebP 파일을 놓아 주세요."
          : "웹 미리보기 주소가 아니라 Download Image로 내려받은 PNG·JPG·WebP 파일을 놓아 주세요.",
    };
  }
  if (items.length !== 1) {
    return {
      file: null,
      error: `그리드 결과는 이미지 한 장만 받을 수 있습니다. 현재 ${items.length}개가 선택됐습니다.`,
    };
  }
  const file = items[0]!;
  if (file.size > AI_GRID_RESULT_MAX_BYTES) {
    return {
      file: null,
      error: `${file.name}: 결과 이미지는 최대 16MB까지 가져올 수 있습니다.`,
    };
  }
  return { file, error: null };
}

export function aiGridStepForStatus(workspace: AiGridWorkspace) {
  if (workspace.status === "prepared" || workspace.status === "awaiting_result") return 3;
  if (workspace.status === "layout_review_pending") return workspace.candidateCount > 0 ? 5 : 4;
  if (workspace.status === "completed") return 5;
  return 1;
}

export function buildAiGridPrompt(
  workspace: AiGridWorkspace,
  userPrompt: string,
  service: AiGridWebService = "gemini_web",
  backgroundPolicy: AiGridResultBackgroundPolicy = "preserve_transparency",
) {
  if (service === "novelai_web") {
    return buildNovelAiGridPrompt(workspace, userPrompt, backgroundPolicy);
  }
  const layout = workspace.layout;
  const geometry = `${layout.canvasWidth}×${layout.canvasHeight}px canvas, ${layout.rows} rows × ${layout.columns} columns, ${layout.cellSize}×${layout.cellSize}px cells, horizontal/vertical gap ${layout.gapX}/${layout.gapY}px, borders left ${layout.borderLeft}px, top ${layout.borderTop}px, right ${layout.borderRight}px, bottom ${layout.borderBottom}px`;
  const names = workspace.items
    .map((item) => `${item.itemIndex + 1}. ${item.targetNameSnapshot}`)
    .join("\n");
  const requireTransparency =
    workspace.requestScope === "grid_edit" ||
    backgroundPolicy === "preserve_transparency";
  const operation = workspace.requestScope === "grid_edit"
    ? "Edit every existing icon in the attached sprite sheet. Preserve the exact canvas, cell positions, cell count, ordering, transparent gaps, and one-icon-per-cell boundaries. Never merge, remove, add, reorder, resize, or move cells."
    : workspace.inputArtifact
      ? `The attached image is a REFERENCE BOARD only, not the output template or output geometry. Use its character identity, palette, proportions, and drawing-style cues consistently, but do not copy its layout. Generate exactly ${workspace.itemCount} distinct emoticon icons as one new sprite sheet using only the required output geometry and row-major cell positions below. ${requireTransparency ? "Leave unused cells fully transparent." : "Keep unused cells visually empty using the same single flat background color."}`
      : `Generate exactly ${workspace.itemCount} distinct emoticon icons as one sprite sheet. Use the exact canvas and row-major cell positions below. ${requireTransparency ? "Leave unused cells fully transparent." : "Keep unused cells visually empty using the same single flat background color."}`;
  const request = userPrompt.trim() || "Keep one coherent character and drawing style while making each cell readable as a small emoticon.";
  return [
    requireTransparency
      ? "Return one static PNG sprite sheet only. Do not add captions, labels, grid lines, borders, watermarks, margins, or a background."
      : "Return one static sprite sheet image only. Do not add captions, labels, grid lines, borders, watermarks, or extra margins.",
    operation,
    `Required geometry: ${geometry}.`,
    "Cell order:",
    names,
    "User request:",
    request,
    requireTransparency
      ? "Use real alpha transparency: every pixel outside each icon, including every gap and unused cell, must have alpha 0. Never draw or rasterize a checkerboard, transparency grid, gray-and-white tiles, matte, or opaque background to imitate transparency."
      : "Transparent PNG is preferred. If the service can only return JPEG or an opaque image, use one plain uniform background color with no texture, gradient, shadow, checkerboard, transparency grid, gray-and-white tiles, or fake alpha pattern. The app will keep that background until the user removes it later.",
    "Put all visible pixels inside their assigned cells. Do not output prose.",
  ].join("\n");
}

export function buildNovelAiGridPrompt(
  workspace: AiGridWorkspace,
  userPrompt: string,
  backgroundPolicy: AiGridResultBackgroundPolicy = "preserve_transparency",
) {
  const request = normalizeNovelAiPromptInput(userPrompt);
  const baseTags = [
    "emoticon set",
    "sprite sheet",
    "multiple views",
    "consistent character",
    "consistent style",
    "clean lineart",
    backgroundPolicy === "preserve_transparency"
      ? "transparent background"
      : "plain uniform background",
  ];
  if (request) baseTags.push(request);

  const { rows, columns } = workspace.layout;
  const structure = workspace.requestScope === "grid_edit"
    ? `Keep the original ${rows} by ${columns} cell layout, one icon per cell, and row-major order unchanged.`
    : workspace.inputArtifact
      ? `Create exactly ${workspace.itemCount} icons in a ${rows} by ${columns} row-major sprite sheet. The uploaded image is reference only, not the output layout.`
      : `Create exactly ${workspace.itemCount} icons in a ${rows} by ${columns} row-major sprite sheet.`;
  return `${baseTags.join(", ")}\n${structure}`;
}

export function sheetSettingsFromLayout(layout: AiGridLayout): SheetGridSettings {
  return {
    mode: "cell_size",
    rows: layout.rows,
    columns: layout.columns,
    cellWidth: layout.cellSize,
    cellHeight: layout.cellSize,
    borderLeft: layout.borderLeft,
    borderTop: layout.borderTop,
    borderRight: layout.borderRight,
    borderBottom: layout.borderBottom,
    gapX: layout.gapX,
    gapY: layout.gapY,
    readOrder: "row_major",
    emptyCellThreshold: 0.99,
  };
}

export function defaultResultMapping(
  workspace: AiGridWorkspace,
  cells: SheetCell[],
): Map<number, number> {
  const usable = cells.filter((cell) => !cell.outOfBounds).map((cell) => cell.index);
  return new Map(
    workspace.items.map((item, position) => [
      item.itemIndex,
      usable[position] ?? item.itemIndex,
    ]),
  );
}

export function reviewDecisions(
  workspace: AiGridWorkspace,
  cells: SheetCell[],
  mapping: ReadonlyMap<number, number>,
  includedItemIndexes: ReadonlySet<number>,
): ReviewedAiGridDecision[] {
  const cellsByIndex = new Map(cells.map((cell) => [cell.index, cell]));
  return workspace.items.map((item) => {
    const include = workspace.requestScope === "grid_edit" || includedItemIndexes.has(item.itemIndex);
    const resultCellIndex = mapping.get(item.itemIndex) ?? -1;
    const cell = cellsByIndex.get(resultCellIndex);
    return {
      resultCellIndex,
      targetItemIndex: item.itemIndex,
      include,
      crop: include && cell
        ? { x: cell.x, y: cell.y, width: cell.w, height: cell.h }
        : null,
    };
  });
}

export function validateReviewDecisions(
  workspace: AiGridWorkspace,
  decisions: ReviewedAiGridDecision[],
  analysis: SheetGridAnalysis,
) {
  const structureError = validateAiGridStructure(workspace, analysis);
  if (structureError) return structureError;

  const validCells = new Set(
    analysis.cells
      .filter((cell) => !cell.outOfBounds)
      .map((cell) => cell.index),
  );
  const included = decisions.filter((decision) => decision.include);
  if (workspace.requestScope === "grid_edit" && included.length !== workspace.itemCount) {
    return "여러 아이콘 수정은 모든 대상 셀을 함께 저장해야 합니다.";
  }
  if (included.length === 0) return "저장할 결과 셀을 하나 이상 포함해 주세요.";
  if (included.some((decision) => !validCells.has(decision.resultCellIndex))) {
    return "결과 시트 범위 안의 셀을 각 항목에 지정해 주세요.";
  }
  const mapped = included.map((decision) => decision.resultCellIndex);
  if (new Set(mapped).size !== mapped.length) return "하나의 결과 셀을 두 항목에 중복 지정할 수 없습니다.";
  const emptyCells = new Set(analysis.emptyCellCandidates);
  if (included.some((decision) => emptyCells.has(decision.resultCellIndex))) {
    return workspace.requestScope === "grid_edit"
      ? "편집 대상에 비어 있는 결과 셀이 있습니다. 모든 아이콘이 채워진 결과를 다시 받아 주세요."
      : "비어 있는 결과 셀은 제외하거나 아이콘이 채워진 결과를 다시 받아 주세요.";
  }
  return null;
}

export function validateAiGridStructure(
  workspace: AiGridWorkspace,
  analysis: SheetGridAnalysis,
) {
  const { layout } = workspace;
  if (
    analysis.sheetWidth !== layout.canvasWidth ||
    analysis.sheetHeight !== layout.canvasHeight
  ) {
    return `결과 캔버스가 예상 크기와 다릅니다. ${layout.canvasWidth}×${layout.canvasHeight}px 결과를 다시 받아 주세요.`;
  }
  if (
    analysis.computedRows !== layout.rows ||
    analysis.computedColumns !== layout.columns
  ) {
    return `결과 셀 구조가 예상 ${layout.rows}행 × ${layout.columns}열과 다릅니다.`;
  }
  if (
    analysis.outOfBoundsCells.length > 0 ||
    analysis.cells.length !== layout.rows * layout.columns
  ) {
    return "결과 셀 구조가 캔버스 범위를 벗어나거나 셀 수가 예상과 다릅니다.";
  }

  const cellsByIndex = new Map(
    analysis.cells.map((cell) => [cell.index, cell]),
  );
  for (let index = 0; index < layout.rows * layout.columns; index += 1) {
    const cell = cellsByIndex.get(index);
    const row = Math.floor(index / layout.columns);
    const column = index % layout.columns;
    const expectedX =
      layout.borderLeft + column * (layout.cellSize + layout.gapX);
    const expectedY =
      layout.borderTop + row * (layout.cellSize + layout.gapY);
    if (
      !cell ||
      cell.outOfBounds ||
      cell.x !== expectedX ||
      cell.y !== expectedY ||
      cell.w !== layout.cellSize ||
      cell.h !== layout.cellSize
    ) {
      return "결과 셀의 위치·크기·간격이 준비한 그리드 구조와 다릅니다.";
    }
  }
  return null;
}
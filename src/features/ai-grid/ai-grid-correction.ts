import type { AiGridResultBackgroundPolicy } from "@/features/ai-grid/ai-grid-workspace-model";
import type {
  AiGridWorkspace,
} from "@/features/ai-grid/types";
import type { SheetGridAnalysis } from "@/features/sheets/types";

export function buildAiGridMissingAlphaCorrectionPrompt() {
  return [
    "[Transparency correction request]",
    "- Use real alpha transparency and set every background pixel outside the icons to alpha 0.",
    "- Keep every gap and unused cell fully transparent.",
    "- Never draw or rasterize a checkerboard, transparency grid, gray-and-white tiles, matte, or opaque background to imitate transparency.",
    "- Return exactly one static PNG image with an alpha channel and no prose. For multi-icon work, preserve the original sprite-sheet geometry and cell order.",
  ].join("\n");
}

export function buildAiGridCorrectionPrompt(
  workspace: AiGridWorkspace,
  analysis: SheetGridAnalysis,
  backgroundPolicy: AiGridResultBackgroundPolicy = "preserve_transparency",
) {
  const instructions: string[] = [];
  if (
    analysis.sheetWidth !== workspace.layout.canvasWidth ||
    analysis.sheetHeight !== workspace.layout.canvasHeight
  ) {
    instructions.push(
      `출력 캔버스를 정확히 ${workspace.layout.canvasWidth}×${workspace.layout.canvasHeight}px로 유지하고 크기를 변경하지 마세요.`,
    );
  }

  const usableCellCount = analysis.cells.filter(
    (cell) => !cell.outOfBounds,
  ).length;
  const cellsByIndex = new Map(
    analysis.cells.map((cell) => [cell.index, cell]),
  );
  const hasCellGeometryMismatch = Array.from(
    { length: workspace.layout.rows * workspace.layout.columns },
    (_, index) => index,
  ).some((index) => {
    const cell = cellsByIndex.get(index);
    const row = Math.floor(index / workspace.layout.columns);
    const column = index % workspace.layout.columns;
    return (
      !cell ||
      cell.x !==
        workspace.layout.borderLeft +
          column * (workspace.layout.cellSize + workspace.layout.gapX) ||
      cell.y !==
        workspace.layout.borderTop +
          row * (workspace.layout.cellSize + workspace.layout.gapY) ||
      cell.w !== workspace.layout.cellSize ||
      cell.h !== workspace.layout.cellSize
    );
  });
  const hasGridStructureMismatch =
    analysis.computedRows !== workspace.layout.rows ||
    analysis.computedColumns !== workspace.layout.columns ||
    analysis.cells.length !== workspace.layout.rows * workspace.layout.columns ||
    analysis.outOfBoundsCells.length > 0 ||
    hasCellGeometryMismatch;
  if (usableCellCount < workspace.itemCount || hasGridStructureMismatch) {
    instructions.push(
      `${workspace.layout.rows}행 × ${workspace.layout.columns}열, 셀 ${workspace.layout.cellSize}×${workspace.layout.cellSize}px 배치를 그대로 사용하고 모든 셀을 캔버스 안에 유지하세요.`,
    );
  }
  if (usableCellCount < workspace.itemCount) {
    instructions.push(
      `아이콘을 정확히 ${workspace.itemCount}개 반환하고 셀을 추가·삭제·병합·재정렬하지 마세요.`,
    );
  }
  const expectedEmptyCells = analysis.emptyCellCandidates.filter(
    (cellIndex) => cellIndex < workspace.itemCount,
  );
  if (
    workspace.requestScope === "grid_edit" &&
    expectedEmptyCells.length > 0
  ) {
    instructions.push(
      `편집 대상 ${workspace.itemCount}개 셀을 모두 채우고 투명한 빈 셀을 남기지 마세요.`,
    );
  }

  if (instructions.length === 0) return null;
  const outputRule =
    backgroundPolicy === "allow_opaque"
      ? "- 정적 PNG·JPG·WebP 스프라이트 한 장만 반환하세요. 배경을 포함한다면 체커무늬·가짜 투명 패턴·질감·그라데이션·그림자 없는 하나의 균일한 단색을 사용하고 설명문은 출력하지 마세요."
      : "- 정적 투명 PNG 스프라이트 한 장만 반환하고 설명문은 출력하지 마세요.";
  return [
    "[구조 수정 요청]",
    ...instructions.map((instruction) => `- ${instruction}`),
    outputRule,
  ].join("\n");
}

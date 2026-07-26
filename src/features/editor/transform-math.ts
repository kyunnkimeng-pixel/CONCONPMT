import type { IconShape } from "@/features/editor/types";

export interface IconTransformDraft {
  shape: IconShape;
  cellWidth: number;
  cellHeight: number;
  transformQuarterTurns: 0 | 1 | 2 | 3;
  transformFlipHorizontal: boolean;
  transformFlipVertical: boolean;
  pieceIds: string[];
}

export type QuarterTurnDirection = "left" | "right";
export type FlipAxis = "horizontal" | "vertical";

export function sourceViewportGeometry(draft: IconTransformDraft) {
  const canonical = canonicalDraft(draft);
  const swapsAxes = canonical.transformQuarterTurns % 2 === 1;
  const shape = swapsAxes ? swapDoubleShape(canonical.shape) : canonical.shape;
  const cellWidth = swapsAxes ? canonical.cellHeight : canonical.cellWidth;
  const cellHeight = swapsAxes ? canonical.cellWidth : canonical.cellHeight;

  return { shape, cellWidth, cellHeight };
}

export function rotateIconDraft<T extends IconTransformDraft>(
  draft: T,
  direction: QuarterTurnDirection,
): T {
  const canonical = canonicalDraft(draft);
  const previousShape = canonical.shape;
  const reflected = canonical.transformFlipHorizontal;
  const delta =
    direction === "right"
      ? reflected
        ? 3
        : 1
      : reflected
        ? 1
        : 3;

  return {
    ...canonical,
    shape: swapDoubleShape(previousShape),
    cellWidth: canonical.cellHeight,
    cellHeight: canonical.cellWidth,
    transformQuarterTurns: ((canonical.transformQuarterTurns + delta) % 4) as
      | 0
      | 1
      | 2
      | 3,
    transformFlipVertical: false,
    pieceIds: rotatePieceIds(canonical.pieceIds, previousShape, direction),
  };
}

export function flipIconDraft<T extends IconTransformDraft>(draft: T, axis: FlipAxis): T {
  const canonical = canonicalDraft(draft);
  if (axis === "horizontal") {
    return {
      ...canonical,
      transformFlipHorizontal: !canonical.transformFlipHorizontal,
      transformFlipVertical: false,
      pieceIds:
        canonical.shape === "horizontal_double"
          ? reversed(canonical.pieceIds)
          : canonical.pieceIds,
    };
  }

  return {
    ...canonical,
    transformQuarterTurns: ((canonical.transformQuarterTurns + 2) % 4) as
      | 0
      | 1
      | 2
      | 3,
    transformFlipHorizontal: !canonical.transformFlipHorizontal,
    transformFlipVertical: false,
    pieceIds:
      canonical.shape === "vertical_double"
        ? reversed(canonical.pieceIds)
        : canonical.pieceIds,
  };
}

export function transformSummary(draft: IconTransformDraft) {
  let quarterTurns = draft.transformQuarterTurns;
  let flipHorizontal = draft.transformFlipHorizontal;
  if (draft.transformFlipVertical) {
    quarterTurns = ((quarterTurns + 2) % 4) as 0 | 1 | 2 | 3;
    flipHorizontal = !flipHorizontal;
  }

  const summaries = [
    ["변형 없음", "좌우 반전"],
    ["오른쪽 90°", "오른쪽 90° 후 좌우 반전"],
    ["180°", "상하 반전"],
    ["왼쪽 90°", "왼쪽 90° 후 좌우 반전"],
  ] as const;
  return summaries[quarterTurns][flipHorizontal ? 1 : 0];
}

function swapDoubleShape(shape: IconShape): IconShape {
  if (shape === "horizontal_double") {
    return "vertical_double";
  }
  if (shape === "vertical_double") {
    return "horizontal_double";
  }
  return "single";
}

function rotatePieceIds(
  pieceIds: string[],
  previousShape: IconShape,
  direction: QuarterTurnDirection,
) {
  const shouldReverse =
    (previousShape === "horizontal_double" && direction === "left") ||
    (previousShape === "vertical_double" && direction === "right");
  return shouldReverse ? reversed(pieceIds) : pieceIds;
}

function reversed(values: string[]) {
  return [...values].reverse();
}

function canonicalDraft<T extends IconTransformDraft>(draft: T): T {
  if (!draft.transformFlipVertical) {
    return draft;
  }

  return {
    ...draft,
    transformQuarterTurns: ((draft.transformQuarterTurns + 2) % 4) as 0 | 1 | 2 | 3,
    transformFlipHorizontal: !draft.transformFlipHorizontal,
    transformFlipVertical: false,
  };
}

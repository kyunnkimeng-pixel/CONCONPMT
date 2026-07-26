import { describe, expect, it } from "vitest";

import {
  flipIconDraft,
  rotateIconDraft,
  sourceViewportGeometry,
  transformSummary,
} from "@/features/editor/transform-math";
import type { IconTransformDraft } from "@/features/editor/transform-math";

function draft(
  overrides: Partial<IconTransformDraft> = {},
): IconTransformDraft {
  return {
    shape: "horizontal_double",
    cellWidth: 120,
    cellHeight: 80,
    transformQuarterTurns: 0,
    transformFlipHorizontal: false,
    transformFlipVertical: false,
    pieceIds: ["left-content", "right-content"],
    ...overrides,
  };
}

describe("non-destructive icon transform composition", () => {
  it("keeps source crop geometry stable while rotating a non-square double icon", () => {
    const rotated = rotateIconDraft(draft(), "right");

    expect(rotated).toMatchObject({
      shape: "vertical_double",
      cellWidth: 80,
      cellHeight: 120,
      transformQuarterTurns: 1,
      pieceIds: ["left-content", "right-content"],
    });
    expect(sourceViewportGeometry(rotated)).toEqual({
      shape: "horizontal_double",
      cellWidth: 120,
      cellHeight: 80,
    });
  });

  it("keeps piece identity attached to visual content for every quarter-turn mapping", () => {
    expect(rotateIconDraft(draft(), "left").pieceIds).toEqual([
      "right-content",
      "left-content",
    ]);
    expect(
      rotateIconDraft(
        draft({
          shape: "vertical_double",
          pieceIds: ["top-content", "bottom-content"],
        }),
        "right",
      ).pieceIds,
    ).toEqual(["bottom-content", "top-content"]);
  });

  it("returns to identity after four right rotations", () => {
    const initial = draft();
    const rotated = [0, 1, 2, 3].reduce(
      (current) => rotateIconDraft(current, "right"),
      initial,
    );

    expect(rotated).toEqual(initial);
  });

  it("composes rotation in the visible output axes after a flip", () => {
    const flipped = flipIconDraft(draft(), "horizontal");
    const rotated = rotateIconDraft(flipped, "right");

    expect(rotated.transformFlipHorizontal).toBe(true);
    expect(rotated.transformFlipVertical).toBe(false);
    expect(rotated.transformQuarterTurns).toBe(3);
  });

  it("applies flips to the matching double-icon axis and cancels on the second click", () => {
    const horizontal = flipIconDraft(draft(), "horizontal");
    expect(horizontal.pieceIds).toEqual(["right-content", "left-content"]);
    expect(transformSummary(horizontal)).toBe("좌우 반전");
    expect(flipIconDraft(horizontal, "horizontal")).toEqual(draft());

    const verticalDraft = draft({
      shape: "vertical_double",
      pieceIds: ["top-content", "bottom-content"],
    });
    const vertical = flipIconDraft(verticalDraft, "vertical");
    expect(vertical.pieceIds).toEqual(["bottom-content", "top-content"]);
    expect(transformSummary(vertical)).toBe("상하 반전");
    expect(flipIconDraft(vertical, "vertical")).toEqual(verticalDraft);
  });

  it("describes canonical and legacy-equivalent visual states without leaking internals", () => {
    const legacyVertical = draft({
      transformQuarterTurns: 0,
      transformFlipVertical: true,
    });
    const canonicalVertical = draft({
      transformQuarterTurns: 2,
      transformFlipHorizontal: true,
    });

    expect(transformSummary(legacyVertical)).toBe("상하 반전");
    expect(rotateIconDraft(legacyVertical, "right")).toEqual(
      rotateIconDraft(canonicalVertical, "right"),
    );
    expect(
      transformSummary(
        draft({
          transformQuarterTurns: 3,
          transformFlipHorizontal: true,
        }),
      ),
    ).toBe("왼쪽 90° 후 좌우 반전");
  });
});

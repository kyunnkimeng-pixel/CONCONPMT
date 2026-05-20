import { describe, expect, it } from "vitest";

import type { IconSummary } from "@/features/collections/types";
import {
  findDuplicateAltPieceIds,
  isDuplicateAltDraft,
  validateDcinsideAltText,
} from "@/lib/validation";

describe("DCInside alt validation", () => {
  it("accepts Korean, ASCII letters, digits, and allowed specials", () => {
    expect(validateDcinsideAltText("가1+").isValid).toBe(true);
    expect(validateDcinsideAltText("abc").isValid).toBe(true);
  });

  it("rejects empty, long, whitespace, and emoji values", () => {
    expect(validateDcinsideAltText("").isValid).toBe(false);
    expect(validateDcinsideAltText("가나다라").isValid).toBe(false);
    expect(validateDcinsideAltText("가 나").isValid).toBe(false);
    expect(validateDcinsideAltText("🙂").isValid).toBe(false);
  });

  it("finds duplicate alt piece IDs across icons", () => {
    const icons = [
      icon("icon-a", "piece-a", "가"),
      icon("icon-b", "piece-b", "나"),
      icon("icon-c", "piece-c", "가"),
    ];

    expect(findDuplicateAltPieceIds(icons)).toEqual(new Set(["piece-a", "piece-c"]));
  });

  it("rejects duplicate alt drafts while allowing the current piece value", () => {
    const icons = [icon("icon-a", "piece-a", "가"), icon("icon-b", "piece-b", "나")];

    expect(isDuplicateAltDraft(icons, "piece-b", "가")).toBe(true);
    expect(isDuplicateAltDraft(icons, "piece-b", "나")).toBe(false);
  });
});

function icon(iconId: string, pieceId: string, altText: string): IconSummary {
  return {
    id: iconId,
    collectionId: "collection",
    sourceFileId: `source-${iconId}`,
    displayName: iconId,
    note: null,
    iconKind: "image",
    readiness: "complete",
    placeholderText: null,
    shape: "single",
    orderIndex: 0,
    cellWidthOverride: null,
    cellHeightOverride: null,
    thumbnailUrl: null,
    thumbnailOverrideUrl: null,
    currentPreviewUrl: null,
    gifLoopMode: "preserve",
    gifLoopCount: null,
    createdAt: "2026-05-10T00:00:00.000Z",
    updatedAt: "2026-05-10T00:00:00.000Z",
    pieces: [
      {
        id: pieceId,
        iconId,
        pieceIndex: 0,
        pieceRole: "single",
        altText,
        generatedPreviewUrl: null,
        lastExportUrl: null,
        exportStatus: "not_exported",
        createdAt: "2026-05-10T00:00:00.000Z",
        updatedAt: "2026-05-10T00:00:00.000Z",
      },
    ],
  };
}

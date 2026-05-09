import { describe, expect, it } from "vitest";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import {
  appendUsagePreviewIcon,
  buildUsagePreviewIcons,
  DCINSIDE_USAGE_DISPLAY_SIZE,
  hasAnimatedPreview,
} from "@/features/preview/preview-model";

const collection: CollectionSummary = {
  id: "collection-1",
  name: "테스트 모음",
  coverSourceFileId: null,
  coverIconId: null,
  coverImageUrl: null,
  iconCount: 1,
  defaultCellWidth: 200,
  defaultCellHeight: 200,
  previewWidth: 100,
  previewHeight: 100,
  exportFormat: "png",
  maxBytes: 2_097_152,
  createdAt: "2026-05-10T00:00:00.000Z",
  updatedAt: "2026-05-10T00:00:00.000Z",
};

describe("usage preview model", () => {
  it("uses generated piece previews before icon previews and thumbnails", () => {
    const [previewIcon] = buildUsagePreviewIcons(collection, [
      icon({
        currentPreviewUrl: "asset://preview/full.png",
        thumbnailUrl: "asset://thumb.png",
        pieces: [
          piece({
            id: "piece-1",
            generatedPreviewUrl: "asset://preview/piece-00.png",
          }),
        ],
      }),
    ]);

    expect(previewIcon.usesProcessedOutput).toBe(true);
    expect(previewIcon.pieces[0].imageUrl).toBe("asset://preview/piece-00.png");
    expect(previewIcon.pieces[0].imageSource).toBe("generated-piece");
    expect(previewIcon.pieces[0].displayWidth).toBe(DCINSIDE_USAGE_DISPLAY_SIZE);
    expect(previewIcon.pieces[0].displayHeight).toBe(DCINSIDE_USAGE_DISPLAY_SIZE);
  });

  it("keeps multi-piece insertion order", () => {
    const [previewIcon] = buildUsagePreviewIcons(collection, [
      icon({
        shape: "horizontal_double",
        pieces: [
          piece({ id: "left", pieceIndex: 0, pieceRole: "left", altText: "좌" }),
          piece({ id: "right", pieceIndex: 1, pieceRole: "right", altText: "우" }),
        ],
      }),
    ]);

    const inserted = appendUsagePreviewIcon([], previewIcon, "test");

    expect(inserted).toHaveLength(1);
    expect(inserted[0].pieces.map((previewPiece) => previewPiece.pieceRole)).toEqual([
      "left",
      "right",
    ]);
  });

  it("detects animated gif preview URLs in inserted items", () => {
    const [previewIcon] = buildUsagePreviewIcons(collection, [
      icon({
        pieces: [piece({ generatedPreviewUrl: "asset://preview/piece-00.gif?v=1" })],
      }),
    ]);
    const inserted = appendUsagePreviewIcon([], previewIcon, "gif");

    expect(hasAnimatedPreview([], inserted)).toBe(true);
  });
});

function icon(overrides: Partial<IconSummary> = {}): IconSummary {
  return {
    id: "icon-1",
    collectionId: collection.id,
    sourceFileId: "source-1",
    displayName: "아이콘",
    shape: "single",
    orderIndex: 0,
    cellWidthOverride: null,
    cellHeightOverride: null,
    thumbnailUrl: "asset://thumb.png",
    currentPreviewUrl: null,
    gifLoopMode: "preserve",
    gifLoopCount: null,
    createdAt: "2026-05-10T00:00:00.000Z",
    updatedAt: "2026-05-10T00:00:00.000Z",
    pieces: [piece()],
    ...overrides,
  };
}

function piece(
  overrides: Partial<IconSummary["pieces"][number]> = {},
): IconSummary["pieces"][number] {
  return {
    id: "piece-1",
    iconId: "icon-1",
    pieceIndex: 0,
    pieceRole: "single",
    altText: "가",
    generatedPreviewUrl: null,
    exportStatus: "not_exported",
    createdAt: "2026-05-10T00:00:00.000Z",
    updatedAt: "2026-05-10T00:00:00.000Z",
    ...overrides,
  };
}

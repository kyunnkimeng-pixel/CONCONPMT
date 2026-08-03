import { describe, expect, it } from "vitest";

import { getAiGridEditDisabledReason } from "@/features/ai-grid/ai-grid-eligibility";
import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";

const collection: CollectionSummary = {
  id: "collection-1",
  name: "테스트",
  coverSourceFileId: null,
  coverIconId: null,
  coverImageUrl: null,
  iconCount: 2,
  defaultCellWidth: 200,
  defaultCellHeight: 200,
  previewWidth: 100,
  previewHeight: 100,
  exportFormat: "png",
  maxBytes: 2_000_000,
  createdAt: "2026-07-29T00:00:00Z",
  updatedAt: "2026-07-29T00:00:00Z",
};

function icon(id: string, overrides: Partial<IconSummary> = {}): IconSummary {
  return {
    id,
    collectionId: collection.id,
    sourceFileId: `source-${id}`,
    displayName: id,
    note: null,
    iconKind: "image",
    readiness: "working",
    placeholderText: null,
    shape: "single",
    orderIndex: 0,
    cellWidthOverride: null,
    cellHeightOverride: null,
    thumbnailUrl: `asset://localhost/${id}.png`,
    thumbnailOverrideUrl: null,
    currentPreviewUrl: `asset://localhost/${id}.png`,
    transformQuarterTurns: 0,
    transformFlipHorizontal: false,
    transformFlipVertical: false,
    gifLoopMode: "preserve",
    gifLoopCount: null,
    createdAt: "2026-07-29T00:00:00Z",
    updatedAt: "2026-07-29T00:00:00Z",
    pieces: [],
    ...overrides,
  };
}

describe("getAiGridEditDisabledReason", () => {
  it("accepts two through sixteen square static single icons", () => {
    const icons = Array.from({ length: 16 }, (_, index) =>
      icon(`icon-${index}`),
    );
    expect(
      getAiGridEditDisabledReason(
        collection,
        icons,
        icons.map((item) => item.id),
      ),
    ).toBeNull();
  });

  it("explains count limits", () => {
    expect(
      getAiGridEditDisabledReason(collection, [icon("one")], ["one"]),
    ).toContain("2개 이상");
    const icons = Array.from({ length: 17 }, (_, index) =>
      icon(`icon-${index}`),
    );
    expect(
      getAiGridEditDisabledReason(
        collection,
        icons,
        icons.map((item) => item.id),
      ),
    ).toContain("최대 16개");
  });

  it("routes GIFs to frame-sheet roundtrip", () => {
    const targets = [
      icon("static"),
      icon("gif", { currentPreviewUrl: "asset://localhost/icon.gif" }),
    ];
    expect(
      getAiGridEditDisabledReason(
        collection,
        targets,
        targets.map((item) => item.id),
      ),
    ).toContain("프레임 작업시트");
  });

  it("rejects placeholders, multi-piece icons, and non-square cells", () => {
    expect(
      getAiGridEditDisabledReason(
        collection,
        [icon("a"), icon("b", { iconKind: "placeholder" })],
        ["a", "b"],
      ),
    ).toContain("빈 디시콘");
    expect(
      getAiGridEditDisabledReason(
        collection,
        [icon("a"), icon("b", { shape: "horizontal_double" })],
        ["a", "b"],
      ),
    ).toContain("단일 아이콘");
    expect(
      getAiGridEditDisabledReason(
        collection,
        [icon("a"), icon("b", { cellHeightOverride: 100 })],
        ["a", "b"],
      ),
    ).toContain("정사각형");
  });
});

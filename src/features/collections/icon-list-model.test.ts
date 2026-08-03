import { describe, expect, it } from "vitest";

import { upsertIconSummary } from "@/features/collections/icon-list-model";
import type { IconSummary } from "@/features/collections/types";

function icon(id: string, orderIndex: number, displayName = id): IconSummary {
  return {
    id,
    collectionId: "collection_1",
    sourceFileId: `source_${id}`,
    displayName,
    note: null,
    iconKind: "image",
    readiness: "complete",
    placeholderText: null,
    shape: "single",
    orderIndex,
    cellWidthOverride: null,
    cellHeightOverride: null,
    thumbnailUrl: null,
    thumbnailOverrideUrl: null,
    currentPreviewUrl: null,
    transformQuarterTurns: 0,
    transformFlipHorizontal: false,
    transformFlipVertical: false,
    gifLoopMode: "preserve",
    gifLoopCount: null,
    createdAt: "2026-07-27T00:00:00Z",
    updatedAt: "2026-07-27T00:00:00Z",
    pieces: [],
  };
}

describe("upsertIconSummary", () => {
  it("replaces an existing icon and inserts a new icon in orderIndex order", () => {
    const first = icon("icon_1", 0);
    const third = icon("icon_3", 2);
    const updatedFirst = icon("icon_1", 0, "수정됨");

    const updated = upsertIconSummary([first, third], updatedFirst);
    expect(updated).toHaveLength(2);
    expect(updated[0]).toBe(updatedFirst);

    const inserted = upsertIconSummary(updated, icon("icon_2", 1));
    expect(inserted.map((item) => item.id)).toEqual([
      "icon_1",
      "icon_2",
      "icon_3",
    ]);
  });
});

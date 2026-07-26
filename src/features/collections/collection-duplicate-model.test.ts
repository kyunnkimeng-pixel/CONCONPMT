import { describe, expect, it } from "vitest";

import { canStartCollectionDuplicate } from "@/features/collections/collection-duplicate-model";

describe("collection duplicate availability", () => {
  it("requires a selected collection and blocks import or duplicate overlap", () => {
    expect(
      canStartCollectionDuplicate({
        collectionId: "collection-1",
        isDuplicating: false,
        isImporting: false,
      }),
    ).toBe(true);
    expect(
      canStartCollectionDuplicate({
        collectionId: null,
        isDuplicating: false,
        isImporting: false,
      }),
    ).toBe(false);
    expect(
      canStartCollectionDuplicate({
        collectionId: "collection-1",
        isDuplicating: false,
        isImporting: true,
      }),
    ).toBe(false);
    expect(
      canStartCollectionDuplicate({
        collectionId: "collection-1",
        isDuplicating: true,
        isImporting: false,
      }),
    ).toBe(false);
  });
});

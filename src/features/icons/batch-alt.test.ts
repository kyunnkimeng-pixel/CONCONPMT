import { describe, expect, it } from "vitest";

import { createUniqueBatchAltUpdates } from "@/features/icons/batch-alt";

describe("createUniqueBatchAltUpdates", () => {
  it("keeps the exact value when only one piece is targeted", () => {
    expect(
      createUniqueBatchAltUpdates(
        [
          { id: "a", altText: "" },
          { id: "b", altText: "울1" },
        ],
        ["a"],
        "울",
      ),
    ).toEqual([{ pieceId: "a", altText: "울" }]);
  });

  it("adds numeric suffixes for each selected piece", () => {
    expect(
      createUniqueBatchAltUpdates(
        [
          { id: "a", altText: "" },
          { id: "b", altText: "" },
        ],
        ["a", "b"],
        "울",
      ),
    ).toEqual([
      { pieceId: "a", altText: "울1" },
      { pieceId: "b", altText: "울2" },
    ]);
  });

  it("skips suffixes already used by unselected pieces", () => {
    expect(
      createUniqueBatchAltUpdates(
        [
          { id: "a", altText: "" },
          { id: "b", altText: "울1" },
          { id: "c", altText: "" },
        ],
        ["a", "c"],
        "울",
      ),
    ).toEqual([
      { pieceId: "a", altText: "울2" },
      { pieceId: "c", altText: "울3" },
    ]);
  });
});

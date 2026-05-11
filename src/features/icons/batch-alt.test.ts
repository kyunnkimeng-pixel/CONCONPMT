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

  it("assigns comma separated alt values in selection order", () => {
    expect(
      createUniqueBatchAltUpdates(
        [
          { id: "a", altText: "" },
          { id: "b", altText: "" },
          { id: "c", altText: "" },
        ],
        ["a", "b", "c"],
        "one,two,three",
      ),
    ).toEqual([
      { pieceId: "a", altText: "one" },
      { pieceId: "b", altText: "two" },
      { pieceId: "c", altText: "three" },
    ]);
  });

  it("extends the last comma value with numeric suffixes when values are short", () => {
    expect(
      createUniqueBatchAltUpdates(
        [
          { id: "a", altText: "" },
          { id: "b", altText: "" },
          { id: "c", altText: "" },
          { id: "d", altText: "" },
          { id: "e", altText: "" },
        ],
        ["a", "b", "c", "d", "e"],
        "one,two,three",
      ),
    ).toEqual([
      { pieceId: "a", altText: "one" },
      { pieceId: "b", altText: "two" },
      { pieceId: "c", altText: "three1" },
      { pieceId: "d", altText: "three2" },
      { pieceId: "e", altText: "three3" },
    ]);
  });

  it("uses sequential numbers when the input is empty", () => {
    expect(
      createUniqueBatchAltUpdates(
        [
          { id: "a", altText: "" },
          { id: "b", altText: "" },
          { id: "c", altText: "" },
        ],
        ["a", "b", "c"],
        "",
      ),
    ).toEqual([
      { pieceId: "a", altText: "1" },
      { pieceId: "b", altText: "2" },
      { pieceId: "c", altText: "3" },
    ]);
  });
});

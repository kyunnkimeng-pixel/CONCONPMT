import { describe, expect, it } from "vitest";

import {
  pruneSelection,
  selectIcon,
  selectIconForContextMenu,
} from "@/features/icons/selection/selection-model";

const orderedIds = ["a", "b", "c", "d"];

describe("icon selection model", () => {
  it("selects a single icon without modifiers", () => {
    expect(
      selectIcon({ selectedIds: ["a", "b"], anchorId: "a" }, orderedIds, "c", {}),
    ).toEqual({
      selectedIds: ["c"],
      anchorId: "c",
    });
  });

  it("toggles Ctrl selection in grid order", () => {
    const first = selectIcon(
      { selectedIds: ["b"], anchorId: "b" },
      orderedIds,
      "d",
      { ctrlKey: true },
    );
    expect(first.selectedIds).toEqual(["b", "d"]);

    const second = selectIcon(first, orderedIds, "b", { ctrlKey: true });
    expect(second.selectedIds).toEqual(["d"]);
  });

  it("selects a Shift range from the anchor", () => {
    expect(
      selectIcon(
        { selectedIds: ["b"], anchorId: "b" },
        orderedIds,
        "d",
        { shiftKey: true },
      ),
    ).toEqual({
      selectedIds: ["b", "c", "d"],
      anchorId: "b",
    });
  });

  it("preserves selection when opening a context menu on a selected icon", () => {
    expect(
      selectIconForContextMenu(
        { selectedIds: ["b", "c"], anchorId: "b" },
        orderedIds,
        "c",
      ),
    ).toEqual({
      selectedIds: ["b", "c"],
      anchorId: "b",
    });
  });

  it("prunes deleted icons from selection", () => {
    expect(
      pruneSelection({ selectedIds: ["a", "c"], anchorId: "c" }, ["a", "b"]),
    ).toEqual({
      selectedIds: ["a"],
      anchorId: null,
    });
  });
});

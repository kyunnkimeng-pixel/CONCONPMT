import { describe, expect, it } from "vitest";
import { renderToString } from "react-dom/server";

import { SheetExportPreview } from "@/features/sheets/components/SheetExportPreview";
import {
  defaultExportSheetRequest,
  defaultSheetGridSettings,
  estimateSheetPages,
  includedNonEmptyCellIndexes,
  nextSelectionAfterCellClick,
} from "@/features/sheets/sheet-ui-model";
import type { SheetCell } from "@/features/sheets/types";

describe("sheet-ui-model", () => {
  it("updates include selection without auto-selecting unrelated cells", () => {
    const selected = nextSelectionAfterCellClick(new Set([1]), 2, { multi: true });

    expect([...selected].sort()).toEqual([1, 2]);
    expect([...nextSelectionAfterCellClick(selected, 1, { multi: true })]).toEqual([2]);
    expect([...nextSelectionAfterCellClick(selected, 5, { multi: false })]).toEqual([5]);
  });

  it("excludes empty and out-of-bounds cells from import target list", () => {
    const cells: SheetCell[] = [
      cell(0, false, false),
      cell(1, true, false),
      cell(2, false, true),
    ];

    expect(includedNonEmptyCellIndexes(cells, new Set([0, 1, 2]))).toEqual([0]);
  });

  it("estimates page count using max sheet size", () => {
    const request = {
      ...defaultExportSheetRequest("collection_1"),
      cellWidth: 200,
      cellHeight: 200,
      columns: 8,
      gapX: 8,
      gapY: 8,
      borderX: 16,
      borderY: 16,
      maxSheetWidth: 2048,
      maxSheetHeight: 240,
    };

    expect(estimateSheetPages(10, request)).toBe(2);
  });

  it("renders the export preview summary", () => {
    const html = renderToString(
      <SheetExportPreview itemCount={12} request={defaultExportSheetRequest("collection_1")} />,
    );

    expect(html).toContain("시트 예상");
    expect(defaultSheetGridSettings().emptyCellThreshold).toBe(0.98);
  });
});

function cell(index: number, emptyCandidate: boolean, outOfBounds: boolean): SheetCell {
  return {
    index,
    page: 0,
    row: index,
    col: 0,
    x: 0,
    y: index * 10,
    w: 10,
    h: 10,
    emptyCandidate,
    outOfBounds,
  };
}

import type { ExportEditSheetRequest, SheetCell, SheetGridSettings } from "./types";

export function defaultSheetGridSettings(): SheetGridSettings {
  return {
    mode: "rows_columns",
    rows: 4,
    columns: 5,
    cellWidth: null,
    cellHeight: null,
    borderLeft: 0,
    borderTop: 0,
    borderRight: 0,
    borderBottom: 0,
    gapX: 0,
    gapY: 0,
    readOrder: "row_major",
    emptyCellThreshold: 0.98,
  };
}

export function defaultExportSheetRequest(collectionId: string): ExportEditSheetRequest {
  return {
    collectionId,
    selectedIconIds: [],
    source: "current_collection",
    cellWidth: 200,
    cellHeight: 200,
    columns: 5,
    gapX: 8,
    gapY: 8,
    borderX: 16,
    borderY: 16,
    background: "transparent",
    includeCleanSheet: true,
    includeGuideSheet: true,
    includeManifest: true,
    labelOptions: {
      cellNumber: true,
      iconName: true,
      altValue: true,
      exportNumber: true,
    },
    maxSheetWidth: 2048,
    maxSheetHeight: 2048,
    outputDirectory: null,
    openOutputFolder: false,
  };
}

export function nextSelectionAfterCellClick(
  selected: Set<number>,
  cellIndex: number,
  options: { multi: boolean },
) {
  if (!options.multi) {
    return new Set([cellIndex]);
  }

  const next = new Set(selected);
  if (next.has(cellIndex)) {
    next.delete(cellIndex);
  } else {
    next.add(cellIndex);
  }
  return next;
}

export function includedNonEmptyCellIndexes(cells: SheetCell[], selected: Set<number>) {
  return cells
    .filter(
      (cell) =>
        selected.has(cell.index) && !cell.emptyCandidate && !cell.outOfBounds,
    )
    .map((cell) => cell.index);
}

export function estimateSheetPages(
  itemCount: number,
  settings: Pick<
    ExportEditSheetRequest,
    | "cellWidth"
    | "cellHeight"
    | "columns"
    | "gapX"
    | "gapY"
    | "borderX"
    | "borderY"
    | "maxSheetWidth"
    | "maxSheetHeight"
  >,
) {
  if (itemCount <= 0) {
    return 0;
  }
  const maxColumns = Math.max(
    1,
    Math.floor(
      (settings.maxSheetWidth - settings.borderX * 2 + settings.gapX) /
        (settings.cellWidth + settings.gapX),
    ),
  );
  const columns = Math.max(1, Math.min(settings.columns, maxColumns));
  const rowsPerPage = Math.max(
    1,
    Math.floor(
      (settings.maxSheetHeight - settings.borderY * 2 + settings.gapY) /
        (settings.cellHeight + settings.gapY),
    ),
  );
  return Math.ceil(itemCount / (columns * rowsPerPage));
}

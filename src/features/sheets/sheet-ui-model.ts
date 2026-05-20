import type { IconSummary } from "@/features/collections/types";
import type {
  ExportEditSheetRequest,
  GifFrameSheetSettings,
  GuideLabelOptions,
  SheetCell,
  SheetGridPreset,
  SheetGridPresetInput,
  SheetGridSettings,
} from "./types";

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

export function defaultGifFrameSheetSettings(
  cellWidth = 200,
  cellHeight = 200,
): GifFrameSheetSettings {
  return {
    frameCellWidth: cellWidth,
    frameCellHeight: cellHeight,
    columns: 8,
    framesPerPage: 64,
    gapX: 8,
    gapY: 8,
    borderX: 16,
    borderY: 16,
    maxSheetWidth: 2048,
    maxSheetHeight: 2048,
    background: "transparent",
    includeCleanSheet: true,
    includeGuideSheet: true,
    includeManifest: true,
    outputDirectory: null,
    openOutputFolder: false,
  };
}

export function estimateGifFrameSheetPages(
  frameCount: number,
  settings: GifFrameSheetSettings,
) {
  if (frameCount <= 0) {
    return 0;
  }

  const maxColumns = Math.max(
    1,
    Math.floor(
      (settings.maxSheetWidth - settings.borderX * 2 + settings.gapX) /
        (settings.frameCellWidth + settings.gapX),
    ),
  );
  const columns = Math.max(1, Math.min(settings.columns, maxColumns));
  const maxRowsByHeight = Math.max(
    1,
    Math.floor(
      (settings.maxSheetHeight - settings.borderY * 2 + settings.gapY) /
        (settings.frameCellHeight + settings.gapY),
    ),
  );
  const maxRowsByFrames = settings.framesPerPage
    ? Math.max(1, Math.ceil(settings.framesPerPage / columns))
    : maxRowsByHeight;
  const rowsPerPage = Math.max(1, Math.min(maxRowsByHeight, maxRowsByFrames));

  return Math.ceil(frameCount / Math.max(1, columns * rowsPerPage));
}

export function isGifIcon(icon: IconSummary) {
  const preview = icon.currentPreviewUrl ?? icon.thumbnailUrl ?? "";
  return /\.gif(?:$|[?#])/i.test(preview);
}

export function applyPresetToImportSettings(
  settings: SheetGridSettings,
  preset: SheetGridPreset,
): SheetGridSettings {
  const importMode =
    preset.mode === "rows_columns" && !preset.rows && preset.cellWidth > 0 && preset.cellHeight > 0
      ? "cell_size"
      : preset.mode;
  return {
    ...settings,
    mode: importMode,
    rows: preset.rows,
    columns: preset.columns,
    cellWidth: preset.cellWidth,
    cellHeight: preset.cellHeight,
    borderLeft: preset.borderLeft,
    borderTop: preset.borderTop,
    borderRight: preset.borderRight,
    borderBottom: preset.borderBottom,
    gapX: preset.gapX,
    gapY: preset.gapY,
    readOrder: preset.readOrder,
  };
}

export function applyPresetToExportRequest(
  request: ExportEditSheetRequest,
  preset: SheetGridPreset,
): ExportEditSheetRequest {
  return {
    ...request,
    cellWidth: preset.cellWidth,
    cellHeight: preset.cellHeight,
    columns: Math.max(1, preset.columns ?? request.columns),
    gapX: preset.gapX,
    gapY: preset.gapY,
    borderX: preset.borderLeft,
    borderY: preset.borderTop,
    background: preset.background,
    includeCleanSheet: preset.includeCleanSheet,
    includeGuideSheet: preset.includeGuideSheet,
    includeManifest: preset.includeManifest,
    labelOptions: parseGuideLabelOptions(preset.guideLabelOptionsJson, request.labelOptions),
    maxSheetWidth: preset.maxSheetWidth,
    maxSheetHeight: preset.maxSheetHeight,
  };
}

export function applyPresetToGifFrameSettings(
  settings: GifFrameSheetSettings,
  preset: SheetGridPreset,
): GifFrameSheetSettings {
  return {
    ...settings,
    frameCellWidth: preset.cellWidth,
    frameCellHeight: preset.cellHeight,
    columns: Math.max(1, preset.columns ?? settings.columns),
    framesPerPage: preset.framesPerPage ?? settings.framesPerPage,
    gapX: preset.gapX,
    gapY: preset.gapY,
    borderX: preset.borderLeft,
    borderY: preset.borderTop,
    background: preset.background,
    includeCleanSheet: preset.includeCleanSheet,
    includeGuideSheet: preset.includeGuideSheet,
    includeManifest: preset.includeManifest,
    maxSheetWidth: preset.maxSheetWidth,
    maxSheetHeight: preset.maxSheetHeight,
  };
}

export function presetInputFromImportSettings(
  name: string,
  collectionId: string,
  settings: SheetGridSettings,
): SheetGridPresetInput {
  return {
    name,
    scope: "collection",
    collectionId,
    kind: "static_import_export",
    cellWidth: settings.cellWidth ?? 200,
    cellHeight: settings.cellHeight ?? 200,
    rows: settings.rows,
    columns: settings.columns,
    mode: settings.mode,
    gapX: settings.gapX,
    gapY: settings.gapY,
    borderLeft: settings.borderLeft,
    borderTop: settings.borderTop,
    borderRight: settings.borderRight,
    borderBottom: settings.borderBottom,
    readOrder: settings.readOrder,
    background: "transparent",
    maxSheetWidth: 2048,
    maxSheetHeight: 2048,
    framesPerPage: null,
    includeCleanSheet: true,
    includeGuideSheet: true,
    includeManifest: true,
    guideLabelOptionsJson: JSON.stringify(defaultExportSheetRequest(collectionId).labelOptions),
  };
}

export function presetInputFromExportRequest(
  name: string,
  request: ExportEditSheetRequest,
): SheetGridPresetInput {
  return {
    name,
    scope: "collection",
    collectionId: request.collectionId,
    kind: "static_import_export",
    cellWidth: request.cellWidth,
    cellHeight: request.cellHeight,
    rows: null,
    columns: request.columns,
    mode: "rows_columns",
    gapX: request.gapX,
    gapY: request.gapY,
    borderLeft: request.borderX,
    borderTop: request.borderY,
    borderRight: request.borderX,
    borderBottom: request.borderY,
    readOrder: "row_major",
    background: request.background,
    maxSheetWidth: request.maxSheetWidth,
    maxSheetHeight: request.maxSheetHeight,
    framesPerPage: null,
    includeCleanSheet: request.includeCleanSheet,
    includeGuideSheet: request.includeGuideSheet,
    includeManifest: request.includeManifest,
    guideLabelOptionsJson: JSON.stringify(request.labelOptions),
  };
}

export function presetInputFromGifFrameSettings(
  name: string,
  collectionId: string,
  settings: GifFrameSheetSettings,
): SheetGridPresetInput {
  return {
    name,
    scope: "collection",
    collectionId,
    kind: "gif_frame_export",
    cellWidth: settings.frameCellWidth,
    cellHeight: settings.frameCellHeight,
    rows: null,
    columns: settings.columns,
    mode: "rows_columns",
    gapX: settings.gapX,
    gapY: settings.gapY,
    borderLeft: settings.borderX,
    borderTop: settings.borderY,
    borderRight: settings.borderX,
    borderBottom: settings.borderY,
    readOrder: "row_major",
    background: settings.background,
    maxSheetWidth: settings.maxSheetWidth,
    maxSheetHeight: settings.maxSheetHeight,
    framesPerPage: settings.framesPerPage,
    includeCleanSheet: settings.includeCleanSheet,
    includeGuideSheet: settings.includeGuideSheet,
    includeManifest: settings.includeManifest,
    guideLabelOptionsJson: JSON.stringify({
      cellNumber: true,
      iconName: false,
      altValue: false,
      exportNumber: true,
    }),
  };
}

function parseGuideLabelOptions(
  json: string,
  fallback: GuideLabelOptions,
): GuideLabelOptions {
  try {
    const parsed = JSON.parse(json) as Partial<GuideLabelOptions>;
    return {
      cellNumber: parsed.cellNumber ?? fallback.cellNumber,
      iconName: parsed.iconName ?? fallback.iconName,
      altValue: parsed.altValue ?? fallback.altValue,
      exportNumber: parsed.exportNumber ?? fallback.exportNumber,
    };
  } catch {
    return fallback;
  }
}

import type { IconSummary } from "@/features/collections/types";

export type SheetGridMode = "rows_columns" | "cell_size";
export type SheetReadOrder = "row_major" | "column_major";
export type SheetBackground = "transparent" | "checker" | "white" | "black";

export interface SheetFilePayload {
  originalFilename: string;
  bytes: number[];
}

export interface SheetGridSettings {
  mode: SheetGridMode;
  rows: number | null;
  columns: number | null;
  cellWidth: number | null;
  cellHeight: number | null;
  borderLeft: number;
  borderTop: number;
  borderRight: number;
  borderBottom: number;
  gapX: number;
  gapY: number;
  readOrder: SheetReadOrder;
  emptyCellThreshold: number;
}

export interface SheetCell {
  index: number;
  page: number;
  row: number;
  col: number;
  x: number;
  y: number;
  w: number;
  h: number;
  outOfBounds: boolean;
  emptyCandidate: boolean;
}

export interface SheetGridAnalysis {
  sheetWidth: number;
  sheetHeight: number;
  computedRows: number;
  computedColumns: number;
  cellCount: number;
  outOfBoundsCells: number[];
  emptyCellCandidates: number[];
  cells: SheetCell[];
  warnings: string[];
}

export interface ImportSheetCellsResult {
  importedIcons: IconSummary[];
  skippedCells: { index: number; reason: string }[];
  warnings: string[];
  preservedSheetPath: string;
  importedCount: number;
}

export interface GuideLabelOptions {
  cellNumber: boolean;
  iconName: boolean;
  altValue: boolean;
  exportNumber: boolean;
}

export interface ExportEditSheetRequest {
  collectionId: string;
  selectedIconIds: string[];
  source: "current_collection" | "selected_icons";
  cellWidth: number;
  cellHeight: number;
  columns: number;
  gapX: number;
  gapY: number;
  borderX: number;
  borderY: number;
  background: SheetBackground;
  includeCleanSheet: boolean;
  includeGuideSheet: boolean;
  includeManifest: boolean;
  labelOptions: GuideLabelOptions;
  maxSheetWidth: number;
  maxSheetHeight: number;
  outputDirectory: string | null;
  openOutputFolder: boolean;
}

export interface ExportEditSheetResult {
  cleanSheetPaths: string[];
  guideSheetPaths: string[];
  manifestPath: string | null;
  outputDirectory: string;
  itemCount: number;
  pageCount: number;
  warnings: string[];
}

export interface ReimportEditSheetResult {
  updatedItems: Array<{
    iconId: string;
    pieceId: string | null;
    newIconId: string | null;
    variantPath: string | null;
  }>;
  createdVariants: string[];
  skippedItems: Array<{ index: number; iconId: string; reason: string }>;
  warnings: string[];
  errors: string[];
}

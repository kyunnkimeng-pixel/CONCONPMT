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

export interface AutoDetectSheetGridProposal {
  id: string;
  label: string;
  method: "alpha" | "solid_background" | string;
  confidence: "high" | "medium" | "low" | string;
  confidenceScore: number;
  gridSettings: SheetGridSettings;
  computedRows: number;
  computedColumns: number;
  cellCount: number;
  warnings: string[];
}

export interface AutoDetectSheetGridResult {
  sheetWidth: number;
  sheetHeight: number;
  hasAlpha: boolean;
  proposals: AutoDetectSheetGridProposal[];
  warnings: string[];
}

export interface ImportSheetCellsResult {
  importedIcons: IconSummary[];
  skippedCells: { index: number; reason: string }[];
  warnings: string[];
  preservedSheetPath: string;
  importedCount: number;
}

export type FrameSheetGifDirection = "forward" | "reverse" | "pingpong";
export type FrameSheetGifLoopMode = "once" | "infinite" | "count";

export interface FrameSheetGifFrameInput {
  sourceCellIndex: number;
  durationMs: number;
}

export interface FrameSheetGifRequest {
  targetCollectionId: string;
  gridSettings: SheetGridSettings;
  frames: FrameSheetGifFrameInput[];
  direction: FrameSheetGifDirection;
  loopMode: FrameSheetGifLoopMode;
  loopCount: number | null;
  displayName: string;
  expectedRenderHash: string | null;
}

export interface FrameSheetGifMeasurement {
  previewPath: string;
  renderHash: string;
  byteSize: number;
  maxBytes: number;
  passesByteLimit: boolean;
  sourceFrameCount: number;
  generatedFrameCount: number;
  durationMs: number;
  width: number;
  height: number;
  normalizedFrameDurationsMs: number[];
  warnings: string[];
}

export interface FrameSheetGifCreateResult {
  icon: IconSummary;
  measurement: FrameSheetGifMeasurement;
  preservedSheetPath: string;
  recipeId: string;
}

export interface ManualSlice {
  sliceId: string;
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  orderIndex: number;
  include: boolean;
  notes: string | null;
}

export interface ManualSlicePreview {
  sliceId: string;
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  orderIndex: number;
  include: boolean;
  outOfBounds: boolean;
  warnings: string[];
}

export interface ManualSliceAnalysis {
  sheetWidth: number;
  sheetHeight: number;
  sliceCount: number;
  includedCount: number;
  outOfBoundsSliceIds: string[];
  slices: ManualSlicePreview[];
  warnings: string[];
}

export interface ImportManualSlicesResult {
  importedIcons: IconSummary[];
  skippedSlices: { sliceId: string; orderIndex: number; reason: string }[];
  warnings: string[];
  preservedSheetPath: string;
  importedCount: number;
}

export interface ManualSliceSaveResult {
  savedCount: number;
  metadataPath: string;
  warnings: string[];
}

export interface ManualSliceLoadResult {
  sheetId: string;
  slices: ManualSlice[];
  metadataPath: string;
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

export interface GifFrameSheetSettings {
  frameCellWidth: number;
  frameCellHeight: number;
  columns: number;
  framesPerPage: number | null;
  gapX: number;
  gapY: number;
  borderX: number;
  borderY: number;
  maxSheetWidth: number;
  maxSheetHeight: number;
  background: SheetBackground;
  includeCleanSheet: boolean;
  includeGuideSheet: boolean;
  includeManifest: boolean;
  outputDirectory: string | null;
  openOutputFolder: boolean;
}

export interface GifFrameSheetExportAnalysis {
  iconId: string;
  displayName: string;
  sourceFormat: string;
  frameCount: number;
  durationMs: number;
  loopMode: string;
  loopCount: number | null;
  pageCount: number;
  sheetWidth: number;
  sheetHeight: number;
  columns: number;
  rowsPerPage: number;
  warnings: string[];
}

export interface GifFrameSheetExportResult {
  frameSheetPaths: string[];
  guideSheetPaths: string[];
  manifestPath: string | null;
  outputDirectory: string;
  frameCount: number;
  pageCount: number;
  warnings: string[];
}

export interface GifFrameSheetReimportValidation {
  frameCount: number;
  detectedFrameCount: number;
  pageCount: number;
  missingPages: number[];
  wrongDimensionPages: number[];
  loopMode: string;
  loopCount: number | null;
  durationMs: number;
  warnings: string[];
  errors: string[];
}

export interface GifFrameSheetReimportResult {
  variantId: string | null;
  outputPath: string | null;
  frameCount: number;
  durationMs: number;
  activeVariantSet: boolean;
  warnings: string[];
  errors: string[];
}

export type SheetGridPresetKind =
  | "static_import_export"
  | "static_import"
  | "static_export"
  | "gif_frame_export";

export type SheetGridPresetScope = "global" | "collection";
export type SheetGridPresetTarget = "import" | "export" | "gif_frame";

export interface SheetGridPreset {
  id: string;
  name: string;
  scope: SheetGridPresetScope;
  collectionId: string | null;
  kind: SheetGridPresetKind;
  cellWidth: number;
  cellHeight: number;
  rows: number | null;
  columns: number | null;
  mode: SheetGridMode;
  gapX: number;
  gapY: number;
  borderLeft: number;
  borderTop: number;
  borderRight: number;
  borderBottom: number;
  readOrder: SheetReadOrder;
  background: SheetBackground;
  maxSheetWidth: number;
  maxSheetHeight: number;
  framesPerPage: number | null;
  includeCleanSheet: boolean;
  includeGuideSheet: boolean;
  includeManifest: boolean;
  guideLabelOptionsJson: string;
  isDefaultForImport: boolean;
  isDefaultForExport: boolean;
  isDefaultForGifFrame: boolean;
  isBuiltin: boolean;
  createdAt: string;
  updatedAt: string;
}

export type SheetGridPresetInput = Omit<
  SheetGridPreset,
  | "id"
  | "isDefaultForImport"
  | "isDefaultForExport"
  | "isDefaultForGifFrame"
  | "isBuiltin"
  | "createdAt"
  | "updatedAt"
>;

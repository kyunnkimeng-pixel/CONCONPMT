export type ExportFormat = "jpg" | "png" | "gif" | "source";
export type ExportProfileType = "dcinside" | "custom";
export type FilenameMode = "sequence" | "alt";
export type ResizeFilter =
  | "nearest"
  | "triangle"
  | "catmull_rom"
  | "gaussian"
  | "lanczos3";
export type ExportItemStatus =
  | "pending"
  | "excluded"
  | "preflight_ok"
  | "preflight_warning"
  | "preflight_not_upload_ready"
  | "rendering"
  | "written_ok"
  | "written_with_warning"
  | "written_not_upload_ready"
  | "failed_to_render"
  | "optimized"
  | "cancelled";

export interface ExportProfile {
  id: string;
  collectionId: string;
  name: string;
  profileType: ExportProfileType;
  targetFormat: ExportFormat;
  targetCellWidth: number;
  targetCellHeight: number;
  previewWidth: number;
  previewHeight: number;
  maxBytes: number;
  allowedFormats: string[];
  filenameMode: FilenameMode;
  includeAltTxt: boolean;
  strictWarnings: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ExportRequestPayload {
  profileId: string;
  targetFormat: ExportFormat;
  targetCellWidth: number;
  targetCellHeight: number;
  maxBytes: number;
  filenameMode: FilenameMode;
  includeAltTxt: boolean;
  strictWarnings: boolean;
  outputDirectory: string | null;
  openFolderAfterExport: boolean;
  openAltTxtAfterExport: boolean;
  excludedPieceIds: string[];
  resizeFilter: ResizeFilter;
}

export interface ExportValidationIssue {
  severity: "error" | "warning";
  blocking: boolean;
  code: string;
  message: string;
  pieceId: string | null;
  iconId: string | null;
}

export interface ExportPlanItem {
  exportIndex: number;
  fileName: string;
  iconId: string;
  pieceId: string;
  pieceRole: "single" | "left" | "right" | "top" | "bottom";
  displayName: string;
  altText: string;
  outputFormat: "jpg" | "png" | "gif";
  width: number;
  height: number;
  byteSize: number | null;
  limitBytes: number;
  included: boolean;
  isAnimated: boolean;
  sourcePreviewUrl: string | null;
  exportPath: string | null;
  status: ExportItemStatus;
}

export interface ExportValidationResult {
  canExport: boolean;
  profile: ExportProfile;
  outputCount: number;
  errors: ExportValidationIssue[];
  warnings: ExportValidationIssue[];
  items: ExportPlanItem[];
}

export interface ExportCollectionResult {
  validation: ExportValidationResult;
  exportDirectory: string | null;
  altTxtPath: string | null;
  manifestPath: string | null;
  reportTxtPath: string | null;
  reportJsonPath: string | null;
  issuesCsvPath: string | null;
}

export interface ExportAssetAnalysis {
  iconId: string;
  profileId: string;
  pieceId: string;
  baselineVariantId: string;
  baselineBytes: number;
  targetMaxBytes: number;
  overByBytes: number;
  overRatio: number;
  format: "jpg" | "png" | "gif";
  width: number;
  height: number;
  frameCount: number | null;
  durationMs: number | null;
  averageFps: number | null;
  loopMode: string | null;
  hasTransparency: boolean | null;
  status: string;
  explanationForUser: string;
}

export interface OptimizationCandidate {
  id: string;
  iconId: string;
  profileId: string;
  pieceId: string;
  preset: "quality" | "balanced" | "smallest" | "custom" | string;
  path: string;
  previewUrl: string;
  format: "jpg" | "png" | "gif";
  measuredByteSize: number;
  targetMaxBytes: number;
  passes: boolean;
  width: number;
  height: number;
  frameCount: number | null;
  originalFrameCount: number | null;
  durationMs: number | null;
  originalDurationMs: number | null;
  loopMode: string | null;
  colorLimit: number | null;
  fpsLimit: number | null;
  quality: number | null;
  qualityImpact: string;
  settingsJson: string;
  summary: string;
  isActiveForExport: boolean;
}

export interface OptimizationResult {
  analysis: ExportAssetAnalysis;
  candidates: OptimizationCandidate[];
  alreadyPasses: boolean;
  fallbackSuggestions: string[];
  message: string;
}

export interface OptimizationAdvancedSettings {
  targetMaxBytes?: number | null;
  safetyMarginPercent?: number | null;
  fpsLimit?: number | null;
  playbackFps?: number | null;
  frameStep?: number | null;
  colorLimit?: number | null;
  jpegQuality?: number | null;
}

export interface ApplyOptimizationResult {
  candidate: OptimizationCandidate;
  message: string;
}

export interface GifPlaybackPreviewResult {
  previewPath: string;
  playbackFps: number;
  generatedAt: string;
}

export interface ClearOptimizationResult {
  iconId: string;
  profileId: string;
  pieceId: string | null;
  clearedCount: number;
  message: string;
}

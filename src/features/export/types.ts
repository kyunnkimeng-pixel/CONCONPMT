export type ExportFormat = "jpg" | "png" | "gif" | "source";
export type ExportProfileType = "dcinside" | "custom";
export type FilenameMode = "sequence" | "alt";

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
}

export interface ExportValidationIssue {
  severity: "error" | "warning";
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
  displayName: string;
  altText: string;
  outputFormat: "jpg" | "png" | "gif";
  width: number;
  height: number;
  byteSize: number | null;
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
}

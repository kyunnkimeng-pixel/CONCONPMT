export interface CollectionSummary {
  id: string;
  name: string;
  coverSourceFileId: string | null;
  coverIconId: string | null;
  coverImageUrl: string | null;
  iconCount: number;
  defaultCellWidth: number;
  defaultCellHeight: number;
  previewWidth: number;
  previewHeight: number;
  exportFormat: "jpg" | "png" | "gif" | "source";
  maxBytes: number;
  createdAt: string;
  updatedAt: string;
}

export interface IconPieceSummary {
  id: string;
  iconId: string;
  pieceIndex: number;
  pieceRole: "single" | "left" | "right" | "top" | "bottom";
  altText: string;
  generatedPreviewUrl: string | null;
  lastExportUrl: string | null;
  exportStatus: "not_exported" | "ready" | "warning" | "error";
  createdAt: string;
  updatedAt: string;
}

export interface IconSummary {
  id: string;
  collectionId: string;
  sourceFileId: string;
  displayName: string;
  shape: "single" | "horizontal_double" | "vertical_double";
  orderIndex: number;
  cellWidthOverride: number | null;
  cellHeightOverride: number | null;
  thumbnailUrl: string | null;
  thumbnailOverrideUrl: string | null;
  currentPreviewUrl: string | null;
  gifLoopMode: "preserve" | "infinite" | "once" | "count";
  gifLoopCount: number | null;
  createdAt: string;
  updatedAt: string;
  pieces: IconPieceSummary[];
}

export interface CollectionSettingsPayload {
  defaultCellWidth: number;
  defaultCellHeight: number;
  previewWidth: number;
  previewHeight: number;
  exportFormat: CollectionSummary["exportFormat"];
  maxBytes: number;
}

export interface AppSettings {
  lastOpenCollectionId: string | null;
  lastViewMode: "explorer" | "usagePreview";
}

export interface LibraryCleanupResult {
  orphanedSourceFiles: number;
  removedOriginalFiles: number;
  removedThumbnailFiles: number;
  removedTempFiles: number;
}

export interface RejectedImportFile {
  originalFilename: string;
  reason: string;
}

export interface ImportImagesResult {
  collection: CollectionSummary;
  importedIcons: IconSummary[];
  rejectedFiles: RejectedImportFile[];
}

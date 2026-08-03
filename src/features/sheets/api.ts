import { normalizeIconSummary } from "@/features/icons/api";
import {
  fileToImportPayload,
  filesToImportPayloads,
} from "@/lib/import-file";
import { invokeCommand } from "@/lib/tauri";
import type {
  ExportEditSheetRequest,
  ExportEditSheetResult,
  AutoDetectSheetGridResult,
  FrameSheetGifCreateResult,
  FrameSheetGifMeasurement,
  FrameSheetGifRequest,
  GifFrameSheetExportAnalysis,
  GifFrameSheetExportResult,
  GifFrameSheetPageDragResult,
  GifFrameSheetReimportResult,
  GifFrameSheetReimportValidation,
  GifFrameSheetSettings,
  ImportSheetCellsResult,
  ImportManualSlicesResult,
  ManualSlice,
  ManualSliceAnalysis,
  ManualSliceLoadResult,
  ManualSliceSaveResult,
  ReimportEditSheetResult,
  SheetFilePayload,
  SheetGridAnalysis,
  SheetGridPreset,
  SheetGridPresetInput,
  SheetGridPresetTarget,
  SheetGridSettings,
} from "@/features/sheets/types";
import type { GifFrameTransparencyMode } from "@/features/sheets/gif-frame-reimport-model";

export type GifFrameManifestSource =
  | { kind: "retained_path"; path: string }
  | { kind: "manual_file"; file: File };

export async function analyzeSheetGrid(file: File, gridSettings: SheetGridSettings) {
  return invokeCommand<SheetGridAnalysis>("analyze_sheet_grid", {
    request: {
      sheetFile: await fileToSheetPayload(file),
      ...gridSettings,
    },
  });
}

export async function autoDetectSheetGrid(file: File) {
  return invokeCommand<AutoDetectSheetGridResult>("auto_detect_sheet_grid", {
    request: {
      sheetFile: await fileToSheetPayload(file),
      alphaSeparatorThreshold: 0.98,
      backgroundSeparatorThreshold: 0.98,
      backgroundTolerance: 8,
      minCellWidth: 16,
      minCellHeight: 16,
    },
  });
}

export async function importSheetCells(
  collectionId: string,
  file: File,
  gridSettings: SheetGridSettings,
  selectedCellIndexes: number[],
  displayNamePattern: string,
) {
  return invokeCommand<ImportSheetCellsResult>("import_sheet_cells", {
    request: {
      sheetFile: await fileToSheetPayload(file),
      targetCollectionId: collectionId,
      gridSettings,
      selectedCellIndexes,
      defaultDisplayNamePattern: displayNamePattern,
      preserveAlpha: true,
    },
  }).then((result) => ({
    ...result,
    importedIcons: result.importedIcons.map(normalizeIconSummary),
  }));
}

export async function measureFrameSheetGif(file: File, request: FrameSheetGifRequest) {
  return invokeCommand<FrameSheetGifMeasurement>("measure_frame_sheet_gif", {
    request: {
      sheetFile: await fileToSheetPayload(file),
      sheetPath: null,
      ...request,
    },
  });
}

export async function createFrameSheetGif(file: File, request: FrameSheetGifRequest) {
  return invokeCommand<FrameSheetGifCreateResult>("create_frame_sheet_gif", {
    request: {
      sheetFile: await fileToSheetPayload(file),
      sheetPath: null,
      ...request,
    },
  }).then((result) => ({
    ...result,
    icon: normalizeIconSummary(result.icon),
  }));
}

export async function analyzeManualSlices(file: File, slices: ManualSlice[]) {
  return invokeCommand<ManualSliceAnalysis>("analyze_manual_slices", {
    request: {
      sheetFile: await fileToSheetPayload(file),
      slices,
    },
  });
}

export async function importManualSlices(
  collectionId: string,
  file: File,
  slices: ManualSlice[],
  displayNamePattern: string,
) {
  return invokeCommand<ImportManualSlicesResult>("import_manual_slices", {
    request: {
      sheetFile: await fileToSheetPayload(file),
      targetCollectionId: collectionId,
      slices,
      defaultDisplayNamePattern: displayNamePattern,
      preserveAlpha: true,
    },
  }).then((result) => ({
    ...result,
    importedIcons: result.importedIcons.map(normalizeIconSummary),
  }));
}

export function saveManualSlices(sheetId: string, slices: ManualSlice[]) {
  return invokeCommand<ManualSliceSaveResult>("save_manual_slices", {
    request: { sheetId, slices },
  });
}

export function loadManualSlices(sheetId: string) {
  return invokeCommand<ManualSliceLoadResult>("load_manual_slices", { sheetId });
}

export function exportEditSheet(request: ExportEditSheetRequest) {
  return invokeCommand<ExportEditSheetResult>("export_edit_sheet", { request });
}

export async function reimportEditSheet(
  collectionId: string,
  manifestFile: File,
  editedSheetFiles: File[],
  reimportMode: "create_new_icons" | "create_processed_variants" = "create_new_icons",
) {
  const [manifestPayload, ...editedSheetPayloads] = await filesToImportPayloads([
    manifestFile,
    ...editedSheetFiles,
  ]);
  return invokeCommand<ReimportEditSheetResult>("reimport_edit_sheet", {
    request: {
      manifestPath: "",
      manifestFile: manifestPayload,
      editedSheetFiles: editedSheetPayloads,
      editedSheetPaths: [],
      targetCollectionId: collectionId,
      reimportMode,
    },
  });
}

export function analyzeGifFrameSheetExport(iconId: string, settings: GifFrameSheetSettings) {
  return invokeCommand<GifFrameSheetExportAnalysis>("analyze_gif_frame_sheet_export", {
    request: {
      iconId,
      settings,
    },
  });
}

export function exportGifFrameSheet(iconId: string, settings: GifFrameSheetSettings) {
  return invokeCommand<GifFrameSheetExportResult>("export_gif_frame_sheet", {
    request: {
      iconId,
      settings,
    },
  });
}

function normalizeGifFrameManifestSource(
  source: GifFrameManifestSource | File,
): GifFrameManifestSource {
  return source instanceof File ? { kind: "manual_file", file: source } : source;
}

function gifFrameResultBackgroundPolicy(mode: GifFrameTransparencyMode) {
  return mode === "allow_opaque" ? "allowOpaque" : "preserveTransparency";
}

async function gifFrameManifestRequestFields(
  sourceInput: GifFrameManifestSource | File,
) {
  const source = normalizeGifFrameManifestSource(sourceInput);
  if (source.kind === "retained_path") {
    return { manifestPath: source.path, manifestFile: null };
  }
  return {
    manifestPath: "",
    manifestFile: await fileToImportPayload(source.file),
  };
}

export async function validateGifFrameSheetReimport(
  manifestSource: GifFrameManifestSource | File,
  editedFrameSheetFiles: File[],
  editedFrameSheetPageIndexes: number[] = editedFrameSheetFiles.map(
    (_, pageIndex) => pageIndex,
  ),
  transparencyMode: GifFrameTransparencyMode = "preserve_alpha",
) {
  const [manifestFields, editedFrameSheetPayloads] = await Promise.all([
    gifFrameManifestRequestFields(manifestSource),
    filesToImportPayloads(editedFrameSheetFiles),
  ]);
  return invokeCommand<GifFrameSheetReimportValidation>("validate_gif_frame_sheet_reimport", {
    request: {
      ...manifestFields,
      editedFrameSheetFiles: editedFrameSheetPayloads,
      editedFrameSheetPaths: [],
      editedFrameSheetPageIndexes,
      resultBackgroundPolicy: gifFrameResultBackgroundPolicy(transparencyMode),
    },
  });
}

export async function reimportGifFrameSheet(
  targetIconId: string,
  manifestSource: GifFrameManifestSource | File,
  editedFrameSheetFiles: File[],
  setActiveVariant: boolean,
  targetProfileId: string | null,
  editedFrameSheetPageIndexes: number[] = editedFrameSheetFiles.map(
    (_, pageIndex) => pageIndex,
  ),
  transparencyMode: GifFrameTransparencyMode = "preserve_alpha",
) {
  const [manifestFields, editedFrameSheetPayloads] = await Promise.all([
    gifFrameManifestRequestFields(manifestSource),
    filesToImportPayloads(editedFrameSheetFiles),
  ]);
  return invokeCommand<GifFrameSheetReimportResult>("reimport_gif_frame_sheet", {
    request: {
      ...manifestFields,
      editedFrameSheetFiles: editedFrameSheetPayloads,
      editedFrameSheetPaths: [],
      editedFrameSheetPageIndexes,
      resultBackgroundPolicy: gifFrameResultBackgroundPolicy(transparencyMode),
      targetIconId,
      createVariant: true,
      setActiveVariant,
      targetProfileId,
    },
  });
}

export function startGifFrameSheetPageDrag(
  manifestPath: string,
  pageIndex: number,
) {
  return invokeCommand<GifFrameSheetPageDragResult>(
    "start_gif_frame_sheet_page_drag",
    { manifestPath, pageIndex },
  );
}

export function revealGifFrameSheetPage(
  manifestPath: string,
  pageIndex: number,
) {
  return invokeCommand<void>("reveal_gif_frame_sheet_page", {
    manifestPath,
    pageIndex,
  });
}
export function listSheetGridPresets(collectionId: string | null) {
  return invokeCommand<SheetGridPreset[]>("list_sheet_grid_presets", {
    collectionId,
  });
}

export function createSheetGridPreset(input: SheetGridPresetInput) {
  return invokeCommand<SheetGridPreset>("create_sheet_grid_preset", { input });
}

export function updateSheetGridPreset(id: string, input: SheetGridPresetInput) {
  return invokeCommand<SheetGridPreset>("update_sheet_grid_preset", { id, input });
}

export function deleteSheetGridPreset(id: string) {
  return invokeCommand<void>("delete_sheet_grid_preset", { id });
}

export function duplicateSheetGridPreset(id: string) {
  return invokeCommand<SheetGridPreset>("duplicate_sheet_grid_preset", { id });
}

export function setDefaultSheetGridPreset(
  id: string,
  target: SheetGridPresetTarget,
  collectionId: string | null,
) {
  return invokeCommand<SheetGridPreset>("set_default_sheet_grid_preset", {
    id,
    target,
    collectionId,
  });
}

export function getDefaultSheetGridPreset(
  target: SheetGridPresetTarget,
  collectionId: string | null,
) {
  return invokeCommand<SheetGridPreset | null>("get_default_sheet_grid_preset", {
    target,
    collectionId,
  });
}

export async function fileToSheetPayload(file: File): Promise<SheetFilePayload> {
  return fileToImportPayload(file);
}

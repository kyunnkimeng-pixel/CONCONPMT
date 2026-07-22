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
  GifFrameSheetExportAnalysis,
  GifFrameSheetExportResult,
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

export async function validateGifFrameSheetReimport(
  manifestFile: File,
  editedFrameSheetFiles: File[],
) {
  const [manifestPayload, ...editedFrameSheetPayloads] = await filesToImportPayloads([
    manifestFile,
    ...editedFrameSheetFiles,
  ]);
  return invokeCommand<GifFrameSheetReimportValidation>("validate_gif_frame_sheet_reimport", {
    request: {
      manifestPath: "",
      manifestFile: manifestPayload,
      editedFrameSheetFiles: editedFrameSheetPayloads,
      editedFrameSheetPaths: [],
    },
  });
}

export async function reimportGifFrameSheet(
  targetIconId: string,
  manifestFile: File,
  editedFrameSheetFiles: File[],
  setActiveVariant: boolean,
  targetProfileId: string | null,
) {
  const [manifestPayload, ...editedFrameSheetPayloads] = await filesToImportPayloads([
    manifestFile,
    ...editedFrameSheetFiles,
  ]);
  return invokeCommand<GifFrameSheetReimportResult>("reimport_gif_frame_sheet", {
    request: {
      manifestPath: "",
      manifestFile: manifestPayload,
      editedFrameSheetFiles: editedFrameSheetPayloads,
      editedFrameSheetPaths: [],
      targetIconId,
      createVariant: true,
      setActiveVariant,
      targetProfileId,
    },
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

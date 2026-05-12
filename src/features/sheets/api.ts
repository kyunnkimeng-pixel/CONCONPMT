import { normalizeIconSummary } from "@/features/icons/api";
import { invokeCommand } from "@/lib/tauri";
import type {
  ExportEditSheetRequest,
  ExportEditSheetResult,
  ImportSheetCellsResult,
  ReimportEditSheetResult,
  SheetFilePayload,
  SheetGridAnalysis,
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

export function exportEditSheet(request: ExportEditSheetRequest) {
  return invokeCommand<ExportEditSheetResult>("export_edit_sheet", { request });
}

export async function reimportEditSheet(
  collectionId: string,
  manifestFile: File,
  editedSheetFiles: File[],
  reimportMode: "create_new_icons" | "create_processed_variants" = "create_new_icons",
) {
  return invokeCommand<ReimportEditSheetResult>("reimport_edit_sheet", {
    request: {
      manifestPath: "",
      manifestFile: await fileToSheetPayload(manifestFile),
      editedSheetFiles: await Promise.all(editedSheetFiles.map(fileToSheetPayload)),
      editedSheetPaths: [],
      targetCollectionId: collectionId,
      reimportMode,
    },
  });
}

export async function fileToSheetPayload(file: File): Promise<SheetFilePayload> {
  return {
    originalFilename: file.name,
    bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
  };
}

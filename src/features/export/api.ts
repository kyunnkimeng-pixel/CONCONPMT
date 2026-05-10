import { invokeCommand } from "@/lib/tauri";
import type {
  ExportCollectionResult,
  ExportProfile,
  ExportRequestPayload,
  ExportValidationResult,
} from "@/features/export/types";

export function listExportProfiles(collectionId: string) {
  return invokeCommand<ExportProfile[]>("list_export_profiles", { collectionId });
}

export function saveExportProfileSettings(
  collectionId: string,
  payload: ExportRequestPayload,
) {
  return invokeCommand<ExportProfile>("save_export_profile_settings", {
    collectionId,
    payload,
  });
}

export function validateExportCollection(
  collectionId: string,
  payload: ExportRequestPayload,
) {
  return invokeCommand<ExportValidationResult>("validate_export_collection", {
    collectionId,
    payload,
  });
}

export function exportCollection(collectionId: string, payload: ExportRequestPayload) {
  return invokeCommand<ExportCollectionResult>("export_collection", {
    collectionId,
    payload,
  });
}

export function openExportPath(path: string) {
  return invokeCommand<void>("open_export_path", { path });
}

export function pickExportDirectory(initialDirectory: string | null) {
  return invokeCommand<string | null>("pick_export_directory", {
    initialDirectory,
  });
}

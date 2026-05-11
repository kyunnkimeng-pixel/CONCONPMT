import { invokeCommand } from "@/lib/tauri";
import type {
  ExportCollectionResult,
  ApplyOptimizationResult,
  ClearOptimizationResult,
  ExportProfile,
  ExportRequestPayload,
  ExportValidationResult,
  OptimizationAdvancedSettings,
  OptimizationResult,
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

export function generateGifOptimizationCandidates(
  iconId: string,
  profileId: string,
  pieceId: string,
  advancedSettings: OptimizationAdvancedSettings | null = null,
) {
  return invokeCommand<OptimizationResult>("generate_gif_optimization_candidates", {
    iconId,
    profileId,
    pieceId,
    mode: advancedSettings ? "custom" : "auto",
    advancedSettings,
  });
}

export function generateStaticOptimizationCandidates(
  iconId: string,
  profileId: string,
  pieceId: string,
  advancedSettings: OptimizationAdvancedSettings | null = null,
) {
  return invokeCommand<OptimizationResult>("generate_static_optimization_candidates", {
    iconId,
    profileId,
    pieceId,
    mode: advancedSettings ? "custom" : "auto",
    advancedSettings,
  });
}

export function applyOptimizationCandidate(candidateId: string) {
  return invokeCommand<ApplyOptimizationResult>("apply_optimization_candidate", {
    candidateId,
  });
}

export function clearOptimizationCandidate(
  iconId: string,
  profileId: string,
  pieceId: string,
) {
  return invokeCommand<ClearOptimizationResult>("clear_optimization_candidate", {
    iconId,
    profileId,
    pieceId,
  });
}

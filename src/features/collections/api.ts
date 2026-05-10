import { invokeCommand } from "@/lib/tauri";
import type {
  AppSettings,
  CollectionSettingsPayload,
  CollectionSummary,
  LibraryCleanupResult,
} from "@/features/collections/types";
import { filePathToAssetUrl } from "@/lib/asset-url";

export function listCollections() {
  return invokeCommand<CollectionSummary[]>("list_collections").then((collections) =>
    collections.map(normalizeCollectionSummary),
  );
}

export function createCollection(name?: string) {
  return invokeCommand<CollectionSummary>("create_collection", { name }).then(
    normalizeCollectionSummary,
  );
}

export function renameCollection(collectionId: string, name: string) {
  return invokeCommand<CollectionSummary>("rename_collection", { collectionId, name }).then(
    normalizeCollectionSummary,
  );
}

export function deleteCollection(collectionId: string) {
  return invokeCommand<void>("delete_collection", { collectionId });
}

export function duplicateCollection(collectionId: string) {
  return invokeCommand<CollectionSummary>("duplicate_collection", { collectionId }).then(
    normalizeCollectionSummary,
  );
}

export function setCollectionCoverIcon(collectionId: string, iconId: string) {
  return invokeCommand<CollectionSummary>("set_collection_cover_icon", {
    collectionId,
    iconId,
  }).then(normalizeCollectionSummary);
}

export async function importCollectionCoverImage(collectionId: string, file: File) {
  return invokeCommand<CollectionSummary>("import_collection_cover_image", {
    collectionId,
    file: await fileToImportPayload(file),
  }).then(normalizeCollectionSummary);
}

export function updateCollectionSettings(
  collectionId: string,
  payload: CollectionSettingsPayload,
) {
  return invokeCommand<CollectionSummary>("update_collection_settings", {
    collectionId,
    payload,
  }).then(normalizeCollectionSummary);
}

export function getAppSettings() {
  return invokeCommand<AppSettings>("get_app_settings");
}

export function saveAppSettings(payload: AppSettings) {
  return invokeCommand<AppSettings>("save_app_settings", { payload });
}

export function previewLibraryCleanup() {
  return invokeCommand<LibraryCleanupResult>("preview_library_cleanup");
}

export function cleanupLibrary() {
  return invokeCommand<LibraryCleanupResult>("cleanup_library");
}

export function normalizeCollectionSummary(collection: CollectionSummary): CollectionSummary {
  return {
    ...collection,
    coverImageUrl: filePathToAssetUrl(collection.coverImageUrl, collection.updatedAt),
  };
}

async function fileToImportPayload(file: File) {
  const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));

  return {
    originalFilename: file.name,
    bytes,
  };
}

import { invokeCommand } from "@/lib/tauri";
import type { CollectionSummary } from "@/features/collections/types";
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

export function normalizeCollectionSummary(collection: CollectionSummary): CollectionSummary {
  return {
    ...collection,
    coverImageUrl: filePathToAssetUrl(collection.coverImageUrl, collection.updatedAt),
  };
}

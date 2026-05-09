import { normalizeCollectionSummary } from "@/features/collections/api";
import type {
  CollectionSummary,
  IconSummary,
  ImportImagesResult,
} from "@/features/collections/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import { invokeCommand } from "@/lib/tauri";

export function listIcons(collectionId: string) {
  return invokeCommand<IconSummary[]>("list_icons", { collectionId }).then((icons) =>
    icons.map(normalizeIconSummary),
  );
}

export async function importImagesIntoCollection(collectionId: string, files: File[]) {
  const payload = await Promise.all(files.map(fileToImportPayload));

  return invokeCommand<ImportImagesResult>("import_image_files", {
    collectionId,
    files: payload,
  }).then((result) => ({
    ...result,
    collection: normalizeCollectionSummary(result.collection),
    importedIcons: result.importedIcons.map(normalizeIconSummary),
  }));
}

export function updateIconPieceAlt(
  collectionId: string,
  pieceId: string,
  altText: string,
) {
  return invokeCommand<IconSummary>("update_icon_piece_alt", {
    collectionId,
    pieceId,
    altText,
  }).then(normalizeIconSummary);
}

export function duplicateIcon(collectionId: string, iconId: string) {
  return invokeCommand<IconSummary>("duplicate_icon", {
    collectionId,
    iconId,
  }).then(normalizeIconSummary);
}

export function deleteIcons(collectionId: string, iconIds: string[]) {
  return invokeCommand<CollectionSummary>("delete_icons", {
    collectionId,
    iconIds,
  }).then(normalizeCollectionSummary);
}

export function reorderIcons(collectionId: string, iconIds: string[]) {
  return invokeCommand<IconSummary[]>("reorder_icons", {
    collectionId,
    iconIds,
  }).then((icons) => icons.map(normalizeIconSummary));
}

export function normalizeIconSummary(icon: IconSummary): IconSummary {
  return {
    ...icon,
    thumbnailUrl: filePathToAssetUrl(icon.thumbnailUrl, icon.updatedAt),
    currentPreviewUrl: filePathToAssetUrl(icon.currentPreviewUrl, icon.updatedAt),
    pieces: icon.pieces.map((piece) => ({
      ...piece,
      generatedPreviewUrl: filePathToAssetUrl(
        piece.generatedPreviewUrl,
        piece.updatedAt,
      ),
    })),
  };
}

async function fileToImportPayload(file: File) {
  const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));

  return {
    originalFilename: file.name,
    bytes,
  };
}

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

export function renameIcon(
  collectionId: string,
  iconId: string,
  displayName: string,
) {
  return invokeCommand<IconSummary>("rename_icon", {
    collectionId,
    iconId,
    displayName,
  }).then(normalizeIconSummary);
}

export async function setIconThumbnailOverride(
  collectionId: string,
  iconId: string,
  file: File,
) {
  return invokeCommand<IconSummary>("set_icon_thumbnail_override", {
    collectionId,
    iconId,
    file: await fileToImportPayload(file),
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

export function revealIconOriginal(collectionId: string, iconId: string) {
  return invokeCommand<void>("reveal_icon_original", { collectionId, iconId });
}

export function revealIconExportResult(collectionId: string, iconId: string) {
  return invokeCommand<void>("reveal_icon_export_result", { collectionId, iconId });
}

export function normalizeIconSummary(icon: IconSummary): IconSummary {
  return {
    ...icon,
    thumbnailUrl: filePathToAssetUrl(icon.thumbnailUrl, icon.updatedAt),
    thumbnailOverrideUrl: filePathToAssetUrl(
      icon.thumbnailOverrideUrl,
      icon.updatedAt,
    ),
    currentPreviewUrl: filePathToAssetUrl(icon.currentPreviewUrl, icon.updatedAt),
    pieces: icon.pieces.map((piece) => ({
      ...piece,
      generatedPreviewUrl: filePathToAssetUrl(
        piece.generatedPreviewUrl,
        piece.updatedAt,
      ),
      lastExportUrl: filePathToAssetUrl(piece.lastExportUrl, piece.updatedAt),
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

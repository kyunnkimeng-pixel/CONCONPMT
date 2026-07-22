import { normalizeCollectionSummary } from "@/features/collections/api";
import type {
  CollectionSummary,
  IconSummary,
  ImportImagesResult,
} from "@/features/collections/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import { fileToImportPayload, importFileSizeError } from "@/lib/import-file";
import { getCommandErrorMessage, invokeCommand } from "@/lib/tauri";

export function listIcons(collectionId: string) {
  return invokeCommand<IconSummary[]>("list_icons", { collectionId }).then((icons) =>
    icons.map(normalizeIconSummary),
  );
}

export interface ImportProgressEvent {
  current: number;
  total: number;
  fileName: string;
}

export async function importImagesIntoCollection(
  collectionId: string,
  files: File[],
  onProgress?: (event: ImportProgressEvent) => void,
) {
  const importedIcons: IconSummary[] = [];
  const rejectedFiles: ImportImagesResult["rejectedFiles"] = [];
  let latestResult: ImportImagesResult | null = null;
  const total = files.length + 1;

  for (const [index, file] of files.entries()) {
    onProgress?.({ current: index + 1, total, fileName: file.name });
    const sizeError = importFileSizeError(file);
    if (sizeError) {
      rejectedFiles.push({ originalFilename: file.name, reason: sizeError });
      continue;
    }

    try {
      const result = await invokeCommand<ImportImagesResult>("import_image_files", {
        collectionId,
        files: [await fileToImportPayload(file)],
      });
      latestResult = result;
      importedIcons.push(...result.importedIcons.map(normalizeIconSummary));
      rejectedFiles.push(...result.rejectedFiles);
    } catch (error) {
      const reason = getCommandErrorMessage(error);
      rejectedFiles.push({ originalFilename: file.name, reason });
      for (const remainingFile of files.slice(index + 1)) {
        rejectedFiles.push({
          originalFilename: remainingFile.name,
          reason: `앞선 파일 처리 오류로 가져오기를 중단했습니다: ${reason}`,
        });
      }
      break;
    }
  }
  onProgress?.({
    current: total,
    total,
    fileName: "앱 라이브러리 등록 완료",
  });

  latestResult ??= await invokeCommand<ImportImagesResult>("import_image_files", {
    collectionId,
    files: [],
  });

  return {
    ...latestResult,
    collection: normalizeCollectionSummary(latestResult.collection),
    importedIcons,
    rejectedFiles,
  };
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

export function createPlaceholderIcon(collectionId: string, label: string) {
  return invokeCommand<IconSummary>("create_placeholder_icon", {
    collectionId,
    payload: { label },
  }).then(normalizeIconSummary);
}

export async function replaceIconSource(
  collectionId: string,
  iconId: string,
  file: File,
) {
  return invokeCommand<IconSummary>("replace_icon_source", {
    collectionId,
    iconId,
    file: await fileToImportPayload(file),
  }).then(normalizeIconSummary);
}

export function setIconsReadiness(
  collectionId: string,
  iconIds: string[],
  readiness: IconSummary["readiness"],
) {
  return invokeCommand<IconSummary[]>("set_icons_readiness", {
    collectionId,
    iconIds,
    readiness,
  }).then((icons) => icons.map(normalizeIconSummary));
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

export function getIconNote(collectionId: string, iconId: string) {
  return invokeCommand<string | null>("get_icon_note", { collectionId, iconId });
}

export function updateIconNote(collectionId: string, iconId: string, note: string) {
  return invokeCommand<IconSummary>("update_icon_note", {
    collectionId,
    iconId,
    note,
  }).then(normalizeIconSummary);
}

export function clearIconNote(collectionId: string, iconId: string) {
  return invokeCommand<IconSummary>("clear_icon_note", {
    collectionId,
    iconId,
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

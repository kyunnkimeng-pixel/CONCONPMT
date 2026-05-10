const IMPORTABLE_IMAGE_EXTENSIONS = new Set(["jpg", "jpeg", "png", "gif"]);

export const IMPORTABLE_IMAGE_ACCEPT = ".jpg,.jpeg,.png,.gif,image/jpeg,image/png,image/gif";
export const COVER_IMAGE_ACCEPT = ".jpg,.jpeg,.png,image/jpeg,image/png";

export interface ImportablePartition {
  accepted: File[];
  rejected: File[];
}

export function partitionImportableImageFiles(files: File[]): ImportablePartition {
  const accepted: File[] = [];
  const rejected: File[] = [];

  for (const file of files) {
    if (isImportableImageFile(file)) {
      accepted.push(file);
    } else {
      rejected.push(file);
    }
  }

  return { accepted, rejected };
}

export function isImportableImageFile(file: File) {
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";

  return IMPORTABLE_IMAGE_EXTENSIONS.has(extension);
}

export function isCoverImageFile(file: File) {
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";

  return extension === "jpg" || extension === "jpeg" || extension === "png";
}

export function sortFilesForImport(files: File[]) {
  return [...files].sort((left, right) =>
    fileImportKey(left).localeCompare(fileImportKey(right), "ko-KR", {
      numeric: true,
      sensitivity: "base",
    }),
  );
}

function fileImportKey(file: File) {
  const relativePath = (file as File & { webkitRelativePath?: string }).webkitRelativePath;
  return relativePath?.trim() || file.name;
}

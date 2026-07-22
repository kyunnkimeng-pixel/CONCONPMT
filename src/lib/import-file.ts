export const MAX_IMPORT_FILE_BYTES = 64 * 1024 * 1024;
export const MAX_IMPORT_BATCH_BYTES = 64 * 1024 * 1024;

export interface ImportFilePayload {
  originalFilename: string;
  bytes: number[];
}

export interface ImportRejectionSummary {
  originalFilename: string;
  reason: string;
}

export function partitionFilesByImportSize(files: File[]) {
  const accepted: File[] = [];
  const rejected: ImportRejectionSummary[] = [];

  for (const file of files) {
    const reason = importFileSizeError(file);
    if (reason) {
      rejected.push({ originalFilename: file.name, reason });
    } else {
      accepted.push(file);
    }
  }

  return { accepted, rejected };
}

export function importFileSizeError(file: Pick<File, "name" | "size">) {
  if (file.size <= MAX_IMPORT_FILE_BYTES) {
    return null;
  }

  return `${file.name}: 원본 파일은 최대 64MB까지 가져올 수 있습니다.`;
}

export function importBatchSizeError(files: ReadonlyArray<Pick<File, "size">>) {
  const totalBytes = files.reduce((sum, file) => sum + file.size, 0);
  return totalBytes > MAX_IMPORT_BATCH_BYTES
    ? "한 번에 전송하는 파일의 합계는 최대 64MB까지 지원합니다. 파일을 나눠서 처리해 주세요."
    : null;
}

export async function fileToImportPayload(file: File): Promise<ImportFilePayload> {
  const sizeError = importFileSizeError(file);
  if (sizeError) {
    throw new Error(sizeError);
  }

  return {
    originalFilename: file.name,
    bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
  };
}

export async function filesToImportPayloads(files: File[]) {
  const batchError = importBatchSizeError(files);
  if (batchError) {
    throw new Error(batchError);
  }

  const payloads: ImportFilePayload[] = [];
  for (const file of files) {
    payloads.push(await fileToImportPayload(file));
  }
  return payloads;
}

export function formatImportResultMessage(
  importedCount: number,
  unsupportedFiles: ReadonlyArray<Pick<File, "name">>,
  rejectedFiles: readonly ImportRejectionSummary[],
) {
  const skippedCount = unsupportedFiles.length + rejectedFiles.length;
  const resultMessage = importedCount === 0
    ? "가져온 이미지가 없습니다."
    : `${importedCount}개 이미지를 가져왔습니다.`;

  if (skippedCount === 0) {
    return resultMessage;
  }

  const rejectionDetails: ImportRejectionSummary[] = [
    ...unsupportedFiles.map((file) => ({
      originalFilename: file.name,
      reason: "지원 형식이 아닙니다.",
    })),
    ...rejectedFiles,
  ];
  const visibleDetails = rejectionDetails.slice(0, 2).map(formatRejectionDetail);
  const hiddenCount = rejectionDetails.length - visibleDetails.length;
  const hiddenMessage = hiddenCount > 0 ? ` 외 ${hiddenCount}개` : "";

  return `${resultMessage} ${skippedCount}개 파일을 건너뛰었습니다: ${visibleDetails.join(" / ")}${hiddenMessage}`;
}

function formatRejectionDetail(rejection: ImportRejectionSummary) {
  return rejection.reason.startsWith(`${rejection.originalFilename}:`)
    ? rejection.reason
    : `${rejection.originalFilename}: ${rejection.reason}`;
}

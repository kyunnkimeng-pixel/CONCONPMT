export interface GifFrameReimportPageSlot {
  pageIndex: number;
  expectedFileName: string;
  width: number;
  height: number;
}

export interface GifFrameReimportFileSelection {
  manifestFile: File | null;
  imageFiles: File[];
  error: string | null;
}

export const MAX_GIF_FRAME_REIMPORT_MANIFEST_BYTES = 4 * 1024 * 1024;
export const MAX_GIF_FRAME_REIMPORT_PAGE_BYTES = 64 * 1024 * 1024;
export const MAX_GIF_FRAME_REIMPORT_TOTAL_BYTES = 64 * 1024 * 1024;
export const MAX_GIF_FRAME_REIMPORT_PAGE_COUNT = 500;

export type GifFrameTransparencyMode = "preserve_alpha" | "allow_opaque";

type JsonRecord = Record<string, unknown>;

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSafePngFileName(value: string) {
  return (
    value.length > 0 &&
    value.length <= 255 &&
    /^[a-z0-9._-]+\.png$/i.test(value) &&
    !value.includes("..")
  );
}

export function isSupportedGifFrameResultFileName(fileName: string) {
  return /\.(?:png|jpe?g|webp)$/i.test(fileName.trim());
}

export function gifFrameResultNeedsOpaqueWarning(file: File) {
  return /\.jpe?g$/i.test(file.name) || file.type === "image/jpeg";
}

function stripBrowserDuplicateSuffix(stem: string) {
  let current = stem.trimEnd();
  while (current.endsWith(")")) {
    const opening = current.lastIndexOf("(");
    if (opening < 0) break;
    const suffix = current.slice(opening + 1, -1);
    if (!/^\d+$/.test(suffix)) break;
    const prefix = current.slice(0, opening).trimEnd();
    if (!prefix) break;
    current = prefix;
  }
  return current;
}

export function normalizedGifFramePageFileName(fileName: string) {
  const separator = Math.max(fileName.lastIndexOf("/"), fileName.lastIndexOf("\\"));
  const leaf = fileName.slice(separator + 1).trim();
  const dot = leaf.lastIndexOf(".");
  if (dot <= 0 || !/^(?:png|jpe?g|webp)$/i.test(leaf.slice(dot + 1))) return null;
  const stem = stripBrowserDuplicateSuffix(leaf.slice(0, dot));
  return stem ? `${stem.toLowerCase()}.png` : null;
}

export function classifyGifFrameReimportFiles(
  files: Iterable<File> | ArrayLike<File>,
): GifFrameReimportFileSelection {
  const items = Array.from(files as ArrayLike<File>);
  if (items.length === 0) {
    return {
      manifestFile: null,
      imageFiles: [],
      error: "frames_manifest.json과 수정한 PNG를 선택해 주세요.",
    };
  }
  const manifests = items.filter((file) => /\.json$/i.test(file.name));
  const imageFiles = items.filter((file) =>
    isSupportedGifFrameResultFileName(file.name),
  );
  const unsupported = items.filter(
    (file) =>
      !/\.json$/i.test(file.name) &&
      !isSupportedGifFrameResultFileName(file.name),
  );
  if (unsupported.length > 0) {
    return {
      manifestFile: null,
      imageFiles: [],
      error: `${unsupported.map((file) => file.name).join(", ")}: 수정 결과는 PNG, JPG/JPEG 또는 정적 WebP만 사용할 수 있습니다. GIF·애니메이션 WebP는 지원하지 않습니다.`,
    };
  }
  if (manifests.length > 1) {
    return {
      manifestFile: null,
      imageFiles: [],
      error: "manifest JSON은 한 개만 선택해 주세요.",
    };
  }
  if (imageFiles.length > MAX_GIF_FRAME_REIMPORT_PAGE_COUNT) {
    return {
      manifestFile: null,
      imageFiles: [],
      error: `수정 결과 이미지는 최대 ${MAX_GIF_FRAME_REIMPORT_PAGE_COUNT}개까지 한 번에 가져올 수 있습니다. 현재 ${imageFiles.length}개가 선택되었습니다.`,
    };
  }
  const oversizedManifest = manifests.find(
    (file) => file.size > MAX_GIF_FRAME_REIMPORT_MANIFEST_BYTES,
  );
  if (oversizedManifest) {
    return {
      manifestFile: null,
      imageFiles: [],
      error: `${oversizedManifest.name}: manifest JSON은 최대 4MB까지 읽을 수 있습니다.`,
    };
  }
  const oversizedImage = imageFiles.find(
    (file) => file.size > MAX_GIF_FRAME_REIMPORT_PAGE_BYTES,
  );
  if (oversizedImage) {
    return {
      manifestFile: null,
      imageFiles: [],
      error: `${oversizedImage.name}: 수정 결과 이미지 한 장은 최대 64MB까지 처리할 수 있습니다.`,
    };
  }
  const totalBytes = items.reduce((total, file) => total + file.size, 0);
  if (totalBytes > MAX_GIF_FRAME_REIMPORT_TOTAL_BYTES) {
    return {
      manifestFile: null,
      imageFiles: [],
      error: `manifest JSON과 수정 결과 이미지의 합계는 64MB까지 처리할 수 있습니다. 현재 약 ${Math.ceil(totalBytes / (1024 * 1024))}MB입니다.`,
    };
  }
  return {
    manifestFile: manifests[0] ?? null,
    imageFiles,
    error: null,
  };
}

export async function readGifFrameReimportPageSlots(
  manifestFile: File,
): Promise<GifFrameReimportPageSlot[]> {
  if (manifestFile.size > MAX_GIF_FRAME_REIMPORT_MANIFEST_BYTES) {
    throw new Error(`${manifestFile.name}: manifest JSON은 최대 4MB까지 읽을 수 있습니다.`);
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(await manifestFile.text());
  } catch {
    throw new Error("manifest JSON을 읽을 수 없습니다. 내보낸 frames_manifest.json을 다시 선택해 주세요.");
  }
  if (!isRecord(parsed) || !Array.isArray(parsed.pages) || parsed.pages.length === 0) {
    throw new Error("manifest에 GIF 프레임 시트 페이지 정보가 없습니다.");
  }
  if (parsed.pages.length > MAX_GIF_FRAME_REIMPORT_PAGE_COUNT) {
    throw new Error(
      `manifest 페이지는 최대 ${MAX_GIF_FRAME_REIMPORT_PAGE_COUNT}개까지 처리할 수 있습니다. 현재 ${parsed.pages.length}개입니다.`,
    );
  }
  const slots = parsed.pages.map((page, index) => {
    if (!isRecord(page)) {
      throw new Error(`${index + 1}번째 manifest 페이지 정보가 올바르지 않습니다.`);
    }
    const pageIndex = page.page_index;
    const expectedFileName = page.clean_sheet_file ?? page.sheet_file;
    const width = page.width;
    const height = page.height;
    if (
      !Number.isSafeInteger(pageIndex) ||
      Number(pageIndex) < 0 ||
      typeof expectedFileName !== "string" ||
      !isSafePngFileName(expectedFileName) ||
      !Number.isSafeInteger(width) ||
      Number(width) <= 0 ||
      !Number.isSafeInteger(height) ||
      Number(height) <= 0
    ) {
      throw new Error(`${index + 1}번째 manifest 페이지 정보가 올바르지 않습니다.`);
    }
    return {
      pageIndex: Number(pageIndex),
      expectedFileName,
      width: Number(width),
      height: Number(height),
    };
  });
  slots.sort((left, right) => left.pageIndex - right.pageIndex);
  if (
    new Set(slots.map((slot) => slot.pageIndex)).size !== slots.length ||
    new Set(slots.map((slot) => slot.expectedFileName.toLowerCase())).size !== slots.length
  ) {
    throw new Error("manifest의 페이지 번호 또는 파일명이 중복되었습니다.");
  }
  return slots;
}

export function autoAssignGifFrameFiles(
  slots: ReadonlyArray<GifFrameReimportPageSlot>,
  files: ReadonlyArray<File>,
) {
  const assignments: Array<number | null> = slots.map(() => null);
  const used = new Set<number>();
  slots.forEach((slot, slotIndex) => {
    const expected = normalizedGifFramePageFileName(slot.expectedFileName);
    const candidates = files
      .map((file, fileIndex) => ({
        fileIndex,
        normalized: normalizedGifFramePageFileName(file.name),
      }))
      .filter(
        ({ fileIndex, normalized }) =>
          !used.has(fileIndex) && normalized !== null && normalized === expected,
      );
    if (candidates.length === 1) {
      assignments[slotIndex] = candidates[0]!.fileIndex;
      used.add(candidates[0]!.fileIndex);
    }
  });
  const openSlots = assignments
    .map((fileIndex, slotIndex) => (fileIndex === null ? slotIndex : null))
    .filter((slotIndex): slotIndex is number => slotIndex !== null);
  const unusedFiles = files
    .map((_, fileIndex) => (used.has(fileIndex) ? null : fileIndex))
    .filter((fileIndex): fileIndex is number => fileIndex !== null);
  if (
    slots.length === 1 &&
    files.length === 1 &&
    openSlots.length === 1 &&
    unusedFiles.length === 1
  ) {
    assignments[openSlots[0]!] = unusedFiles[0]!;
  }
  return assignments;
}

export function assignGifFrameFileToSlot(
  current: ReadonlyArray<number | null>,
  slotIndex: number,
  fileIndex: number | null,
) {
  return current.map((assignedFileIndex, index) => {
    if (index === slotIndex) return fileIndex;
    if (fileIndex !== null && assignedFileIndex === fileIndex) return null;
    return assignedFileIndex;
  });
}

export function mappedGifFrameFiles(
  slots: ReadonlyArray<GifFrameReimportPageSlot>,
  files: ReadonlyArray<File>,
  assignments: ReadonlyArray<number | null>,
) {
  if (slots.length === 0 || assignments.length !== slots.length) return null;
  const mapped = slots.map((_slot, slotIndex) => {
    const fileIndex = assignments[slotIndex];
    const file = fileIndex === null ? null : files[fileIndex] ?? null;
    if (!file) return null;
    return file;
  });
  return mapped.every((file): file is File => file !== null) ? mapped : null;
}

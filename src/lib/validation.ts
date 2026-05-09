import type { IconSummary } from "@/features/collections/types";

export interface ValidationResult {
  isValid: boolean;
  message: string | null;
}

const allowedAltSpecials = new Set(["*", "^", "!", "~", "+"]);
interface GraphemeSegmenter {
  segment(value: string): Iterable<unknown>;
}

const SegmenterConstructor = (Intl as unknown as {
  Segmenter?: new (
    locale: string,
    options: { granularity: "grapheme" },
  ) => GraphemeSegmenter;
}).Segmenter;
const segmenter = SegmenterConstructor
  ? new SegmenterConstructor("ko", { granularity: "grapheme" })
  : null;

export function validateDcinsideAltText(value: string): ValidationResult {
  const normalized = normalizeAltText(value);
  const length = countGraphemes(normalized);

  if (length < 1 || length > 3) {
    return {
      isValid: false,
      message: "alt 값은 한글 기준 1~3글자여야 합니다.",
    };
  }

  if (!Array.from(normalized).every(isAllowedAltCharacter)) {
    return {
      isValid: false,
      message: "한글, 영문, 숫자, * ^ ! ~ + 만 사용할 수 있습니다.",
    };
  }

  return { isValid: true, message: null };
}

export function normalizeAltText(value: string) {
  return value.trim();
}

export function findDuplicateAltPieceIds(icons: IconSummary[]) {
  const pieceIdsByAlt = new Map<string, string[]>();

  for (const icon of icons) {
    for (const piece of icon.pieces) {
      const altText = normalizeAltText(piece.altText);
      if (!altText) {
        continue;
      }

      const pieceIds = pieceIdsByAlt.get(altText) ?? [];
      pieceIds.push(piece.id);
      pieceIdsByAlt.set(altText, pieceIds);
    }
  }

  const duplicateIds = new Set<string>();
  for (const pieceIds of pieceIdsByAlt.values()) {
    if (pieceIds.length > 1) {
      for (const pieceId of pieceIds) {
        duplicateIds.add(pieceId);
      }
    }
  }

  return duplicateIds;
}

export function isDuplicateAltDraft(
  icons: IconSummary[],
  currentPieceId: string,
  draft: string,
) {
  const normalizedDraft = normalizeAltText(draft);
  if (!normalizedDraft) {
    return false;
  }

  return icons.some((icon) =>
    icon.pieces.some(
      (piece) =>
        piece.id !== currentPieceId &&
        normalizeAltText(piece.altText) === normalizedDraft,
    ),
  );
}

function countGraphemes(value: string) {
  if (segmenter) {
    return Array.from(segmenter.segment(value)).length;
  }

  return Array.from(value).length;
}

function isAllowedAltCharacter(character: string) {
  return (
    /^[A-Za-z0-9]$/u.test(character) ||
    /^[\u1100-\u11ff\u3130-\u318f\uac00-\ud7a3]$/u.test(character) ||
    allowedAltSpecials.has(character)
  );
}

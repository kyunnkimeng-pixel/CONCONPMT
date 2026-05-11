import { normalizeAltText } from "@/lib/validation";

export interface BatchAltPiece {
  id: string;
  altText: string;
}

export interface BatchAltUpdate {
  pieceId: string;
  altText: string;
}

export function createUniqueBatchAltUpdates(
  allPieces: BatchAltPiece[],
  targetPieceIds: string[],
  baseValue: string,
): BatchAltUpdate[] {
  const targetSet = new Set(targetPieceIds);
  const used = new Set(
    allPieces
      .filter((piece) => !targetSet.has(piece.id))
      .map((piece) => normalizeAltText(piece.altText))
      .filter(Boolean),
  );
  const base = normalizeAltText(baseValue);

  if (targetPieceIds.length <= 1) {
    return targetPieceIds.map((pieceId) => ({ pieceId, altText: base }));
  }

  return targetPieceIds.map((pieceId, index) => {
    const altText = nextAvailableAlt(base, index + 1, used);
    used.add(altText);
    return { pieceId, altText };
  });
}

function nextAvailableAlt(base: string, startNumber: number, used: Set<string>) {
  let suffix = Math.max(1, startNumber);

  while (true) {
    const candidate = `${base}${suffix}`;
    if (!used.has(candidate)) {
      return candidate;
    }
    suffix += 1;
  }
}

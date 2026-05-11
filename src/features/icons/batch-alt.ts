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
  inputValue: string,
): BatchAltUpdate[] {
  const targetSet = new Set(targetPieceIds);
  const used = new Set(
    allPieces
      .filter((piece) => !targetSet.has(piece.id))
      .map((piece) => normalizeAltText(piece.altText))
      .filter(Boolean),
  );
  const values = parseBatchAltValues(inputValue);
  const targetCount = targetPieceIds.length;

  return targetPieceIds.map((pieceId, index) => {
    const desiredAlt = desiredAltForIndex(values, index, targetCount);
    const altText = nextAvailableAlt(desiredAlt, used);
    used.add(altText);
    return { pieceId, altText };
  });
}

function parseBatchAltValues(inputValue: string) {
  if (!inputValue.includes(",")) {
    const value = normalizeAltText(inputValue);
    return value ? [value] : [];
  }

  return inputValue
    .split(",")
    .map((value) => normalizeAltText(value))
    .filter(Boolean);
}

function desiredAltForIndex(values: string[], index: number, targetCount: number) {
  if (values.length === 0) {
    return `${index + 1}`;
  }
  if (values.length >= targetCount) {
    return values[index] ?? "";
  }
  if (index < values.length - 1) {
    return values[index] ?? "";
  }

  const base = values[values.length - 1] ?? "";
  return `${base}${index - (values.length - 1) + 1}`;
}

function nextAvailableAlt(desiredAlt: string, used: Set<string>) {
  if (desiredAlt && !used.has(desiredAlt)) {
    return desiredAlt;
  }

  const { base, startNumber } = splitTrailingNumber(desiredAlt);
  let suffix = startNumber;

  while (true) {
    const candidate = `${base}${suffix}`;
    if (!used.has(candidate)) {
      return candidate;
    }
    suffix += 1;
  }
}

function splitTrailingNumber(value: string) {
  const match = value.match(/^(.*?)(\d+)$/);
  if (!match) {
    return { base: value, startNumber: 1 };
  }

  return {
    base: match[1] ?? "",
    startNumber: Number(match[2]) + 1,
  };
}

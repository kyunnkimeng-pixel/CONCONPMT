import type {
  CollectionSummary,
  IconPieceSummary,
  IconSummary,
} from "@/features/collections/types";

export const DCINSIDE_USAGE_DISPLAY_SIZE = 100;

export type PreviewImageSource =
  | "generated-piece"
  | "processed-icon"
  | "thumbnail"
  | "missing";

export interface UsagePreviewPiece {
  id: string;
  iconId: string;
  displayName: string;
  pieceIndex: number;
  pieceRole: IconPieceSummary["pieceRole"];
  altText: string;
  imageUrl: string | null;
  imageSource: PreviewImageSource;
  cellWidth: number;
  cellHeight: number;
  displayWidth: number;
  displayHeight: number;
}

export interface UsagePreviewIcon {
  id: string;
  displayName: string;
  shape: IconSummary["shape"];
  pieces: UsagePreviewPiece[];
  usesProcessedOutput: boolean;
}

export interface InsertedPreviewIcon {
  id: string;
  sourceIconId: string;
  displayName: string;
  shape: IconSummary["shape"];
  pieces: UsagePreviewPiece[];
}

export function buildUsagePreviewIcons(
  collection: CollectionSummary,
  icons: IconSummary[],
): UsagePreviewIcon[] {
  return icons.map((icon) => {
    const pieces = icon.pieces.map((piece) =>
      buildUsagePreviewPiece(collection, icon, piece),
    );

    return {
      id: icon.id,
      displayName: icon.displayName,
      shape: icon.shape,
      pieces,
      usesProcessedOutput: pieces.some(
        (piece) =>
          piece.imageSource === "generated-piece" ||
          piece.imageSource === "processed-icon",
      ),
    };
  });
}

export function appendUsagePreviewIcon(
  currentItems: InsertedPreviewIcon[],
  icon: UsagePreviewIcon,
  idSuffix = `${Date.now()}`,
): InsertedPreviewIcon[] {
  return [
    ...currentItems,
    {
      id: `${icon.id}-${idSuffix}`,
      sourceIconId: icon.id,
      displayName: icon.displayName,
      shape: icon.shape,
      pieces: icon.pieces,
    },
  ];
}

export function hasAnimatedPreview(
  icons: UsagePreviewIcon[],
  insertedItems: InsertedPreviewIcon[] = [],
) {
  return [...icons, ...insertedItems].some((item) =>
    item.pieces.some((piece) => isGifPreviewUrl(piece.imageUrl)),
  );
}

export function isGifPreviewUrl(url: string | null) {
  return Boolean(url?.match(/\.gif(?:[?#]|$)/i));
}

function buildUsagePreviewPiece(
  collection: CollectionSummary,
  icon: IconSummary,
  piece: IconPieceSummary,
): UsagePreviewPiece {
  const { imageUrl, imageSource } = resolvePreviewImage(piece, icon);

  return {
    id: piece.id,
    iconId: icon.id,
    displayName: icon.displayName,
    pieceIndex: piece.pieceIndex,
    pieceRole: piece.pieceRole,
    altText: piece.altText,
    imageUrl,
    imageSource,
    cellWidth: icon.cellWidthOverride ?? collection.defaultCellWidth,
    cellHeight: icon.cellHeightOverride ?? collection.defaultCellHeight,
    displayWidth: DCINSIDE_USAGE_DISPLAY_SIZE,
    displayHeight: DCINSIDE_USAGE_DISPLAY_SIZE,
  };
}

function resolvePreviewImage(
  piece: IconPieceSummary,
  icon: IconSummary,
): Pick<UsagePreviewPiece, "imageUrl" | "imageSource"> {
  if (piece.generatedPreviewUrl) {
    return {
      imageUrl: piece.generatedPreviewUrl,
      imageSource: "generated-piece",
    };
  }

  if (icon.currentPreviewUrl) {
    return {
      imageUrl: icon.currentPreviewUrl,
      imageSource: "processed-icon",
    };
  }

  if (icon.thumbnailUrl) {
    return {
      imageUrl: icon.thumbnailUrl,
      imageSource: "thumbnail",
    };
  }

  return {
    imageUrl: null,
    imageSource: "missing",
  };
}

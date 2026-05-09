import type { CropRect, IconShape, PresetPosition } from "@/features/editor/types";

export interface Dimensions {
  width: number;
  height: number;
}

export type CropHandle = "nw" | "ne" | "sw" | "se";

const MIN_CROP_SIZE = 8;

export function viewportSizeForShape(
  shape: IconShape,
  cellWidth: number,
  cellHeight: number,
): Dimensions {
  switch (shape) {
    case "single":
      return { width: cellWidth, height: cellHeight };
    case "horizontal_double":
      return { width: cellWidth * 2, height: cellHeight };
    case "vertical_double":
      return { width: cellWidth, height: cellHeight * 2 };
  }
}

export function aspectRatioForShape(
  shape: IconShape,
  cellWidth: number,
  cellHeight: number,
) {
  const viewport = viewportSizeForShape(shape, cellWidth, cellHeight);
  return viewport.width / viewport.height;
}

export function centeredFreeCrop(
  source: Dimensions,
  shape: IconShape,
  cellWidth: number,
  cellHeight: number,
): CropRect {
  const aspectRatio = aspectRatioForShape(shape, cellWidth, cellHeight);
  const sourceAspectRatio = source.width / source.height;
  const width =
    sourceAspectRatio > aspectRatio ? source.height * aspectRatio : source.width;
  const height = width / aspectRatio;

  return {
    x: (source.width - width) / 2,
    y: (source.height - height) / 2,
    width,
    height,
  };
}

export function fixedCropForPreset(
  source: Dimensions,
  shape: IconShape,
  cellWidth: number,
  cellHeight: number,
  preset: PresetPosition,
): CropRect {
  const viewport = viewportSizeForShape(shape, cellWidth, cellHeight);
  const anchor = presetAnchor(preset === "custom" ? "center" : preset);

  return {
    x: anchoredStart(source.width, viewport.width, anchor.x),
    y: anchoredStart(source.height, viewport.height, anchor.y),
    width: viewport.width,
    height: viewport.height,
  };
}

export function constrainCropToAspect(
  crop: CropRect,
  source: Dimensions,
  aspectRatio: number,
): CropRect {
  const centerX = crop.x + crop.width / 2;
  const centerY = crop.y + crop.height / 2;
  let width = Math.min(crop.width, source.width);
  let height = width / aspectRatio;

  if (height > source.height) {
    height = source.height;
    width = height * aspectRatio;
  }

  return clampCropPosition(
    {
      x: centerX - width / 2,
      y: centerY - height / 2,
      width: Math.max(MIN_CROP_SIZE, width),
      height: Math.max(MIN_CROP_SIZE, height),
    },
    source,
  );
}

export function clampCropPosition(crop: CropRect, source: Dimensions): CropRect {
  return {
    ...crop,
    x: clampStart(crop.x, crop.width, source.width),
    y: clampStart(crop.y, crop.height, source.height),
  };
}

export function resizeCropFromCorner(
  crop: CropRect,
  handle: CropHandle,
  pointer: { x: number; y: number },
  source: Dimensions,
  aspectRatio: number,
): CropRect {
  const oppositeX = handle.includes("w") ? crop.x + crop.width : crop.x;
  const oppositeY = handle.includes("n") ? crop.y + crop.height : crop.y;
  const rawWidth =
    handle.includes("e") ? pointer.x - oppositeX : oppositeX - pointer.x;
  const rawHeight =
    handle.includes("s") ? pointer.y - oppositeY : oppositeY - pointer.y;
  const maxWidth = handle.includes("e") ? source.width - oppositeX : oppositeX;
  const maxHeight = handle.includes("s") ? source.height - oppositeY : oppositeY;
  let width = Math.max(MIN_CROP_SIZE, Math.abs(rawWidth));
  let height = width / aspectRatio;

  if (height > Math.abs(rawHeight)) {
    height = Math.max(MIN_CROP_SIZE, Math.abs(rawHeight));
    width = height * aspectRatio;
  }

  width = Math.min(width, Math.max(MIN_CROP_SIZE, maxWidth), maxHeight * aspectRatio);
  height = width / aspectRatio;

  const nextCrop = {
    x: handle.includes("w") ? oppositeX - width : oppositeX,
    y: handle.includes("n") ? oppositeY - height : oppositeY,
    width,
    height,
  };

  return clampCropPosition(nextCrop, source);
}

export function fitCanvasSize(
  source: Dimensions,
  maxWidth: number,
  maxHeight: number,
): Dimensions & { scale: number } {
  const scale = Math.min(maxWidth / source.width, maxHeight / source.height, 4);

  return {
    width: Math.max(1, Math.round(source.width * scale)),
    height: Math.max(1, Math.round(source.height * scale)),
    scale,
  };
}

export function presetAnchor(preset: PresetPosition): { x: number; y: number } {
  switch (preset) {
    case "top_left":
      return { x: 0, y: 0 };
    case "top":
      return { x: 0.5, y: 0 };
    case "top_right":
      return { x: 1, y: 0 };
    case "left":
      return { x: 0, y: 0.5 };
    case "right":
      return { x: 1, y: 0.5 };
    case "bottom_left":
      return { x: 0, y: 1 };
    case "bottom":
      return { x: 0.5, y: 1 };
    case "bottom_right":
      return { x: 1, y: 1 };
    case "center":
    case "custom":
      return { x: 0.5, y: 0.5 };
  }
}

function anchoredStart(sourceSize: number, cropSize: number, anchor: number) {
  if (cropSize > sourceSize) {
    return (sourceSize - cropSize) / 2;
  }

  return (sourceSize - cropSize) * anchor;
}

function clampStart(position: number, cropSize: number, sourceSize: number) {
  if (cropSize > sourceSize) {
    return (sourceSize - cropSize) / 2;
  }

  return Math.min(Math.max(position, 0), sourceSize - cropSize);
}

import { useEffect, useMemo, useState } from "react";
import type { KonvaEventObject } from "konva/lib/Node";
import { Circle, Image as KonvaImage, Layer, Line, Rect, Stage } from "react-konva";

import {
  aspectRatioForShape,
  clampCropPosition,
  fitCanvasSize,
  resizeCropFromCorner,
} from "@/features/editor/crop-math";
import type {
  CropHandle,
  Dimensions,
} from "@/features/editor/crop-math";
import type { CropMode, CropRect, IconShape } from "@/features/editor/types";

interface CropCanvasProps {
  sourceUrl: string;
  sourceWidth: number;
  sourceHeight: number;
  crop: CropRect;
  cropMode: CropMode;
  shape: IconShape;
  cellWidth: number;
  cellHeight: number;
  onCropChange: (crop: CropRect) => void;
}

const HANDLE_RADIUS = 6;
const MAX_CANVAS_WIDTH = 520;
const MAX_CANVAS_HEIGHT = 420;

export function CropCanvas({
  sourceUrl,
  sourceWidth,
  sourceHeight,
  crop,
  cropMode,
  shape,
  cellWidth,
  cellHeight,
  onCropChange,
}: CropCanvasProps) {
  const [imageElement, setImageElement] = useState<HTMLImageElement | null>(null);
  const source = useMemo(
    () => ({ width: sourceWidth, height: sourceHeight }),
    [sourceHeight, sourceWidth],
  );
  const bounds = useMemo(() => cropCanvasBounds(source, crop), [crop, source]);
  const canvas = useMemo(
    () => fitCanvasSize(bounds, MAX_CANVAS_WIDTH, MAX_CANVAS_HEIGHT),
    [bounds],
  );
  const scale = canvas.scale;
  const cropStage = sourceRectToStageRect(crop, bounds, scale);
  const sourceStage = sourceRectToStageRect(
    { x: 0, y: 0, width: sourceWidth, height: sourceHeight },
    bounds,
    scale,
  );
  const aspectRatio = aspectRatioForShape(shape, cellWidth, cellHeight);
  const handles = cropHandles(cropStage);

  useEffect(() => {
    const nextImage = new window.Image();
    nextImage.onload = () => setImageElement(nextImage);
    nextImage.src = sourceUrl;

    return () => {
      nextImage.onload = null;
    };
  }, [sourceUrl]);

  const updateCropPosition = (event: KonvaEventObject<DragEvent>) => {
    const nextCrop = clampCropPosition(
      {
        ...crop,
        x: event.target.x() / scale + bounds.x,
        y: event.target.y() / scale + bounds.y,
      },
      source,
    );
    event.target.position(stagePoint(nextCrop.x, nextCrop.y, bounds, scale));
    onCropChange(nextCrop);
  };

  const updateCropSize =
    (handle: CropHandle) => (event: KonvaEventObject<DragEvent>) => {
      const pointer = {
        x: event.target.x() / scale + bounds.x,
        y: event.target.y() / scale + bounds.y,
      };
      const nextCrop = resizeCropFromCorner(crop, handle, pointer, source, aspectRatio);
      onCropChange(nextCrop);
    };

  return (
    <div className="overflow-auto rounded-md border border-border bg-preview p-3">
      <Stage height={canvas.height} width={canvas.width}>
        <Layer>
          <Rect
            fill="#ffffff"
            height={canvas.height}
            stroke="#d0d5dd"
            width={canvas.width}
            x={0}
            y={0}
          />
          {imageElement ? (
            <KonvaImage
              height={sourceStage.height}
              image={imageElement}
              width={sourceStage.width}
              x={sourceStage.x}
              y={sourceStage.y}
            />
          ) : null}
          <Rect
            dash={[6, 4]}
            draggable
            fill="rgba(37, 99, 235, 0.08)"
            height={cropStage.height}
            stroke="#2563eb"
            strokeWidth={2}
            width={cropStage.width}
            x={cropStage.x}
            y={cropStage.y}
            onDragEnd={updateCropPosition}
            onDragMove={updateCropPosition}
          />
          {shape === "horizontal_double" ? (
            <Line
              dash={[5, 5]}
              points={[
                cropStage.x + cropStage.width / 2,
                cropStage.y,
                cropStage.x + cropStage.width / 2,
                cropStage.y + cropStage.height,
              ]}
              stroke="#0f766e"
              strokeWidth={2}
            />
          ) : null}
          {shape === "vertical_double" ? (
            <Line
              dash={[5, 5]}
              points={[
                cropStage.x,
                cropStage.y + cropStage.height / 2,
                cropStage.x + cropStage.width,
                cropStage.y + cropStage.height / 2,
              ]}
              stroke="#0f766e"
              strokeWidth={2}
            />
          ) : null}
          {cropMode === "free"
            ? handles.map((handle) => (
                <Circle
                  draggable
                  fill="#ffffff"
                  key={handle.id}
                  radius={HANDLE_RADIUS}
                  stroke="#2563eb"
                  strokeWidth={2}
                  x={handle.x}
                  y={handle.y}
                  onDragEnd={updateCropSize(handle.id)}
                  onDragMove={updateCropSize(handle.id)}
                />
              ))
            : null}
        </Layer>
      </Stage>
    </div>
  );
}

interface CanvasBounds extends Dimensions {
  x: number;
  y: number;
}

function cropCanvasBounds(source: Dimensions, crop: CropRect): CanvasBounds {
  const x = Math.min(0, crop.x);
  const y = Math.min(0, crop.y);
  const right = Math.max(source.width, crop.x + crop.width);
  const bottom = Math.max(source.height, crop.y + crop.height);

  return {
    x,
    y,
    width: right - x,
    height: bottom - y,
  };
}

function sourceRectToStageRect(crop: CropRect, bounds: CanvasBounds, scale: number) {
  return {
    x: (crop.x - bounds.x) * scale,
    y: (crop.y - bounds.y) * scale,
    width: crop.width * scale,
    height: crop.height * scale,
  };
}

function stagePoint(x: number, y: number, bounds: CanvasBounds, scale: number) {
  return {
    x: (x - bounds.x) * scale,
    y: (y - bounds.y) * scale,
  };
}

function cropHandles(crop: CropRect) {
  return [
    { id: "nw" as const, x: crop.x, y: crop.y },
    { id: "ne" as const, x: crop.x + crop.width, y: crop.y },
    { id: "sw" as const, x: crop.x, y: crop.y + crop.height },
    { id: "se" as const, x: crop.x + crop.width, y: crop.y + crop.height },
  ];
}

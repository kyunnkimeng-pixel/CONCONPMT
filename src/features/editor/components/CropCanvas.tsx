import { useMemo } from "react";
import type { CSSProperties } from "react";
import type { KonvaEventObject } from "konva/lib/Node";
import { Circle, Layer, Line, Rect, Stage } from "react-konva";

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
import type { TextOverlaySettings } from "@/features/editor/types";

interface CropCanvasProps {
  sourceUrl: string;
  sourceWidth: number;
  sourceHeight: number;
  crop: CropRect;
  cropMode: CropMode;
  shape: IconShape;
  cellWidth: number;
  cellHeight: number;
  textOverlay?: TextOverlaySettings | null;
  onCropChange: (crop: CropRect) => void;
}

const HANDLE_RADIUS = 6;
const MAX_CANVAS_WIDTH = 520;
const MAX_CANVAS_HEIGHT = 420;
const nonDraggableImageStyle: CSSProperties & { WebkitUserDrag: string } = {
  WebkitUserDrag: "none",
  userSelect: "none",
};

export function CropCanvas({
  sourceUrl,
  sourceWidth,
  sourceHeight,
  crop,
  cropMode,
  shape,
  cellWidth,
  cellHeight,
  textOverlay,
  onCropChange,
}: CropCanvasProps) {
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

  const updateCropPosition = (event: KonvaEventObject<DragEvent>) => {
    event.cancelBubble = true;
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
      event.cancelBubble = true;
      const pointer = {
        x: event.target.x() / scale + bounds.x,
        y: event.target.y() / scale + bounds.y,
      };
      const nextCrop = resizeCropFromCorner(crop, handle, pointer, source, aspectRatio);
      onCropChange(nextCrop);
  };

  return (
    <div className="overflow-auto rounded-md border border-border bg-preview p-3">
      <div
        className="relative bg-white"
        data-testid="crop-canvas"
        style={{ height: canvas.height, width: canvas.width }}
        onDragStart={(event) => event.preventDefault()}
      >
        <img
          alt=""
          className="pointer-events-none absolute select-none"
          data-testid="crop-source-image"
          draggable={false}
          src={sourceUrl}
          style={{
            ...nonDraggableImageStyle,
            height: sourceStage.height,
            left: sourceStage.x,
            top: sourceStage.y,
            width: sourceStage.width,
          }}
          onDragStart={(event) => event.preventDefault()}
        />
        <SourceTextOverlay
          scale={scale}
          sourceStage={sourceStage}
          textOverlay={textOverlay}
        />
        <Stage className="absolute left-0 top-0" height={canvas.height} width={canvas.width}>
          <Layer>
            <Rect
              height={canvas.height}
              listening={false}
              stroke="#d0d5dd"
              width={canvas.width}
              x={0}
              y={0}
            />
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
                    onDragStart={(event) => {
                      event.cancelBubble = true;
                    }}
                    onDragEnd={updateCropSize(handle.id)}
                    onDragMove={updateCropSize(handle.id)}
                  />
                ))
              : null}
          </Layer>
        </Stage>
      </div>
    </div>
  );
}

function SourceTextOverlay({
  scale,
  sourceStage,
  textOverlay,
}: {
  scale: number;
  sourceStage: ReturnType<typeof sourceRectToStageRect>;
  textOverlay?: TextOverlaySettings | null;
}) {
  if (!textOverlay?.enabled || !textOverlay.text.trim()) {
    return null;
  }

  return (
    <div
      className="pointer-events-none absolute z-10 select-none whitespace-pre-line text-center font-semibold leading-[1.2]"
      data-testid="crop-source-text-overlay"
      style={{
        color: textOverlay.color,
        fontSize: Math.max(1, textOverlay.fontSize * scale),
        left: sourceStage.x + sourceStage.width * textOverlay.x,
        textShadow: textOverlay.strokeWidth
          ? `${textOverlay.strokeColor} 0 0 ${Math.max(1, textOverlay.strokeWidth * scale)}px`
          : undefined,
        top: sourceStage.y + sourceStage.height * textOverlay.y,
        transform: "translate(-50%, -50%)",
      }}
    >
      {textOverlay.text}
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

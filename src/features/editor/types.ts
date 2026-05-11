import type { IconSummary } from "@/features/collections/types";

export type IconShape = "single" | "horizontal_double" | "vertical_double";
export type CropMode = "free" | "fixed";
export type PresetPosition =
  | "center"
  | "top_left"
  | "top"
  | "top_right"
  | "left"
  | "right"
  | "bottom_left"
  | "bottom"
  | "bottom_right"
  | "custom";
export type GifLoopMode = "preserve" | "infinite" | "once" | "count" | "pingpong";

export interface SourceFileSummary {
  id: string;
  originalFilename: string;
  originalImageUrl: string;
  mimeType: string;
  width: number;
  height: number;
  byteSize: number;
  isAnimated: boolean;
  frameCount: number | null;
  originalLoopMode: GifLoopMode;
  originalLoopCount: number | null;
}

export interface CropSettings {
  cropMode: CropMode;
  cropX: number;
  cropY: number;
  cropW: number;
  cropH: number;
  presetPosition: PresetPosition;
  sourceWidthAtApply: number | null;
  sourceHeightAtApply: number | null;
  viewportWidthAtApply: number;
  viewportHeightAtApply: number;
  updatedAt: string;
}

export interface IconEditorState {
  icon: IconSummary;
  source: SourceFileSummary;
  crop: CropSettings;
  textOverlay: TextOverlaySettings;
}

export interface TextOverlaySettings {
  enabled: boolean;
  text: string;
  fontPath: string | null;
  fontSize: number;
  x: number;
  y: number;
  color: string;
  strokeColor: string;
  strokeWidth: number;
}

export interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ApplyIconCropInput {
  iconId: string;
  shape: IconShape;
  cropMode: CropMode;
  cropX: number;
  cropY: number;
  cropW: number;
  cropH: number;
  presetPosition: PresetPosition;
  cellWidth: number;
  cellHeight: number;
  gifLoopMode: GifLoopMode;
  gifLoopCount: number | null;
}

export interface UpdateIconTextOverlayInput extends TextOverlaySettings {
  iconId: string;
}

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

interface EffectBase {
  id: string;
  enabled: boolean;
}

export interface PixelateEffect extends EffectBase {
  kind: "pixelate";
  blockSize: number;
}

export interface ColorAdjustEffect extends EffectBase {
  kind: "color_adjust";
  brightness: number;
  contrast: number;
  saturation: number;
  hue: number;
}

export interface ToneEffect extends EffectBase {
  kind: "tone";
  mode: "grayscale" | "sepia";
  amount: number;
}

export interface BlurEffect extends EffectBase {
  kind: "blur";
  radius: number;
}

export interface SharpenEffect extends EffectBase {
  kind: "sharpen";
  amount: number;
}

export interface OutlineEffect extends EffectBase {
  kind: "outline";
  radius: number;
  color: string;
}

export interface ShadowEffect extends EffectBase {
  kind: "shadow";
  offsetX: number;
  offsetY: number;
  blurRadius: number;
  color: string;
}

export type IconEffect =
  | PixelateEffect
  | ColorAdjustEffect
  | ToneEffect
  | BlurEffect
  | SharpenEffect
  | OutlineEffect
  | ShadowEffect;

export interface EffectRecipeV1 {
  version: 1;
  effects: IconEffect[];
}

export type MotionInterpolation = "nearest" | "bilinear";
export type MotionEdgeMode = "transparent" | "clamp" | "mirror";
export type MotionAxis = "horizontal" | "vertical";

interface MotionPresetBase {
  enabled: boolean;
  cyclesPerLoop: number;
}

export type SpatialMotion =
  | (MotionPresetBase & {
      kind: "shake";
      amplitudeX: number;
      amplitudeY: number;
    })
  | (MotionPresetBase & {
      kind: "bounce";
      heightPx: number;
    })
  | (MotionPresetBase & {
      kind: "breathe";
      scalePercent: number;
    })
  | (MotionPresetBase & {
      kind: "rock";
      angleDegrees: number;
    })
  | (MotionPresetBase & {
      kind: "spin";
      clockwise: boolean;
    });

export type DisplacementMotion =
  | (MotionPresetBase & {
      kind: "wave";
      axis: MotionAxis;
      amplitudePx: number;
      wavelengthPx: number;
    })
  | (MotionPresetBase & {
      kind: "jelly";
      amplitudeX: number;
      amplitudeY: number;
      wavelengthX: number;
      wavelengthY: number;
    })
  | (MotionPresetBase & {
      kind: "ripple";
      amplitudePx: number;
      wavelengthPx: number;
      centerXPercent: number;
      centerYPercent: number;
    })
  | (MotionPresetBase & {
      kind: "glitchBands";
      amplitudePx: number;
      bandHeightPx: number;
      stepsPerCycle: number;
    });

export type ColorOpacityMotion =
  | (MotionPresetBase & {
      kind: "hueCycle";
      rangeDegrees: number;
    })
  | (MotionPresetBase & {
      kind: "tintPulse";
      color: string;
      amountPercent: number;
    })
  | (MotionPresetBase & {
      kind: "brightnessSaturationPulse";
      brightnessPercent: number;
      saturationPercent: number;
    })
  | (MotionPresetBase & {
      kind: "flash";
      color: string;
      intensityPercent: number;
    });

export type OverlayMotion =
  | (MotionPresetBase & {
      kind: "focusLines";
      color: string;
      lineCount: number;
      lineWidthPx: number;
      innerRadiusPercent: number;
      opacityPercent: number;
    })
  | (MotionPresetBase & {
      kind: "sparkle";
      color: string;
      count: number;
      sizePx: number;
      opacityPercent: number;
    })
  | (MotionPresetBase & {
      kind: "expansionRing";
      color: string;
      lineWidthPx: number;
      maxRadiusPercent: number;
      opacityPercent: number;
    });

export interface MotionRecipeV1 {
  version: 1;
  durationMs: number;
  fps: number;
  seed: number;
  interpolation: MotionInterpolation;
  edgeMode: MotionEdgeMode;
  spatial: SpatialMotion | null;
  displacement: DisplacementMotion | null;
  colorOpacity: ColorOpacityMotion | null;
  overlay: OverlayMotion | null;
}

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
  effectRecipe: EffectRecipeV1;
  effectRevision: number;
  motionRecipe: MotionRecipeV1;
  motionRevision: number;
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
  transformQuarterTurns: 0 | 1 | 2 | 3;
  transformFlipHorizontal: boolean;
  transformFlipVertical: boolean;
  pieceIds: string[];
  gifLoopMode: GifLoopMode;
  gifLoopCount: number | null;
}

export interface UpdateIconTextOverlayInput extends TextOverlaySettings {
  iconId: string;
}

export interface PreviewIconEffectsInput {
  iconId: string;
  recipe: EffectRecipeV1;
}

export interface UpdateIconEffectsInput extends PreviewIconEffectsInput {
  expectedRevision: number;
}

export interface IconEffectPreview {
  previewPath: string;
  byteSize: number;
  maxPieceByteSize: number;
  maxBytes: number;
  frameCount: number;
  processingMs: number;
  warnings: string[];
  recipeSignature: string;
  generatedAt: string;
}

export interface PreviewIconMotionInput {
  iconId: string;
  recipe: MotionRecipeV1;
}

export interface UpdateIconMotionInput extends PreviewIconMotionInput {
  expectedRevision: number;
  expectedRenderSignature: string;
}

export const MOTION_TIMING_SOURCES = ["source_gif", "generated"] as const;
export type MotionTimingSource = (typeof MOTION_TIMING_SOURCES)[number];

export const MOTION_PREVIEW_LOOP_MODES = [
  "once",
  "infinite",
  "count",
  "pingpong",
] as const;
export type MotionPreviewLoopMode = (typeof MOTION_PREVIEW_LOOP_MODES)[number];

export interface MotionPreviewDto {
  previewPath: string;
  posterPath: string;
  byteSize: number;
  pieceByteSizes: number[];
  maxPieceByteSize: number;
  maxBytes: number;
  passesByteLimit: boolean;
  frameCount: number;
  durationMs: number;
  effectiveFps: number;
  timingSource: MotionTimingSource;
  loopMode: MotionPreviewLoopMode;
  loopCount: number | null;
  clipped: boolean;
  clippedFrameCount: number;
  processingMs: number;
  warnings: string[];
  renderSignature: string;
  generatedAt: string;
}

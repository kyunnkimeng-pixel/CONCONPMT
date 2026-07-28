import type { IconSummary } from "@/features/collections/types";
import type {
  AiNormalizationAlignment,
  AiNormalizationMode,
  AiNormalizationOptions,
  AiNormalizationResizeFilter,
} from "@/features/editor/ai-normalization-model";

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
  originalExtension: string;
  mimeType: string;
  sha256: string;
  hasAlpha: boolean | null;
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

export interface EffectiveVisualSource {
  originalSource: SourceFileSummary;
  effectiveRenderSource: SourceFileSummary;
  originalLineageId: string;
  originalLineageGeneration: number;
  activeVersionId: string | null;
  activeCandidateId: string | null;
  activationRevision: number;
  normalizationRecipeHash: string | null;
}

export type AiManualServiceSurface =
  | "gemini_web"
  | "novelai_web"
  | "other_manual";

export interface AiCandidateUsageSummary {
  createdIconCount: number;
  latestCreatedIcon: IconSummary | null;
}

export interface AiCandidate {
  id: string;
  requestId: string;
  candidateIndex: number;
  serviceSurface: AiManualServiceSurface;
  source: SourceFileSummary;
  createdAt: string;
  isMaterialized: boolean;
  isStale: boolean;
  staleReason: string | null;
  isAvailable: boolean;
  unavailableReason: string | null;
  createdIconUsage: AiCandidateUsageSummary;
}

export interface AiVersion {
  id: string;
  candidateId: string;
  parentVersionId: string | null;
  source: SourceFileSummary;
  normalizationRecipeHash: string;
  normalizationSummary: AiVersionNormalizationSummary | null;
  isActive: boolean;
  isAvailable: boolean;
  unavailableReason: string | null;
  createdAt: string;
}

export interface AiVersionNormalizationSummary {
  kind: "identity" | "contain_pad" | "cover_crop";
  mode: AiNormalizationMode | null;
  alignment: AiNormalizationAlignment | null;
  resizeFilter: AiNormalizationResizeFilter | null;
  targetCanvasWidth: number;
  targetCanvasHeight: number;
}

export interface AiReviewState {
  visualSource: EffectiveVisualSource;
  nativeRecipeSignature: string;
  candidates: AiCandidate[];
  versions: AiVersion[];
}

export interface PreviewAiCandidateNormalizationInput {
  iconId: string;
  candidateId: string;
  expectedRevision: number;
  normalization: AiNormalizationOptions;
}

export interface AiNormalizationGeometry {
  kind: "identity" | "contain_pad" | "cover_crop";
  resizedWidth: number;
  resizedHeight: number;
  cropX: number;
  cropY: number;
  pasteX: number;
  pasteY: number;
}

export interface AiNormalizationCompatibility {
  allowed: boolean;
  reasonCode: string | null;
  reason: string | null;
}

export interface AiNormalizationPreviewWarning {
  code: string;
  severity: "info" | "warning";
  message: string;
}

export interface AiNormalizationPreview {
  candidateId: string;
  rawSource: SourceFileSummary;
  normalizedPreviewPath: string;
  finalPreviewPath: string;
  targetCanvasWidth: number;
  targetCanvasHeight: number;
  finalRenderWidth: number;
  finalRenderHeight: number;
  pieceWidth: number;
  pieceHeight: number;
  normalizationRecipeHash: string;
  previewSignature: string;
  nativeRecipeSignature: string;
  geometry: AiNormalizationGeometry;
  normalizedHasAlpha: boolean;
  currentIconCompatibility: AiNormalizationCompatibility;
  newIconCompatibility: AiNormalizationCompatibility;
  warnings: AiNormalizationPreviewWarning[];
  existingVersionId: string | null;
  isCurrentRecipe: boolean;
}

export interface ActivateAiCandidateInput {
  iconId: string;
  candidateId: string;
  expectedRevision: number;
  normalization: AiNormalizationOptions;
  expectedPreviewSignature: string;
}

export interface CreateAiIconRootInput {
  iconId: string;
  candidateId: string;
  expectedRevision: number;
  normalization: AiNormalizationOptions;
  expectedPreviewSignature: string;
}

export interface CreateAiIconRootResult {
  createdIcon: IconSummary;
  sourceReviewState: AiReviewState;
  createdIconUsage: AiCandidateUsageSummary;
}

export interface AiSourceMutationResult {
  reviewState: AiReviewState;
  editorState: IconEditorState;
}

export interface RestoreAiVersionInput {
  iconId: string;
  versionId: string | null;
  expectedRevision: number;
}

export interface IconEditorState {
  icon: IconSummary;
  source: SourceFileSummary;
  crop: CropSettings;
  visualSource: EffectiveVisualSource;
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

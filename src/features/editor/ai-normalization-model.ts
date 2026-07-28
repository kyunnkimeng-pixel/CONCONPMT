export const AI_NORMALIZATION_MODES = [
  "contain_pad",
  "cover_crop",
] as const;
export type AiNormalizationMode = (typeof AI_NORMALIZATION_MODES)[number];

export const AI_NORMALIZATION_ALIGNMENTS = [
  "top_left",
  "top",
  "top_right",
  "left",
  "center",
  "right",
  "bottom_left",
  "bottom",
  "bottom_right",
] as const;
export type AiNormalizationAlignment =
  (typeof AI_NORMALIZATION_ALIGNMENTS)[number];

export const AI_NORMALIZATION_RESIZE_FILTERS = [
  "lanczos3",
  "nearest",
] as const;
export type AiNormalizationResizeFilter =
  (typeof AI_NORMALIZATION_RESIZE_FILTERS)[number];

export type AiNormalizationRgba = readonly [
  red: number,
  green: number,
  blue: number,
  alpha: number,
];

export interface AiNormalizationOptions {
  mode: AiNormalizationMode;
  alignment: AiNormalizationAlignment;
  resizeFilter: AiNormalizationResizeFilter;
  padRgba: AiNormalizationRgba;
}

interface AiNormalizationOption<Value extends string> {
  value: Value;
  label: string;
  description: string;
}

export const AI_NORMALIZATION_MODE_OPTIONS = [
  {
    value: "contain_pad",
    label: "전체 보이기 · 권장",
    description: "그림을 모두 보이게 맞추고 남는 공간에 여백을 넣습니다.",
  },
  {
    value: "cover_crop",
    label: "빈틈 없이 채우기",
    description: "캔버스를 가득 채우고 밖으로 나간 부분을 잘라냅니다.",
  },
] as const satisfies ReadonlyArray<
  AiNormalizationOption<AiNormalizationMode>
>;

export const AI_NORMALIZATION_ALIGNMENT_OPTIONS = [
  {
    value: "top_left",
    label: "왼쪽 위",
    description: "그림 또는 자르기 기준을 왼쪽 위에 맞춥니다.",
  },
  {
    value: "top",
    label: "위",
    description: "그림 또는 자르기 기준을 위쪽 가운데에 맞춥니다.",
  },
  {
    value: "top_right",
    label: "오른쪽 위",
    description: "그림 또는 자르기 기준을 오른쪽 위에 맞춥니다.",
  },
  {
    value: "left",
    label: "왼쪽",
    description: "그림 또는 자르기 기준을 왼쪽 가운데에 맞춥니다.",
  },
  {
    value: "center",
    label: "가운데",
    description: "그림 또는 자르기 기준을 가운데에 맞춥니다.",
  },
  {
    value: "right",
    label: "오른쪽",
    description: "그림 또는 자르기 기준을 오른쪽 가운데에 맞춥니다.",
  },
  {
    value: "bottom_left",
    label: "왼쪽 아래",
    description: "그림 또는 자르기 기준을 왼쪽 아래에 맞춥니다.",
  },
  {
    value: "bottom",
    label: "아래",
    description: "그림 또는 자르기 기준을 아래쪽 가운데에 맞춥니다.",
  },
  {
    value: "bottom_right",
    label: "오른쪽 아래",
    description: "그림 또는 자르기 기준을 오른쪽 아래에 맞춥니다.",
  },
] as const satisfies ReadonlyArray<
  AiNormalizationOption<AiNormalizationAlignment>
>;

export const AI_NORMALIZATION_RESIZE_FILTER_OPTIONS = [
  {
    value: "lanczos3",
    label: "부드럽게 · 일반 그림",
    description: "일러스트와 사진을 부드럽고 선명하게 크기 조절합니다.",
  },
  {
    value: "nearest",
    label: "픽셀 유지 · 픽셀 아트",
    description: "새 색을 섞지 않고 픽셀 경계를 또렷하게 유지합니다.",
  },
] as const satisfies ReadonlyArray<
  AiNormalizationOption<AiNormalizationResizeFilter>
>;

export const AI_TRANSPARENT_PAD_RGBA = [0, 0, 0, 0] as const;

export const DEFAULT_AI_NORMALIZATION_OPTIONS: Readonly<AiNormalizationOptions> =
  Object.freeze({
    mode: "contain_pad",
    alignment: "center",
    resizeFilter: "lanczos3",
    padRgba: AI_TRANSPARENT_PAD_RGBA,
  });

export function createDefaultAiNormalizationOptions(): AiNormalizationOptions {
  return {
    ...DEFAULT_AI_NORMALIZATION_OPTIONS,
    padRgba: [...AI_TRANSPARENT_PAD_RGBA],
  };
}

export function aiNormalizationModeLabel(mode: AiNormalizationMode) {
  return AI_NORMALIZATION_MODE_OPTIONS.find((option) => option.value === mode)!
    .label;
}

export function aiNormalizationAlignmentLabel(
  alignment: AiNormalizationAlignment,
) {
  return AI_NORMALIZATION_ALIGNMENT_OPTIONS.find(
    (option) => option.value === alignment,
  )!.label;
}

export function aiNormalizationResizeFilterLabel(
  filter: AiNormalizationResizeFilter,
) {
  return AI_NORMALIZATION_RESIZE_FILTER_OPTIONS.find(
    (option) => option.value === filter,
  )!.label;
}

export interface AiNormalizationPreviewRequestKeyInput {
  candidateId: string;
  rawSourceFileId: string;
  rawSourceSha256: string;
  providerNativeWidth: number;
  providerNativeHeight: number;
  targetCanvasWidth: number;
  targetCanvasHeight: number;
  originalLineageId: string;
  originalLineageGeneration: number;
  activationRevision: number;
  nativeRecipeSignature: string;
  options: AiNormalizationOptions;
}

const AI_NORMALIZATION_PREVIEW_KEY_SCHEMA =
  "pmtcon-ai-normalization-preview-v1";

/**
 * Creates a stable client-side request identity for preview de-duplication.
 * This key is not an authorization token; the backend must still validate the
 * preview signature when applying or creating an icon.
 */
export function createAiNormalizationPreviewRequestKey(
  input: AiNormalizationPreviewRequestKeyInput,
) {
  validatePreviewRequestKeyInput(input);

  return JSON.stringify({
    schema: AI_NORMALIZATION_PREVIEW_KEY_SCHEMA,
    candidateId: input.candidateId,
    rawSourceFileId: input.rawSourceFileId,
    rawSourceSha256: input.rawSourceSha256,
    providerNativeWidth: input.providerNativeWidth,
    providerNativeHeight: input.providerNativeHeight,
    targetCanvasWidth: input.targetCanvasWidth,
    targetCanvasHeight: input.targetCanvasHeight,
    originalLineageId: input.originalLineageId,
    originalLineageGeneration: input.originalLineageGeneration,
    activationRevision: input.activationRevision,
    nativeRecipeSignature: input.nativeRecipeSignature,
    mode: input.options.mode,
    alignment: input.options.alignment,
    resizeFilter: input.options.resizeFilter,
    padRgba: [...input.options.padRgba],
  });
}

function validatePreviewRequestKeyInput(
  input: AiNormalizationPreviewRequestKeyInput,
) {
  for (const [name, value] of [
    ["candidateId", input.candidateId],
    ["rawSourceFileId", input.rawSourceFileId],
    ["rawSourceSha256", input.rawSourceSha256],
    ["originalLineageId", input.originalLineageId],
    ["nativeRecipeSignature", input.nativeRecipeSignature],
  ] as const) {
    if (value.length === 0) {
      throw new TypeError(`${name} must not be empty`);
    }
  }

  for (const [name, value] of [
    ["providerNativeWidth", input.providerNativeWidth],
    ["providerNativeHeight", input.providerNativeHeight],
    ["targetCanvasWidth", input.targetCanvasWidth],
    ["targetCanvasHeight", input.targetCanvasHeight],
  ] as const) {
    if (!Number.isInteger(value) || value <= 0) {
      throw new RangeError(`${name} must be a positive integer`);
    }
  }

  for (const [name, value] of [
    ["originalLineageGeneration", input.originalLineageGeneration],
    ["activationRevision", input.activationRevision],
  ] as const) {
    if (!Number.isInteger(value) || value < 0) {
      throw new RangeError(`${name} must be a non-negative integer`);
    }
  }

  if (!AI_NORMALIZATION_MODES.includes(input.options.mode)) {
    throw new TypeError("mode is not supported");
  }
  if (!AI_NORMALIZATION_ALIGNMENTS.includes(input.options.alignment)) {
    throw new TypeError("alignment is not supported");
  }
  if (
    !AI_NORMALIZATION_RESIZE_FILTERS.includes(input.options.resizeFilter)
  ) {
    throw new TypeError("resizeFilter is not supported");
  }
  if (
    input.options.padRgba.length !== 4 ||
    input.options.padRgba.some(
      (channel) =>
        !Number.isInteger(channel) || channel < 0 || channel > 255,
    )
  ) {
    throw new RangeError("padRgba channels must be integers from 0 to 255");
  }
}

export type AiNormalizationPreviewStatusCode =
  | "select_candidate"
  | "needs_preview"
  | "previewing"
  | "ready"
  | "stale"
  | "error";

export interface AiNormalizationPreviewStatus {
  code: AiNormalizationPreviewStatusCode;
  tone: "neutral" | "busy" | "success" | "warning" | "error";
  label: string;
  message: string;
  canCommit: boolean;
}

export interface DeriveAiNormalizationPreviewStatusInput {
  hasSelectedCandidate: boolean;
  expectedRequestKey: string | null;
  previewRequestKey: string | null;
  isPreviewing: boolean;
  errorMessage?: string | null;
}

export function deriveAiNormalizationPreviewStatus(
  input: DeriveAiNormalizationPreviewStatusInput,
): AiNormalizationPreviewStatus {
  if (!input.hasSelectedCandidate) {
    return {
      code: "select_candidate",
      tone: "neutral",
      label: "AI 후보를 선택해 주세요",
      message: "가져온 후보를 선택하면 규격화 미리보기를 만들 수 있습니다.",
      canCommit: false,
    };
  }

  if (input.isPreviewing) {
    return {
      code: "previewing",
      tone: "busy",
      label: "규격화 미리보기 만드는 중",
      message: "AI 원본은 변경하지 않고 별도의 미리보기를 만들고 있습니다.",
      canCommit: false,
    };
  }

  if (input.errorMessage) {
    return {
      code: "error",
      tone: "error",
      label: "미리보기를 만들지 못했습니다",
      message: input.errorMessage,
      canCommit: false,
    };
  }

  if (
    input.expectedRequestKey !== null &&
    input.previewRequestKey === input.expectedRequestKey
  ) {
    return {
      code: "ready",
      tone: "success",
      label: "적용할 준비가 됐습니다",
      message: "현재 설정과 일치하는 규격화 결과를 확인했습니다.",
      canCommit: true,
    };
  }

  if (input.previewRequestKey !== null) {
    return {
      code: "stale",
      tone: "warning",
      label: "미리보기를 다시 만들어야 합니다",
      message: "후보, 크기 맞춤 설정 또는 편집 상태가 바뀌었습니다.",
      canCommit: false,
    };
  }

  return {
    code: "needs_preview",
    tone: "neutral",
    label: "규격화 미리보기가 필요합니다",
    message: "크기 맞춤 설정을 확인한 뒤 미리보기를 만들어 주세요.",
    canCommit: false,
  };
}

export type AiNormalizationWarningCode =
  | "animation_not_supported"
  | "contain_padding"
  | "cover_crop"
  | "opaque_background_preserved"
  | "alpha_unknown"
  | "source_upscaled";

export interface AiNormalizationWarning {
  code: AiNormalizationWarningCode;
  severity: "info" | "warning";
  message: string;
}

export interface DeriveAiNormalizationWarningsInput {
  sourceWidth: number;
  sourceHeight: number;
  sourceHasAlpha: boolean | null;
  sourceIsAnimated?: boolean;
  targetCanvasWidth: number;
  targetCanvasHeight: number;
  options: AiNormalizationOptions;
}

export function deriveAiNormalizationWarnings(
  input: DeriveAiNormalizationWarningsInput,
): AiNormalizationWarning[] {
  const warnings: AiNormalizationWarning[] = [];

  if (input.sourceIsAnimated) {
    warnings.push({
      code: "animation_not_supported",
      severity: "warning",
      message:
        "현재 AI 후보 정규화는 정적 JPG/PNG만 지원합니다. GIF는 프레임 작업 단계에서 지원할 예정입니다.",
    });
  }

  const dimensionsAreUsable =
    isPositiveDimension(input.sourceWidth) &&
    isPositiveDimension(input.sourceHeight) &&
    isPositiveDimension(input.targetCanvasWidth) &&
    isPositiveDimension(input.targetCanvasHeight);

  if (dimensionsAreUsable) {
    const hasDifferentAspectRatio =
      input.sourceWidth * input.targetCanvasHeight !==
      input.sourceHeight * input.targetCanvasWidth;

    if (hasDifferentAspectRatio && input.options.mode === "contain_pad") {
      const direction = containPaddingDirection(input);
      const paddingKind =
        input.options.padRgba[3] === 0
          ? "투명 여백"
          : input.options.padRgba[3] === 255
            ? "선택한 색의 여백"
            : "반투명 여백";
      warnings.push({
        code: "contain_padding",
        severity: "info",
        message: `비율을 유지하기 위해 ${direction}에 ${paddingKind}이 생깁니다.`,
      });
    }

    if (hasDifferentAspectRatio && input.options.mode === "cover_crop") {
      warnings.push({
        code: "cover_crop",
        severity: "warning",
        message: `캔버스를 채우기 위해 ${coverCropDirection(input)} 일부가 잘릴 수 있습니다.`,
      });
    }

    const scale =
      input.options.mode === "contain_pad"
        ? Math.min(
            input.targetCanvasWidth / input.sourceWidth,
            input.targetCanvasHeight / input.sourceHeight,
          )
        : Math.max(
            input.targetCanvasWidth / input.sourceWidth,
            input.targetCanvasHeight / input.sourceHeight,
          );
    if (scale > 1) {
      warnings.push({
        code: "source_upscaled",
        severity: "warning",
        message: "AI 원본을 확대하므로 결과가 흐리거나 픽셀이 도드라질 수 있습니다.",
      });
    }
  }

  if (input.sourceHasAlpha === false) {
    warnings.push({
      code: "opaque_background_preserved",
      severity: "warning",
      message:
        "AI 원본의 불투명 배경은 자동으로 제거되지 않습니다. 투명 여백과 배경 제거는 서로 다른 기능입니다.",
    });
  } else if (input.sourceHasAlpha === null) {
    warnings.push({
      code: "alpha_unknown",
      severity: "info",
      message: "AI 원본의 투명 영역 여부를 아직 확인하지 못했습니다.",
    });
  }

  return warnings;
}

function isPositiveDimension(value: number) {
  return Number.isFinite(value) && value > 0;
}

function containPaddingDirection(
  input: Pick<
    DeriveAiNormalizationWarningsInput,
    "sourceWidth" | "sourceHeight" | "targetCanvasWidth" | "targetCanvasHeight"
  >,
) {
  return input.sourceWidth * input.targetCanvasHeight >
    input.sourceHeight * input.targetCanvasWidth
    ? "위·아래"
    : "왼쪽·오른쪽";
}

function coverCropDirection(
  input: Pick<
    DeriveAiNormalizationWarningsInput,
    "sourceWidth" | "sourceHeight" | "targetCanvasWidth" | "targetCanvasHeight"
  >,
) {
  return input.sourceWidth * input.targetCanvasHeight >
    input.sourceHeight * input.targetCanvasWidth
    ? "왼쪽·오른쪽"
    : "위·아래";
}

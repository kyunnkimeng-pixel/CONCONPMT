import type {
  ColorOpacityMotion,
  DisplacementMotion,
  MotionRecipeV1,
  OverlayMotion,
  SpatialMotion,
} from "@/features/editor/types";

export type MotionCategory =
  | "spatial"
  | "displacement"
  | "colorOpacity"
  | "overlay";
export type SpatialMotionKind = SpatialMotion["kind"];
export type DisplacementMotionKind = DisplacementMotion["kind"];
export type ColorOpacityMotionKind = ColorOpacityMotion["kind"];
export type OverlayMotionKind = OverlayMotion["kind"];
export type MotionPresetKind =
  | SpatialMotionKind
  | DisplacementMotionKind
  | ColorOpacityMotionKind
  | OverlayMotionKind;
export type MotionPreset =
  | SpatialMotion
  | DisplacementMotion
  | ColorOpacityMotion
  | OverlayMotion;

export const MOTION_CATEGORY_OPTIONS: ReadonlyArray<{
  category: MotionCategory;
  label: string;
  description: string;
}> = [
  {
    category: "spatial",
    label: "공간 변형",
    description: "결합 화면의 위치, 크기와 회전을 움직입니다.",
  },
  {
    category: "displacement",
    label: "일렁임·변위",
    description: "픽셀 위치를 절차적으로 움직여 물결과 왜곡을 만듭니다.",
  },
  {
    category: "colorOpacity",
    label: "색상·불투명도",
    description: "시간에 따라 색조, 밝기와 번쩍임을 바꿉니다.",
  },
  {
    category: "overlay",
    label: "오버레이",
    description: "집중선, 반짝이와 확산 링을 화면 위에 합성합니다.",
  },
];

export const MOTION_PRESET_OPTIONS = {
  spatial: [
    { kind: "shake", label: "흔들기" },
    { kind: "bounce", label: "통통 튀기" },
    { kind: "breathe", label: "두근·호흡" },
    { kind: "rock", label: "까딱 회전" },
    { kind: "spin", label: "회전" },
  ],
  displacement: [
    { kind: "wave", label: "사인파 물결" },
    { kind: "jelly", label: "젤리 일렁임" },
    { kind: "ripple", label: "방사형 리플" },
    { kind: "glitchBands", label: "글리치 밴드" },
  ],
  colorOpacity: [
    { kind: "hueCycle", label: "색조 순환" },
    { kind: "tintPulse", label: "지정색 박동" },
    { kind: "brightnessSaturationPulse", label: "밝기·채도 박동" },
    { kind: "flash", label: "번쩍임" },
  ],
  overlay: [
    { kind: "focusLines", label: "집중선" },
    { kind: "sparkle", label: "반짝이" },
    { kind: "expansionRing", label: "확산 링" },
  ],
} as const;

export const MOTION_LIMITS = {
  durationMs: { min: 100, max: 10_000 },
  fps: { min: 1, max: 50 },
  frameCount: { min: 2, max: 500 },
  cyclesPerLoop: { min: 1, max: 16 },
  amplitude: { min: 0, max: 128 },
  wavelength: { min: 2, max: 1_024 },
  scalePercent: { min: 0, max: 50 },
  angleDegrees: { min: 0, max: 45 },
  percent: { min: 0, max: 100 },
  hueRange: { min: 0, max: 180 },
  bandHeight: { min: 1, max: 256 },
  stepsPerCycle: { min: 1, max: 32 },
  lineCount: { min: 4, max: 64 },
  sparkleCount: { min: 1, max: 64 },
  lineWidth: { min: 1, max: 16 },
  innerRadiusPercent: { min: 0, max: 90 },
  sparkleSize: { min: 1, max: 32 },
  ringRadius: { min: 10, max: 100 },
  seed: { min: 0, max: 0xffff_ffff },
} as const;

export function emptyMotionRecipe(): MotionRecipeV1 {
  return {
    version: 1,
    durationMs: 1_000,
    fps: 12,
    seed: 1,
    interpolation: "bilinear",
    edgeMode: "transparent",
    spatial: null,
    displacement: null,
    colorOpacity: null,
    overlay: null,
  };
}

export function createDefaultMotionPreset(
  category: "spatial",
  kind: SpatialMotionKind,
): SpatialMotion;
export function createDefaultMotionPreset(
  category: "displacement",
  kind: DisplacementMotionKind,
): DisplacementMotion;
export function createDefaultMotionPreset(
  category: "colorOpacity",
  kind: ColorOpacityMotionKind,
): ColorOpacityMotion;
export function createDefaultMotionPreset(
  category: "overlay",
  kind: OverlayMotionKind,
): OverlayMotion;
export function createDefaultMotionPreset(
  category: MotionCategory,
  kind: MotionPresetKind,
): MotionPreset {
  const base = { enabled: true, cyclesPerLoop: 1 };

  if (category === "spatial") {
    switch (kind as SpatialMotionKind) {
      case "shake":
        return { ...base, kind: "shake", amplitudeX: 5, amplitudeY: 5 };
      case "bounce":
        return { ...base, kind: "bounce", heightPx: 16 };
      case "breathe":
        return { ...base, kind: "breathe", scalePercent: 8 };
      case "rock":
        return { ...base, kind: "rock", angleDegrees: 8, cyclesPerLoop: 2 };
      case "spin":
        return { ...base, kind: "spin", clockwise: true };
    }
  }

  if (category === "displacement") {
    switch (kind as DisplacementMotionKind) {
      case "wave":
        return {
          ...base,
          kind: "wave",
          axis: "horizontal",
          amplitudePx: 5,
          wavelengthPx: 48,
        };
      case "jelly":
        return {
          ...base,
          kind: "jelly",
          amplitudeX: 4,
          amplitudeY: 6,
          wavelengthX: 56,
          wavelengthY: 48,
        };
      case "ripple":
        return {
          ...base,
          kind: "ripple",
          amplitudePx: 6,
          wavelengthPx: 48,
          centerXPercent: 50,
          centerYPercent: 50,
        };
      case "glitchBands":
        return {
          ...base,
          kind: "glitchBands",
          amplitudePx: 8,
          bandHeightPx: 10,
          stepsPerCycle: 6,
          cyclesPerLoop: 2,
        };
    }
  }

  if (category === "colorOpacity") {
    switch (kind as ColorOpacityMotionKind) {
      case "hueCycle":
        return { ...base, kind: "hueCycle", rangeDegrees: 180 };
      case "tintPulse":
        return {
          ...base,
          kind: "tintPulse",
          color: "#ff80a0",
          amountPercent: 35,
        };
      case "brightnessSaturationPulse":
        return {
          ...base,
          kind: "brightnessSaturationPulse",
          brightnessPercent: 20,
          saturationPercent: 20,
        };
      case "flash":
        return {
          ...base,
          kind: "flash",
          color: "#ffffff",
          intensityPercent: 65,
          cyclesPerLoop: 2,
        };
    }
  }

  switch (kind as OverlayMotionKind) {
    case "focusLines":
      return {
        ...base,
        kind: "focusLines",
        color: "#ffffff",
        lineCount: 16,
        lineWidthPx: 2,
        innerRadiusPercent: 35,
        opacityPercent: 70,
      };
    case "sparkle":
      return {
        ...base,
        kind: "sparkle",
        color: "#ffffff",
        count: 12,
        sizePx: 4,
        opacityPercent: 90,
      };
    case "expansionRing":
      return {
        ...base,
        kind: "expansionRing",
        color: "#ffffff",
        lineWidthPx: 3,
        maxRadiusPercent: 90,
        opacityPercent: 80,
      };
  }
}

export function setMotionPreset(
  recipe: MotionRecipeV1,
  category: MotionCategory,
  kind: MotionPresetKind | null,
): MotionRecipeV1 {
  if (kind === null) {
    return recipe[category] === null ? recipe : { ...recipe, [category]: null };
  }
  if (!presetBelongsToCategory(category, kind)) {
    return recipe;
  }
  return {
    ...recipe,
    [category]: createDefaultMotionPresetForCategory(category, kind),
  };
}

export function updateMotionPreset(
  recipe: MotionRecipeV1,
  category: MotionCategory,
  updater: (preset: MotionPreset) => MotionPreset,
): MotionRecipeV1 {
  const current = recipe[category];
  if (!current) {
    return recipe;
  }
  const updated = normalizeMotionPreset(category, updater(current));
  if (stableSerialize(current) === stableSerialize(updated)) {
    return recipe;
  }
  return { ...recipe, [category]: updated };
}

export function resetMotionPreset(
  recipe: MotionRecipeV1,
  category: MotionCategory,
): MotionRecipeV1 {
  const current = recipe[category];
  if (!current) {
    return recipe;
  }
  const reset = createDefaultMotionPresetForCategory(category, current.kind);
  return {
    ...recipe,
    [category]: { ...reset, enabled: current.enabled },
  };
}

export function disableAllMotion(recipe: MotionRecipeV1): MotionRecipeV1 {
  let changed = false;
  const next = { ...recipe };
  for (const { category } of MOTION_CATEGORY_OPTIONS) {
    const current = recipe[category];
    if (current?.enabled) {
      changed = true;
      next[category] = { ...current, enabled: false } as never;
    }
  }
  return changed ? next : recipe;
}

export function normalizeMotionRecipe(recipe: MotionRecipeV1): MotionRecipeV1 {
  return {
    version: 1,
    durationMs: clampInteger(
      recipe.durationMs,
      MOTION_LIMITS.durationMs.min,
      MOTION_LIMITS.durationMs.max,
    ),
    fps: clampInteger(
      recipe.fps,
      MOTION_LIMITS.fps.min,
      MOTION_LIMITS.fps.max,
    ),
    seed: clampInteger(
      recipe.seed,
      MOTION_LIMITS.seed.min,
      MOTION_LIMITS.seed.max,
    ),
    interpolation:
      recipe.interpolation === "nearest" ? "nearest" : "bilinear",
    edgeMode:
      recipe.edgeMode === "clamp" || recipe.edgeMode === "mirror"
        ? recipe.edgeMode
        : "transparent",
    spatial: recipe.spatial
      ? (normalizeMotionPreset("spatial", recipe.spatial) as SpatialMotion)
      : null,
    displacement: recipe.displacement
      ? (normalizeMotionPreset(
          "displacement",
          recipe.displacement,
        ) as DisplacementMotion)
      : null,
    colorOpacity: recipe.colorOpacity
      ? (normalizeMotionPreset(
          "colorOpacity",
          recipe.colorOpacity,
        ) as ColorOpacityMotion)
      : null,
    overlay: recipe.overlay
      ? (normalizeMotionPreset("overlay", recipe.overlay) as OverlayMotion)
      : null,
  };
}

export function motionRecipeSignature(recipe: MotionRecipeV1): string {
  return `motion-recipe-v1:${stableSerialize(normalizeMotionRecipe(recipe))}`;
}

export function motionRecipeStateSignature(recipe: MotionRecipeV1): string {
  return motionRecipeSignature(recipe);
}

export function motionFrameCount(recipe: MotionRecipeV1): number {
  const normalized = normalizeMotionRecipe(recipe);
  return Math.min(
    MOTION_LIMITS.frameCount.max,
    Math.max(
      MOTION_LIMITS.frameCount.min,
      Math.round((normalized.durationMs * normalized.fps) / 1_000),
    ),
  );
}

export function hasEnabledMotion(recipe: MotionRecipeV1): boolean {
  return MOTION_CATEGORY_OPTIONS.some(
    ({ category }) => recipe[category]?.enabled === true,
  );
}

export function enabledMotionCount(recipe: MotionRecipeV1): number {
  return MOTION_CATEGORY_OPTIONS.filter(
    ({ category }) => recipe[category]?.enabled === true,
  ).length;
}

export function motionPresetLabel(kind: MotionPresetKind): string {
  for (const { category } of MOTION_CATEGORY_OPTIONS) {
    const option = MOTION_PRESET_OPTIONS[category].find(
      (candidate) => candidate.kind === kind,
    );
    if (option) {
      return option.label;
    }
  }
  return "알 수 없는 모션";
}

export function motionPresetSummary(preset: MotionPreset): string {
  const cycles = `루프당 ${preset.cyclesPerLoop}회`;
  switch (preset.kind) {
    case "shake":
      return `X ${preset.amplitudeX}px · Y ${preset.amplitudeY}px · ${cycles}`;
    case "bounce":
      return `높이 ${preset.heightPx}px · ${cycles}`;
    case "breathe":
      return `크기 ${preset.scalePercent}% · ${cycles}`;
    case "rock":
      return `각도 ${preset.angleDegrees}° · ${cycles}`;
    case "spin":
      return `${preset.clockwise ? "시계 방향" : "반시계 방향"} · ${cycles}`;
    case "wave":
      return `${preset.axis === "horizontal" ? "가로" : "세로"} · 진폭 ${preset.amplitudePx}px · ${cycles}`;
    case "jelly":
      return `X ${preset.amplitudeX}px · Y ${preset.amplitudeY}px · ${cycles}`;
    case "ripple":
      return `진폭 ${preset.amplitudePx}px · 중심 ${preset.centerXPercent}/${preset.centerYPercent}%`;
    case "glitchBands":
      return `이동 ${preset.amplitudePx}px · 밴드 ${preset.bandHeightPx}px · ${cycles}`;
    case "hueCycle":
      return `범위 ${preset.rangeDegrees}° · ${cycles}`;
    case "tintPulse":
      return `${preset.color} · 혼합 ${preset.amountPercent}% · ${cycles}`;
    case "brightnessSaturationPulse":
      return `밝기 ${preset.brightnessPercent}% · 채도 ${preset.saturationPercent}%`;
    case "flash":
      return `${preset.color} · 강도 ${preset.intensityPercent}% · ${cycles}`;
    case "focusLines":
      return `${preset.lineCount}개 · ${preset.color} · 불투명도 ${preset.opacityPercent}%`;
    case "sparkle":
      return `${preset.count}개 · 크기 ${preset.sizePx}px · ${cycles}`;
    case "expansionRing":
      return `반경 ${preset.maxRadiusPercent}% · ${preset.color} · ${cycles}`;
  }
}

export function motionPreviewRequestKey(input: {
  iconId: string;
  iconUpdatedAt: string;
  effectRevision: number;
  motionRevision: number;
  draftSignature: string;
  maxBytes: number;
}) {
  return JSON.stringify([
    input.iconId,
    input.iconUpdatedAt,
    input.effectRevision,
    input.motionRevision,
    input.draftSignature,
    input.maxBytes,
  ]);
}

export function nextMotionSeed(seed: number): number {
  return (Math.imul(seed >>> 0, 1_664_525) + 1_013_904_223) >>> 0;
}

function createDefaultMotionPresetForCategory(
  category: MotionCategory,
  kind: MotionPresetKind,
): MotionPreset {
  switch (category) {
    case "spatial":
      return createDefaultMotionPreset(category, kind as SpatialMotionKind);
    case "displacement":
      return createDefaultMotionPreset(
        category,
        kind as DisplacementMotionKind,
      );
    case "colorOpacity":
      return createDefaultMotionPreset(
        category,
        kind as ColorOpacityMotionKind,
      );
    case "overlay":
      return createDefaultMotionPreset(category, kind as OverlayMotionKind);
  }
}

function presetBelongsToCategory(
  category: MotionCategory,
  kind: MotionPresetKind,
) {
  return MOTION_PRESET_OPTIONS[category].some(
    (option) => option.kind === kind,
  );
}

function normalizeMotionPreset(
  category: MotionCategory,
  preset: MotionPreset,
): MotionPreset {
  if (!presetBelongsToCategory(category, preset.kind)) {
    return createDefaultMotionPresetForCategory(
      category,
      MOTION_PRESET_OPTIONS[category][0].kind,
    );
  }

  const base = {
    enabled: preset.enabled === true,
    cyclesPerLoop: clampInteger(
      preset.cyclesPerLoop,
      MOTION_LIMITS.cyclesPerLoop.min,
      MOTION_LIMITS.cyclesPerLoop.max,
    ),
  };
  switch (preset.kind) {
    case "shake":
      return {
        ...preset,
        ...base,
        amplitudeX: amplitude(preset.amplitudeX),
        amplitudeY: amplitude(preset.amplitudeY),
      };
    case "bounce":
      return { ...preset, ...base, heightPx: amplitude(preset.heightPx) };
    case "breathe":
      return {
        ...preset,
        ...base,
        scalePercent: bounded(preset.scalePercent, MOTION_LIMITS.scalePercent),
      };
    case "rock":
      return {
        ...preset,
        ...base,
        angleDegrees: bounded(preset.angleDegrees, MOTION_LIMITS.angleDegrees),
      };
    case "spin":
      return { ...preset, ...base, clockwise: preset.clockwise !== false };
    case "wave":
      return {
        ...preset,
        ...base,
        axis: preset.axis === "vertical" ? "vertical" : "horizontal",
        amplitudePx: amplitude(preset.amplitudePx),
        wavelengthPx: wavelength(preset.wavelengthPx),
      };
    case "jelly":
      return {
        ...preset,
        ...base,
        amplitudeX: amplitude(preset.amplitudeX),
        amplitudeY: amplitude(preset.amplitudeY),
        wavelengthX: wavelength(preset.wavelengthX),
        wavelengthY: wavelength(preset.wavelengthY),
      };
    case "ripple":
      return {
        ...preset,
        ...base,
        amplitudePx: amplitude(preset.amplitudePx),
        wavelengthPx: wavelength(preset.wavelengthPx),
        centerXPercent: percent(preset.centerXPercent),
        centerYPercent: percent(preset.centerYPercent),
      };
    case "glitchBands":
      return {
        ...preset,
        ...base,
        amplitudePx: amplitude(preset.amplitudePx),
        bandHeightPx: bounded(preset.bandHeightPx, MOTION_LIMITS.bandHeight),
        stepsPerCycle: bounded(
          preset.stepsPerCycle,
          MOTION_LIMITS.stepsPerCycle,
        ),
      };
    case "hueCycle":
      return {
        ...preset,
        ...base,
        rangeDegrees: bounded(preset.rangeDegrees, MOTION_LIMITS.hueRange),
      };
    case "tintPulse":
      return {
        ...preset,
        ...base,
        color: normalizedColor(preset.color, "#ff80a0"),
        amountPercent: percent(preset.amountPercent),
      };
    case "brightnessSaturationPulse":
      return {
        ...preset,
        ...base,
        brightnessPercent: percent(preset.brightnessPercent),
        saturationPercent: percent(preset.saturationPercent),
      };
    case "flash":
      return {
        ...preset,
        ...base,
        color: normalizedColor(preset.color, "#ffffff"),
        intensityPercent: percent(preset.intensityPercent),
      };
    case "focusLines":
      return {
        ...preset,
        ...base,
        color: normalizedColor(preset.color, "#ffffff"),
        lineCount: bounded(preset.lineCount, MOTION_LIMITS.lineCount),
        lineWidthPx: bounded(preset.lineWidthPx, MOTION_LIMITS.lineWidth),
        innerRadiusPercent: bounded(
          preset.innerRadiusPercent,
          MOTION_LIMITS.innerRadiusPercent,
        ),
        opacityPercent: percent(preset.opacityPercent),
      };
    case "sparkle":
      return {
        ...preset,
        ...base,
        color: normalizedColor(preset.color, "#ffffff"),
        count: bounded(preset.count, MOTION_LIMITS.sparkleCount),
        sizePx: bounded(preset.sizePx, MOTION_LIMITS.sparkleSize),
        opacityPercent: percent(preset.opacityPercent),
      };
    case "expansionRing":
      return {
        ...preset,
        ...base,
        color: normalizedColor(preset.color, "#ffffff"),
        lineWidthPx: bounded(preset.lineWidthPx, MOTION_LIMITS.lineWidth),
        maxRadiusPercent: bounded(
          preset.maxRadiusPercent,
          MOTION_LIMITS.ringRadius,
        ),
        opacityPercent: percent(preset.opacityPercent),
      };
  }
}

function amplitude(value: number) {
  return bounded(value, MOTION_LIMITS.amplitude);
}

function wavelength(value: number) {
  return bounded(value, MOTION_LIMITS.wavelength);
}

function percent(value: number) {
  return bounded(value, MOTION_LIMITS.percent);
}

function bounded(
  value: number,
  limits: { readonly min: number; readonly max: number },
) {
  return clampInteger(value, limits.min, limits.max);
}

function clampInteger(value: number, min: number, max: number): number {
  const finite = Number.isFinite(value) ? value : min;
  return Math.min(max, Math.max(min, Math.round(finite)));
}

function normalizedColor(value: string, fallback: string): string {
  const normalized = value.trim();
  return /^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/i.test(normalized)
    ? normalized.toLowerCase()
    : fallback;
}

function stableSerialize(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableSerialize).join(",")}]`;
  }
  const record = value as Record<string, unknown>;
  return `{${Object.keys(record)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${stableSerialize(record[key])}`)
    .join(",")}}`;
}

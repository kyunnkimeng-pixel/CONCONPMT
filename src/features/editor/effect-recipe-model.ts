import type {
  EffectRecipeV1,
  IconEffect,
} from "@/features/editor/types";

export type EffectKind = IconEffect["kind"];
export const MAX_EFFECT_STEPS = 16;

export const EFFECT_KIND_OPTIONS: ReadonlyArray<{
  kind: EffectKind;
  label: string;
}> = [
  { kind: "pixelate", label: "픽셀화" },
  { kind: "color_adjust", label: "색상 조정" },
  { kind: "tone", label: "색감 프리셋" },
  { kind: "blur", label: "블러" },
  { kind: "sharpen", label: "선명화" },
  { kind: "outline", label: "윤곽선" },
  { kind: "shadow", label: "그림자" },
];

export const EFFECT_LIMITS = {
  pixelateBlockSize: { min: 1, max: 64 },
  colorAdjustment: { min: -100, max: 100 },
  hue: { min: -180, max: 180 },
  toneAmount: { min: 0, max: 100 },
  blurRadius: { min: 0, max: 32 },
  sharpenAmount: { min: 0, max: 100 },
  outlineRadius: { min: 1, max: 32 },
  shadowOffset: { min: -128, max: 128 },
  shadowBlurRadius: { min: 0, max: 32 },
} as const;

export function emptyEffectRecipe(): EffectRecipeV1 {
  return { version: 1, effects: [] };
}

export function createDefaultEffect(
  kind: EffectKind,
  id: string,
  enabled = true,
): IconEffect {
  const normalizedId = normalizedEffectId(id, kind);

  switch (kind) {
    case "pixelate":
      return {
        id: normalizedId,
        kind,
        enabled,
        blockSize: 4,
      };
    case "color_adjust":
      return {
        id: normalizedId,
        kind,
        enabled,
        brightness: 0,
        contrast: 0,
        saturation: 0,
        hue: 0,
      };
    case "tone":
      return {
        id: normalizedId,
        kind,
        enabled,
        mode: "grayscale",
        amount: 100,
      };
    case "blur":
      return {
        id: normalizedId,
        kind,
        enabled,
        radius: 2,
      };
    case "sharpen":
      return {
        id: normalizedId,
        kind,
        enabled,
        amount: 25,
      };
    case "outline":
      return {
        id: normalizedId,
        kind,
        enabled,
        radius: 2,
        color: "#ffffff",
      };
    case "shadow":
      return {
        id: normalizedId,
        kind,
        enabled,
        offsetX: 4,
        offsetY: 4,
        blurRadius: 4,
        color: "#000000",
      };
  }
}

export function addEffect(
  recipe: EffectRecipeV1,
  kind: EffectKind,
  requestedId?: string,
): EffectRecipeV1 {
  if (recipe.effects.length >= MAX_EFFECT_STEPS) {
    return recipe;
  }

  const id = requestedId?.trim()
    ? uniqueEffectId(requestedId.trim(), recipe.effects)
    : nextEffectId(kind, recipe.effects);
  return {
    version: 1,
    effects: [...recipe.effects, createDefaultEffect(kind, id)],
  };
}

export function moveEffect(
  recipe: EffectRecipeV1,
  effectId: string,
  targetIndex: number,
): EffectRecipeV1 {
  const currentIndex = recipe.effects.findIndex((effect) => effect.id === effectId);
  if (currentIndex === -1 || recipe.effects.length < 2) {
    return recipe;
  }

  const normalizedTarget = Math.min(
    recipe.effects.length - 1,
    Math.max(0, Math.round(targetIndex)),
  );
  if (normalizedTarget === currentIndex) {
    return recipe;
  }

  const effects = [...recipe.effects];
  const [effect] = effects.splice(currentIndex, 1);
  effects.splice(normalizedTarget, 0, effect);
  return { version: 1, effects };
}

export function moveEffectByOffset(
  recipe: EffectRecipeV1,
  effectId: string,
  offset: -1 | 1,
): EffectRecipeV1 {
  const currentIndex = recipe.effects.findIndex((effect) => effect.id === effectId);
  if (currentIndex === -1) {
    return recipe;
  }
  return moveEffect(recipe, effectId, currentIndex + offset);
}

export function removeEffect(
  recipe: EffectRecipeV1,
  effectId: string,
): EffectRecipeV1 {
  const effects = recipe.effects.filter((effect) => effect.id !== effectId);
  return effects.length === recipe.effects.length
    ? recipe
    : { version: 1, effects };
}

export function toggleEffect(
  recipe: EffectRecipeV1,
  effectId: string,
  enabled?: boolean,
): EffectRecipeV1 {
  return updateEffect(recipe, effectId, (effect) => ({
    ...effect,
    enabled: enabled ?? !effect.enabled,
  }));
}

export function updateEffect(
  recipe: EffectRecipeV1,
  effectId: string,
  updater: (effect: IconEffect) => IconEffect,
): EffectRecipeV1 {
  const effectIndex = recipe.effects.findIndex((effect) => effect.id === effectId);
  if (effectIndex === -1) {
    return recipe;
  }

  const current = recipe.effects[effectIndex];
  const updated = normalizeEffect({
    ...updater(current),
    id: current.id,
  } as IconEffect);
  if (stableSerialize(current) === stableSerialize(updated)) {
    return recipe;
  }

  const effects = [...recipe.effects];
  effects[effectIndex] = updated;
  return { version: 1, effects };
}

export function resetEffect(
  recipe: EffectRecipeV1,
  effectId: string,
): EffectRecipeV1 {
  return updateEffect(recipe, effectId, (effect) =>
    createDefaultEffect(effect.kind, effect.id, effect.enabled),
  );
}

export function disableAllEffects(recipe: EffectRecipeV1): EffectRecipeV1 {
  if (recipe.effects.every((effect) => !effect.enabled)) {
    return recipe;
  }
  return {
    version: 1,
    effects: recipe.effects.map((effect) => ({ ...effect, enabled: false })),
  };
}

export function normalizeEffectRecipe(recipe: EffectRecipeV1): EffectRecipeV1 {
  return {
    version: 1,
    effects: recipe.effects.map(normalizeEffect),
  };
}

export function effectRecipeSignature(recipe: EffectRecipeV1): string {
  const renderRecipe = {
    version: 1,
    effects: recipe.effects.map((effect) => {
      const renderEffect = { ...normalizeEffect(effect) } as Record<
        string,
        unknown
      >;
      delete renderEffect.id;
      return renderEffect;
    }),
  };
  return `effect-recipe-v1:${stableSerialize(renderRecipe)}`;
}

export function effectRecipeStateSignature(recipe: EffectRecipeV1): string {
  return `effect-recipe-state-v1:${stableSerialize(
    normalizeEffectRecipe(recipe),
  )}`;
}

export function effectKindLabel(kind: EffectKind): string {
  return (
    EFFECT_KIND_OPTIONS.find((option) => option.kind === kind)?.label ??
    "알 수 없는 효과"
  );
}

export function effectSummary(effect: IconEffect): string {
  switch (effect.kind) {
    case "pixelate":
      return `블록 ${effect.blockSize}px`;
    case "color_adjust":
      return `밝기 ${signed(effect.brightness)} · 대비 ${signed(effect.contrast)} · 채도 ${signed(effect.saturation)} · 색조 ${signed(effect.hue)}°`;
    case "tone":
      return `${effect.mode === "grayscale" ? "흑백" : "세피아"} ${effect.amount}%`;
    case "blur":
      return `반경 ${effect.radius}px`;
    case "sharpen":
      return `강도 ${effect.amount}%`;
    case "outline":
      return `두께 ${effect.radius}px · ${effect.color}`;
    case "shadow":
      return `X ${signed(effect.offsetX)} · Y ${signed(effect.offsetY)} · 흐림 ${effect.blurRadius}px`;
  }
}

function normalizeEffect(effect: IconEffect): IconEffect {
  const id = normalizedEffectId(effect.id, effect.kind);
  const enabled = effect.enabled === true;

  switch (effect.kind) {
    case "pixelate":
      return {
        ...effect,
        id,
        enabled,
        blockSize: clampInteger(
          effect.blockSize,
          EFFECT_LIMITS.pixelateBlockSize.min,
          EFFECT_LIMITS.pixelateBlockSize.max,
        ),
      };
    case "color_adjust":
      return {
        ...effect,
        id,
        enabled,
        brightness: clampInteger(
          effect.brightness,
          EFFECT_LIMITS.colorAdjustment.min,
          EFFECT_LIMITS.colorAdjustment.max,
        ),
        contrast: clampInteger(
          effect.contrast,
          EFFECT_LIMITS.colorAdjustment.min,
          EFFECT_LIMITS.colorAdjustment.max,
        ),
        saturation: clampInteger(
          effect.saturation,
          EFFECT_LIMITS.colorAdjustment.min,
          EFFECT_LIMITS.colorAdjustment.max,
        ),
        hue: clampInteger(
          effect.hue,
          EFFECT_LIMITS.hue.min,
          EFFECT_LIMITS.hue.max,
        ),
      };
    case "tone":
      return {
        ...effect,
        id,
        enabled,
        mode: effect.mode === "sepia" ? "sepia" : "grayscale",
        amount: clampInteger(
          effect.amount,
          EFFECT_LIMITS.toneAmount.min,
          EFFECT_LIMITS.toneAmount.max,
        ),
      };
    case "blur":
      return {
        ...effect,
        id,
        enabled,
        radius: clampInteger(
          effect.radius,
          EFFECT_LIMITS.blurRadius.min,
          EFFECT_LIMITS.blurRadius.max,
        ),
      };
    case "sharpen":
      return {
        ...effect,
        id,
        enabled,
        amount: clampInteger(
          effect.amount,
          EFFECT_LIMITS.sharpenAmount.min,
          EFFECT_LIMITS.sharpenAmount.max,
        ),
      };
    case "outline":
      return {
        ...effect,
        id,
        enabled,
        radius: clampInteger(
          effect.radius,
          EFFECT_LIMITS.outlineRadius.min,
          EFFECT_LIMITS.outlineRadius.max,
        ),
        color: normalizedColor(effect.color, "#ffffff"),
      };
    case "shadow":
      return {
        ...effect,
        id,
        enabled,
        offsetX: clampInteger(
          effect.offsetX,
          EFFECT_LIMITS.shadowOffset.min,
          EFFECT_LIMITS.shadowOffset.max,
        ),
        offsetY: clampInteger(
          effect.offsetY,
          EFFECT_LIMITS.shadowOffset.min,
          EFFECT_LIMITS.shadowOffset.max,
        ),
        blurRadius: clampInteger(
          effect.blurRadius,
          EFFECT_LIMITS.shadowBlurRadius.min,
          EFFECT_LIMITS.shadowBlurRadius.max,
        ),
        color: normalizedColor(effect.color, "#000000"),
      };
  }
}

function nextEffectId(kind: EffectKind, effects: readonly IconEffect[]): string {
  return uniqueEffectId(`effect-${kind}`, effects);
}

function uniqueEffectId(candidate: string, effects: readonly IconEffect[]): string {
  const occupied = new Set(effects.map((effect) => effect.id));
  if (!occupied.has(candidate)) {
    return candidate;
  }

  let suffix = 2;
  while (occupied.has(`${candidate}-${suffix}`)) {
    suffix += 1;
  }
  return `${candidate}-${suffix}`;
}

function normalizedEffectId(id: string, kind: EffectKind): string {
  const normalized = id.trim();
  return normalized || `effect-${kind}`;
}

function normalizedColor(value: string, fallback: string): string {
  const normalized = value.trim();
  return /^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/i.test(normalized)
    ? normalized.toLowerCase()
    : fallback;
}

function clampInteger(value: number, min: number, max: number): number {
  const finite = Number.isFinite(value) ? value : min;
  return Math.min(max, Math.max(min, Math.round(finite)));
}

function signed(value: number): string {
  return value > 0 ? `+${value}` : String(value);
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

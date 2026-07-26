import { describe, expect, it } from "vitest";

import {
  addEffect,
  createDefaultEffect,
  disableAllEffects,
  effectRecipeSignature,
  effectRecipeStateSignature,
  effectSummary,
  emptyEffectRecipe,
  moveEffect,
  moveEffectByOffset,
  MAX_EFFECT_STEPS,
  normalizeEffectRecipe,
  removeEffect,
  resetEffect,
  toggleEffect,
  updateEffect,
} from "@/features/editor/effect-recipe-model";
import type {
  EffectRecipeV1,
  IconEffect,
} from "@/features/editor/types";

describe("effect-recipe-model", () => {
  it("adds default effects with stable unique draft IDs", () => {
    const first = addEffect(emptyEffectRecipe(), "pixelate");
    const second = addEffect(first, "pixelate");
    const third = addEffect(second, "tone", "custom-effect");

    expect(third.effects).toEqual([
      {
        id: "effect-pixelate",
        kind: "pixelate",
        enabled: true,
        blockSize: 4,
      },
      {
        id: "effect-pixelate-2",
        kind: "pixelate",
        enabled: true,
        blockSize: 4,
      },
      {
        id: "custom-effect",
        kind: "tone",
        enabled: true,
        mode: "grayscale",
        amount: 100,
      },
    ]);
    expect(first.effects).toHaveLength(1);
  });

  it("caps recipes at the backend limit of 16 stages", () => {
    const atLimit = Array.from({ length: MAX_EFFECT_STEPS }).reduce<
      EffectRecipeV1
    >((recipe) => addEffect(recipe, "blur"), emptyEffectRecipe());
    const rejected = addEffect(atLimit, "shadow");

    expect(atLimit.effects).toHaveLength(16);
    expect(rejected).toBe(atLimit);
  });

  it("moves effects by target or offset without mutating the source", () => {
    const recipe = recipeWithKinds(["pixelate", "blur", "outline"]);
    const moved = moveEffect(recipe, "effect-pixelate", 2);

    expect(moved.effects.map((effect) => effect.kind)).toEqual([
      "blur",
      "outline",
      "pixelate",
    ]);
    expect(recipe.effects.map((effect) => effect.kind)).toEqual([
      "pixelate",
      "blur",
      "outline",
    ]);
    expect(moveEffectByOffset(moved, "effect-pixelate", -1).effects[1].kind).toBe(
      "pixelate",
    );
    expect(moveEffect(recipe, "missing", 1)).toBe(recipe);
    expect(moveEffect(recipe, "effect-pixelate", -50)).toBe(recipe);
  });

  it("removes, toggles, and disables effects while preserving parameters", () => {
    const recipe = recipeWithKinds(["pixelate", "shadow"]);
    const toggled = toggleEffect(recipe, recipe.effects[0].id, false);
    const allDisabled = disableAllEffects(toggled);
    const removed = removeEffect(allDisabled, allDisabled.effects[0].id);

    expect(toggled.effects[0].enabled).toBe(false);
    expect(allDisabled.effects.every((effect) => !effect.enabled)).toBe(true);
    expect(allDisabled.effects[1]).toMatchObject({
      kind: "shadow",
      offsetX: 4,
      offsetY: 4,
      blurRadius: 4,
    });
    expect(removed.effects).toHaveLength(1);
    expect(removeEffect(recipe, "missing")).toBe(recipe);
    expect(disableAllEffects(allDisabled)).toBe(allDisabled);
  });

  it("updates and clamps every parameter family", () => {
    const recipe: EffectRecipeV1 = {
      version: 1,
      effects: [
        {
          id: "pixel",
          kind: "pixelate",
          enabled: true,
          blockSize: 999,
        },
        {
          id: "color",
          kind: "color_adjust",
          enabled: true,
          brightness: -999,
          contrast: 999,
          saturation: 42.6,
          hue: 999,
        },
        {
          id: "tone",
          kind: "tone",
          enabled: true,
          mode: "sepia",
          amount: -1,
        },
        {
          id: "blur",
          kind: "blur",
          enabled: true,
          radius: 500,
        },
        {
          id: "sharp",
          kind: "sharpen",
          enabled: true,
          amount: 500,
        },
        {
          id: "outline",
          kind: "outline",
          enabled: true,
          radius: 0,
          color: "#AABBCCDD",
        },
        {
          id: "shadow",
          kind: "shadow",
          enabled: true,
          offsetX: -999,
          offsetY: 999,
          blurRadius: 999,
          color: "",
        },
      ],
    };

    expect(normalizeEffectRecipe(recipe).effects).toEqual([
      expect.objectContaining({ blockSize: 64 }),
      expect.objectContaining({
        brightness: -100,
        contrast: 100,
        saturation: 43,
        hue: 180,
      }),
      expect.objectContaining({ mode: "sepia", amount: 0 }),
      expect.objectContaining({ radius: 32 }),
      expect.objectContaining({ amount: 100 }),
      expect.objectContaining({ radius: 1, color: "#aabbccdd" }),
      expect.objectContaining({
        offsetX: -128,
        offsetY: 128,
        blurRadius: 32,
        color: "#000000",
      }),
    ]);

    const updated = updateEffect(recipe, "pixel", (effect) =>
      effect.kind === "pixelate"
        ? { ...effect, blockSize: Number.POSITIVE_INFINITY }
        : effect,
    );
    expect(updated.effects[0]).toMatchObject({ blockSize: 1 });
    expect(updateEffect(recipe, "missing", (effect) => effect)).toBe(recipe);
  });

  it("resets only one card while retaining identity and enabled state", () => {
    const recipe: EffectRecipeV1 = {
      version: 1,
      effects: [
        {
          id: "custom-shadow",
          kind: "shadow",
          enabled: false,
          offsetX: 90,
          offsetY: -20,
          blurRadius: 30,
          color: "#ff00ff",
        },
      ],
    };

    expect(resetEffect(recipe, "custom-shadow").effects[0]).toEqual({
      id: "custom-shadow",
      kind: "shadow",
      enabled: false,
      offsetX: 4,
      offsetY: 4,
      blurRadius: 4,
      color: "#000000",
    });
  });

  it("signs ordered render fields but not draft IDs", () => {
    const first = recipeWithKinds(["pixelate", "blur"]);
    const renamed: EffectRecipeV1 = {
      version: 1,
      effects: first.effects.map((effect, index) => ({
        ...effect,
        id: `replacement-${index}`,
      })),
    };
    const reordered = moveEffect(first, first.effects[0].id, 1);
    const disabled = toggleEffect(first, first.effects[0].id, false);

    expect(effectRecipeSignature(renamed)).toBe(effectRecipeSignature(first));
    expect(effectRecipeStateSignature(renamed)).not.toBe(
      effectRecipeStateSignature(first),
    );
    expect(effectRecipeSignature(reordered)).not.toBe(
      effectRecipeSignature(first),
    );
    expect(effectRecipeSignature(disabled)).not.toBe(
      effectRecipeSignature(first),
    );
  });

  it("returns concise Korean summaries", () => {
    expect(effectSummary(createDefaultEffect("pixelate", "p"))).toBe(
      "블록 4px",
    );
    expect(
      effectSummary({
        id: "c",
        kind: "color_adjust",
        enabled: true,
        brightness: 10,
        contrast: -5,
        saturation: 0,
        hue: 30,
      }),
    ).toContain("밝기 +10");
  });
});

function recipeWithKinds(kinds: IconEffect["kind"][]): EffectRecipeV1 {
  return kinds.reduce(
    (recipe, kind) => addEffect(recipe, kind),
    emptyEffectRecipe(),
  );
}

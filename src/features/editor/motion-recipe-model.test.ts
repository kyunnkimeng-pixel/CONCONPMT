import { describe, expect, it } from "vitest";

import {
  createDefaultMotionPreset,
  disableAllMotion,
  emptyMotionRecipe,
  hasEnabledMotion,
  MOTION_CATEGORY_OPTIONS,
  MOTION_PRESET_OPTIONS,
  motionFrameCount,
  motionPresetSummary,
  motionPreviewRequestKey,
  motionRecipeSignature,
  nextMotionSeed,
  normalizeMotionRecipe,
  resetMotionPreset,
  setMotionPreset,
  updateMotionPreset,
} from "@/features/editor/motion-recipe-model";
import type { MotionRecipeV1 } from "@/features/editor/types";

describe("motion-recipe-model", () => {
  it("creates the four fixed empty category slots in canonical order", () => {
    const recipe = emptyMotionRecipe();

    expect(MOTION_CATEGORY_OPTIONS.map((option) => option.category)).toEqual([
      "spatial",
      "displacement",
      "colorOpacity",
      "overlay",
    ]);
    expect(recipe).toMatchObject({
      version: 1,
      durationMs: 1_000,
      fps: 12,
      interpolation: "bilinear",
      edgeMode: "transparent",
      spatial: null,
      displacement: null,
      colorOpacity: null,
      overlay: null,
    });
    expect(hasEnabledMotion(recipe)).toBe(false);
  });

  it("offers every implemented preset and creates a default for each", () => {
    expect(MOTION_PRESET_OPTIONS.spatial.map((option) => option.kind)).toEqual([
      "shake",
      "bounce",
      "breathe",
      "rock",
      "spin",
    ]);
    expect(
      MOTION_PRESET_OPTIONS.displacement.map((option) => option.kind),
    ).toEqual(["wave", "jelly", "ripple", "glitchBands"]);
    expect(
      MOTION_PRESET_OPTIONS.colorOpacity.map((option) => option.kind),
    ).toEqual([
      "hueCycle",
      "tintPulse",
      "brightnessSaturationPulse",
      "flash",
    ]);
    expect(MOTION_PRESET_OPTIONS.overlay.map((option) => option.kind)).toEqual([
      "focusLines",
      "sparkle",
      "expansionRing",
    ]);

    for (const option of MOTION_PRESET_OPTIONS.spatial) {
      expect(createDefaultMotionPreset("spatial", option.kind)).toMatchObject({
        kind: option.kind,
        enabled: true,
      });
    }
    for (const option of MOTION_PRESET_OPTIONS.displacement) {
      expect(
        createDefaultMotionPreset("displacement", option.kind),
      ).toMatchObject({ kind: option.kind, enabled: true });
    }
    for (const option of MOTION_PRESET_OPTIONS.colorOpacity) {
      expect(
        createDefaultMotionPreset("colorOpacity", option.kind),
      ).toMatchObject({ kind: option.kind, enabled: true });
    }
    for (const option of MOTION_PRESET_OPTIONS.overlay) {
      expect(createDefaultMotionPreset("overlay", option.kind)).toMatchObject({
        kind: option.kind,
        enabled: true,
      });
    }
  });

  it("replaces only one fixed category instead of building an arbitrary stack", () => {
    const withShake = setMotionPreset(
      emptyMotionRecipe(),
      "spatial",
      "shake",
    );
    const withBreathe = setMotionPreset(withShake, "spatial", "breathe");
    const rejected = setMotionPreset(
      withBreathe,
      "spatial",
      "focusLines",
    );

    expect(withBreathe.spatial?.kind).toBe("breathe");
    expect(withBreathe.displacement).toBeNull();
    expect(rejected).toBe(withBreathe);
    expect(setMotionPreset(withBreathe, "spatial", null).spatial).toBeNull();
  });

  it("clamps timing, seed and every parameter family to the backend limits", () => {
    const recipe: MotionRecipeV1 = {
      version: 1,
      durationMs: 99_999,
      fps: -3,
      seed: Number.POSITIVE_INFINITY,
      interpolation: "nearest",
      edgeMode: "mirror",
      spatial: {
        kind: "shake",
        enabled: true,
        cyclesPerLoop: 999,
        amplitudeX: -20,
        amplitudeY: 999,
      },
      displacement: {
        kind: "ripple",
        enabled: true,
        cyclesPerLoop: 0,
        amplitudePx: 999,
        wavelengthPx: 1,
        centerXPercent: -4,
        centerYPercent: 999,
      },
      colorOpacity: {
        kind: "tintPulse",
        enabled: true,
        cyclesPerLoop: 1,
        color: "#AABBCCDD",
        amountPercent: 999,
      },
      overlay: {
        kind: "focusLines",
        enabled: true,
        cyclesPerLoop: 1,
        color: "invalid",
        lineCount: 2,
        lineWidthPx: 99,
        innerRadiusPercent: 999,
        opacityPercent: 999,
      },
    };

    expect(normalizeMotionRecipe(recipe)).toEqual({
      version: 1,
      durationMs: 10_000,
      fps: 1,
      seed: 0,
      interpolation: "nearest",
      edgeMode: "mirror",
      spatial: {
        kind: "shake",
        enabled: true,
        cyclesPerLoop: 16,
        amplitudeX: 0,
        amplitudeY: 128,
      },
      displacement: {
        kind: "ripple",
        enabled: true,
        cyclesPerLoop: 1,
        amplitudePx: 128,
        wavelengthPx: 2,
        centerXPercent: 0,
        centerYPercent: 100,
      },
      colorOpacity: {
        kind: "tintPulse",
        enabled: true,
        cyclesPerLoop: 1,
        color: "#aabbccdd",
        amountPercent: 100,
      },
      overlay: {
        kind: "focusLines",
        enabled: true,
        cyclesPerLoop: 1,
        color: "#ffffff",
        lineCount: 4,
        lineWidthPx: 16,
        innerRadiusPercent: 90,
        opacityPercent: 100,
      },
    });
  });

  it("updates, resets and disables categories without mutating the source", () => {
    const recipe = setMotionPreset(
      setMotionPreset(emptyMotionRecipe(), "overlay", "sparkle"),
      "spatial",
      "rock",
    );
    const updated = updateMotionPreset(recipe, "spatial", (preset) =>
      preset.kind === "rock" ? { ...preset, angleDegrees: 30 } : preset,
    );
    const reset = resetMotionPreset(updated, "spatial");
    const disabled = disableAllMotion(updated);

    expect(updated.spatial).toMatchObject({ angleDegrees: 30 });
    expect(recipe.spatial).toMatchObject({ angleDegrees: 8 });
    expect(reset.spatial).toMatchObject({
      kind: "rock",
      angleDegrees: 8,
      enabled: true,
    });
    expect(hasEnabledMotion(disabled)).toBe(false);
    expect(disabled.overlay).toMatchObject({ kind: "sparkle", count: 12 });
    expect(disableAllMotion(disabled)).toBe(disabled);
  });

  it("signs every render input and derives a bounded GIF frame count", () => {
    const base = setMotionPreset(emptyMotionRecipe(), "spatial", "shake");
    const signature = motionRecipeSignature(base);

    expect(motionFrameCount(base)).toBe(12);
    expect(
      motionFrameCount({ ...base, durationMs: 100, fps: 1 }),
    ).toBe(2);
    expect(
      motionFrameCount({ ...base, durationMs: 10_000, fps: 50 }),
    ).toBe(500);
    expect(
      motionRecipeSignature({ ...base, seed: base.seed + 1 }),
    ).not.toBe(signature);
    expect(
      motionRecipeSignature({ ...base, edgeMode: "clamp" }),
    ).not.toBe(signature);
    expect(
      motionRecipeSignature({
        ...base,
        spatial: base.spatial
          ? { ...base.spatial, enabled: false }
          : null,
      }),
    ).not.toBe(signature);
  });

  it("invalidates measurements for draft and saved-base changes", () => {
    const base = {
      iconId: "icon_1",
      iconUpdatedAt: "2026-07-26T01:00:00Z",
      effectRevision: 2,
      motionRevision: 3,
      draftSignature: "motion-a",
      maxBytes: 2_097_152,
    };

    for (const changed of [
      { ...base, iconUpdatedAt: "2026-07-26T01:00:01Z" },
      { ...base, effectRevision: 4 },
      { ...base, motionRevision: 4 },
      { ...base, draftSignature: "motion-b" },
      { ...base, maxBytes: 1_000_000 },
    ]) {
      expect(motionPreviewRequestKey(changed)).not.toBe(
        motionPreviewRequestKey(base),
      );
    }
  });

  it("changes deterministic patterns and returns concise Korean summaries", () => {
    expect(nextMotionSeed(1)).toBe(nextMotionSeed(1));
    expect(nextMotionSeed(1)).not.toBe(1);
    expect(
      motionPresetSummary(createDefaultMotionPreset("spatial", "shake")),
    ).toContain("루프당");
    expect(
      motionPresetSummary(createDefaultMotionPreset("overlay", "sparkle")),
    ).toContain("12개");
  });
});

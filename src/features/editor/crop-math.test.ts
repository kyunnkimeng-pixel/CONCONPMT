import { describe, expect, it } from "vitest";

import {
  centeredFreeCrop,
  fixedCropForPreset,
  resizeCropFromCorner,
  viewportSizeForShape,
} from "@/features/editor/crop-math";

describe("editor crop math", () => {
  it("maps icon shapes to configurable viewport sizes", () => {
    expect(viewportSizeForShape("single", 120, 80)).toEqual({
      width: 120,
      height: 80,
    });
    expect(viewportSizeForShape("horizontal_double", 120, 80)).toEqual({
      width: 240,
      height: 80,
    });
    expect(viewportSizeForShape("vertical_double", 120, 80)).toEqual({
      width: 120,
      height: 160,
    });
  });

  it("creates a centered free crop with the selected shape ratio", () => {
    const crop = centeredFreeCrop(
      { width: 300, height: 200 },
      "horizontal_double",
      100,
      100,
    );

    expect(crop.width / crop.height).toBeCloseTo(2);
    expect(crop.x).toBeCloseTo(0);
    expect(crop.y).toBeCloseTo(25);
  });

  it("uses fixed crop preset positions without hardcoded 200px cells", () => {
    const crop = fixedCropForPreset(
      { width: 500, height: 300 },
      "vertical_double",
      120,
      80,
      "bottom_right",
    );

    expect(crop).toEqual({
      x: 380,
      y: 140,
      width: 120,
      height: 160,
    });
  });

  it("resizes from corners while preserving the target aspect ratio", () => {
    const crop = resizeCropFromCorner(
      { x: 50, y: 50, width: 100, height: 100 },
      "se",
      { x: 220, y: 180 },
      { width: 300, height: 300 },
      2,
    );

    expect(crop.width / crop.height).toBeCloseTo(2);
    expect(crop.x).toBe(50);
    expect(crop.y).toBe(50);
  });
});

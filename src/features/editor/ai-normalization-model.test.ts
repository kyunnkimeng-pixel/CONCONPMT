import { describe, expect, it } from "vitest";

import {
  AI_NORMALIZATION_ALIGNMENTS,
  AI_NORMALIZATION_ALIGNMENT_OPTIONS,
  AI_NORMALIZATION_MODES,
  AI_NORMALIZATION_MODE_OPTIONS,
  AI_NORMALIZATION_RESIZE_FILTERS,
  AI_NORMALIZATION_RESIZE_FILTER_OPTIONS,
  AI_TRANSPARENT_PAD_RGBA,
  DEFAULT_AI_NORMALIZATION_OPTIONS,
  aiNormalizationAlignmentLabel,
  aiNormalizationModeLabel,
  aiNormalizationResizeFilterLabel,
  createAiNormalizationPreviewRequestKey,
  createDefaultAiNormalizationOptions,
  deriveAiNormalizationPreviewStatus,
  deriveAiNormalizationWarnings,
} from "@/features/editor/ai-normalization-model";
import type {
  AiNormalizationOptions,
  AiNormalizationPreviewRequestKeyInput,
} from "@/features/editor/ai-normalization-model";

const defaultOptions: AiNormalizationOptions = {
  mode: "contain_pad",
  alignment: "center",
  resizeFilter: "lanczos3",
  padRgba: [0, 0, 0, 0],
};

const previewKeyInput: AiNormalizationPreviewRequestKeyInput = {
  candidateId: "candidate_1",
  rawSourceFileId: "source_raw_1",
  rawSourceSha256: "a".repeat(64),
  providerNativeWidth: 1024,
  providerNativeHeight: 768,
  targetCanvasWidth: 640,
  targetCanvasHeight: 640,
  originalLineageId: "lineage_1",
  originalLineageGeneration: 2,
  activationRevision: 4,
  nativeRecipeSignature: "native-recipe-1",
  options: defaultOptions,
};

describe("AI normalization model", () => {
  it("exposes the two normalization modes with Korean labels", () => {
    expect(AI_NORMALIZATION_MODES).toEqual(["contain_pad", "cover_crop"]);
    expect(AI_NORMALIZATION_MODE_OPTIONS.map((option) => option.value)).toEqual(
      AI_NORMALIZATION_MODES,
    );
    expect(aiNormalizationModeLabel("contain_pad")).toBe(
      "전체 보이기 · 권장",
    );
    expect(aiNormalizationModeLabel("cover_crop")).toBe("빈틈 없이 채우기");
    expect(
      AI_NORMALIZATION_MODE_OPTIONS.every(
        (option) => option.label.length > 0 && option.description.length > 0,
      ),
    ).toBe(true);
  });

  it("exposes all 3x3 alignments in visual reading order", () => {
    expect(AI_NORMALIZATION_ALIGNMENTS).toEqual([
      "top_left",
      "top",
      "top_right",
      "left",
      "center",
      "right",
      "bottom_left",
      "bottom",
      "bottom_right",
    ]);
    expect(
      AI_NORMALIZATION_ALIGNMENT_OPTIONS.map((option) => option.value),
    ).toEqual(AI_NORMALIZATION_ALIGNMENTS);
    expect(aiNormalizationAlignmentLabel("top_left")).toBe("왼쪽 위");
    expect(aiNormalizationAlignmentLabel("center")).toBe("가운데");
    expect(aiNormalizationAlignmentLabel("bottom_right")).toBe("오른쪽 아래");
  });

  it("offers Lanczos3 for general art and Nearest for pixel art", () => {
    expect(AI_NORMALIZATION_RESIZE_FILTERS).toEqual([
      "lanczos3",
      "nearest",
    ]);
    expect(
      AI_NORMALIZATION_RESIZE_FILTER_OPTIONS.map((option) => option.value),
    ).toEqual(AI_NORMALIZATION_RESIZE_FILTERS);
    expect(aiNormalizationResizeFilterLabel("lanczos3")).toContain("일반 그림");
    expect(aiNormalizationResizeFilterLabel("nearest")).toContain("픽셀");
  });

  it("defaults to contain, center, Lanczos3, and transparent padding", () => {
    expect(DEFAULT_AI_NORMALIZATION_OPTIONS).toEqual(defaultOptions);
    expect(AI_TRANSPARENT_PAD_RGBA).toEqual([0, 0, 0, 0]);

    const first = createDefaultAiNormalizationOptions();
    const second = createDefaultAiNormalizationOptions();
    expect(first).toEqual(defaultOptions);
    expect(first.padRgba).not.toBe(second.padRgba);
  });

  it("creates a deterministic preview request key from all stale inputs", () => {
    const first = createAiNormalizationPreviewRequestKey(previewKeyInput);
    const second = createAiNormalizationPreviewRequestKey({
      ...previewKeyInput,
      options: {
        ...previewKeyInput.options,
        padRgba: [...previewKeyInput.options.padRgba],
      },
    });

    expect(second).toBe(first);
    expect(JSON.parse(first)).toMatchObject({
      schema: "pmtcon-ai-normalization-preview-v1",
      candidateId: "candidate_1",
      rawSourceFileId: "source_raw_1",
      targetCanvasWidth: 640,
      originalLineageGeneration: 2,
      activationRevision: 4,
      nativeRecipeSignature: "native-recipe-1",
      mode: "contain_pad",
      alignment: "center",
      resizeFilter: "lanczos3",
      padRgba: [0, 0, 0, 0],
    });
  });

  it.each([
    ["raw source", { rawSourceSha256: "b".repeat(64) }],
    ["target canvas", { targetCanvasWidth: 200 }],
    ["lineage", { originalLineageGeneration: 3 }],
    ["activation", { activationRevision: 5 }],
    ["native recipe", { nativeRecipeSignature: "native-recipe-2" }],
    [
      "normalization mode",
      { options: { ...defaultOptions, mode: "cover_crop" as const } },
    ],
    [
      "alignment",
      { options: { ...defaultOptions, alignment: "top" as const } },
    ],
    [
      "resize filter",
      { options: { ...defaultOptions, resizeFilter: "nearest" as const } },
    ],
    [
      "padding",
      { options: { ...defaultOptions, padRgba: [255, 255, 255, 255] as const } },
    ],
  ])("changes the preview request key when %s changes", (_label, changes) => {
    expect(
      createAiNormalizationPreviewRequestKey({
        ...previewKeyInput,
        ...changes,
      }),
    ).not.toBe(createAiNormalizationPreviewRequestKey(previewKeyInput));
  });

  it("rejects invalid request dimensions and padding channels", () => {
    expect(() =>
      createAiNormalizationPreviewRequestKey({
        ...previewKeyInput,
        targetCanvasWidth: 0,
      }),
    ).toThrow("targetCanvasWidth must be a positive integer");
    expect(() =>
      createAiNormalizationPreviewRequestKey({
        ...previewKeyInput,
        options: {
          ...defaultOptions,
          padRgba: [0, 0, 0, 256],
        },
      }),
    ).toThrow("padRgba channels must be integers from 0 to 255");
  });

  it("derives selectable, busy, ready, stale, and error preview states", () => {
    expect(
      deriveAiNormalizationPreviewStatus({
        hasSelectedCandidate: false,
        expectedRequestKey: null,
        previewRequestKey: null,
        isPreviewing: false,
      }),
    ).toMatchObject({ code: "select_candidate", canCommit: false });

    expect(
      deriveAiNormalizationPreviewStatus({
        hasSelectedCandidate: true,
        expectedRequestKey: "request-1",
        previewRequestKey: null,
        isPreviewing: true,
        errorMessage: "이전 오류",
      }),
    ).toMatchObject({ code: "previewing", tone: "busy", canCommit: false });

    expect(
      deriveAiNormalizationPreviewStatus({
        hasSelectedCandidate: true,
        expectedRequestKey: "request-1",
        previewRequestKey: "request-1",
        isPreviewing: false,
      }),
    ).toMatchObject({ code: "ready", tone: "success", canCommit: true });

    expect(
      deriveAiNormalizationPreviewStatus({
        hasSelectedCandidate: true,
        expectedRequestKey: "request-2",
        previewRequestKey: "request-1",
        isPreviewing: false,
      }),
    ).toMatchObject({ code: "stale", tone: "warning", canCommit: false });

    expect(
      deriveAiNormalizationPreviewStatus({
        hasSelectedCandidate: true,
        expectedRequestKey: "request-1",
        previewRequestKey: null,
        isPreviewing: false,
        errorMessage: "AI 후보 이미지를 읽을 수 없습니다.",
      }),
    ).toMatchObject({
      code: "error",
      tone: "error",
      message: "AI 후보 이미지를 읽을 수 없습니다.",
      canCommit: false,
    });
  });

  it("warns about transparent padding and an opaque raw background", () => {
    expect(
      deriveAiNormalizationWarnings({
        sourceWidth: 1024,
        sourceHeight: 768,
        sourceHasAlpha: false,
        targetCanvasWidth: 640,
        targetCanvasHeight: 640,
        options: defaultOptions,
      }),
    ).toEqual([
      {
        code: "contain_padding",
        severity: "info",
        message:
          "비율을 유지하기 위해 위·아래에 투명 여백이 생깁니다.",
      },
      {
        code: "opaque_background_preserved",
        severity: "warning",
        message:
          "AI 원본의 불투명 배경은 자동으로 제거되지 않습니다. 투명 여백과 배경 제거는 서로 다른 기능입니다.",
      },
    ]);
  });

  it("warns about the correct cropped edges in cover mode", () => {
    expect(
      deriveAiNormalizationWarnings({
        sourceWidth: 768,
        sourceHeight: 1024,
        sourceHasAlpha: true,
        targetCanvasWidth: 640,
        targetCanvasHeight: 640,
        options: { ...defaultOptions, mode: "cover_crop" },
      }),
    ).toEqual([
      {
        code: "cover_crop",
        severity: "warning",
        message:
          "캔버스를 채우기 위해 위·아래 일부가 잘릴 수 있습니다.",
      },
    ]);
  });

  it("adds upscale, alpha-unknown, and animated-source warnings when relevant", () => {
    expect(
      deriveAiNormalizationWarnings({
        sourceWidth: 100,
        sourceHeight: 100,
        sourceHasAlpha: null,
        sourceIsAnimated: true,
        targetCanvasWidth: 200,
        targetCanvasHeight: 200,
        options: defaultOptions,
      }).map((warning) => warning.code),
    ).toEqual([
      "animation_not_supported",
      "source_upscaled",
      "alpha_unknown",
    ]);
  });

  it("does not claim padding or crop for matching aspect ratios", () => {
    expect(
      deriveAiNormalizationWarnings({
        sourceWidth: 1024,
        sourceHeight: 1024,
        sourceHasAlpha: true,
        targetCanvasWidth: 200,
        targetCanvasHeight: 200,
        options: defaultOptions,
      }),
    ).toEqual([]);
  });
});

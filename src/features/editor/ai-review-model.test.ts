import { describe, expect, it } from "vitest";

import {
  AI_CANDIDATE_IMAGE_ACCEPT,
  AI_CANDIDATE_MAX_BYTES,
  AI_MANUAL_SERVICE_OPTIONS,
  activeAiSourceLabel,
  aiCandidateActionState,
  aiCandidateFileFormatError,
  aiCandidateFileSizeError,
  aiServiceSurfaceLabel,
  aiSourceActionLockReason,
  formatAiRecordedAt,
} from "@/features/editor/ai-review-model";
import type {
  EffectiveVisualSource,
  SourceFileSummary,
} from "@/features/editor/types";

const source: SourceFileSummary = {
  id: "source_original",
  originalFilename: "original.png",
  originalImageUrl: "asset://original.png",
  originalExtension: "png",
  mimeType: "image/png",
  sha256: "a".repeat(64),
  hasAlpha: true,
  width: 200,
  height: 200,
  byteSize: 12_345,
  isAnimated: false,
  frameCount: null,
  originalLoopMode: "preserve",
  originalLoopCount: null,
};

const visualSource: EffectiveVisualSource = {
  originalSource: source,
  effectiveRenderSource: source,
  originalLineageId: "lineage_1",
  originalLineageGeneration: 0,
  activeVersionId: null,
  activeCandidateId: null,
  activationRevision: 0,
  normalizationRecipeHash: null,
};

describe("AI review model", () => {
  it("exposes only explicit manual/local service labels", () => {
    expect(AI_MANUAL_SERVICE_OPTIONS.map((option) => option.value)).toEqual([
      "other_manual",
      "gemini_web",
      "novelai_web",
    ]);
    expect(aiServiceSurfaceLabel("gemini_web")).toContain("Gemini 웹");
    expect(aiServiceSurfaceLabel("novelai_web")).toContain("NovelAI 웹");
    expect(
      AI_MANUAL_SERVICE_OPTIONS.every((option) => option.label.includes("수동")),
    ).toBe(true);
  });

  it("distinguishes the preserved original from an active AI version", () => {
    expect(activeAiSourceLabel(visualSource)).toBe("원본 사용 중");
    expect(
      activeAiSourceLabel({
        ...visualSource,
        activeVersionId: "version_1",
        activeCandidateId: "candidate_1",
      }),
    ).toBe("AI 소스 사용 중");
  });

  it("locks source-changing actions while editor drafts are unsaved", () => {
    expect(aiSourceActionLockReason(false)).toBeNull();
    expect(aiSourceActionLockReason(true)).toContain("먼저 적용하거나 되돌려");
  });

  it("accepts only static JPG/PNG files for the first AI foundation", () => {
    expect(AI_CANDIDATE_IMAGE_ACCEPT).toBe(
      ".jpg,.jpeg,.png,image/jpeg,image/png",
    );
    expect(aiCandidateFileFormatError({ name: "candidate.JPG" })).toBeNull();
    expect(aiCandidateFileFormatError({ name: "candidate.png" })).toBeNull();
    expect(aiCandidateFileFormatError({ name: "candidate.gif" })).toBe(
      "candidate.gif: 첫 AI 편집 단계에서는 JPG 또는 PNG 정적 이미지만 후보로 가져올 수 있습니다. GIF AI 편집은 프레임/스프라이트 실험 단계에서 추가 예정입니다.",
    );
  });

  it("rejects only AI candidate files above the dedicated 16MB limit", () => {
    expect(
      aiCandidateFileSizeError({
        name: "candidate.png",
        size: AI_CANDIDATE_MAX_BYTES,
      }),
    ).toBeNull();
    expect(
      aiCandidateFileSizeError({
        name: "candidate.png",
        size: AI_CANDIDATE_MAX_BYTES + 1,
      }),
    ).toBe(
      "candidate.png: AI 후보 이미지는 최대 16MB까지 가져올 수 있습니다.",
    );
  });

  it("disables stale candidates with the backend reason", () => {
    expect(
      aiCandidateActionState(
        {
          isMaterialized: false,
          isStale: true,
          staleReason: "현재 편집 상태와 맞지 않습니다.",
        },
        false,
      ),
    ).toEqual({
      disabled: true,
      label: "현재 상태와 맞지 않음",
      reason: "현재 편집 상태와 맞지 않습니다.",
    });
  });

  it("keeps an unparseable provenance timestamp visible", () => {
    expect(formatAiRecordedAt("manual-time")).toBe("manual-time");
  });
});

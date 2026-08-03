import { describe, expect, it, vi } from "vitest";

import {
  AI_OFFICIAL_RESOURCES,
  GEMINI_IMAGE_MODELS,
  buildGeminiEditInput,
  buildNovelAiEditInput,
  consumeSessionCredential,
  copyAiHandoffPrompt,
  createDefaultGeminiEditDraft,
  createDefaultNovelAiEditDraft,
  formatAiProviderExecutionError,
  geminiDraftErrors,
  isOfficialAiResource,
  newestGeneratedCandidateId,
  novelAiDraftErrors,
  providerConfigured,
} from "@/features/editor/ai-provider-model";

describe("AI provider request gates", () => {
  it("does not guess unpublished NovelAI model or action values", () => {
    const draft = createDefaultNovelAiEditDraft();

    expect(draft.model).toBe("");
    expect(draft.action).toBe("");
    expect(novelAiDraftErrors(draft)).toContain(
      "NovelAI 모델 ID를 입력해 주세요.",
    );
    expect(novelAiDraftErrors(draft)).toContain(
      "NovelAI action 값을 입력해 주세요.",
    );
  });

  it("requires every NovelAI contract, rights, transfer, cost, and human gate", () => {
    const draft = {
      ...createDefaultNovelAiEditDraft(),
      prompt: "표정을 더 밝게",
      model: "account-confirmed-model",
      action: "account-confirmed-action",
    };

    expect(novelAiDraftErrors(draft)).toEqual([
      "사람이 직접 시작하는 1회 요청임을 확인해 주세요.",
      "원본과 프롬프트를 사용할 권리를 확인해 주세요.",
      "Image Anlas 또는 구독 사용량이 들 수 있음을 확인해 주세요.",
      "현재 이미지와 프롬프트가 NovelAI로 전송됨을 확인해 주세요.",
      "모델 ID와 action의 실험적 계약을 직접 확인해 주세요.",
    ]);

    const ready = {
      ...draft,
      negativePrompt: "text",
      humanActionConfirmed: true,
      rightsConfirmed: true,
      costConfirmed: true,
      requestContentConfirmed: true,
      contractOverrideConfirmed: true,
    };
    expect(novelAiDraftErrors(ready)).toEqual([]);
    expect(buildNovelAiEditInput("icon_1", ready)).toEqual({
      iconId: "icon_1",
      provider: "novelai",
      prompt: "표정을 더 밝게",
      model: "account-confirmed-model",
      options: {
        negativePrompt: "text",
        action: "account-confirmed-action",
        width: 1024,
        height: 1024,
        steps: 28,
        scale: 5,
        strength: 0.7,
        noise: 0,
      },
      consent: {
        humanActionConfirmed: true,
        rightsConfirmed: true,
        costConfirmed: true,
        requestContentConfirmed: true,
        contractOverrideConfirmed: true,
        adultConfirmed: false,
        under18AudienceExcludedConfirmed: false,
        professionalBusinessConfirmed: false,
        supportedRegionConfirmed: false,
        paidServiceConfirmed: false,
      },
    });
  });

  it("rejects unbounded NovelAI numeric values before invoking", () => {
    const draft = {
      ...createDefaultNovelAiEditDraft(),
      prompt: "edit",
      model: "confirmed",
      action: "confirmed",
      width: 65,
      height: 4160,
      steps: 51,
      scale: 21,
      strength: -0.01,
      noise: 1.01,
    };

    expect(novelAiDraftErrors(draft)).toEqual(
      expect.arrayContaining([
        "너비는 64~4096 사이의 64 배수여야 합니다.",
        "높이는 64~4096 사이의 64 배수여야 합니다.",
        "스텝은 1~50 사이의 정수여야 합니다.",
        "프롬프트 강도(scale)는 0~20 사이여야 합니다.",
        "원본 변화 강도(strength)는 0~1 사이여야 합니다.",
        "노이즈(noise)는 0~1 사이여야 합니다.",
      ]),
    );
  });

  it("allows only the two official Interactions image models and all eligibility gates", () => {
    const draft = {
      ...createDefaultGeminiEditDraft(),
      prompt: "clean transparent background",
      humanActionConfirmed: true,
      rightsConfirmed: true,
      costConfirmed: true,
      requestContentConfirmed: true,
      adultConfirmed: true,
      under18AudienceExcludedConfirmed: true,
      professionalBusinessConfirmed: true,
      supportedRegionConfirmed: true,
      paidServiceConfirmed: true,
    };

    expect(GEMINI_IMAGE_MODELS).toEqual([
      "gemini-2.5-flash-image",
      "gemini-3.1-flash-image",
    ]);
    expect(geminiDraftErrors(draft)).toEqual([]);
    expect(buildGeminiEditInput("icon_1", draft)).toEqual({
      iconId: "icon_1",
      provider: "gemini",
      prompt: "clean transparent background",
      model: "gemini-2.5-flash-image",
      options: {},
      consent: {
        humanActionConfirmed: true,
        rightsConfirmed: true,
        costConfirmed: true,
        requestContentConfirmed: true,
        contractOverrideConfirmed: false,
        adultConfirmed: true,
      under18AudienceExcludedConfirmed: true,
        professionalBusinessConfirmed: true,
        supportedRegionConfirmed: true,
        paidServiceConfirmed: true,
      },
    });
  });
});

describe("AI provider secret and handoff helpers", () => {
  it("consumes and immediately clears a session secret without retaining it", () => {
    const input = { value: "  pst-secret-never-echo  " };

    expect(consumeSessionCredential(input)).toBe("pst-secret-never-echo");
    expect(input.value).toBe("");
    expect(consumeSessionCredential(input)).toBe("");
  });

  it("uses clipboard first, falls back once, and reports an empty prompt", async () => {
    const clipboardWriteText = vi.fn().mockResolvedValue(undefined);
    const fallbackCopy = vi.fn(() => true);

    await expect(
      copyAiHandoffPrompt(" prompt ", { clipboardWriteText, fallbackCopy }),
    ).resolves.toBe("clipboard");
    expect(clipboardWriteText).toHaveBeenCalledOnce();
    expect(clipboardWriteText).toHaveBeenCalledWith("prompt");
    expect(fallbackCopy).not.toHaveBeenCalled();

    clipboardWriteText.mockRejectedValueOnce(new Error("denied"));
    await expect(
      copyAiHandoffPrompt("prompt", { clipboardWriteText, fallbackCopy }),
    ).resolves.toBe("fallback");
    expect(fallbackCopy).toHaveBeenCalledOnce();

    await expect(
      copyAiHandoffPrompt(" ", { clipboardWriteText, fallbackCopy }),
    ).resolves.toBe("empty");
  });

  it("exposes only backend-reviewed official resource identifiers", () => {
    expect(AI_OFFICIAL_RESOURCES).toHaveLength(9);
    for (const resource of AI_OFFICIAL_RESOURCES) {
      expect(isOfficialAiResource(resource)).toBe(true);
    }
    expect(isOfficialAiResource("https://attacker.example")).toBe(false);
    expect(isOfficialAiResource("custom_url")).toBe(false);
  });

  it("reports configured state without returning any credential material", () => {
    const status = { novelAiConfigured: true, geminiConfigured: false };

    expect(providerConfigured("novelai", status)).toBe(true);
    expect(providerConfigured("gemini", status)).toBe(false);
    expect(JSON.stringify(status)).not.toMatch(/pst-|api[-_ ]?key|credential/i);
  });

  it("identifies the provider and appends the no-retry notice only once", () => {
    expect(
      formatAiProviderExecutionError(
        "gemini",
        "AI 공급자가 요청 형식을 거부했습니다.",
      ),
    ).toBe(
      "Gemini 요청 실패: AI 공급자가 요청 형식을 거부했습니다. 자동 재시도하지 않았습니다. 원본은 바뀌지 않았습니다.",
    );
    expect(
      formatAiProviderExecutionError(
        "novelai",
        "AI 공급자에 연결하지 못했습니다. 자동 재시도하지 않았습니다.",
      ).match(/자동 재시도하지 않았습니다/g),
    ).toHaveLength(1);
  });
});

describe("AI provider success transition", () => {
  it("selects the newest newly generated candidate instead of an older entry", () => {
    const previous = [{ id: "old" }];
    const next = [
      { id: "old", createdAt: "2026-01-01T00:00:00Z", candidateIndex: 0 },
      { id: "newer", createdAt: "2026-07-28T10:00:01Z", candidateIndex: 0 },
      { id: "new", createdAt: "2026-07-28T10:00:00Z", candidateIndex: 0 },
    ];

    expect(newestGeneratedCandidateId(previous, next)).toBe("newer");
  });

  it("falls back to the newest existing candidate when a response adds none", () => {
    const candidates = [
      { id: "first", createdAt: "2026-07-28T10:00:00Z", candidateIndex: 0 },
      { id: "second", createdAt: "2026-07-28T10:00:00Z", candidateIndex: 1 },
    ];
    expect(newestGeneratedCandidateId(candidates, candidates)).toBe("second");
    expect(newestGeneratedCandidateId([], [])).toBeNull();
  });
});

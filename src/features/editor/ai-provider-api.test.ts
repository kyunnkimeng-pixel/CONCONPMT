import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AiImageEditInput,
  AiReviewState,
  AiWebHandoffSession,
  SourceFileSummary,
} from "@/features/editor/types";

const mocks = vi.hoisted(() => ({
  filePathToAssetUrl: vi.fn(
    (path: string | null) => (path ? `asset://${path}` : null),
  ),
  invokeCommand: vi.fn(),
}));

vi.mock("@/lib/asset-url", () => ({
  filePathToAssetUrl: mocks.filePathToAssetUrl,
}));

vi.mock("@/lib/import-file", () => ({
  fileToImportPayload: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  invokeCommand: mocks.invokeCommand,
}));

vi.mock("@/features/icons/api", () => ({
  normalizeIconSummary: (icon: unknown) => icon,
}));

import {
  clearAiSessionCredential,
  deleteAiWebHandoffPayload,
  executeAiImageEdit,
  extendAiWebHandoffRetention,
  getAiProviderSessionStatus,
  getAiWebHandoff,
  getLatestAiWebHandoffForIcon,
  openAiOfficialResource,
  prepareAiWebHandoff,
  setAiSessionCredential,
} from "@/features/editor/api";

const source: SourceFileSummary = {
  id: "source_1",
  originalFilename: "result.png",
  originalImageUrl: "C:\\library\\result.png",
  originalExtension: "png",
  mimeType: "image/png",
  sha256: "a".repeat(64),
  hasAlpha: true,
  width: 1024,
  height: 1024,
  byteSize: 42,
  isAnimated: false,
  frameCount: null,
  originalLoopMode: "preserve",
  originalLoopCount: null,
};

const reviewState: AiReviewState = {
  visualSource: {
    originalSource: source,
    effectiveRenderSource: source,
    originalLineageId: "lineage_1",
    originalLineageGeneration: 0,
    activeVersionId: null,
    activeCandidateId: null,
    activationRevision: 0,
    normalizationRecipeHash: null,
  },
  nativeRecipeSignature: "recipe",
  candidates: [
    {
      id: "candidate_1",
      requestId: "request_1",
      candidateIndex: 0,
      serviceSurface: "novelai_api",
      source,
      createdAt: "2026-07-28T10:00:00Z",
      isMaterialized: false,
      isStale: false,
      staleReason: null,
      isAvailable: true,
      unavailableReason: null,
      createdIconUsage: {
        createdIconCount: 0,
        latestCreatedIcon: null,
      },
    },
  ],
  versions: [],
};

const handoffSession: AiWebHandoffSession = {
  requestId: "request_web_1",
  kind: "static_icon_sheet",
  layoutMode: "single",
  operation: "edit",
  serviceSurface: "gemini_web",
  finalPrompt: "keep the exact structure",
  uploadFileName: "upload.png",
  uploadPreviewPath: "C:\\library\\ai\\handoffs\\request_web_1\\upload.png",
  expectedWidth: 200,
  expectedHeight: 200,
  expectedHasAlpha: true,
  createdAt: "2026-07-28T10:00:00Z",
  expiresAt: "2026-08-04T10:00:00Z",
  canExtend: true,
  nativeDragSupported: false,
  warnings: [],
};

const payload: AiImageEditInput = {
  iconId: "icon_1",
  provider: "novelai",
  prompt: "edit",
  model: "confirmed-model",
  options: {
    action: "confirmed-action",
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
};

describe("AI provider command API", () => {
  beforeEach(() => {
    mocks.invokeCommand.mockReset();
    mocks.filePathToAssetUrl.mockClear();
  });

  it("keeps credentials in dedicated session commands and returns status only", async () => {
    const status = { novelAiConfigured: true, geminiConfigured: false };
    mocks.invokeCommand.mockResolvedValue(status);

    await expect(getAiProviderSessionStatus()).resolves.toEqual(status);
    await expect(
      setAiSessionCredential("novelai", "pst-session-secret"),
    ).resolves.toEqual(status);
    await expect(clearAiSessionCredential("novelai")).resolves.toEqual(status);

    expect(mocks.invokeCommand.mock.calls).toEqual([
      ["get_ai_provider_session_status", {}],
      [
        "set_ai_session_credential",
        {
          payload: {
            provider: "novelai",
            credential: "pst-session-secret",
          },
        },
      ],
      ["clear_ai_session_credential", { provider: "novelai" }],
    ]);
    expect(JSON.stringify(status)).not.toContain("pst-session-secret");
  });

  it("invokes exactly one image request and normalizes the returned review state", async () => {
    mocks.invokeCommand.mockResolvedValueOnce(reviewState);

    const result = await executeAiImageEdit("collection_1", payload);

    expect(mocks.invokeCommand).toHaveBeenCalledOnce();
    expect(mocks.invokeCommand).toHaveBeenCalledWith("execute_ai_image_edit", {
      collectionId: "collection_1",
      payload,
    });
    expect(result.candidates[0]?.serviceSurface).toBe("novelai_api");
    expect(result.candidates[0]?.source.originalImageUrl).toBe(
      "asset://C:\\library\\result.png",
    );
    expect(JSON.stringify(payload)).not.toMatch(/pst-|api[-_]?key|credential/i);
  });

  it("normalizes Windows upload paths for prepare, get, and retention responses", async () => {
    mocks.invokeCommand.mockResolvedValue(handoffSession);

    const prepared = await prepareAiWebHandoff(
      "collection_1",
      "icon_1",
      "gemini_web",
      "표정을 밝게",
    );
    const restored = await getAiWebHandoff("request_web_1");
    const latest = await getLatestAiWebHandoffForIcon("collection_1", "icon_1");
    const extended = await extendAiWebHandoffRetention("request_web_1");

    for (const session of [prepared, restored, latest!, extended]) {
      expect(session.uploadPreviewPath).toBe(
        "asset://C:\\library\\ai\\handoffs\\request_web_1\\upload.png",
      );
    }
    expect(mocks.filePathToAssetUrl).toHaveBeenCalledTimes(4);
    expect(mocks.filePathToAssetUrl).toHaveBeenNthCalledWith(
      1,
      handoffSession.uploadPreviewPath,
      handoffSession.requestId,
    );
  });

  it("propagates a failure after one invoke and performs no automatic retry", async () => {
    mocks.invokeCommand.mockRejectedValueOnce({
      code: "rate_limited",
      message: "retry forbidden",
    });

    await expect(
      executeAiImageEdit("collection_1", payload),
    ).rejects.toMatchObject({ code: "rate_limited" });
    expect(mocks.invokeCommand).toHaveBeenCalledOnce();
  });

  it("returns the truthful handoff close and cleanup outcome", async () => {
    const closed = {
      sessionClosed: true,
      payloadDeleted: false,
      cleanupDeferred: true,
    };
    mocks.invokeCommand.mockResolvedValueOnce(closed);

    await expect(deleteAiWebHandoffPayload("request_web_1")).resolves.toEqual(
      closed,
    );
    expect(mocks.invokeCommand).toHaveBeenCalledWith(
      "delete_ai_web_handoff_payload",
      { requestId: "request_web_1" },
    );
  });

  it("opens a backend-enumerated resource instead of accepting a URL", async () => {
    mocks.invokeCommand.mockResolvedValueOnce(undefined);

    await openAiOfficialResource("gemini_terms");

    expect(mocks.invokeCommand).toHaveBeenCalledWith(
      "open_ai_official_resource",
      { resource: "gemini_terms" },
    );
  });
});

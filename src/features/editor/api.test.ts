import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AiReviewState,
  SourceFileSummary,
} from "@/features/editor/types";

const mocks = vi.hoisted(() => ({
  filePathToAssetUrl: vi.fn(
    (path: string | null) => (path ? `asset://${path}` : null),
  ),
  fileToImportPayload: vi.fn(),
  invokeCommand: vi.fn(),
}));

vi.mock("@/lib/asset-url", () => ({
  filePathToAssetUrl: mocks.filePathToAssetUrl,
}));

vi.mock("@/lib/import-file", () => ({
  fileToImportPayload: mocks.fileToImportPayload,
}));

vi.mock("@/lib/tauri", () => ({
  invokeCommand: mocks.invokeCommand,
}));

vi.mock("@/features/icons/api", () => ({
  normalizeIconSummary: (icon: unknown) => icon,
}));

import {
  activateAiCandidate,
  createAiIconRoot,
  getAiReviewState,
  importLocalAiCandidate,
  repairAiToOriginal,
  restoreAiVersion,
} from "@/features/editor/api";

const source: SourceFileSummary = {
  id: "source_1",
  originalFilename: "candidate.png",
  originalImageUrl: "C:\\library\\candidate.png",
  originalExtension: "png",
  mimeType: "image/png",
  sha256: "b".repeat(64),
  hasAlpha: true,
  width: 200,
  height: 200,
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
    activationRevision: 3,
    normalizationRecipeHash: null,
  },
  nativeRecipeSignature: "native-recipe-1",
  candidates: [
    {
      id: "candidate_1",
      requestId: "request_1",
      candidateIndex: 0,
      serviceSurface: "gemini_web",
      source,
      createdAt: "2026-07-27T10:00:00Z",
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

const createdIcon = {
  id: "icon_new",
  collectionId: "collection_1",
};

const editorState = {
  icon: createdIcon,
  source,
  visualSource: reviewState.visualSource,
};

describe("editor AI API", () => {
  beforeEach(() => {
    mocks.filePathToAssetUrl.mockClear();
    mocks.fileToImportPayload.mockReset();
    mocks.invokeCommand.mockReset();
    mocks.invokeCommand.mockResolvedValue(reviewState);
  });

  it("imports one bounded local file with an explicit manual surface", async () => {
    const file = { name: "candidate.png", size: 3 } as File;
    mocks.fileToImportPayload.mockResolvedValue({
      originalFilename: "candidate.png",
      bytes: [1, 2, 3],
    });

    const result = await importLocalAiCandidate(
      "collection_1",
      "icon_1",
      "gemini_web",
      file,
    );

    expect(mocks.fileToImportPayload).toHaveBeenCalledWith(file);
    expect(mocks.invokeCommand).toHaveBeenCalledWith(
      "import_local_ai_candidate",
      {
        collectionId: "collection_1",
        payload: {
          iconId: "icon_1",
          serviceSurface: "gemini_web",
          file: {
            originalFilename: "candidate.png",
            bytes: [1, 2, 3],
          },
        },
      },
    );
    expect(result.candidates[0]?.source.originalImageUrl).toBe(
      "asset://C:\\library\\candidate.png",
    );
  });

  it("rejects GIF candidates before reading or invoking", async () => {
    const file = {
      name: "animated.gif",
      size: 1024,
    } as File;

    await expect(
      importLocalAiCandidate(
        "collection_1",
        "icon_1",
        "other_manual",
        file,
      ),
    ).rejects.toThrow(
      "animated.gif: 첫 AI 편집 단계에서는 JPG 또는 PNG 정적 이미지만 후보로 가져올 수 있습니다. GIF AI 편집은 프레임/스프라이트 실험 단계에서 추가 예정입니다.",
    );
    expect(mocks.fileToImportPayload).not.toHaveBeenCalled();
    expect(mocks.invokeCommand).not.toHaveBeenCalled();
  });

  it("rejects an AI candidate over 16MB before reading or invoking", async () => {
    const file = {
      name: "large-candidate.png",
      size: 16 * 1024 * 1024 + 1,
    } as File;

    await expect(
      importLocalAiCandidate(
        "collection_1",
        "icon_1",
        "other_manual",
        file,
      ),
    ).rejects.toThrow(
      "large-candidate.png: AI 후보 이미지는 최대 16MB까지 가져올 수 있습니다.",
    );
    expect(mocks.fileToImportPayload).not.toHaveBeenCalled();
    expect(mocks.invokeCommand).not.toHaveBeenCalled();
  });

  it("normalizes combined activation and restore responses without a post-mutation GET", async () => {
    mocks.invokeCommand
      .mockResolvedValueOnce(reviewState)
      .mockResolvedValueOnce({ reviewState, editorState })
      .mockResolvedValueOnce({ reviewState, editorState });

    await getAiReviewState("collection_1", "icon_1");
    const activated = await activateAiCandidate("collection_1", {
      iconId: "icon_1",
      candidateId: "candidate_1",
      expectedRevision: 3,
      normalization: {
        mode: "contain_pad",
        alignment: "center",
        resizeFilter: "lanczos3",
        padRgba: [0, 0, 0, 0],
      },
      expectedPreviewSignature: "preview-1",
    });
    const restored = await restoreAiVersion("collection_1", {
      iconId: "icon_1",
      versionId: null,
      expectedRevision: 4,
    });

    expect(activated.reviewState.candidates[0]?.source.originalImageUrl).toBe(
      "asset://C:\\library\\candidate.png",
    );
    expect(activated.editorState.source.originalImageUrl).toBe(
      "asset://C:\\library\\candidate.png",
    );
    expect(restored.editorState.visualSource.originalSource.originalImageUrl).toBe(
      "asset://C:\\library\\candidate.png",
    );
    expect(mocks.invokeCommand.mock.calls).toEqual([
      ["get_ai_review_state", { collectionId: "collection_1", iconId: "icon_1" }],
      [
        "activate_ai_candidate",
        {
          collectionId: "collection_1",
          payload: {
            iconId: "icon_1",
            candidateId: "candidate_1",
            expectedRevision: 3,
            normalization: {
              mode: "contain_pad",
              alignment: "center",
              resizeFilter: "lanczos3",
              padRgba: [0, 0, 0, 0],
            },
            expectedPreviewSignature: "preview-1",
          },
        },
      ],
      [
        "restore_ai_version",
        {
          collectionId: "collection_1",
          payload: {
            iconId: "icon_1",
            versionId: null,
            expectedRevision: 4,
          },
        },
      ],
    ]);
    expect(
      mocks.invokeCommand.mock.calls.filter(([command]) => command === "get_ai_review_state"),
    ).toHaveLength(1);
    expect(JSON.stringify(mocks.invokeCommand.mock.calls)).not.toMatch(
      /apiKey|token|credential/i,
    );
  });

  it("creates a new icon root with the candidate and normalizes the result", async () => {
    mocks.invokeCommand.mockResolvedValueOnce({
      createdIcon,
      sourceReviewState: reviewState,
      createdIconUsage: {
        createdIconCount: 1,
        latestCreatedIcon: createdIcon,
      },
    });

    const result = await createAiIconRoot("collection_1", {
      iconId: "icon_1",
      candidateId: "candidate_1",
      expectedRevision: 3,
      normalization: {
        mode: "cover_crop",
        alignment: "top",
        resizeFilter: "nearest",
        padRgba: [0, 0, 0, 0],
      },
      expectedPreviewSignature: "preview-2",
    });

    expect(mocks.invokeCommand).toHaveBeenCalledWith("create_ai_icon_root", {
      collectionId: "collection_1",
      payload: {
        iconId: "icon_1",
        candidateId: "candidate_1",
        expectedRevision: 3,
        normalization: {
          mode: "cover_crop",
          alignment: "top",
          resizeFilter: "nearest",
          padRgba: [0, 0, 0, 0],
        },
        expectedPreviewSignature: "preview-2",
      },
    });
    expect(result.createdIcon.id).toBe("icon_new");
    expect(result.sourceReviewState.candidates[0]?.source.originalImageUrl).toBe(
      "asset://C:\\library\\candidate.png",
    );
    expect(result.createdIconUsage).toEqual({
      createdIconCount: 1,
      latestCreatedIcon: createdIcon,
    });
    expect(mocks.invokeCommand).toHaveBeenCalledTimes(1);
    expect(JSON.stringify(mocks.invokeCommand.mock.calls)).not.toMatch(
      /apiKey|token|credential/i,
    );
  });

  it("invokes original-source repair without provider credentials", async () => {
    await repairAiToOriginal("collection_1", "icon_1");

    expect(mocks.invokeCommand).toHaveBeenCalledWith(
      "repair_ai_to_original",
      {
        collectionId: "collection_1",
        payload: {
          iconId: "icon_1",
        },
      },
    );
    expect(JSON.stringify(mocks.invokeCommand.mock.calls)).not.toMatch(
      /apiKey|token|credential/i,
    );
  });
});

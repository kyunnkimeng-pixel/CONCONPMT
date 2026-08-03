import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invokeCommand: vi.fn(),
  fileToImportPayload: vi.fn(),
  filesToImportPayloads: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  invokeCommand: mocks.invokeCommand,
}));

vi.mock("@/lib/asset-url", () => ({
  filePathToAssetUrl: vi.fn(() => null),
}));

vi.mock("@/lib/import-file", () => ({
  fileToImportPayload: mocks.fileToImportPayload,
  filesToImportPayloads: mocks.filesToImportPayloads,
}));

vi.mock("@/features/icons/api", () => ({
  normalizeIconSummary: (icon: unknown) => icon,
}));

import {
  attachAiGridOutput,
  MAX_AI_REFERENCE_EXTERNAL_BYTES,
  prepareAiGenerationWorkspace,
  prepareAiGridEditWorkspace,
  revealAiGridInput,
  startAiGridInputDrag,
} from "@/features/ai-grid/api";
import type { AiGridWorkspace } from "@/features/ai-grid/types";

const workspace = {
  requestId: "request-grid-1",
  collectionId: "collection-1",
  requestScope: "grid_edit",
  status: "prepared",
  retryOfRequestId: null,
  layout: {
    canvasWidth: 1024,
    canvasHeight: 1024,
    rows: 1,
    columns: 2,
    cellSize: 504,
    gapX: 0,
    gapY: 0,
    borderLeft: 8,
    borderTop: 8,
    borderRight: 8,
    borderBottom: 8,
  },
  itemCount: 2,
  candidateCount: 0,
  createdIconCount: 0,
  inputArtifact: null,
  outputArtifact: null,
  items: [],
  createdAt: "2026-07-29T00:00:00Z",
  updatedAt: "2026-07-29T00:00:00Z",
} satisfies AiGridWorkspace;

describe("AI grid command API", () => {
  beforeEach(() => {
    mocks.invokeCommand.mockReset();
    mocks.fileToImportPayload.mockReset();
    mocks.filesToImportPayloads.mockReset();
    mocks.filesToImportPayloads.mockResolvedValue([]);
  });

  it("prepares selected edit and source-free generation with bounded defaults", async () => {
    mocks.invokeCommand.mockResolvedValue(workspace);

    await prepareAiGridEditWorkspace("collection-1", ["icon-1", "icon-2"]);
    await prepareAiGenerationWorkspace(
      "collection-1",
      ["새 아이콘 1"],
      "prompt-signature",
    );

    expect(mocks.invokeCommand.mock.calls).toEqual([
      [
        "prepare_ai_grid_edit_workspace",
        {
          collectionId: "collection-1",
          payload: {
            selectedIconIds: ["icon-1", "icon-2"],
            layout: null,
            canvasSize: 1024,
            retryOfRequestId: null,
          },
        },
      ],
      [
        "prepare_ai_generation_workspace",
        {
          collectionId: "collection-1",
          payload: {
            targetNames: ["새 아이콘 1"],
            layout: null,
            canvasSize: 1024,
            payloadInputSignature: "prompt-signature",
            referenceIconIds: [],
            referenceFiles: [],
            retryOfRequestId: null,
          },
        },
      ],
    ]);
  });

  it("forwards optional icon and file references through the generation payload", async () => {
    const referenceFile = new File([new Uint8Array([1, 2, 3])], "style.png", {
      type: "image/png",
    });
    const referenceFilePayloads = [
      {
        originalFilename: "style.png",
        bytes: [1, 2, 3],
      },
    ];
    mocks.filesToImportPayloads.mockResolvedValueOnce(referenceFilePayloads);
    mocks.invokeCommand.mockResolvedValue(workspace);

    await prepareAiGenerationWorkspace(
      "collection-1",
      ["new-icon-1"],
      "prompt-signature",
      null,
      ["reference-icon-1"],
      [referenceFile],
    );

    expect(mocks.filesToImportPayloads).toHaveBeenCalledWith([referenceFile]);
    expect(mocks.invokeCommand).toHaveBeenCalledWith(
      "prepare_ai_generation_workspace",
      {
        collectionId: "collection-1",
        payload: {
          targetNames: ["new-icon-1"],
          layout: null,
          canvasSize: 1024,
          payloadInputSignature: "prompt-signature",
          referenceIconIds: ["reference-icon-1"],
          referenceFiles: referenceFilePayloads,
          retryOfRequestId: null,
        },
      },
    );
  });

  it("rejects oversized references before converting them into IPC number arrays", async () => {
    const oversizedReference = {
      name: "oversized.png",
      size: MAX_AI_REFERENCE_EXTERNAL_BYTES + 1,
    } as File;

    await expect(
      prepareAiGenerationWorkspace(
        "collection-1",
        ["new-icon-1"],
        "prompt-signature",
        null,
        [],
        [oversizedReference],
      ),
    ).rejects.toThrow("합계 16MB");
    expect(mocks.filesToImportPayloads).not.toHaveBeenCalled();
    expect(mocks.invokeCommand).not.toHaveBeenCalled();
  });

  it("forwards the explicit opaque-background decision with the result bytes", async () => {
    const file = new File([new Uint8Array([1, 2, 3])], "result.jpg", {
      type: "image/jpeg",
    });
    const payload = { originalFilename: "result.jpg", bytes: [1, 2, 3] };
    mocks.fileToImportPayload.mockResolvedValue(payload);
    mocks.invokeCommand.mockResolvedValue(workspace);

    await attachAiGridOutput("request-grid-1", file, true);

    expect(mocks.invokeCommand).toHaveBeenCalledWith("attach_ai_grid_output", {
      requestId: "request-grid-1",
      file: payload,
      manifestJson: null,
      allowOpaqueBackground: true,
    });
  });
  it("starts native drag by request id only and keeps Explorer fallback separate", async () => {
    mocks.invokeCommand
      .mockResolvedValueOnce({
        started: true,
        nativeDragSupported: true,
        message: "놓았습니다.",
      })
      .mockResolvedValueOnce(undefined);

    await startAiGridInputDrag("request-grid-1");
    await revealAiGridInput("request-grid-1");

    expect(mocks.invokeCommand.mock.calls).toEqual([
      ["start_ai_grid_input_drag", { requestId: "request-grid-1" }],
      ["reveal_ai_grid_input", { requestId: "request-grid-1" }],
    ]);
    expect(JSON.stringify(mocks.invokeCommand.mock.calls)).not.toContain(
      "filePath",
    );
  });
});

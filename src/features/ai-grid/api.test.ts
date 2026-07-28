import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invokeCommand: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  invokeCommand: mocks.invokeCommand,
}));

vi.mock("@/lib/asset-url", () => ({
  filePathToAssetUrl: vi.fn(() => null),
}));

vi.mock("@/lib/import-file", () => ({
  fileToImportPayload: vi.fn(),
}));

vi.mock("@/features/icons/api", () => ({
  normalizeIconSummary: (icon: unknown) => icon,
}));

import {
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
            retryOfRequestId: null,
          },
        },
      ],
    ]);
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

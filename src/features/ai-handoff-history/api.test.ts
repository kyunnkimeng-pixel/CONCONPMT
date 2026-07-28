import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invokeCommand: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  invokeCommand: mocks.invokeCommand,
}));

import {
  getAiWebHandoffStorageStatus,
  listRecentAiWebHandoffs,
  runAiWebHandoffMaintenance,
} from "@/features/ai-handoff-history/api";

describe("AI handoff history command API", () => {
  beforeEach(() => {
    mocks.invokeCommand.mockReset();
    mocks.invokeCommand.mockResolvedValue([]);
  });

  it("keeps the backend request scope and handoff kind without accepting paths", async () => {
    mocks.invokeCommand.mockResolvedValueOnce([
      {
        requestId: "grid-request-1",
        requestScope: "grid_edit",
        handoffKind: "ai_grid_sheet",
      },
    ]);

    const result = await listRecentAiWebHandoffs(10);

    expect(result[0]).toMatchObject({
      requestId: "grid-request-1",
      requestScope: "grid_edit",
      handoffKind: "ai_grid_sheet",
    });
    expect(mocks.invokeCommand).toHaveBeenCalledWith(
      "list_recent_ai_web_handoffs",
      { limit: 10 },
    );
    expect(JSON.stringify(mocks.invokeCommand.mock.calls)).not.toContain(
      "filePath",
    );
  });

  it("uses bounded history, storage status, and explicit maintenance commands", async () => {
    await listRecentAiWebHandoffs();
    await getAiWebHandoffStorageStatus();
    await runAiWebHandoffMaintenance();

    expect(mocks.invokeCommand.mock.calls).toEqual([
      ["list_recent_ai_web_handoffs", { limit: 30 }],
      ["get_ai_web_handoff_storage_status"],
      ["run_ai_web_handoff_maintenance"],
    ]);
  });
});

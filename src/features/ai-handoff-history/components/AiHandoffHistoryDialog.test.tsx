// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  deletePayload: vi.fn(),
  getStorage: vi.fn(),
  gridCancel: vi.fn(),
  gridReveal: vi.fn(),
  gridStartDrag: vi.fn(),
  listRecent: vi.fn(),
  maintenance: vi.fn(),
  reveal: vi.fn(),
  startDrag: vi.fn(),
}));

vi.mock("@/features/ai-handoff-history/api", () => ({
  getAiWebHandoffStorageStatus: mocks.getStorage,
  listRecentAiWebHandoffs: mocks.listRecent,
  runAiWebHandoffMaintenance: mocks.maintenance,
}));

vi.mock("@/features/ai-grid/api", () => ({
  cancelAiGridWorkspace: mocks.gridCancel,
  revealAiGridInput: mocks.gridReveal,
  startAiGridInputDrag: mocks.gridStartDrag,
}));

vi.mock("@/features/editor/api", () => ({
  deleteAiWebHandoffPayload: mocks.deletePayload,
  revealAiWebHandoffUpload: mocks.reveal,
  startAiWebHandoffDrag: mocks.startDrag,
}));

import { AiHandoffHistoryDialog } from "@/features/ai-handoff-history/components/AiHandoffHistoryDialog";
import type {
  AiWebHandoffHistoryItem,
  AiWebHandoffStorageStatus,
} from "@/features/ai-handoff-history/types";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

const cleanupPendingItem: AiWebHandoffHistoryItem = {
  requestId: "handoff-1",
  requestScope: "icon_edit",
  handoffKind: "static_icon_sheet",
  collectionId: null,
  iconId: null,
  collectionName: "테스트 모음",
  iconName: "웃음",
  serviceSurface: "gemini_web",
  requestStatus: "completed",
  payloadState: "cleanup_pending",
  hasResult: true,
  createdAt: "2026-07-29T00:00:00Z",
  expiresAt: "2026-08-05T00:00:00Z",
  resultReceivedAt: "2026-07-29T00:10:00Z",
  cleanupRequestedAt: "2026-07-29T00:10:00Z",
  payloadDeletedAt: null,
};

const availableWebItem: AiWebHandoffHistoryItem = {
  ...cleanupPendingItem,
  requestId: "handoff-2",
  iconName: "웹 단일 전달",
  requestStatus: "awaiting_result",
  payloadState: "available",
  hasResult: false,
  resultReceivedAt: null,
  cleanupRequestedAt: null,
};

const availableGridItem: AiWebHandoffHistoryItem = {
  ...availableWebItem,
  requestId: "grid-request-1",
  requestScope: "grid_edit",
  handoffKind: "ai_grid_sheet",
  iconName: "아이콘 2개 그리드 편집",
  serviceSurface: "other_manual",
  requestStatus: "awaiting_result",
};

const availableReferenceGenerationItem: AiWebHandoffHistoryItem = {
  ...availableGridItem,
  requestId: "grid-generation-reference-1",
  requestScope: "grid_generate",
  iconName: "참고 이미지로 AI 아이콘 4개 생성",
  requestStatus: "prepared",
  payloadState: "available",
};

const closedSourceFreeItem: AiWebHandoffHistoryItem = {
  ...availableGridItem,
  requestId: "grid-request-2",
  requestScope: "grid_generate",
  iconName: "AI 아이콘 4개 그리드 생성",
  requestStatus: "prepared",
  payloadState: "closed",
};

const storage: AiWebHandoffStorageStatus = {
  quotaBytes: 256 * 1024 * 1024,
  usedBytes: 4 * 1024 * 1024,
  availableBytes: 252 * 1024 * 1024,
  retainedHistoryCount: 5,
  livePayloadCount: 3,
  cleanupPendingCount: 1,
  quotaReached: false,
};

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  Object.defineProperty(window, "confirm", {
    configurable: true,
    value: vi.fn(() => true),
  });
  for (const mock of Object.values(mocks)) mock.mockReset();
  mocks.listRecent.mockResolvedValue([
    cleanupPendingItem,
    availableWebItem,
    availableGridItem,
    availableReferenceGenerationItem,
    closedSourceFreeItem,
  ]);
  mocks.getStorage.mockResolvedValue(storage);
  mocks.reveal.mockResolvedValue(undefined);
  mocks.gridReveal.mockResolvedValue(undefined);
  mocks.gridStartDrag.mockResolvedValue({
    started: true,
    nativeDragSupported: true,
    message: "그리드 입력을 놓았습니다.",
  });
  mocks.gridCancel.mockResolvedValue(undefined);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  delete actEnvironment.IS_REACT_ACT_ENVIRONMENT;
});

async function renderDialog() {
  await act(async () => {
    root.render(<AiHandoffHistoryDialog onClose={() => undefined} />);
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

function row(text: string) {
  const match = Array.from(container.querySelectorAll("li")).find((candidate) =>
    candidate.textContent?.includes(text),
  );
  if (!(match instanceof HTMLLIElement)) {
    throw new Error(`Missing row: ${text}`);
  }
  return match;
}

function rowButton(rowElement: HTMLLIElement, text: string) {
  const match = Array.from(rowElement.querySelectorAll("button")).find(
    (candidate) => candidate.textContent?.includes(text),
  );
  if (!(match instanceof HTMLButtonElement)) {
    throw new Error(`Missing row button: ${text}`);
  }
  return match;
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("AiHandoffHistoryDialog", () => {
  it("surfaces cleanup-pending counts and keeps that state after a result", async () => {
    await renderDialog();

    expect(container.textContent).toContain("정리 대기 1개");
    expect(container.textContent).toContain("결과 받음 · 파일 정리 대기");
  });

  it("keeps the single handoff keyboard fallback on the web commands", async () => {
    await renderDialog();
    const dragButton = rowButton(row("웹 단일 전달"), "파일 끌기");

    expect(dragButton.getAttribute("aria-describedby")).toBe(
      "ai-handoff-drag-keyboard-help",
    );
    await act(async () => {
      dragButton.dispatchEvent(
        new KeyboardEvent("keydown", { bubbles: true, key: "Enter" }),
      );
      await Promise.resolve();
    });

    expect(mocks.reveal).toHaveBeenCalledWith("handoff-2");
    expect(mocks.gridReveal).not.toHaveBeenCalled();
    expect(mocks.startDrag).not.toHaveBeenCalled();
  });

  it("routes grid edit drag, Explorer, and cancellation by request id only", async () => {
    await renderDialog();

    const dragEvent = new Event("pointerdown", { bubbles: true });
    Object.defineProperties(dragEvent, {
      button: { value: 0 },
      pointerType: { value: "mouse" },
    });
    await act(async () => {
      rowButton(row("아이콘 2개 그리드 편집"), "파일 끌기").dispatchEvent(
        dragEvent,
      );
      await Promise.resolve();
    });
    expect(mocks.gridStartDrag).toHaveBeenCalledWith("grid-request-1");
    expect(mocks.startDrag).not.toHaveBeenCalled();
    await settle();

    await act(async () => {
      rowButton(row("아이콘 2개 그리드 편집"), "탐색기").click();
      await Promise.resolve();
    });
    expect(mocks.gridReveal).toHaveBeenCalledWith("grid-request-1");
    expect(mocks.reveal).not.toHaveBeenCalledWith("grid-request-1");
    await settle();

    await act(async () => {
      rowButton(row("아이콘 2개 그리드 편집"), "요청 취소").click();
      await Promise.resolve();
    });
    expect(mocks.gridCancel).toHaveBeenCalledWith("grid-request-1");
    expect(mocks.deletePayload).not.toHaveBeenCalledWith("grid-request-1");
  });

  it("routes a live generation-reference sheet through the grid commands", async () => {
    await renderDialog();

    const referenceRow = row("참고 이미지로 AI 아이콘 4개 생성");
    const dragEvent = new Event("pointerdown", { bubbles: true });
    Object.defineProperties(dragEvent, {
      button: { value: 0 },
      pointerType: { value: "mouse" },
    });
    await act(async () => {
      rowButton(referenceRow, "파일 끌기").dispatchEvent(dragEvent);
      await Promise.resolve();
    });
    expect(mocks.gridStartDrag).toHaveBeenCalledWith(
      "grid-generation-reference-1",
    );
    expect(mocks.startDrag).not.toHaveBeenCalled();
    await settle();

    await act(async () => {
      rowButton(row("참고 이미지로 AI 아이콘 4개 생성"), "탐색기").click();
      await Promise.resolve();
    });
    expect(mocks.gridReveal).toHaveBeenCalledWith(
      "grid-generation-reference-1",
    );
    expect(mocks.reveal).not.toHaveBeenCalledWith(
      "grid-generation-reference-1",
    );
    await settle();

    await act(async () => {
      rowButton(
        row("참고 이미지로 AI 아이콘 4개 생성"),
        "요청 취소",
      ).click();
      await Promise.resolve();
    });
    expect(mocks.gridCancel).toHaveBeenCalledWith(
      "grid-generation-reference-1",
    );
    expect(mocks.deletePayload).not.toHaveBeenCalledWith(
      "grid-generation-reference-1",
    );
  });

  it("does not expose file or cancel actions for source-free grid history", async () => {
    await renderDialog();
    const sourceFreeRow = row("AI 아이콘 4개 그리드 생성");

    expect(sourceFreeRow.textContent).toContain("AI 그리드");
    expect(sourceFreeRow.querySelectorAll("button")).toHaveLength(0);
  });
});
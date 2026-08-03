// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  analyze: vi.fn(),
  attach: vi.fn(),
  cancel: vi.fn(),
  commitGenerated: vi.fn(),
  commitReview: vi.fn(),
  getLatest: vi.fn(),
  markAwaiting: vi.fn(),
  openSite: vi.fn(),
  prepareEdit: vi.fn(),
  prepareGeneration: vi.fn(),
  revealInput: vi.fn(),
  startDrag: vi.fn(),
}));

vi.mock("@/features/ai-grid/api", () => ({
  analyzeAiGridOutput: mocks.analyze,
  attachAiGridOutput: mocks.attach,
  cancelAiGridWorkspace: mocks.cancel,
  commitAiGeneratedIcons: mocks.commitGenerated,
  commitAiGridReview: mocks.commitReview,
  getLatestAiGridWorkspace: mocks.getLatest,
  markAiGridWorkspaceAwaitingResult: mocks.markAwaiting,
  MAX_AI_REFERENCE_EXTERNAL_BYTES: 16 * 1024 * 1024,
  prepareAiGenerationWorkspace: mocks.prepareGeneration,
  prepareAiGridEditWorkspace: mocks.prepareEdit,
  revealAiGridInput: mocks.revealInput,
  startAiGridInputDrag: mocks.startDrag,
}));

vi.mock("@/features/editor/api", () => ({
  openAiOfficialResource: mocks.openSite,
}));

import { AiGridWorkspaceDialog } from "@/features/ai-grid/components/AiGridWorkspaceDialog";
import { buildAiGridMissingAlphaCorrectionPrompt } from "@/features/ai-grid/ai-grid-correction";
import { CommandError } from "@/lib/tauri";
import type { AiGridWorkspace } from "@/features/ai-grid/types";
import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import type { SheetGridAnalysis } from "@/features/sheets/types";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

function deferredVoid() {
  let resolve!: () => void;
  const promise = new Promise<void>((complete) => {
    resolve = () => complete();
  });
  return { promise, resolve };
}

function setControlValue(
  control: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement,
  value: string,
) {
  const descriptor = Object.getOwnPropertyDescriptor(
    Object.getPrototypeOf(control),
    "value",
  );
  descriptor?.set?.call(control, value);
  control.dispatchEvent(
    new Event(control instanceof HTMLSelectElement ? "change" : "input", {
      bubbles: true,
    }),
  );
}

const collection: CollectionSummary = {
  id: "collection-1",
  name: "테스트 모음",
  coverSourceFileId: null,
  coverIconId: null,
  coverImageUrl: null,
  iconCount: 2,
  defaultCellWidth: 200,
  defaultCellHeight: 200,
  previewWidth: 100,
  previewHeight: 100,
  exportFormat: "png",
  maxBytes: 2_000_000,
  createdAt: "2026-07-29T00:00:00Z",
  updatedAt: "2026-07-29T00:00:00Z",
};

const icons: IconSummary[] = [0, 1].map((index) => ({
  id: `icon-${index}`,
  collectionId: collection.id,
  sourceFileId: `source-${index}`,
  displayName: `표정 ${index + 1}`,
  note: null,
  iconKind: "image",
  readiness: "working",
  placeholderText: null,
  shape: "single",
  orderIndex: index,
  cellWidthOverride: null,
  cellHeightOverride: null,
  thumbnailUrl: `asset://icon-${index}.png`,
  thumbnailOverrideUrl: null,
  currentPreviewUrl: `asset://icon-${index}.png`,
  transformQuarterTurns: 0,
  transformFlipHorizontal: false,
  transformFlipVertical: false,
  gifLoopMode: "preserve",
  gifLoopCount: null,
  createdAt: "2026-07-29T00:00:00Z",
  updatedAt: "2026-07-29T00:00:00Z",
  pieces: [],
}));

function workspace(
  overrides: Partial<AiGridWorkspace> = {},
): AiGridWorkspace {
  return {
    requestId: "grid-request-1",
    collectionId: collection.id,
    requestScope: "grid_edit",
    status: "awaiting_result",
    retryOfRequestId: null,
    layout: {
      canvasWidth: 1024,
      canvasHeight: 1024,
      rows: 1,
      columns: 2,
      cellSize: 500,
      gapX: 8,
      gapY: 0,
      borderLeft: 8,
      borderTop: 262,
      borderRight: 8,
      borderBottom: 262,
    },
    itemCount: 2,
    candidateCount: 0,
    createdIconCount: 0,
    inputArtifact: {
      role: "input_sheet",
      sourceFileId: "input-source",
      originalFilename: "input.png",
      filePath: "C:\\managed\\input.png",
      previewUrl: "asset://input.png",
      extension: "png",
      mimeType: "image/png",
      width: 1024,
      height: 1024,
      byteSize: 10,
      sha256: "input-sha",
      hasAlpha: true,
      manifestJson: "{}",
      createdAt: "2026-07-29T00:00:00Z",
    },
    outputArtifact: null,
    items: [0, 1].map((itemIndex) => ({
      id: `item-${itemIndex}`,
      itemIndex,
      originIconId: `icon-${itemIndex}`,
      originIconIdSnapshot: `icon-${itemIndex}`,
      targetNameSnapshot: `표정 ${itemIndex + 1}`,
      shape: "single" as const,
      rowIndex: 0,
      columnIndex: itemIndex,
      inputRect: {
        x: 8 + itemIndex * 508,
        y: 262,
        width: 500,
        height: 500,
      },
      reviewStatus: "pending" as const,
      outputCandidateId: null,
      createdIconId: null,
    })),
    createdAt: "2026-07-29T00:00:00Z",
    updatedAt: "2026-07-29T00:00:00Z",
    ...overrides,
  };
}

function analysis(
  overrides: Partial<SheetGridAnalysis> = {},
): SheetGridAnalysis {
  return {
    sheetWidth: 1024,
    sheetHeight: 1024,
    computedRows: 1,
    computedColumns: 2,
    cellCount: 2,
    outOfBoundsCells: [],
    emptyCellCandidates: [],
    cells: [0, 1].map((index) => ({
      index,
      page: 0,
      row: 0,
      col: index,
      x: 8 + index * 508,
      y: 262,
      w: 500,
      h: 500,
      outOfBounds: false,
      emptyCandidate: false,
    })),
    warnings: [],
    ...overrides,
  };
}

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
  mocks.getLatest.mockResolvedValue(null);
  mocks.analyze.mockResolvedValue(analysis());
  mocks.cancel.mockResolvedValue(workspace({ status: "cancelled" }));
  mocks.revealInput.mockResolvedValue(undefined);
  mocks.startDrag.mockResolvedValue({
    started: true,
    nativeDragSupported: true,
    message: "놓았습니다.",
  });
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.restoreAllMocks();
  delete actEnvironment.IS_REACT_ACT_ENVIRONMENT;
});

async function renderDialog() {
  await act(async () => {
    root.render(
      <AiGridWorkspaceDialog
        collection={collection}
        icons={icons}
        mode="generate"
        selectedIconIds={[]}
        onClose={() => undefined}
        onCompleted={async () => undefined}
      />,
    );
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

function button(text: string) {
  const match = Array.from(container.querySelectorAll("button")).find(
    (candidate) => candidate.textContent?.includes(text),
  );
  if (!(match instanceof HTMLButtonElement)) {
    throw new Error(`Missing button: ${text}`);
  }
  return match;
}

async function confirmRestoredDelivery() {
  const textarea = container.querySelector<HTMLTextAreaElement>(
    'textarea[placeholder^="원래 웹에 전달할"]',
  );
  if (!textarea) throw new Error("Missing restored prompt input");
  await act(async () => {
    const descriptor = Object.getOwnPropertyDescriptor(
      Object.getPrototypeOf(textarea),
      "value",
    );
    descriptor?.set?.call(textarea, "픽셀 아트로 다시 그려줘");
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
  });
  const checkbox = container.querySelector<HTMLInputElement>(
    'aside input[type="checkbox"]',
  );
  if (!checkbox) throw new Error("Missing restored confirmation checkbox");
  await act(async () => checkbox.click());
}

describe("AiGridWorkspaceDialog lifecycle", () => {
  it("keeps NovelAI Undesired Content locked when an old prompt copy finishes after request and provider changes", async () => {
    mocks.getLatest.mockResolvedValue(workspace());
    await renderDialog();

    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    const pendingCopy = deferredVoid();
    const writeText = vi.fn(() => pendingCopy.promise);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    try {
      const service = container.querySelector<HTMLSelectElement>(
        '[data-testid="ai-grid-web-service"]',
      );
      if (!service) throw new Error("Missing grid web service select");
      await act(async () => setControlValue(service, "novelai_web"));
      await confirmRestoredDelivery();

      const undesired = () =>
        container.querySelector<HTMLButtonElement>(
          '[data-testid="novelai-copy-undesired-grid_edit"]',
        );
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        button("1. Prompt 다시 복사").click();
        await Promise.resolve();
      });
      expect(writeText).toHaveBeenCalledTimes(1);

      const request = container.querySelector<HTMLTextAreaElement>(
        'textarea[placeholder^="원래 웹에 전달할"]',
      );
      if (!request) throw new Error("Missing restored prompt input");
      await act(async () => setControlValue(request, "different character motion"));
      await act(async () => setControlValue(service, "gemini_web"));
      await act(async () => setControlValue(service, "novelai_web"));

      const confirmation = container.querySelector<HTMLInputElement>(
        'aside input[type="checkbox"]',
      );
      if (!confirmation) throw new Error("Missing restored confirmation checkbox");
      await act(async () => confirmation.click());
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        pendingCopy.resolve();
        await pendingCopy.promise;
        await Promise.resolve();
      });

      expect(undesired()?.disabled).toBe(true);
      expect(
        container.querySelector(
          '[data-testid="novelai-copy-state-grid_edit"]',
        )?.textContent,
      ).toContain("현재 1/2");
    } finally {
      pendingCopy.resolve();
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
    }
  });

  it("resets the NovelAI copy sequence when generation background policy changes", async () => {
    mocks.getLatest.mockResolvedValue(
      workspace({ requestScope: "grid_generate", inputArtifact: null }),
    );
    await renderDialog();

    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
    });

    try {
      const service = container.querySelector<HTMLSelectElement>(
        '[data-testid="ai-grid-web-service"]',
      );
      if (!service) throw new Error("Missing grid web service select");
      await act(async () => setControlValue(service, "novelai_web"));
      await confirmRestoredDelivery();

      await act(async () => {
        button("1. Prompt 다시 복사").click();
        await Promise.resolve();
      });
      const undesired = container.querySelector<HTMLButtonElement>(
        '[data-testid="novelai-copy-undesired-grid_generate"]',
      );
      expect(undesired?.disabled).toBe(false);

      const policies = container.querySelectorAll<HTMLInputElement>(
        'input[name="ai-grid-result-background-policy"]',
      );
      await act(async () => policies[1]?.click());

      expect(undesired?.disabled).toBe(true);
      expect(
        container.querySelector(
          '[data-testid="novelai-copy-state-grid_generate"]',
        )?.textContent,
      ).toContain("현재 1/2");
      expect(
        container.querySelector<HTMLTextAreaElement>(
          '[data-testid="novelai-undesired-grid_generate"]',
        )?.value,
      ).not.toContain("opaque background");
    } finally {
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
    }
  });

  it("disables restored request and provider controls during delayed NovelAI copy and site opening", async () => {
    mocks.getLatest.mockResolvedValue(workspace());
    await renderDialog();

    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    const pendingCopy = deferredVoid();
    const pendingOpen = deferredVoid();
    const writeText = vi.fn(() => pendingCopy.promise);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    mocks.openSite.mockImplementation(() => pendingOpen.promise);

    try {
      const service = container.querySelector<HTMLSelectElement>(
        '[data-testid="ai-grid-web-service"]',
      );
      if (!service) throw new Error("Missing grid web service select");
      await act(async () => setControlValue(service, "novelai_web"));
      await confirmRestoredDelivery();

      const request = container.querySelector<HTMLTextAreaElement>(
        'textarea[placeholder^="원래 웹에 전달할"]',
      );
      const undesired = () =>
        container.querySelector<HTMLButtonElement>(
          '[data-testid="novelai-copy-undesired-grid_edit"]',
        );
      if (!request) throw new Error("Missing restored prompt input");

      await act(async () => {
        button("1. Prompt 복사 + NovelAI 열기").click();
        await Promise.resolve();
      });

      expect(writeText).toHaveBeenCalledTimes(1);
      expect(service.disabled).toBe(true);
      expect(request.disabled).toBe(true);
      expect(button("1. Prompt 다시 복사").disabled).toBe(true);
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        pendingCopy.resolve();
        await pendingCopy.promise;
        await Promise.resolve();
      });

      expect(mocks.openSite).toHaveBeenCalledWith("novelai_app");
      expect(service.disabled).toBe(true);
      expect(request.disabled).toBe(true);
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        pendingOpen.resolve();
        await pendingOpen.promise;
        await Promise.resolve();
      });

      expect(service.disabled).toBe(false);
      expect(request.disabled).toBe(false);
      expect(undesired()?.disabled).toBe(false);
    } finally {
      pendingCopy.resolve();
      pendingOpen.resolve();
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
    }
  });

  it("requires restored delivery details and can cancel into a new workspace", async () => {
    mocks.getLatest.mockResolvedValue(workspace());
    await renderDialog();

    expect(
      container.querySelector('[data-testid="ai-grid-restored-delivery-notice"]'),
    ).not.toBeNull();
    expect(button("프롬프트 복사 + 웹 열기").disabled).toBe(true);

    const service = container.querySelector<HTMLSelectElement>(
      '[data-testid="ai-grid-web-service"]',
    );
    if (!service) throw new Error("Missing grid web service select");
    await act(async () => {
      const descriptor = Object.getOwnPropertyDescriptor(
        Object.getPrototypeOf(service),
        "value",
      );
      descriptor?.set?.call(service, "novelai_web");
      service.dispatchEvent(new Event("change", { bubbles: true }));
    });
    expect(
      container.querySelector('[data-testid="novelai-web-guide-grid_edit"]'),
    ).not.toBeNull();
    expect(container.textContent).toContain("Image2Image");
    expect(container.textContent).toContain("1024×1024px가 정확히 유지");

    await confirmRestoredDelivery();
    expect(button("1. Prompt 복사 + NovelAI 열기").disabled).toBe(false);

    await act(async () => {
      button("요청 취소 후 새 작업").click();
      await Promise.resolve();
    });

    expect(mocks.cancel).toHaveBeenCalledWith("grid-request-1");
    expect(
      container.querySelector('[data-testid="ai-grid-step-targets"]'),
    ).not.toBeNull();
    expect(container.textContent).toContain("새 작업을 준비할 수 있습니다");
  });

  it("resets opaque consent before preparing a new generation request", async () => {
    mocks.getLatest.mockResolvedValue(
      workspace({ requestScope: "grid_generate", inputArtifact: null }),
    );
    const nextWorkspace = workspace({
      requestId: "grid-request-new",
      requestScope: "single_generate",
      status: "prepared",
      itemCount: 1,
      inputArtifact: null,
      items: [
        {
          ...workspace().items[0],
          id: "new-item-0",
          originIconId: null,
          originIconIdSnapshot: null,
          targetNameSnapshot: "새 이모티콘 1",
        },
      ],
    });
    mocks.prepareGeneration.mockResolvedValue(nextWorkspace);
    mocks.attach.mockRejectedValueOnce(new CommandError("test_stop", "stop"));
    await renderDialog();

    const initialPolicies = container.querySelectorAll<HTMLInputElement>(
      'input[name="ai-grid-result-background-policy"]',
    );
    await act(async () => initialPolicies[1]?.click());
    expect(initialPolicies[1]?.checked).toBe(true);

    await act(async () => {
      button("요청 취소 후 새 작업").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => button("배치 확인").click());
    await act(async () => {
      button("작업공간 준비").click();
      await Promise.resolve();
      await Promise.resolve();
    });

    const nextPolicies = container.querySelectorAll<HTMLInputElement>(
      'input[name="ai-grid-result-background-policy"]',
    );
    expect(nextPolicies[0]?.checked).toBe(true);
    expect(nextPolicies[1]?.checked).toBe(false);
    expect(
      container.querySelector('[data-testid="ai-grid-continue-opaque"]'),
    ).toBeNull();

    const result = new File([new Uint8Array([1])], "new-result.png", {
      type: "image/png",
    });
    const dropTarget = container.querySelector<HTMLElement>(
      '[data-testid="ai-grid-result-drop"]',
    );
    if (!dropTarget) throw new Error("Missing new result drop target");
    const drop = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(drop, "dataTransfer", {
      configurable: true,
      value: { files: [result] },
    });
    await act(async () => {
      dropTarget.dispatchEvent(drop);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.attach).toHaveBeenCalledWith(
      "grid-request-new",
      result,
      false,
    );
  });

  it("prepares generation with selected library and external reference images", async () => {
    const generationItem = {
      ...workspace().items[0],
      originIconId: null,
      originIconIdSnapshot: null,
      targetNameSnapshot: "새 이모티콘 1",
    };
    mocks.prepareGeneration.mockResolvedValue(
      workspace({
        requestScope: "single_generate",
        status: "prepared",
        itemCount: 1,
        inputArtifact: {
          ...workspace().inputArtifact!,
          originalFilename: "pmtcon-ai-generation-references.png",
          manifestJson:
            '{"schema":"pmtcon-ai-grid-v1","kind":"generation_reference"}',
        },
        items: [generationItem],
      }),
    );
    await renderDialog();

    const referenceArea = container.querySelector(
      '[data-testid="ai-generation-references"]',
    );
    const iconCheckbox = referenceArea?.querySelector<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    const fileInput = referenceArea?.querySelector<HTMLInputElement>(
      'input[type="file"]',
    );
    if (!iconCheckbox || !fileInput) throw new Error("Missing reference controls");
    const referenceFile = new File([new Uint8Array([1, 2, 3])], "style.gif", {
      type: "image/gif",
    });
    Object.defineProperty(fileInput, "files", {
      configurable: true,
      value: [referenceFile],
    });
    await act(async () => {
      iconCheckbox.click();
      fileInput.dispatchEvent(new Event("change", { bubbles: true }));
    });
    expect(referenceArea?.textContent).toContain("2/16");
    expect(referenceArea?.textContent).toContain("style.gif");

    await act(async () => button("배치 확인").click());
    await act(async () => {
      button("작업공간 준비").click();
      await Promise.resolve();
    });

    expect(mocks.prepareGeneration).toHaveBeenCalledWith(
      collection.id,
      ["새 이모티콘 1"],
      "source-free-1-새 이모티콘 1",
      null,
      ["icon-0"],
      [referenceFile],
    );
    expect(container.textContent).toContain("2개 참고 이미지");
  });

  it("routes keyboard activation of native drag to the Explorer fallback", async () => {
    mocks.getLatest.mockResolvedValue(workspace());
    await renderDialog();
    await confirmRestoredDelivery();

    await act(async () => {
      button("입력 파일 끌기").dispatchEvent(
        new KeyboardEvent("keydown", { bubbles: true, key: "Enter" }),
      );
      await Promise.resolve();
    });

    expect(mocks.revealInput).toHaveBeenCalledTimes(1);
    expect(mocks.startDrag).not.toHaveBeenCalled();
    expect(container.textContent).toContain("키보드로 활성화하면 안전한");
  });

  it("rejects ambiguous result drops, accepts a WebP download, and keeps web-only opening available", async () => {
    mocks.getLatest.mockResolvedValue(workspace());
    mocks.attach.mockResolvedValue(
      workspace({
        status: "layout_review_pending",
        outputArtifact: {
          ...workspace().inputArtifact!,
          role: "output_sheet",
          originalFilename: "ai-grid-output.png",
          extension: "png",
          mimeType: "image/png",
        },
      }),
    );
    await renderDialog();
    await confirmRestoredDelivery();

    const resultInput = container.querySelector<HTMLInputElement>(
      '[data-testid="ai-grid-result-file-input"]',
    );
    expect(resultInput?.accept).toContain("image/jpeg");
    expect(container.textContent).toContain("정적 PNG·JPG·WebP");

    const dropTarget = container.querySelector<HTMLElement>(
      '[data-testid="ai-grid-result-drop"]',
    );
    if (!dropTarget) throw new Error("Missing result drop target");
    const png = new File([new Uint8Array([1])], "first.png", {
      type: "image/png",
    });
    const webp = new File([new Uint8Array([2])], "novelai s-123.webp", {
      type: "image/webp",
    });
    const multiple = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(multiple, "dataTransfer", {
      configurable: true,
      value: { files: [png, webp] },
    });
    await act(async () => dropTarget.dispatchEvent(multiple));

    expect(mocks.attach).not.toHaveBeenCalled();
    expect(container.textContent).toContain("현재 2개가 선택");

    await act(async () => {
      button("웹사이트만 열기").click();
      await Promise.resolve();
    });
    expect(mocks.openSite).toHaveBeenCalledWith("gemini_ai_studio");

    const single = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(single, "dataTransfer", {
      configurable: true,
      value: { files: [webp] },
    });
    await act(async () => {
      dropTarget.dispatchEvent(single);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.attach).toHaveBeenCalledWith("grid-request-1", webp, false);
  });

  it.each(["single_generate", "grid_generate"] as const)(
    "sends JPG through strict alpha review for %s",
    async (requestScope) => {
      mocks.getLatest.mockResolvedValue(
        workspace({
          requestScope,
          inputArtifact: null,
        }),
      );
      mocks.attach.mockRejectedValueOnce(
        new CommandError(
          "ai_grid_output_alpha_required",
          "AI 그리드 결과에는 실제 투명 픽셀이 필요합니다.",
        ),
      );
      await renderDialog();

      const resultInput = container.querySelector<HTMLInputElement>(
        '[data-testid="ai-grid-result-file-input"]',
      );
      expect(resultInput?.accept).toContain("image/jpeg");
      expect(resultInput?.accept).toContain(".jpg");
      expect(container.textContent).toContain("결과 배경 처리");
      expect(container.textContent).toContain("배경 포함 결과 허용");

      const dropTarget = container.querySelector<HTMLElement>(
        '[data-testid="ai-grid-result-drop"]',
      );
      if (!dropTarget) throw new Error("Missing generated result drop target");
      const jpeg = new File([new Uint8Array([1])], "opaque-result.jpg", {
        type: "image/jpeg",
      });
      const drop = new Event("drop", { bubbles: true, cancelable: true });
      Object.defineProperty(drop, "dataTransfer", {
        configurable: true,
        value: { files: [jpeg] },
      });
      await act(async () => {
        dropTarget.dispatchEvent(drop);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(mocks.attach).toHaveBeenCalledWith(
        "grid-request-1",
        jpeg,
        false,
      );
      expect(
        container.querySelector('[data-testid="ai-grid-continue-opaque"]'),
      ).not.toBeNull();
    },
  );

  it("recovers in step 4 when stored output analysis initially fails", async () => {
    mocks.getLatest.mockResolvedValue(workspace());
    mocks.attach.mockResolvedValue(
      workspace({
        status: "layout_review_pending",
        outputArtifact: {
          ...workspace().inputArtifact!,
          role: "output_sheet",
          originalFilename: "analysis-retry.png",
          extension: "png",
          mimeType: "image/png",
        },
      }),
    );
    mocks.analyze
      .mockRejectedValueOnce(
        new CommandError("ai_grid_analysis_failed", "결과 분석 실패"),
      )
      .mockResolvedValueOnce(analysis());
    await renderDialog();

    const dropTarget = container.querySelector<HTMLElement>(
      '[data-testid="ai-grid-result-drop"]',
    );
    if (!dropTarget) throw new Error("Missing result drop target");
    const png = new File([new Uint8Array([1])], "result.png", {
      type: "image/png",
    });
    const drop = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(drop, "dataTransfer", {
      configurable: true,
      value: { files: [png] },
    });
    await act(async () => {
      dropTarget.dispatchEvent(drop);
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.attach).toHaveBeenCalledTimes(1);
    expect(mocks.analyze).toHaveBeenCalledTimes(1);
    expect(
      container.querySelector('[data-testid="ai-grid-step-delivery"]'),
    ).toBeNull();
    expect(
      container.querySelector('[data-testid="ai-grid-step-review"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('[data-testid="ai-grid-analysis-retry"]'),
    ).not.toBeNull();
    expect(container.textContent).toContain("결과 분석 실패");

    await act(async () => {
      button("결과 다시 분석").click();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.attach).toHaveBeenCalledTimes(1);
    expect(mocks.analyze).toHaveBeenCalledTimes(2);
    expect(
      container.querySelector('[data-testid="ai-grid-analysis-retry"]'),
    ).toBeNull();
    expect(container.textContent).toContain("모든 매핑이 유효합니다");
  });
  it("keeps a rejected opaque file and continues only after explicit background approval", async () => {
    mocks.getLatest.mockResolvedValue(
      workspace({
        requestScope: "grid_generate",
        inputArtifact: null,
      }),
    );
    mocks.attach.mockRejectedValueOnce(
      new CommandError(
        "ai_grid_output_alpha_required",
        "AI 그리드 결과에는 실제 투명 픽셀이 필요합니다.",
      ),
    );
    await renderDialog();

    const firstDropTarget = container.querySelector<HTMLElement>(
      '[data-testid="ai-grid-result-drop"]',
    );
    if (!firstDropTarget) throw new Error("Missing result drop target");
    const opaqueResult = new File([new Uint8Array([1])], "opaque.png", {
      type: "image/png",
    });
    const firstDrop = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(firstDrop, "dataTransfer", {
      configurable: true,
      value: { files: [opaqueResult] },
    });
    await act(async () => {
      firstDropTarget.dispatchEvent(firstDrop);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(
      container.querySelector('[data-testid="ai-grid-step-delivery"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('[data-testid="ai-grid-missing-alpha-result"]'),
    ).not.toBeNull();
    expect(container.textContent).toContain(
      "결과에 실제 투명 배경이 없습니다",
    );
    expect(container.textContent).toContain("단색이나 체커무늬");
    expect(container.textContent).toContain(
      "웹 AI에 추가할 투명 배경 수정 프롬프트",
    );
    expect(container.textContent).toContain("Use real alpha transparency");
    const prompt = container.querySelector<HTMLTextAreaElement>(
      '[data-testid="ai-grid-missing-alpha-prompt"]',
    );
    expect(prompt?.readOnly).toBe(true);
    expect(prompt?.value).toBe(buildAiGridMissingAlphaCorrectionPrompt());

    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    try {
      await act(async () => {
        button("투명 배경 수정 프롬프트 복사").click();
        await Promise.resolve();
      });
      expect(writeText).toHaveBeenCalledWith(
        buildAiGridMissingAlphaCorrectionPrompt(),
      );
    } finally {
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
    }

    const backgroundPolicies = Array.from(
      container.querySelectorAll<HTMLInputElement>(
        'input[name="ai-grid-result-background-policy"]',
      ),
    );
    expect(backgroundPolicies).toHaveLength(2);
    expect(backgroundPolicies[0]?.checked).toBe(true);
    await act(async () => backgroundPolicies[1]?.click());
    expect(backgroundPolicies[1]?.checked).toBe(true);
    expect(
      container.querySelector('[data-testid="ai-grid-continue-opaque"]'),
    ).not.toBeNull();

    mocks.attach.mockResolvedValueOnce(
      workspace({
        requestScope: "grid_generate",
        status: "layout_review_pending",
        inputArtifact: null,
        outputArtifact: {
          ...workspace().inputArtifact!,
          role: "output_sheet",
          sourceFileId: "valid-output-source",
          originalFilename: "valid-output.png",
          filePath: "C:\\managed\\valid-output.png",
          previewUrl: "asset://valid-output.png",
          sha256: "valid-output-sha",
        },
      }),
    );
    await act(async () => {
      button("배경 포함으로 이 파일 가져오기").click();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.attach).toHaveBeenNthCalledWith(
      1,
      "grid-request-1",
      opaqueResult,
      false,
    );
    expect(mocks.attach).toHaveBeenNthCalledWith(
      2,
      "grid-request-1",
      opaqueResult,
      true,
    );

    expect(
      container.querySelector('[data-testid="ai-grid-missing-alpha-result"]'),
    ).toBeNull();
    expect(
      container.querySelector('[data-testid="ai-grid-step-review"]'),
    ).not.toBeNull();
  });
  it("requires renewed background confirmation when a generation review is restored", async () => {
    mocks.getLatest.mockResolvedValue(
      workspace({
        requestScope: "grid_generate",
        status: "layout_review_pending",
        inputArtifact: null,
        outputArtifact: {
          ...workspace().inputArtifact!,
          role: "output_sheet",
          originalFilename: "transparent-result.png",
          filePath: "C:\\managed\\transparent-result.png",
          previewUrl: "asset://transparent-result.png",
          hasAlpha: true,
        },
      }),
    );
    await renderDialog();

    const review = container.querySelector(
      '[data-testid="ai-grid-generation-background-review"]',
    );
    expect(review?.textContent).toContain("투명/반투명 픽셀");
    expect(review?.textContent).toContain("가짜 투명 배경");
    const save = button("포함 셀 후보 저장");
    expect(save.disabled).toBe(true);

    const confirm = container.querySelector<HTMLInputElement>(
      '[data-testid="ai-grid-generation-background-review-confirm"]',
    );
    if (!confirm) throw new Error("Missing generation background confirmation");
    await act(async () => confirm.click());
    expect(save.disabled).toBe(false);

    const currentWorkspace = workspace({
      requestScope: "grid_generate",
      status: "layout_review_pending",
      candidateCount: 2,
      inputArtifact: null,
      outputArtifact: {
        ...workspace().inputArtifact!,
        role: "output_sheet",
        originalFilename: "transparent-result.png",
        filePath: "C:\\managed\\transparent-result.png",
        previewUrl: "asset://transparent-result.png",
        hasAlpha: true,
      },
      items: workspace().items.map((item) => ({
        ...item,
        reviewStatus: "candidate_created" as const,
        outputCandidateId: `candidate-${item.itemIndex}`,
      })),
    });
    mocks.commitReview.mockResolvedValue({
      commit: {
        requestId: currentWorkspace.requestId,
        candidateIds: ["candidate-0", "candidate-1"],
        rejectedItemIndexes: [],
        reviewSignature: "review-signature",
      },
      workspace: currentWorkspace,
    });
    await act(async () => {
      save.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.commitReview).toHaveBeenCalledWith(
      "grid-request-1",
      expect.any(Array),
    );
  });

  it("shows a large saved output preview and re-confirms background before final generation", async () => {
    const outputPreviewUrl = "asset://opaque-result.jpg";
    mocks.getLatest.mockResolvedValue(
      workspace({
        requestScope: "grid_generate",
        status: "layout_review_pending",
        candidateCount: 2,
        inputArtifact: null,
        outputArtifact: {
          ...workspace().inputArtifact!,
          role: "output_sheet",
          originalFilename: "opaque-result.jpg",
          filePath: "C:\\managed\\opaque-result.jpg",
          previewUrl: outputPreviewUrl,
          extension: "jpg",
          mimeType: "image/jpeg",
          hasAlpha: false,
        },
        items: workspace().items.map((item) => ({
          ...item,
          originIconId: null,
          originIconIdSnapshot: null,
          reviewStatus: "candidate_created" as const,
          outputCandidateId: `candidate-${item.itemIndex}`,
        })),
      }),
    );
    await renderDialog();

    const review = container.querySelector(
      '[data-testid="ai-grid-final-background-review"]',
    );
    expect(review?.textContent).toContain("JPG 결과는 불투명");
    expect(review?.textContent).toContain("가짜 투명 배경");
    const preview = container.querySelector<HTMLImageElement>(
      '[data-testid="ai-grid-final-background-review-preview"]',
    );
    const previewLink = container.querySelector<HTMLAnchorElement>(
      '[data-testid="ai-grid-final-background-review-preview-link"]',
    );
    expect(preview?.getAttribute("src")).toBe(outputPreviewUrl);
    expect(previewLink?.getAttribute("href")).toBe(outputPreviewUrl);
    expect(previewLink?.target).toBe("_blank");

    const create = button("2개 새 아이콘 모두 만들기");
    expect(create.disabled).toBe(true);
    const confirm = container.querySelector<HTMLInputElement>(
      '[data-testid="ai-grid-final-background-review-confirm"]',
    );
    if (!confirm) throw new Error("Missing final background confirmation");
    await act(async () => confirm.click());
    expect(create.disabled).toBe(false);
  });

  it("blocks an all-or-none edit review when an included result cell is empty", async () => {
    mocks.getLatest.mockResolvedValue(
      workspace({
        status: "layout_review_pending",
        outputArtifact: {
          ...workspace().inputArtifact!,
          role: "output_sheet",
          sourceFileId: "output-source",
          originalFilename: "output.png",
          filePath: "C:\\managed\\output.png",
          previewUrl: "asset://output.png",
          sha256: "output-sha",
        },
      }),
    );
    mocks.analyze.mockResolvedValue(
      analysis({
        emptyCellCandidates: [1],
        cells: analysis().cells.map((cell) =>
          cell.index === 1 ? { ...cell, emptyCandidate: true } : cell,
        ),
      }),
    );
    await renderDialog();

    expect(container.textContent).toContain("비어 있는 결과 셀");
    expect(button("2개 후보 모두 저장").disabled).toBe(true);
    expect(container.textContent).toContain("4. 전체 시트와 셀 매핑 검토");
  });
});
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
import type { AiGridWorkspace } from "@/features/ai-grid/types";
import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import type { SheetGridAnalysis } from "@/features/sheets/types";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

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
  it("requires restored delivery details and can cancel into a new workspace", async () => {
    mocks.getLatest.mockResolvedValue(workspace());
    await renderDialog();

    expect(
      container.querySelector('[data-testid="ai-grid-restored-delivery-notice"]'),
    ).not.toBeNull();
    expect(button("프롬프트 복사 + 웹 열기").disabled).toBe(true);

    await confirmRestoredDelivery();
    expect(button("프롬프트 복사 + 웹 열기").disabled).toBe(false);

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
// @vitest-environment happy-dom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getSessionStatus: vi.fn(),
  analyzeGif: vi.fn(),
  listPresets: vi.fn(),
  getDefaultPreset: vi.fn(),
}));

vi.mock("@/features/editor/api", () => ({
  clearAiSessionCredential: vi.fn(),
  deleteAiWebHandoffPayload: vi.fn(),
  executeAiImageEdit: vi.fn(),
  extendAiWebHandoffRetention: vi.fn(),
  getAiProviderSessionStatus: mocks.getSessionStatus,
  getLatestAiWebHandoffForIcon: vi.fn(),
  inspectAndCommitAiWebHandoffResult: vi.fn(),
  openAiOfficialResource: vi.fn(),
  prepareAiWebHandoff: vi.fn(),
  revealAiWebHandoffUpload: vi.fn(),
  setAiSessionCredential: vi.fn(),
  startAiWebHandoffDrag: vi.fn(),
}));

vi.mock("@/features/sheets/api", () => ({
  analyzeGifFrameSheetExport: mocks.analyzeGif,
  createSheetGridPreset: vi.fn(),
  deleteSheetGridPreset: vi.fn(),
  duplicateSheetGridPreset: vi.fn(),
  exportGifFrameSheet: vi.fn(),
  getDefaultSheetGridPreset: mocks.getDefaultPreset,
  listSheetGridPresets: mocks.listPresets,
  reimportGifFrameSheet: vi.fn(),
  setDefaultSheetGridPreset: vi.fn(),
  validateGifFrameSheetReimport: vi.fn(),
}));

import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import { AiProviderPanel } from "@/features/editor/components/AiProviderPanel";
import type { SourceFileSummary } from "@/features/editor/types";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

const collection = {
  id: "collection_1",
  name: "GIF 모음",
  defaultCellWidth: 200,
  defaultCellHeight: 200,
} as CollectionSummary;

const icon = {
  id: "icon_1",
  collectionId: collection.id,
  displayName: "움직이는 아이콘",
  cellWidthOverride: null,
  cellHeightOverride: null,
} as IconSummary;

const source = {
  id: "source_gif",
  originalFilename: "animated.gif",
  originalImageUrl: "asset://animated.gif",
  originalExtension: "gif",
  mimeType: "image/gif",
  sha256: "a".repeat(64),
  hasAlpha: true,
  width: 200,
  height: 200,
  byteSize: 12_345,
  isAnimated: true,
  frameCount: 12,
  originalLoopMode: "count",
  originalLoopCount: 3,
} satisfies SourceFileSummary;

beforeEach(() => {
  actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
  mocks.getSessionStatus.mockReset();
  mocks.analyzeGif.mockReset();
  mocks.listPresets.mockReset();
  mocks.getDefaultPreset.mockReset();
  mocks.listPresets.mockResolvedValue([]);
  mocks.getDefaultPreset.mockResolvedValue(null);
  mocks.analyzeGif.mockResolvedValue({
    iconId: icon.id,
    displayName: icon.displayName,
    sourceFormat: "gif",
    frameCount: 12,
    durationMs: 960,
    loopMode: "count",
    loopCount: 3,
    pageCount: 1,
    sheetWidth: 1680,
    sheetHeight: 224,
    columns: 8,
    rowsPerPage: 1,
    warnings: [],
  });
});

afterEach(() => {
  delete actEnvironment.IS_REACT_ACT_ENVIRONMENT;
});

describe("AiProviderPanel GIF workspace entry", () => {
  it("opens the existing GIF frame-sheet dialog without starting provider API setup", async () => {
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);

    try {
      await act(async () => {
        root.render(
          <AiProviderPanel
            collection={collection}
            disabled={false}
            hasUnsavedChanges={false}
            icon={icon}
            source={source}
            onAnnouncement={() => undefined}
            onBusyEnd={() => undefined}
            onBusyStart={() => true}
            onGenerated={() => undefined}
          />,
        );
      });

      const openButton = host.querySelector<HTMLButtonElement>(
        '[data-testid="ai-gif-frame-sheet-open"]',
      );
      expect(openButton).not.toBeNull();
      await act(async () => {
        openButton?.click();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(host.querySelector('[data-testid="gif-frame-sheet-dialog"]')).not
        .toBeNull();
      expect(host.textContent).toContain("수동 웹 AI용 PNG 프레임 시트 왕복");
      expect(host.textContent).toContain("manifest에서 보존·복원");
      expect(mocks.getSessionStatus).not.toHaveBeenCalled();
    } finally {
      act(() => root.unmount());
      host.remove();
    }
  });
});

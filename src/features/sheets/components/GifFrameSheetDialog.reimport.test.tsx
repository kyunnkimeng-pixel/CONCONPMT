// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  analyze: vi.fn(),
  exportFrames: vi.fn(),
  getDefaultPreset: vi.fn(),
  listPresets: vi.fn(),
  listProfiles: vi.fn(),
  openPath: vi.fn(),
  reimport: vi.fn(),
  revealPage: vi.fn(),
  startPageDrag: vi.fn(),
  validate: vi.fn(),
}));

vi.mock("@/features/sheets/api", () => ({
  analyzeGifFrameSheetExport: mocks.analyze,
  exportGifFrameSheet: mocks.exportFrames,
  getDefaultSheetGridPreset: mocks.getDefaultPreset,
  listSheetGridPresets: mocks.listPresets,
  reimportGifFrameSheet: mocks.reimport,
  revealGifFrameSheetPage: mocks.revealPage,
  startGifFrameSheetPageDrag: mocks.startPageDrag,
  validateGifFrameSheetReimport: mocks.validate,
}));

vi.mock("@/features/export/api", () => ({
  listExportProfiles: mocks.listProfiles,
  openExportPath: mocks.openPath,
}));

import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import {
  GifFrameExportResultPanel,
  GifFrameSheetDialog,
} from "@/features/sheets/components/GifFrameSheetDialog";
import {
  MAX_GIF_FRAME_REIMPORT_PAGE_COUNT,
  MAX_GIF_FRAME_REIMPORT_TOTAL_BYTES,
} from "@/features/sheets/gif-frame-reimport-model";
const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

let host: HTMLDivElement;
let root: Root;

const collection = {
  id: "collection-1",
  defaultCellWidth: 200,
  defaultCellHeight: 200,
} as CollectionSummary;
const icon = {
  id: "icon-1",
  displayName: "움직이는 아이콘",
  shape: "single",
  cellWidthOverride: null,
  cellHeightOverride: null,
} as IconSummary;

function manifestFile() {
  return new File(
    [
      JSON.stringify({
        schema: "pmtcon-gif-frame-sheet-v2",
        pages: [
          {
            page_index: 0,
            clean_sheet_file: "frames_sheet_001.png",
            width: 1024,
            height: 1024,
          },
        ],
      }),
    ],
    "frames_manifest.json",
    { type: "application/json" },
  );
}

function dropFiles(target: HTMLElement, files: File[]) {
  const event = new Event("drop", { bubbles: true, cancelable: true });
  Object.defineProperty(event, "dataTransfer", {
    configurable: true,
    value: { files },
  });
  target.dispatchEvent(event);
}
function changeFiles(target: HTMLInputElement, files: File[]) {
  Object.defineProperty(target, "files", {
    configurable: true,
    value: files,
  });
  target.dispatchEvent(new Event("change", { bubbles: true }));
}

function withReportedSize(file: File, size: number) {
  Object.defineProperty(file, "size", { configurable: true, value: size });
  return file;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

function delayedManifestFile(name: string, contents: Promise<string>) {
  const file = new File([], name, { type: "application/json" });
  Object.defineProperty(file, "text", {
    configurable: true,
    value: () => contents,
  });
  return file;
}

function manifestContents(expectedFileName: string) {
  return JSON.stringify({
    schema: "pmtcon-gif-frame-sheet-v2",
    pages: [
      {
        page_index: 0,
        clean_sheet_file: expectedFileName,
        width: 1024,
        height: 1024,
      },
    ],
  });
}

async function flushEffects() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(async () => {
  actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
  mocks.getDefaultPreset.mockResolvedValue(null);
  mocks.listPresets.mockResolvedValue([]);
  mocks.listProfiles.mockResolvedValue([]);
  mocks.revealPage.mockResolvedValue(undefined);
  mocks.startPageDrag.mockResolvedValue({
    message: "드래그를 마쳤습니다.",
    nativeDragSupported: true,
    outcome: "dropped",
  });
  mocks.validate.mockResolvedValue({
    frameCount: 1,
    detectedFrameCount: 1,
    pageCount: 1,
    missingPages: [],
    wrongDimensionPages: [],
    loopMode: "infinite",
    loopCount: null,
    durationMs: 100,
    warnings: [],
    errors: [],
  });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => {
    root.render(
      <GifFrameSheetDialog
        collection={collection}
        icon={icon}
        mode="reimport"
        onClose={vi.fn()}
        onVariantCreated={vi.fn().mockResolvedValue(undefined)}
      />,
    );
  });
  await flushEffects();
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.clearAllMocks();
  delete actEnvironment.IS_REACT_ACT_ENVIRONMENT;
});

describe("GIF frame sheet result drop", () => {
  it("accepts static WebP and explains that animation and alpha are validated", async () => {
    const dropZone = host.querySelector<HTMLElement>(
      '[data-testid="gif-frame-reimport-drop"]',
    );
    expect(dropZone).not.toBeNull();
    const webp = new File(["webp"], "frames_sheet_001.webp", {
      type: "image/webp",
    });

    await act(async () => {
      dropFiles(dropZone!, [manifestFile(), webp]);
      await Promise.resolve();
    });
    await flushEffects();

    expect(host.textContent).toContain("선택한 결과: frames_sheet_001.webp");
    expect(host.textContent).toContain("WebP는 정적 이미지만 사용할 수 있습니다");
    expect(host.textContent).toContain("투명도 유무와 애니메이션 여부는 가져오기 전에 검사");
    expect(mocks.validate).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "manual_file" }),
      [webp],
      [0],
      "preserve_alpha",
    );
  });
  it("requires an explicit background-included choice for JPG and never suggests renaming", async () => {
    const dropZone = host.querySelector<HTMLElement>(
      '[data-testid="gif-frame-reimport-drop"]',
    );
    const jpg = new File(["jpg"], "frames_sheet_001.jpg", {
      type: "image/jpeg",
    });

    await act(async () => {
      dropFiles(dropZone!, [manifestFile(), jpg]);
      await Promise.resolve();
    });
    await flushEffects();

    expect(host.textContent).toContain("JPG/JPEG에는 투명도 정보가 없습니다");
    expect(host.textContent).toContain("확장자만 .png로 바꿔도 투명해지지 않습니다");
    expect(mocks.validate).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: "manual_file" }),
      [jpg],
      [0],
      "preserve_alpha",
    );

    const allowOpaque = Array.from(
      host.querySelectorAll<HTMLInputElement>('input[name="gif-frame-transparency"]'),
    ).find((input) => !input.checked);
    expect(allowOpaque).toBeDefined();
    await act(async () => {
      allowOpaque?.click();
      await Promise.resolve();
    });
    await flushEffects();

    expect(host.textContent).toContain("보이는 배경이 GIF에 그대로 들어갑니다");
    expect(mocks.validate).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: "manual_file" }),
      [jpg],
      [0],
      "allow_opaque",
    );
  });
  it("preserves a PNG selected before its manifest", async () => {
    const [manifestInput, pngInput] = Array.from(
      host.querySelectorAll<HTMLInputElement>('input[type="file"]'),
    );
    const png = new File(["x"], "frames_sheet_001.png", {
      type: "image/png",
    });

    await act(async () => {
      changeFiles(pngInput!, [png]);
    });
    expect(host.textContent).toContain("선택한 결과: frames_sheet_001.png");

    await act(async () => {
      changeFiles(manifestInput!, [manifestFile()]);
      await Promise.resolve();
    });
    await flushEffects();

    expect(host.textContent).toContain("Manifest: frames_manifest.json");
    expect(host.textContent).toContain("선택한 결과: frames_sheet_001.png");
    expect(host.textContent).toContain("1/1 페이지 지정");
  });
  it("keeps a separately selected PNG while the manifest is still being read", async () => {
    const [manifestInput, pngInput] = Array.from(
      host.querySelectorAll<HTMLInputElement>('input[type="file"]'),
    );
    const pendingRead = deferred<string>();
    const manifest = delayedManifestFile("frames_manifest.json", pendingRead.promise);
    const png = new File(["x"], "frames_sheet_001.png", {
      type: "image/png",
    });

    await act(async () => {
      changeFiles(manifestInput!, [manifest]);
      await Promise.resolve();
    });
    await act(async () => {
      changeFiles(pngInput!, [png]);
      await Promise.resolve();
    });
    await act(async () => {
      pendingRead.resolve(manifestContents("frames_sheet_001.png"));
      await pendingRead.promise;
    });
    await flushEffects();

    expect(host.textContent).toContain("Manifest: frames_manifest.json");
    expect(host.textContent).toContain("선택한 결과: frames_sheet_001.png");
    expect(host.textContent).toContain("1/1 페이지 지정");
  });
  it("rejects an over-64MiB aggregate when manifest and PNG are selected separately", async () => {
    const [manifestInput, pngInput] = Array.from(
      host.querySelectorAll<HTMLInputElement>('input[type="file"]'),
    );
    expect(manifestInput).toBeDefined();
    expect(pngInput).toBeDefined();

    const reportedManifestBytes = 3 * 1024 * 1024;
    const manifest = withReportedSize(manifestFile(), reportedManifestBytes);
    const png = withReportedSize(
      new File([], "frames_sheet_001.png", { type: "image/png" }),
      MAX_GIF_FRAME_REIMPORT_TOTAL_BYTES - reportedManifestBytes + 1,
    );

    await act(async () => {
      changeFiles(manifestInput!, [manifest]);
      await Promise.resolve();
    });
    await flushEffects();
    expect(host.textContent).toContain("Manifest: frames_manifest.json");

    await act(async () => {
      changeFiles(pngInput!, [png]);
    });
    await flushEffects();

    expect(host.querySelector('[role="alert"]')?.textContent).toContain("64MB");
    expect(mocks.validate).not.toHaveBeenCalled();
    expect(
      host.querySelector<HTMLButtonElement>(
        '[data-testid="gif-frame-reimport-create"]',
      )?.disabled,
    ).toBe(true);
  });

  it("keeps the newer manifest when an older deferred read resolves last", async () => {
    const manifestInput = host.querySelector<HTMLInputElement>(
      'input[type="file"]',
    );
    expect(manifestInput).not.toBeNull();
    const firstRead = deferred<string>();
    const secondRead = deferred<string>();
    const firstManifest = delayedManifestFile("manifest-a.json", firstRead.promise);
    const secondManifest = delayedManifestFile("manifest-b.json", secondRead.promise);

    await act(async () => {
      changeFiles(manifestInput!, [firstManifest]);
      await Promise.resolve();
    });
    await act(async () => {
      changeFiles(manifestInput!, [secondManifest]);
      await Promise.resolve();
    });

    await act(async () => {
      secondRead.resolve(manifestContents("page-b.png"));
      await secondRead.promise;
    });
    await flushEffects();
    expect(host.textContent).toContain("Manifest: manifest-b.json");
    expect(host.textContent).toContain("page-b.png");

    await act(async () => {
      firstRead.resolve(manifestContents("page-a.png"));
      await firstRead.promise;
    });
    await flushEffects();

    expect(host.textContent).toContain("Manifest: manifest-b.json");
    expect(host.textContent).toContain("page-b.png");
    expect(host.textContent).not.toContain("page-a.png");
  });

  it("starts native drag for the selected page and keeps Explorer fallback page-specific", async () => {
    await act(async () => {
      root.render(
        <GifFrameExportResultPanel
          key="page-actions"
          result={{
            frameSheetPaths: [
              "C:\\managed\\frames_sheet_001.png",
              "C:\\managed\\frames_sheet_002.png",
            ],
            guideSheetPaths: ["C:\\managed\\frames_guide_001.png"],
            manifestPath: "C:\\managed\\frames_manifest.json",
            outputDirectory: "C:\\managed",
            frameCount: 2,
            pageCount: 2,
            warnings: [],
          }}
          onOpenFolder={vi.fn()}
        />,
      );
    });

    const dragButton = host.querySelector<HTMLButtonElement>(
      '[data-testid="gif-frame-page-native-drag"]',
    );
    const pointerDown = new Event("pointerdown", {
      bubbles: true,
      cancelable: true,
    });
    Object.defineProperties(pointerDown, {
      pointerType: { value: "mouse" },
      button: { value: 0 },
    });
    await act(async () => {
      dragButton?.dispatchEvent(pointerDown);
      await Promise.resolve();
    });
    await flushEffects();
    expect(mocks.startPageDrag).toHaveBeenCalledWith(
      "C:\\managed\\frames_manifest.json",
      0,
    );

    await act(async () => {
      host
        .querySelector<HTMLButtonElement>('[data-testid="gif-frame-upload-page-1"]')
        ?.click();
    });
    await flushEffects();
    await act(async () => {
      host
        .querySelector<HTMLButtonElement>('[data-testid="gif-frame-page-reveal"]')
        ?.click();
      await Promise.resolve();
    });
    await flushEffects();
    expect(mocks.revealPage).toHaveBeenCalledWith(
      "C:\\managed\\frames_manifest.json",
      1,
    );
  });
  it("retains the exported manifest across export to reimport and keeps manual JSON behind recovery", async () => {
    mocks.analyze.mockResolvedValue({
      iconId: icon.id,
      displayName: icon.displayName,
      sourceFormat: "gif",
      frameCount: 1,
      durationMs: 100,
      loopMode: "infinite",
      loopCount: null,
      pageCount: 1,
      pages: [{ pageIndex: 0, itemCount: 1, width: 1024, height: 1024 }],
      sheetWidth: 1024,
      sheetHeight: 1024,
      columns: 4,
      rowsPerPage: 4,
      warnings: [],
    });
    mocks.exportFrames.mockResolvedValue({
      frameSheetPaths: ["C:\\managed\\frames_sheet_001.png"],
      guideSheetPaths: ["C:\\managed\\frames_guide_001.png"],
      manifestPath: "C:\\managed\\frames_manifest.json",
      outputDirectory: "C:\\managed",
      frameCount: 1,
      pageCount: 1,
      warnings: [],
    });

    await act(async () => {
      root.render(
        <GifFrameSheetDialog
          key="same-session"
          aiWebWorkflow
          collection={collection}
          icon={icon}
          mode="export"
          onClose={vi.fn()}
          onOpenAiSite={vi.fn().mockResolvedValue(undefined)}
          onVariantCreated={vi.fn().mockResolvedValue(undefined)}
        />,
      );
    });
    await flushEffects();

    const exportButton = Array.from(host.querySelectorAll("button")).find(
      (button) => button.textContent === "GIF 프레임 시트 내보내기",
    );
    expect(exportButton).toBeDefined();
    await act(async () => {
      exportButton?.click();
      await Promise.resolve();
    });
    await flushEffects();
    expect(host.textContent).toContain("앱 복원용 · AI 업로드 금지 · manifest");

    const reimportTab = Array.from(host.querySelectorAll("button")).find(
      (button) => button.textContent === "다시 가져오기",
    );
    await act(async () => reimportTab?.click());
    await flushEffects();

    expect(host.textContent).toContain("manifest · 자동 유지됨");
    expect(host.textContent).toContain("결과 이미지만 놓으세요");
    expect(
      host.querySelector('[data-testid="gif-frame-manifest-file"]'),
    ).toBeNull();
    expect(host.textContent).toContain("이전 작업 복구 · manifest 수동 선택");

    const png = new File(["png"], "frames_sheet_001.png", {
      type: "image/png",
    });
    const resultInput = host.querySelector<HTMLInputElement>(
      '[data-testid="gif-frame-result-files"]',
    );
    await act(async () => {
      changeFiles(resultInput!, [png]);
      await Promise.resolve();
    });
    await flushEffects();

    expect(mocks.validate).toHaveBeenLastCalledWith(
      {
        kind: "retained_path",
        path: "C:\\managed\\frames_manifest.json",
      },
      [png],
      [0],
      "preserve_alpha",
    );

    const recoveryButton = host.querySelector<HTMLButtonElement>(
      '[data-testid="gif-frame-manifest-recovery-open"]',
    );
    await act(async () => recoveryButton?.click());
    expect(
      host.querySelector('[data-testid="gif-frame-manifest-file"]'),
    ).not.toBeNull();
    expect(host.textContent).toContain("수동 복구는 앱을 다시 켰거나");
  });
  it("renders only one pending page selector for a 500-page arbitrary-name payload", async () => {
    const dropZone = host.querySelector<HTMLElement>(
      '[data-testid="gif-frame-reimport-drop"]',
    );
    expect(dropZone).not.toBeNull();
    const pages = Array.from(
      { length: MAX_GIF_FRAME_REIMPORT_PAGE_COUNT },
      (_, pageIndex) => ({
        page_index: pageIndex,
        clean_sheet_file: `expected-page-${pageIndex}.png`,
        width: 1,
        height: 1,
      }),
    );
    const manifest = new File(
      [JSON.stringify({ schema: "pmtcon-gif-frame-sheet-v2", pages })],
      "frames_manifest.json",
      { type: "application/json" },
    );
    const pngFiles = pages.map(
      (_, pageIndex) =>
        new File(["x"], `novelai-result-${pageIndex}.png`, {
          type: "image/png",
        }),
    );

    await act(async () => {
      dropFiles(dropZone!, [manifest, ...pngFiles]);
      await Promise.resolve();
    });
    await flushEffects();

    const selectors = host.querySelectorAll<HTMLSelectElement>(
      '[data-testid^="gif-frame-page-slot-select-"]',
    );
    expect(selectors).toHaveLength(1);
    expect(selectors[0]?.options.length).toBeLessThanOrEqual(
      MAX_GIF_FRAME_REIMPORT_PAGE_COUNT + 1,
    );
  });
});

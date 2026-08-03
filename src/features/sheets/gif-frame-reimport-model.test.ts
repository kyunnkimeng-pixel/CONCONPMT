import { describe, expect, it } from "vitest";

import {
  assignGifFrameFileToSlot,
  autoAssignGifFrameFiles,
  classifyGifFrameReimportFiles,
  gifFrameResultNeedsOpaqueWarning,
  mappedGifFrameFiles,
  MAX_GIF_FRAME_REIMPORT_MANIFEST_BYTES,
  MAX_GIF_FRAME_REIMPORT_PAGE_BYTES,
  MAX_GIF_FRAME_REIMPORT_PAGE_COUNT,
  MAX_GIF_FRAME_REIMPORT_TOTAL_BYTES,
  normalizedGifFramePageFileName,
  readGifFrameReimportPageSlots,
} from "@/features/sheets/gif-frame-reimport-model";

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
          {
            page_index: 1,
            clean_sheet_file: "frames_sheet_002.png",
            width: 1024,
            height: 256,
          },
        ],
      }),
    ],
    "frames_manifest.json",
    { type: "application/json" },
  );
}

function withReportedSize(file: File, size: number) {
  Object.defineProperty(file, "size", { configurable: true, value: size });
  return file;
}

describe("GIF frame sheet result file selection", () => {
  it("normalizes Chrome duplicate suffixes and PNG extension case", () => {
    expect(normalizedGifFramePageFileName("FRAMES_SHEET_001 (1).PNG")).toBe(
      "frames_sheet_001.png",
    );
    expect(normalizedGifFramePageFileName("frames_sheet_002 (1) (2).png")).toBe(
      "frames_sheet_002.png",
    );
    expect(normalizedGifFramePageFileName("result.webp")).toBe("result.png");
  });

  it("accepts PNG, JPG/JPEG and static WebP candidates but rejects animated GIF", () => {
    const png = new File(["png"], "frames_sheet_001.png", { type: "image/png" });
    const jpg = new File(["jpg"], "frames_sheet_002.jpg", { type: "image/jpeg" });
    const webp = new File(["webp"], "frames_sheet_003.webp", { type: "image/webp" });

    const accepted = classifyGifFrameReimportFiles([png, jpg, webp]);
    expect(accepted.error).toBeNull();
    expect(accepted.imageFiles).toEqual([png, jpg, webp]);
    expect(gifFrameResultNeedsOpaqueWarning(jpg)).toBe(true);
    expect(gifFrameResultNeedsOpaqueWarning(webp)).toBe(false);

    const rejected = classifyGifFrameReimportFiles([
      png,
      new File(["gif"], "animated.gif", { type: "image/gif" }),
    ]);
    expect(rejected.imageFiles).toEqual([]);
    expect(rejected.error).toContain("GIF·애니메이션 WebP는 지원하지 않습니다");
  });

  it("auto maps browser-renamed pages without pretending their bytes changed format", async () => {
    const slots = await readGifFrameReimportPageSlots(manifestFile());
    const files = [
      new File(["first"], "FRAMES_SHEET_001 (1).PNG", { type: "image/png" }),
      new File(["second"], "frames_sheet_002 (7).png", { type: "image/png" }),
    ];
    const assignments = autoAssignGifFrameFiles(slots, files);
    const mapped = mappedGifFrameFiles(slots, files, assignments);

    expect(assignments).toEqual([0, 1]);
    expect(mapped?.map((file) => file.name)).toEqual([
      "FRAMES_SHEET_001 (1).PNG",
      "frames_sheet_002 (7).png",
    ]);
  });

  it("requires explicit page-slot choices for unrelated multi-page names", async () => {
    const slots = await readGifFrameReimportPageSlots(manifestFile());
    const files = [
      new File(["first"], "redrawn-a.png", { type: "image/png" }),
      new File(["second"], "redrawn-b.png", { type: "image/png" }),
    ];
    const automatic = autoAssignGifFrameFiles(slots, files);
    const first = assignGifFrameFileToSlot(automatic, 0, 1);
    const second = assignGifFrameFileToSlot(first, 1, 0);

    expect(automatic).toEqual([null, null]);
    expect(second).toEqual([1, 0]);
    expect(mappedGifFrameFiles(slots, files, second)?.map((file) => file.name)).toEqual([
      "redrawn-b.png",
      "redrawn-a.png",
    ]);
  });

  it("rejects oversized manifest and PNG files before reading their bytes", async () => {
    const oversizedManifest = withReportedSize(
      new File([], "frames_manifest.json", { type: "application/json" }),
      MAX_GIF_FRAME_REIMPORT_MANIFEST_BYTES + 1,
    );
    const oversizedPng = withReportedSize(
      new File([], "huge-page.png", { type: "image/png" }),
      MAX_GIF_FRAME_REIMPORT_PAGE_BYTES + 1,
    );

    expect(classifyGifFrameReimportFiles([oversizedManifest]).error).toContain(
      "frames_manifest.json",
    );
    expect(classifyGifFrameReimportFiles([oversizedPng]).error).toContain(
      "huge-page.png",
    );
    await expect(readGifFrameReimportPageSlots(oversizedManifest)).rejects.toThrow(
      "최대 4MB",
    );
  });

  it("rejects aggregate payloads and excessive page counts before IPC", async () => {
    const first = withReportedSize(
      new File([], "frames_sheet_001.png", { type: "image/png" }),
      MAX_GIF_FRAME_REIMPORT_TOTAL_BYTES / 2 + 1,
    );
    const second = withReportedSize(
      new File([], "frames_sheet_002.png", { type: "image/png" }),
      MAX_GIF_FRAME_REIMPORT_TOTAL_BYTES / 2 + 1,
    );
    expect(classifyGifFrameReimportFiles([first, second]).error).toContain(
      "합계는 64MB",
    );

    const tooManyPages = new File(
      [
        JSON.stringify({
          pages: Array.from(
            { length: MAX_GIF_FRAME_REIMPORT_PAGE_COUNT + 1 },
            (_, pageIndex) => ({
              page_index: pageIndex,
              clean_sheet_file: `frames_sheet_${pageIndex}.png`,
              width: 1,
              height: 1,
            }),
          ),
        }),
      ],
      "frames_manifest.json",
      { type: "application/json" },
    );
    await expect(readGifFrameReimportPageSlots(tooManyPages)).rejects.toThrow(
      `최대 ${MAX_GIF_FRAME_REIMPORT_PAGE_COUNT}개`,
    );
  });

  it("does not auto-fill the last slot with an unrelated guide PNG", async () => {
    const slots = await readGifFrameReimportPageSlots(manifestFile());
    const files = [
      new File(["first"], "frames_sheet_001.png", { type: "image/png" }),
      new File(["guide"], "frames_guide_002.png", { type: "image/png" }),
    ];

    expect(autoAssignGifFrameFiles(slots, files)).toEqual([0, null]);
  });

  it("allows an arbitrary browser filename only for a one-page one-file result", () => {
    const slots = [
      {
        pageIndex: 0,
        expectedFileName: "frames_sheet_001.png",
        width: 1024,
        height: 1024,
      },
    ];
    const files = [new File(["page"], "novelai-result.png", { type: "image/png" })];

    expect(autoAssignGifFrameFiles(slots, files)).toEqual([0]);
  });
});

import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { IconSummary } from "@/features/collections/types";
import {
  buildGifAiWebPrompt,
  GifFrameExportPanel,
  GifFrameExportResultPanel,
  GifFrameVariantResult,
} from "@/features/sheets/components/GifFrameSheetDialog";
import { defaultGifFrameSheetSettings } from "@/features/sheets/sheet-ui-model";

describe("GifFrameSheetDialog hardened roundtrip UX", () => {
  it("offers Explorer opening after frame-sheet export", () => {
    const html = renderToString(
      <GifFrameExportPanel
        collectionId="collection_1"
        icon={{ id: "icon_1" } as IconSummary}
        settings={defaultGifFrameSheetSettings()}
        onSettingsChange={() => undefined}
      />,
    );

    expect(html).toContain("완료 후 폴더 열기");
  });

  it("offers a result-folder action after export", () => {
    const html = renderToString(
      <GifFrameExportResultPanel
        result={{
          frameSheetPaths: ["C:\\exports\\frames_sheet_001.png"],
          guideSheetPaths: ["C:\\exports\\frames_guide_001.png"],
          manifestPath: "C:\\exports\\frames_manifest.json",
          outputDirectory: "C:\\exports",
          frameCount: 4,
          pageCount: 1,
          warnings: [],
        }}
        onOpenFolder={() => undefined}
      />,
    );

    expect(html).toContain("결과 폴더 열기");
    expect(html).toContain('data-testid="gif-frame-export-open-folder"');
  });

  it("builds a PNG-only web prompt that locks character, geometry, order, alpha, timing, and loop", () => {
    const settings = {
      ...defaultGifFrameSheetSettings(96, 80),
      columns: 4,
      framesPerPage: 8,
      gapX: 6,
      gapY: 4,
      borderX: 12,
      borderY: 10,
    };
    const prompt = buildGifAiWebPrompt({
      analysis: {
        iconId: "icon_1",
        displayName: "움직이는 캐릭터",
        sourceFormat: "gif",
        frameCount: 12,
        durationMs: 960,
        loopMode: "count",
        loopCount: 3,
        pageCount: 2,
        sheetWidth: 438,
        sheetHeight: 178,
        columns: 4,
        rowsPerPage: 2,
        warnings: [],
      },
      result: {
        frameSheetPaths: [
          "C:\\exports\\frames_sheet_001.png",
          "C:\\exports\\frames_sheet_002.png",
        ],
        guideSheetPaths: [],
        manifestPath: "C:\\exports\\frames_manifest.json",
        outputDirectory: "C:\\exports",
        frameCount: 12,
        pageCount: 2,
        warnings: [],
      },
      settings,
    });

    expect(prompt).toContain("모든 페이지와 모든 프레임에서 캐릭터");
    expect(prompt).toContain("각 PNG 캔버스 438×178px");
    expect(prompt).toContain("셀 96×80px");
    expect(prompt).toContain("row-major 셀 순서");
    expect(prompt).toContain("픽셀별 alpha");
    expect(prompt).toContain("PNG만 반환하세요");
    expect(prompt).toContain("총 재생시간 960ms");
    expect(prompt).toContain("3회 반복");
    expect(prompt).toContain("manifest에서 복원됩니다");
  });

  it("offers copy-and-open choices and a direct reimport continuation after AI export", () => {
    const html = renderToString(
      <GifFrameExportResultPanel
        aiWebPrompt="PNG만 반환하세요."
        result={{
          frameSheetPaths: ["C:\\exports\\frames_sheet_001.png"],
          guideSheetPaths: ["C:\\exports\\frames_guide_001.png"],
          manifestPath: "C:\\exports\\frames_manifest.json",
          outputDirectory: "C:\\exports",
          frameCount: 4,
          pageCount: 1,
          warnings: [],
        }}
        onContinueToReimport={() => undefined}
        onOpenAiSite={async () => undefined}
        onOpenFolder={() => undefined}
      />,
    );

    expect(html).toContain("프롬프트 복사 + Gemini AI Studio 열기");
    expect(html).toContain("프롬프트 복사 + NovelAI 열기");
    expect(html).toContain("수정 PNG를 받았어요 · 다시 가져오기");
    expect(html).toContain("원본 GIF와 frame timing·loop는 바뀌지 않습니다.");
  });
  it("previews the rebuilt GIF as an export-only variant", () => {
    const html = renderToString(
      <GifFrameVariantResult
        result={{
          variantId: "variant_1",
          outputPath: "asset://localhost/rebuilt.gif",
          frameCount: 4,
          durationMs: 260,
          activeVariantSet: false,
          warnings: [],
          errors: [],
        }}
        onOpenPath={() => undefined}
      />,
    );

    expect(html).toContain("재조립한 GIF 처리 버전 미리보기");
    expect(html).toContain("원본 GIF와 현재 편집 소스는 그대로 유지됩니다.");
    expect(html).toContain("결과 GIF 위치 열기");
  });
});

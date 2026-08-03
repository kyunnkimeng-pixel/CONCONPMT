import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { IconSummary } from "@/features/collections/types";
import {
  buildGifAiWebPrompt,
  buildGifAiWebPromptWithUserRequest,
  buildNovelAiGifWebPrompt,
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

  it("builds an edit-only web prompt that locks character, geometry, order, alpha, timing, and loop", () => {
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
        pages: [
          { pageIndex: 0, itemCount: 8, width: 426, height: 184 },
          { pageIndex: 1, itemCount: 4, width: 426, height: 100 },
        ],
        sheetWidth: 426,
        sheetHeight: 184,
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

    expect(prompt).toContain("캐릭터를 재해석하거나 새 디자인으로 다시 그리지 마세요");
    expect(prompt).toContain("frames_sheet_001.png=426×184px");
    expect(prompt).toContain("frames_sheet_002.png=426×100px");
    expect(prompt).toContain("셀 96×80px");
    expect(prompt).toContain("row-major 셀 순서");
    expect(prompt).toContain("픽셀별 alpha");
    expect(prompt).toContain("JPG는 투명도를 보존할 수 없습니다");
    expect(prompt).toContain("총 재생시간 960ms");
    expect(prompt).toContain("3회 반복");
    expect(prompt).toContain("manifest를 AI에 올릴 필요도");
    expect(prompt).not.toContain("같은 파일명의 PNG");
    expect(prompt).not.toContain("파일 수와 파일명을");
    expect(prompt).toContain("guide와 manifest는 절대 첨부하지 마세요");
    expect(prompt).toContain("요청한 부분만 편집하세요");
  });

  it("builds a compact NovelAI GIF tag prompt and appends user tags before its short structure rules", () => {
    const settings = {
      ...defaultGifFrameSheetSettings(200, 200),
      columns: 4,
      framesPerPage: 16,
      gapX: 56,
      gapY: 56,
      borderX: 28,
      borderY: 28,
    };
    const analysis = {
      iconId: "icon_1",
      displayName: "움직이는 캐릭터",
      sourceFormat: "gif",
      frameCount: 17,
      durationMs: 1200,
      loopMode: "infinite",
      loopCount: null,
      pageCount: 2,
      pages: [
        { pageIndex: 0, itemCount: 16, width: 1024, height: 1024 },
        { pageIndex: 1, itemCount: 1, width: 1024, height: 256 },
      ],
      sheetWidth: 1024,
      sheetHeight: 1024,
      columns: 4,
      rowsPerPage: 4,
      warnings: [],
    };
    const result = {
      frameSheetPaths: [
        "C:\\exports\\frames_sheet_001.png",
        "C:\\exports\\frames_sheet_002.png",
      ],
      guideSheetPaths: [],
      manifestPath: "C:\\exports\\frames_manifest.json",
      outputDirectory: "C:\\exports",
      frameCount: 17,
      pageCount: 2,
      warnings: [],
    };
    const basePrompt = buildNovelAiGifWebPrompt({ analysis, result, settings });
    const finalPrompt = buildGifAiWebPromptWithUserRequest(
      basePrompt,
      "Wavy MOTION; shifting COLORS",
      "novelai_app",
    );

    expect(basePrompt.split("\n")).toHaveLength(3);
    expect(basePrompt).toContain("animated emoticon, frame sequence");
    expect(basePrompt).toContain("frames_sheet_001.png=1024×1024px");
    expect(basePrompt).toContain("frames_sheet_002.png=1024×256px");
    expect(basePrompt).toContain("Process one page at a time");
    expect(basePrompt).toContain("Return only the current page");
    expect(basePrompt).not.toContain("original filenames unchanged");
    expect(basePrompt).not.toContain("exact filenames");
    expect(finalPrompt.split("\n")[0]).toContain(
      "wavy motion, shifting colors",
    );
    expect(finalPrompt).not.toContain("사용자 편집 요청:");
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

    expect(html).toContain("웹 서비스");
    expect(html).toContain("Gemini AI Studio");
    expect(html).toContain("NovelAI");
    expect(html).toContain("원하는 GIF 수정");
    expect(html).toContain('data-testid="gif-ai-open-selected"');
    expect(html).toContain("결과 이미지를 받았어요 · 다시 가져오기");
    expect(html).toContain("원본 GIF와 frame timing·loop는 바뀌지 않습니다.");
    expect(html).toContain("AI 업로드 대상 · frames_sheet");
    expect(html).toContain("사람 확인용 · AI 업로드 금지 · frames_guide");
    expect(html).toContain("앱 복원용 · AI 업로드 금지 · manifest");
    expect(html).toContain("이 페이지 끌기");
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

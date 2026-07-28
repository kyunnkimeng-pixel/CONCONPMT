import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { IconSummary } from "@/features/collections/types";
import {
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

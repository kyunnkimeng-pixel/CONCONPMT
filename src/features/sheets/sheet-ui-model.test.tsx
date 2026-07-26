import { describe, expect, it } from "vitest";
import { renderToString } from "react-dom/server";

import { SheetImagePicker } from "@/features/sheets/components/SheetImagePicker";
import { SheetExportPreview } from "@/features/sheets/components/SheetExportPreview";
import { SheetExportDialog } from "@/features/sheets/components/SheetExportDialog";
import { SheetAutoDetectPanel } from "@/features/sheets/components/SheetAutoDetectPanel";
import { FrameSheetToGifDialog } from "@/features/sheets/components/FrameSheetToGifDialog";
import { SheetGridSettingsPanel } from "@/features/sheets/components/SheetGridSettingsPanel";
import { SheetReviewButton } from "@/features/sheets/components/SheetImportWizard";
import { ManualSliceCanvas } from "@/features/sheets/components/ManualSliceCanvas";
import { SheetReimportDialog } from "@/features/sheets/components/SheetReimportDialog";
import {
  applyPresetToExportRequest,
  applyPresetToGifFrameSettings,
  applyPresetToImportSettings,
  defaultExportSheetRequest,
  defaultGifFrameSheetSettings,
  estimateGifFrameSheetPages,
  defaultSheetGridSettings,
  estimateSheetPages,
  includedNonEmptyCellIndexes,
  isGifIcon,
  nextSelectionAfterCellClick,
} from "@/features/sheets/sheet-ui-model";
import type { IconSummary } from "@/features/collections/types";
import type { CollectionSummary } from "@/features/collections/types";
import type { SheetCell, SheetGridPreset } from "@/features/sheets/types";

describe("sheet-ui-model", () => {
  it("updates include selection without auto-selecting unrelated cells", () => {
    const selected = nextSelectionAfterCellClick(new Set([1]), 2, { multi: true });

    expect([...selected].sort()).toEqual([1, 2]);
    expect([...nextSelectionAfterCellClick(selected, 1, { multi: true })]).toEqual([2]);
    expect([...nextSelectionAfterCellClick(selected, 5, { multi: false })]).toEqual([5]);
  });

  it("excludes empty and out-of-bounds cells from import target list", () => {
    const cells: SheetCell[] = [
      cell(0, false, false),
      cell(1, true, false),
      cell(2, false, true),
    ];

    expect(includedNonEmptyCellIndexes(cells, new Set([0, 1, 2]))).toEqual([0]);
  });

  it("estimates page count using max sheet size", () => {
    const request = {
      ...defaultExportSheetRequest("collection_1"),
      cellWidth: 200,
      cellHeight: 200,
      columns: 8,
      gapX: 8,
      gapY: 8,
      borderX: 16,
      borderY: 16,
      maxSheetWidth: 2048,
      maxSheetHeight: 240,
    };

    expect(estimateSheetPages(10, request)).toBe(2);
  });

  it("renders the export preview summary", () => {
    const html = renderToString(
      <SheetExportPreview itemCount={12} request={defaultExportSheetRequest("collection_1")} />,
    );

    expect(html).toContain("시트 예상");
    expect(defaultSheetGridSettings().emptyCellThreshold).toBe(0.98);
  });

  it("names the grid reset action after the settings it affects", () => {
    const html = renderToString(
      <SheetGridSettingsPanel
        settings={defaultSheetGridSettings()}
        onChange={() => {}}
        onPreview={() => {}}
        onReset={() => {}}
      />,
    );

    expect(html).toContain("분할 설정 초기값");
    expect(html).not.toContain(">Reset<");
  });

  it("keeps cell review disabled until a fresh grid analysis exists", () => {
    const unavailable = renderToString(
      <SheetReviewButton hasAnalysis={false} onReview={() => {}} />,
    );
    const available = renderToString(
      <SheetReviewButton hasAnalysis onReview={() => {}} />,
    );

    expect(unavailable).toContain('disabled=""');
    expect(unavailable).toContain("분할 미리보기를 먼저 갱신하세요.");
    expect(available).not.toContain('disabled=""');
  });

  it("warns when a JPG sheet cannot preserve alpha", () => {
    const html = renderToString(
      <SheetImagePicker
        file={{ name: "source-sheet.jpg", type: "image/jpeg" } as File}
        onFileChange={() => {}}
      />,
    );

    expect(html).toContain("JPG/JPEG");
    expect(html).toContain("alpha");
    expect(html).toContain("드래그");
  });

  it("renders the complete frame-sheet-to-GIF workflow without a dead action", () => {
    const html = renderToString(
      <FrameSheetToGifDialog
        collection={collection()}
        onClose={() => {}}
        onCreated={async () => {}}
      />,
    );

    expect(html).toContain("프레임 시트로 새 GIF 만들기");
    expect(html).toContain("프레임 스트립");
    expect(html).toContain("GIF 미리 생성 및 용량 측정");
    expect(html).toContain("새 GIF 아이콘 만들기");
    expect(html).toContain("핑퐁");
    expect(html).toContain("FPS");
  });

  it("renders drag-drop guidance for manifest reimport", () => {
    const html = renderToString(
      <SheetReimportDialog collectionId="collection_1" onImported={async () => {}} />,
    );

    expect(html).toContain("Manifest JSON");
    expect(html).toContain("드래그");
  });

  it("renders the manual slice production surface", () => {
    const html = renderToString(
      <ManualSliceCanvas
        collection={collection()}
        file={{ name: "manual-sheet.png", size: 1280, type: "image/png" } as File}
        imageUrl={null}
        onImported={async () => {}}
      />,
    );

    expect(html).toContain("직접 Slice 지정");
    expect(html).toContain("Slice 추가");
    expect(html).toContain("metadata 저장");
  });

  it("renders auto-detect proposals as reviewable grid settings", () => {
    const html = renderToString(
      <SheetAutoDetectPanel
        errorMessage={null}
        file={{ name: "auto-sheet.png", size: 1280, type: "image/png" } as File}
        isRunning={false}
        result={{
          sheetWidth: 424,
          sheetHeight: 216,
          hasAlpha: true,
          warnings: ["자동 감지는 실험 기능입니다."],
          proposals: [
            {
              id: "alpha_2x2",
              label: "투명 여백 감지",
              method: "alpha",
              confidence: "high",
              confidenceScore: 0.92,
              computedRows: 2,
              computedColumns: 2,
              cellCount: 4,
              warnings: [],
              gridSettings: {
                ...defaultSheetGridSettings(),
                rows: 2,
                columns: 2,
                cellWidth: 200,
                cellHeight: 200,
                borderLeft: 8,
                borderTop: 8,
                borderRight: 8,
                borderBottom: 8,
                gapX: 8,
                gapY: 8,
              },
            },
          ],
        }}
        onApplyProposal={() => {}}
        onRun={() => {}}
      />,
    );

    expect(html).toContain("자동 감지 제안");
    expect(html).toContain("confidence");
    expect(html).toContain("high");
    expect(html).toContain("200x200");
  });

  it("estimates GIF frame sheet pages using frames-per-page and max size", () => {
    const settings = {
      ...defaultGifFrameSheetSettings(200, 200),
      columns: 8,
      framesPerPage: 64,
      maxSheetHeight: 2048,
    };

    expect(estimateGifFrameSheetPages(65, settings)).toBe(2);
  });

  it("detects GIF icons so frame sheet actions are gated", () => {
    expect(isGifIcon(iconWithPreview("C:/library/source.gif"))).toBe(true);
    expect(isGifIcon(iconWithPreview("C:/library/source.png"))).toBe(false);
  });

  it("applies one shared grid preset to import, static export, and GIF frame export", () => {
    const preset = gridPreset({
      cellWidth: 128,
      cellHeight: 128,
      columns: 4,
      gapX: 6,
      gapY: 7,
      borderLeft: 12,
      borderTop: 14,
      borderRight: 12,
      borderBottom: 14,
      framesPerPage: 32,
    });

    expect(applyPresetToImportSettings(defaultSheetGridSettings(), preset)).toMatchObject({
      mode: "cell_size",
      cellWidth: 128,
      cellHeight: 128,
      rows: null,
      columns: 4,
      gapX: 6,
      gapY: 7,
      borderLeft: 12,
      borderTop: 14,
    });
    expect(applyPresetToExportRequest(defaultExportSheetRequest("collection_1"), preset)).toMatchObject({
      cellWidth: 128,
      cellHeight: 128,
      columns: 4,
      gapX: 6,
      gapY: 7,
      borderX: 12,
      borderY: 14,
    });
    expect(applyPresetToGifFrameSettings(defaultGifFrameSheetSettings(), preset)).toMatchObject({
      frameCellWidth: 128,
      frameCellHeight: 128,
      columns: 4,
      framesPerPage: 32,
      gapX: 6,
      gapY: 7,
    });
  });

  it("renders selected-icon sheet export scope", () => {
    const html = renderToString(
      <SheetExportDialog
        collection={collection()}
        icons={[iconWithPreview("C:/library/a.png"), { ...iconWithPreview("C:/library/b.gif"), id: "icon_2" }]}
        selectedIconIds={["icon_2"]}
        onClose={() => {}}
      />,
    );

    expect(html).toContain("선택한 1개 아이콘");
    expect(html).toContain("GIF는 첫 프레임만 작업 시트에 포함됩니다");
  });
});

function cell(index: number, emptyCandidate: boolean, outOfBounds: boolean): SheetCell {
  return {
    index,
    page: 0,
    row: index,
    col: 0,
    x: 0,
    y: index * 10,
    w: 10,
    h: 10,
    emptyCandidate,
    outOfBounds,
  };
}

function iconWithPreview(currentPreviewUrl: string): IconSummary {
  return {
    id: "icon_1",
    collectionId: "collection_1",
    sourceFileId: "source_1",
    displayName: "icon",
    note: null,
    iconKind: "image",
    readiness: "working",
    placeholderText: null,
    shape: "single",
    orderIndex: 0,
    cellWidthOverride: null,
    cellHeightOverride: null,
    thumbnailUrl: null,
    thumbnailOverrideUrl: null,
    currentPreviewUrl,
    gifLoopMode: "preserve",
    gifLoopCount: null,
    transformQuarterTurns: 0,
    transformFlipHorizontal: false,
    transformFlipVertical: false,
    createdAt: "now",
    updatedAt: "now",
    pieces: [],
  };
}

function collection(): CollectionSummary {
  return {
    id: "collection_1",
    name: "QA",
    coverSourceFileId: null,
    coverIconId: null,
    coverImageUrl: null,
    iconCount: 2,
    defaultCellWidth: 200,
    defaultCellHeight: 200,
    previewWidth: 100,
    previewHeight: 100,
    exportFormat: "png",
    maxBytes: 2_097_152,
    createdAt: "now",
    updatedAt: "now",
  };
}

function gridPreset(overrides: Partial<SheetGridPreset> = {}): SheetGridPreset {
  return {
    id: "preset_1",
    name: "QA preset",
    scope: "collection",
    collectionId: "collection_1",
    kind: "static_import_export",
    cellWidth: 200,
    cellHeight: 200,
    rows: null,
    columns: 5,
    mode: "rows_columns",
    gapX: 8,
    gapY: 8,
    borderLeft: 16,
    borderTop: 16,
    borderRight: 16,
    borderBottom: 16,
    readOrder: "row_major",
    background: "transparent",
    maxSheetWidth: 2048,
    maxSheetHeight: 2048,
    framesPerPage: null,
    includeCleanSheet: true,
    includeGuideSheet: true,
    includeManifest: true,
    guideLabelOptionsJson:
      '{"cellNumber":true,"iconName":true,"altValue":true,"exportNumber":true}',
    isDefaultForImport: false,
    isDefaultForExport: false,
    isDefaultForGifFrame: false,
    isBuiltin: false,
    createdAt: "now",
    updatedAt: "now",
    ...overrides,
  };
}

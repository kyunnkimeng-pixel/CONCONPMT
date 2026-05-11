import { describe, expect, it } from "vitest";

import {
  filterExportItems,
  problemExportNumbers,
  summarizeExportWorkspace,
} from "@/features/export/export-workspace-model";
import type {
  ExportPlanItem,
  ExportProfile,
  ExportValidationResult,
} from "@/features/export/types";

describe("export workspace model", () => {
  it("summarizes partial-success exports", () => {
    const result = validationResult([
      item({ pieceId: "ok", status: "written_ok", byteSize: 1200 }),
      item({
        pieceId: "oversized",
        fileName: "002.gif",
        outputFormat: "gif",
        status: "written_not_upload_ready",
        byteSize: 3_000_000,
      }),
      item({ pieceId: "failed", fileName: "003.png", status: "failed_to_render" }),
      item({ pieceId: "excluded", included: false, status: "excluded" }),
    ]);
    result.errors.push({
      blocking: false,
      code: "max_bytes",
      iconId: "icon-oversized",
      message: "002.gif exceeds the limit",
      pieceId: "oversized",
      severity: "error",
    });

    expect(summarizeExportWorkspace(result)).toMatchObject({
      included: 3,
      excluded: 1,
      success: 2,
      notUploadReady: 1,
      failed: 1,
      oversized: 1,
    });
  });

  it("filters included, excluded, failed, gif, and oversized items", () => {
    const result = validationResult([
      item({ pieceId: "ok", status: "written_ok" }),
      item({
        pieceId: "gif",
        fileName: "002.gif",
        outputFormat: "gif",
        status: "written_not_upload_ready",
      }),
      item({ pieceId: "failed", fileName: "003.png", status: "failed_to_render" }),
      item({ pieceId: "excluded", included: false, status: "excluded" }),
    ]);
    result.errors.push({
      blocking: false,
      code: "max_bytes",
      iconId: "icon-gif",
      message: "002.gif exceeds the limit",
      pieceId: "gif",
      severity: "error",
    });

    expect(filterExportItems(result, "included").map((next) => next.pieceId)).toEqual([
      "ok",
      "gif",
      "failed",
    ]);
    expect(filterExportItems(result, "excluded").map((next) => next.pieceId)).toEqual([
      "excluded",
    ]);
    expect(filterExportItems(result, "failed").map((next) => next.pieceId)).toEqual([
      "failed",
    ]);
    expect(filterExportItems(result, "gif").map((next) => next.pieceId)).toEqual([
      "gif",
    ]);
    expect(filterExportItems(result, "oversized").map((next) => next.pieceId)).toEqual([
      "gif",
    ]);
  });

  it("returns problem export numbers without excluded items", () => {
    const result = validationResult([
      item({ pieceId: "ok", exportIndex: 1, status: "written_ok" }),
      item({ pieceId: "warn", exportIndex: 2, status: "written_with_warning" }),
      item({ pieceId: "failed", exportIndex: 3, status: "failed_to_render" }),
      item({ pieceId: "excluded", exportIndex: 0, included: false, status: "excluded" }),
    ]);

    expect(problemExportNumbers(result)).toEqual(["002", "003"]);
  });
});

function validationResult(items: ExportPlanItem[]): ExportValidationResult {
  return {
    canExport: true,
    errors: [],
    items,
    outputCount: items.filter((next) => next.included).length,
    profile,
    warnings: [],
  };
}

function item(overrides: Partial<ExportPlanItem> = {}): ExportPlanItem {
  const pieceId = overrides.pieceId ?? "piece-1";

  return {
    altText: "가",
    byteSize: null,
    displayName: "아이콘",
    exportIndex: 1,
    exportPath: null,
    fileName: "001.png",
    height: 200,
    iconId: `icon-${pieceId}`,
    included: true,
    isAnimated: overrides.outputFormat === "gif",
    limitBytes: 2_097_152,
    outputFormat: "png",
    pieceId,
    pieceRole: "single",
    sourcePreviewUrl: "asset://preview.png",
    status: "preflight_ok",
    width: 200,
    ...overrides,
  };
}

const profile: ExportProfile = {
  allowedFormats: ["jpg", "png", "gif"],
  collectionId: "collection",
  createdAt: "2026-05-11T00:00:00.000Z",
  filenameMode: "sequence",
  id: "profile",
  includeAltTxt: true,
  maxBytes: 2_097_152,
  name: "DCInside",
  previewHeight: 100,
  previewWidth: 100,
  profileType: "dcinside",
  strictWarnings: false,
  targetCellHeight: 200,
  targetCellWidth: 200,
  targetFormat: "png",
  updatedAt: "2026-05-11T00:00:00.000Z",
};

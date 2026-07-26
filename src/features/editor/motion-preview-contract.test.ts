import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import {
  MOTION_PREVIEW_LOOP_MODES,
  MOTION_TIMING_SOURCES,
  type MotionPreviewLoopMode,
} from "@/features/editor/types";

const backendMotionEditorSource = readFileSync(
  new URL(
    "../../../src-tauri/src/db/repositories/motion_editor.rs",
    import.meta.url,
  ),
  "utf8",
);
const backendModelsSource = readFileSync(
  new URL("../../../src-tauri/src/models.rs", import.meta.url),
  "utf8",
);

describe("motion preview TypeScript/Rust contract", () => {
  it("keeps timing and effective loop values aligned with Rust output", () => {
    expect(MOTION_TIMING_SOURCES).toEqual(["source_gif", "generated"]);
    expect(MOTION_PREVIEW_LOOP_MODES).toEqual([
      "once",
      "infinite",
      "count",
      "pingpong",
    ]);
    const loopCountByMode = {
      once: null,
      infinite: null,
      count: 2,
      pingpong: null,
    } satisfies Record<MotionPreviewLoopMode, number | null>;
    expect(loopCountByMode.count).toBeGreaterThan(0);

    const dtoBuilder = sourceSection(
      backendMotionEditorSource,
      "Ok(MotionPreviewDto {",
      "fn effective_repeat_metadata",
    );
    for (const timingSource of MOTION_TIMING_SOURCES) {
      expect(dtoBuilder).toContain(`"${timingSource}".to_string()`);
    }

    const repeatMetadata = sourceSection(
      backendMotionEditorSource,
      "fn effective_repeat_metadata",
      "fn update_persisted_preview",
    );
    for (const loopMode of MOTION_PREVIEW_LOOP_MODES) {
      expect(repeatMetadata).toContain(`"${loopMode}".to_string()`);
    }
    expect(repeatMetadata).not.toContain('"preserve".to_string()');
  });

  it("keeps camelCase DTO field names backed by the Rust serializer", () => {
    const dtoModel = sourceSection(
      backendModelsSource,
      "pub struct MotionPreviewDto",
      "pub struct ImportImageFilePayload",
    );

    expect(backendModelsSource).toContain('#[serde(rename_all = "camelCase")]');
    expect(dtoModel).toContain("pub timing_source: String");
    expect(dtoModel).toContain("pub loop_mode: String");
    expect(dtoModel).toContain("pub loop_count: Option<i64>");
  });
});

function sourceSection(source: string, start: string, end: string) {
  return source.slice(source.indexOf(start), source.indexOf(end));
}

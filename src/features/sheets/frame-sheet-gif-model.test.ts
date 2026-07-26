import { describe, expect, it } from "vitest";

import {
  applyDurationToSelected,
  createFramesFromCells,
  deleteSelectedFrames,
  duplicateSelectedFrames,
  frameSheetGifSignature,
  materializeFrames,
  MAX_FRAME_DURATION_MS,
  MIN_FRAME_DURATION_MS,
  moveFrame,
  normalizeDurationMs,
  reverseSelectedFrames,
  selectFrameIds,
  totalFrameDuration,
  updateFrameDuration,
  type FrameSheetGifSignatureInput,
  type FrameStripItem,
} from "./frame-sheet-gif-model";
import type { SheetGridSettings } from "./types";

describe("frame-sheet-gif-model", () => {
  it("creates stable frames from reviewed cells in caller order", () => {
    expect(createFramesFromCells([{ index: 4 }, { index: 1 }, 4, 2], 96)).toEqual([
      { id: "frame-cell-4", sourceCellIndex: 4, durationMs: 100 },
      { id: "frame-cell-1", sourceCellIndex: 1, durationMs: 100 },
      { id: "frame-cell-2", sourceCellIndex: 2, durationMs: 100 },
    ]);
  });

  it("filters analyzed cells by reviewed selection without using Set insertion order", () => {
    expect(
      createFramesFromCells(
        [{ index: 4 }, { index: 1 }, { index: 2 }],
        new Set([2, 4]),
        75,
      ),
    ).toEqual([
      { id: "frame-cell-4", sourceCellIndex: 4, durationMs: 80 },
      { id: "frame-cell-2", sourceCellIndex: 2, durationMs: 80 },
    ]);
  });

  it("rejects invalid source cell indexes instead of creating unstable IDs", () => {
    expect(() => createFramesFromCells([-1])).toThrow(RangeError);
    expect(() => createFramesFromCells([1.5])).toThrow(RangeError);
  });

  it("moves one frame using dnd-kit arrayMove semantics", () => {
    const frames = makeFrames([0, 1, 2, 3]);

    expect(ids(moveFrame(frames, "frame-cell-0", "frame-cell-2"))).toEqual([
      "frame-cell-1",
      "frame-cell-2",
      "frame-cell-0",
      "frame-cell-3",
    ]);
    expect(ids(moveFrame(frames, "frame-cell-3", "frame-cell-1"))).toEqual([
      "frame-cell-0",
      "frame-cell-3",
      "frame-cell-1",
      "frame-cell-2",
    ]);
    expect(moveFrame(frames, "missing", "frame-cell-1")).toBe(frames);
  });

  it("supports single, Ctrl/Cmd toggle, Shift range, and additive range selection", () => {
    const orderedIds = ids(makeFrames([0, 1, 2, 3, 4]));
    const single = selectFrameIds(
      { selectedIds: [], anchorId: null },
      orderedIds,
      orderedIds[1],
    );
    expect(single).toEqual({
      selectedIds: [orderedIds[1]],
      anchorId: orderedIds[1],
    });

    const toggled = selectFrameIds(single, orderedIds, orderedIds[4], {
      ctrlKey: true,
    });
    expect(toggled.selectedIds).toEqual([orderedIds[1], orderedIds[4]]);
    expect(
      selectFrameIds(toggled, orderedIds, orderedIds[1], { metaKey: true })
        .selectedIds,
    ).toEqual([orderedIds[4]]);

    const range = selectFrameIds(single, orderedIds, orderedIds[3], {
      shiftKey: true,
    });
    expect(range).toEqual({
      selectedIds: orderedIds.slice(1, 4),
      anchorId: orderedIds[1],
    });

    const additiveRange = selectFrameIds(
      {
        selectedIds: [orderedIds[0], orderedIds[1]],
        anchorId: orderedIds[1],
      },
      orderedIds,
      orderedIds[3],
      { ctrlKey: true, shiftKey: true },
    );
    expect(additiveRange.selectedIds).toEqual(orderedIds.slice(0, 4));
  });

  it("keeps selection unchanged when the target is not in the strip", () => {
    const selection = {
      selectedIds: ["frame-cell-0"],
      anchorId: "frame-cell-0",
    };
    expect(
      selectFrameIds(selection, ["frame-cell-0"], "missing", { shiftKey: true }),
    ).toBe(selection);
  });

  it("duplicates every selected frame beside its source with stable unique IDs", () => {
    const firstPass = duplicateSelectedFrames(makeFrames([0, 1, 2]), [
      "frame-cell-0",
      "frame-cell-2",
    ]);
    expect(firstPass).toEqual([
      { id: "frame-cell-0", sourceCellIndex: 0, durationMs: 100 },
      { id: "frame-cell-0-copy", sourceCellIndex: 0, durationMs: 100 },
      { id: "frame-cell-1", sourceCellIndex: 1, durationMs: 100 },
      { id: "frame-cell-2", sourceCellIndex: 2, durationMs: 100 },
      { id: "frame-cell-2-copy", sourceCellIndex: 2, durationMs: 100 },
    ]);

    const secondPass = duplicateSelectedFrames(firstPass, ["frame-cell-0"]);
    expect(ids(secondPass).slice(0, 3)).toEqual([
      "frame-cell-0",
      "frame-cell-0-copy-2",
      "frame-cell-0-copy",
    ]);
  });

  it("deletes selected frames without mutating or re-identifying survivors", () => {
    const frames = makeFrames([0, 1, 2]);
    const next = deleteSelectedFrames(frames, [
      "frame-cell-0",
      "frame-cell-2",
    ]);
    expect(next).toEqual([frames[1]]);
    expect(deleteSelectedFrames(frames, ["missing"])).toBe(frames);
  });

  it("reverses only selected positions, including a non-contiguous selection", () => {
    const frames = makeFrames([0, 1, 2, 3, 4]);
    expect(
      ids(
        reverseSelectedFrames(frames, [
          "frame-cell-1",
          "frame-cell-3",
          "frame-cell-4",
        ]),
      ),
    ).toEqual([
      "frame-cell-0",
      "frame-cell-4",
      "frame-cell-2",
      "frame-cell-3",
      "frame-cell-1",
    ]);
    expect(reverseSelectedFrames(frames, ["frame-cell-1"])).toBe(frames);
  });

  it("normalizes individual and batch duration edits to GIF 10ms units", () => {
    const frames = makeFrames([0, 1, 2]);
    const individual = updateFrameDuration(frames, "frame-cell-1", 125);
    expect(individual.map((frame) => frame.durationMs)).toEqual([100, 130, 100]);
    expect(frames[1].durationMs).toBe(100);

    const batch = applyDurationToSelected(
      individual,
      ["frame-cell-0", "frame-cell-2"],
      84,
    );
    expect(batch.map((frame) => frame.durationMs)).toEqual([80, 130, 80]);
    expect(updateFrameDuration(batch, "missing", 500)).toBe(batch);
    expect(applyDurationToSelected(batch, [], 500)).toBe(batch);
  });

  it("bounds duration and uses a safe fallback for non-finite input", () => {
    expect(normalizeDurationMs(1)).toBe(MIN_FRAME_DURATION_MS);
    expect(normalizeDurationMs(14)).toBe(10);
    expect(normalizeDurationMs(15)).toBe(20);
    expect(normalizeDurationMs(60_001)).toBe(MAX_FRAME_DURATION_MS);
    expect(normalizeDurationMs(Number.NaN)).toBe(100);
    expect(normalizeDurationMs(Number.POSITIVE_INFINITY, 245)).toBe(250);
    expect(normalizeDurationMs(Number.NaN, Number.NaN)).toBe(100);
  });

  it("materializes forward, reverse, and endpoint-safe ping-pong sequences", () => {
    const frames = makeFrames([0, 1, 2, 3]);
    expect(ids(materializeFrames(frames, "forward"))).toEqual(ids(frames));
    expect(ids(materializeFrames(frames, "reverse"))).toEqual([
      "frame-cell-3",
      "frame-cell-2",
      "frame-cell-1",
      "frame-cell-0",
    ]);
    expect(ids(materializeFrames(frames, "pingpong"))).toEqual([
      "frame-cell-0",
      "frame-cell-1",
      "frame-cell-2",
      "frame-cell-3",
      "frame-cell-2",
      "frame-cell-1",
    ]);
    expect(ids(materializeFrames(makeFrames([0, 1]), "pingpong"))).toEqual([
      "frame-cell-0",
      "frame-cell-1",
    ]);
    expect(ids(materializeFrames(makeFrames([0, 1, 2]), "pingpong", "once"))).toEqual([
      "frame-cell-0",
      "frame-cell-1",
      "frame-cell-2",
      "frame-cell-1",
      "frame-cell-0",
    ]);
  });

  it("computes normalized total duration from the generated sequence", () => {
    const frames: FrameStripItem[] = [
      { id: "a", sourceCellIndex: 0, durationMs: 84 },
      { id: "b", sourceCellIndex: 1, durationMs: 125 },
      { id: "c", sourceCellIndex: 2, durationMs: 201 },
      { id: "d", sourceCellIndex: 3, durationMs: 96 },
    ];
    expect(totalFrameDuration(frames)).toBe(510);
    expect(totalFrameDuration(frames, "reverse")).toBe(510);
    expect(totalFrameDuration(frames, "pingpong")).toBe(840);
    expect(totalFrameDuration(frames, "pingpong", "once")).toBe(920);
  });

  it("creates the same signature for render-equivalent draft IDs and durations", () => {
    const first = signatureInput();
    const second: FrameSheetGifSignatureInput = {
      ...first,
      frames: first.frames.map((frame, index) => ({
        ...frame,
        id: `replacement-${index}`,
        durationMs: frame.durationMs + (index === 0 ? 4 : 0),
      })),
    };
    expect(frameSheetGifSignature(second)).toBe(frameSheetGifSignature(first));
  });

  it("invalidates the signature for every render-affecting recipe change", () => {
    const base = signatureInput();
    const signature = frameSheetGifSignature(base);
    const changedInputs: FrameSheetGifSignatureInput[] = [
      { ...base, sourceKey: "sheet-b" },
      {
        ...base,
        gridSettings: { ...base.gridSettings, gapX: 1 },
      },
      {
        ...base,
        frames: [...base.frames].reverse(),
      },
      {
        ...base,
        frames: updateFrameDuration(base.frames, base.frames[0].id, 180),
      },
      { ...base, direction: "reverse" },
      { ...base, loopMode: "infinite" },
      { ...base, loopCount: 4 },
      { ...base, maxBytes: 1_000_000 },
    ];

    for (const changed of changedInputs) {
      expect(frameSheetGifSignature(changed)).not.toBe(signature);
    }
  });

  it("ignores loop count when the selected loop mode does not encode it", () => {
    const base = { ...signatureInput(), loopMode: "infinite" as const };
    expect(
      frameSheetGifSignature({ ...base, loopCount: 2 }),
    ).toBe(frameSheetGifSignature({ ...base, loopCount: 99 }));
  });
});

function makeFrames(indexes: number[]) {
  return createFramesFromCells(indexes);
}

function ids(frames: readonly FrameStripItem[]) {
  return frames.map((frame) => frame.id);
}

function signatureInput(): FrameSheetGifSignatureInput {
  return {
    sourceKey: "sheet-a",
    gridSettings: gridSettings(),
    frames: createFramesFromCells([0, 1], 84),
    direction: "forward",
    loopMode: "count",
    loopCount: 3,
    maxBytes: 2_097_152,
  };
}

function gridSettings(): SheetGridSettings {
  return {
    mode: "rows_columns",
    rows: 1,
    columns: 2,
    cellWidth: 200,
    cellHeight: 200,
    borderLeft: 0,
    borderTop: 0,
    borderRight: 0,
    borderBottom: 0,
    gapX: 0,
    gapY: 0,
    readOrder: "row_major",
    emptyCellThreshold: 0.98,
  };
}

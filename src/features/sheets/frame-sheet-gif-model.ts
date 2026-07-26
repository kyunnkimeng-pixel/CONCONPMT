import type {
  FrameSheetGifDirection,
  FrameSheetGifLoopMode,
  SheetCell,
  SheetGridSettings,
} from "./types";

export const DEFAULT_FRAME_DURATION_MS = 100;
export const MIN_FRAME_DURATION_MS = 10;
export const MAX_FRAME_DURATION_MS = 60_000;

export interface FrameStripItem {
  id: string;
  sourceCellIndex: number;
  durationMs: number;
}

export interface FrameSelectionState {
  selectedIds: string[];
  anchorId: string | null;
}

export interface FrameSelectionModifiers {
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
}

export interface FrameSheetGifSignatureInput {
  sourceKey: string;
  gridSettings: SheetGridSettings;
  frames: readonly FrameStripItem[];
  direction: FrameSheetGifDirection;
  loopMode: FrameSheetGifLoopMode;
  loopCount: number | null;
  maxBytes?: number;
}

type FrameSourceCell = number | Pick<SheetCell, "index">;

/**
 * Builds the initial strip in the order supplied by the reviewed cells.
 * Repeated cell indexes are ignored; intentional repeats belong to the
 * duplicate-frame action, where each copy receives its own stable draft ID.
 */
export function createFramesFromCells(
  cells: readonly FrameSourceCell[],
  durationMs?: number,
): FrameStripItem[];
export function createFramesFromCells(
  cells: readonly FrameSourceCell[],
  selectedCellIndexes: Iterable<number>,
  durationMs?: number,
): FrameStripItem[];
export function createFramesFromCells(
  cells: readonly FrameSourceCell[],
  selectedCellIndexesOrDuration: Iterable<number> | number =
    DEFAULT_FRAME_DURATION_MS,
  durationMs = DEFAULT_FRAME_DURATION_MS,
): FrameStripItem[] {
  const selectedCellIndexes =
    typeof selectedCellIndexesOrDuration === "number"
      ? null
      : new Set(selectedCellIndexesOrDuration);
  const normalizedDuration = normalizeDurationMs(durationMs);
  const effectiveDuration =
    typeof selectedCellIndexesOrDuration === "number"
      ? normalizeDurationMs(selectedCellIndexesOrDuration)
      : normalizedDuration;
  const seenIndexes = new Set<number>();
  const frames: FrameStripItem[] = [];

  for (const cell of cells) {
    const sourceCellIndex = typeof cell === "number" ? cell : cell.index;
    assertSourceCellIndex(sourceCellIndex);
    if (
      seenIndexes.has(sourceCellIndex) ||
      (selectedCellIndexes && !selectedCellIndexes.has(sourceCellIndex))
    ) {
      continue;
    }

    seenIndexes.add(sourceCellIndex);
    frames.push({
      id: `frame-cell-${sourceCellIndex}`,
      sourceCellIndex,
      durationMs: effectiveDuration,
    });
  }

  return frames;
}

/**
 * Mirrors dnd-kit's arrayMove behavior: the active frame is inserted at the
 * current position of the frame it was dropped over.
 */
export function moveFrame(
  frames: readonly FrameStripItem[],
  activeId: string,
  overId: string,
): FrameStripItem[] {
  const activeIndex = frames.findIndex((frame) => frame.id === activeId);
  const overIndex = frames.findIndex((frame) => frame.id === overId);
  if (activeIndex === -1 || overIndex === -1 || activeIndex === overIndex) {
    return frames as FrameStripItem[];
  }

  const next = [...frames];
  const [activeFrame] = next.splice(activeIndex, 1);
  next.splice(overIndex, 0, activeFrame);
  return next;
}

/**
 * Explorer-like selection for a frame strip. Shift selects an anchor range,
 * Ctrl/Cmd toggles one frame, and Ctrl/Cmd+Shift adds a range.
 */
export function selectFrameIds(
  current: FrameSelectionState,
  orderedIds: readonly string[],
  targetId: string,
  modifiers: FrameSelectionModifiers = {},
): FrameSelectionState {
  if (!orderedIds.includes(targetId)) {
    return current;
  }

  const shouldToggle = modifiers.ctrlKey === true || modifiers.metaKey === true;
  if (modifiers.shiftKey === true) {
    const anchorId =
      current.anchorId && orderedIds.includes(current.anchorId)
        ? current.anchorId
        : targetId;
    const rangeIds = idsBetween(orderedIds, anchorId, targetId);
    const selectedIds = shouldToggle
      ? orderedSelection(orderedIds, new Set([...current.selectedIds, ...rangeIds]))
      : rangeIds;
    return { selectedIds, anchorId };
  }

  if (shouldToggle) {
    const selectedIds = new Set(current.selectedIds);
    if (selectedIds.has(targetId)) {
      selectedIds.delete(targetId);
    } else {
      selectedIds.add(targetId);
    }
    return {
      selectedIds: orderedSelection(orderedIds, selectedIds),
      anchorId: targetId,
    };
  }

  return {
    selectedIds: [targetId],
    anchorId: targetId,
  };
}

/**
 * Inserts one copy immediately after each selected frame. Copy IDs are derived
 * only from the current strip, so the operation is deterministic and pure.
 */
export function duplicateSelectedFrames(
  frames: readonly FrameStripItem[],
  selectedIds: Iterable<string>,
): FrameStripItem[] {
  const selected = new Set(selectedIds);
  if (selected.size === 0) {
    return frames as FrameStripItem[];
  }

  const occupiedIds = new Set(frames.map((frame) => frame.id));
  const next: FrameStripItem[] = [];
  let didDuplicate = false;

  for (const frame of frames) {
    next.push(frame);
    if (!selected.has(frame.id)) {
      continue;
    }

    const copyId = nextCopyId(frame.id, occupiedIds);
    occupiedIds.add(copyId);
    next.push({ ...frame, id: copyId });
    didDuplicate = true;
  }

  return didDuplicate ? next : (frames as FrameStripItem[]);
}

export function deleteSelectedFrames(
  frames: readonly FrameStripItem[],
  selectedIds: Iterable<string>,
): FrameStripItem[] {
  const selected = new Set(selectedIds);
  if (selected.size === 0) {
    return frames as FrameStripItem[];
  }

  const next = frames.filter((frame) => !selected.has(frame.id));
  return next.length === frames.length ? (frames as FrameStripItem[]) : next;
}

/**
 * Reverses only the items occupying selected positions. Unselected positions
 * stay fixed, which makes non-contiguous selection behavior predictable.
 */
export function reverseSelectedFrames(
  frames: readonly FrameStripItem[],
  selectedIds: Iterable<string>,
): FrameStripItem[] {
  const selected = new Set(selectedIds);
  const selectedFrames = frames.filter((frame) => selected.has(frame.id));
  if (selectedFrames.length < 2) {
    return frames as FrameStripItem[];
  }

  selectedFrames.reverse();
  let replacementIndex = 0;
  return frames.map((frame) =>
    selected.has(frame.id) ? selectedFrames[replacementIndex++] : frame,
  );
}

export function updateFrameDuration(
  frames: readonly FrameStripItem[],
  frameId: string,
  durationMs: number,
): FrameStripItem[] {
  const normalizedDuration = normalizeDurationMs(durationMs);
  let didChange = false;
  const next = frames.map((frame) => {
    if (frame.id !== frameId || frame.durationMs === normalizedDuration) {
      return frame;
    }
    didChange = true;
    return { ...frame, durationMs: normalizedDuration };
  });
  return didChange ? next : (frames as FrameStripItem[]);
}

export function applyDurationToSelected(
  frames: readonly FrameStripItem[],
  selectedIds: Iterable<string>,
  durationMs: number,
): FrameStripItem[] {
  const selected = new Set(selectedIds);
  if (selected.size === 0) {
    return frames as FrameStripItem[];
  }

  const normalizedDuration = normalizeDurationMs(durationMs);
  let didChange = false;
  const next = frames.map((frame) => {
    if (!selected.has(frame.id) || frame.durationMs === normalizedDuration) {
      return frame;
    }
    didChange = true;
    return { ...frame, durationMs: normalizedDuration };
  });
  return didChange ? next : (frames as FrameStripItem[]);
}

/**
 * Produces the actual encoded order. A repeating ping-pong cycle avoids duplicate
 * endpoints: [a,b,c,d] becomes [a,b,c,d,c,b]. A one-shot cycle appends `a`
 * so the visible playback actually returns to its starting frame.
 */
export function materializeFrames(
  frames: readonly FrameStripItem[],
  direction: FrameSheetGifDirection,
  loopMode: FrameSheetGifLoopMode = "infinite",
): FrameStripItem[] {
  if (direction === "reverse") {
    return [...frames].reverse();
  }
  if (direction === "pingpong" && frames.length > 1) {
    const materialized = [...frames, ...frames.slice(1, -1).reverse()];
    if (loopMode === "once") {
      materialized.push(frames[0]);
    }
    return materialized;
  }
  return [...frames];
}

export function totalFrameDuration(
  frames: readonly FrameStripItem[],
  direction: FrameSheetGifDirection = "forward",
  loopMode: FrameSheetGifLoopMode = "infinite",
): number {
  return materializeFrames(frames, direction, loopMode).reduce(
    (total, frame) => total + normalizeDurationMs(frame.durationMs),
    0,
  );
}

/**
 * GIF frame delays are encoded in 10 ms units. Values are rounded half-up to
 * the nearest unit and bounded to the renderer contract.
 */
export function normalizeDurationMs(
  durationMs: number,
  fallbackMs = DEFAULT_FRAME_DURATION_MS,
): number {
  const finiteFallback = Number.isFinite(fallbackMs)
    ? fallbackMs
    : DEFAULT_FRAME_DURATION_MS;
  const finiteValue = Number.isFinite(durationMs) ? durationMs : finiteFallback;
  const rounded = Math.round(finiteValue / 10) * 10;
  return Math.min(
    MAX_FRAME_DURATION_MS,
    Math.max(MIN_FRAME_DURATION_MS, rounded),
  );
}

/**
 * Returns a canonical render-recipe string for frontend measurement staleness
 * checks. Draft IDs and display names are intentionally excluded because they
 * do not change encoded GIF bytes.
 */
export function frameSheetGifSignature(
  input: FrameSheetGifSignatureInput,
): string {
  const payload = {
    version: 1,
    sourceKey: input.sourceKey,
    gridSettings: input.gridSettings,
    frames: input.frames.map((frame) => ({
      sourceCellIndex: frame.sourceCellIndex,
      durationMs: normalizeDurationMs(frame.durationMs),
    })),
    direction: input.direction,
    loopMode: input.loopMode,
    loopCount: input.loopMode === "count" ? input.loopCount : null,
    maxBytes: input.maxBytes ?? null,
  };
  return `frame-sheet-gif-v1:${stableSerialize(payload)}`;
}

function assertSourceCellIndex(sourceCellIndex: number) {
  if (!Number.isSafeInteger(sourceCellIndex) || sourceCellIndex < 0) {
    throw new RangeError("sourceCellIndex must be a non-negative safe integer.");
  }
}

function idsBetween(
  orderedIds: readonly string[],
  firstId: string,
  secondId: string,
) {
  const firstIndex = orderedIds.indexOf(firstId);
  const secondIndex = orderedIds.indexOf(secondId);
  if (firstIndex === -1 || secondIndex === -1) {
    return [secondId];
  }

  const start = Math.min(firstIndex, secondIndex);
  const end = Math.max(firstIndex, secondIndex);
  return orderedIds.slice(start, end + 1);
}

function orderedSelection(
  orderedIds: readonly string[],
  selectedIds: Set<string>,
) {
  return orderedIds.filter((id) => selectedIds.has(id));
}

function nextCopyId(sourceId: string, occupiedIds: ReadonlySet<string>) {
  const firstCandidate = `${sourceId}-copy`;
  if (!occupiedIds.has(firstCandidate)) {
    return firstCandidate;
  }

  let copyNumber = 2;
  while (occupiedIds.has(`${firstCandidate}-${copyNumber}`)) {
    copyNumber += 1;
  }
  return `${firstCandidate}-${copyNumber}`;
}

function stableSerialize(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableSerialize).join(",")}]`;
  }

  const record = value as Record<string, unknown>;
  return `{${Object.keys(record)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${stableSerialize(record[key])}`)
    .join(",")}}`;
}

import { useMemo, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { Copy, Plus, Save, Trash2 } from "lucide-react";

import type { CollectionSummary } from "@/features/collections/types";
import {
  analyzeManualSlices,
  importManualSlices,
  saveManualSlices,
} from "@/features/sheets/api";
import type { ManualSlice, ManualSliceAnalysis } from "@/features/sheets/types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

interface ManualSliceCanvasProps {
  collection: CollectionSummary;
  file: File;
  imageUrl: string | null;
  onImported: () => Promise<void>;
}

interface ImageSize {
  width: number;
  height: number;
}

interface DragState {
  mode: "create" | "move" | "resize";
  sliceId: string;
  startPoint: { x: number; y: number };
  original: ManualSlice;
}

export function ManualSliceCanvas({
  collection,
  file,
  imageUrl,
  onImported,
}: ManualSliceCanvasProps) {
  const imageRef = useRef<HTMLImageElement>(null);
  const dragRef = useRef<DragState | null>(null);
  const [imageSize, setImageSize] = useState<ImageSize | null>(null);
  const [slices, setSlices] = useState<ManualSlice[]>([]);
  const [selectedSliceId, setSelectedSliceId] = useState<string | null>(null);
  const [displayNamePattern, setDisplayNamePattern] = useState("slice_{number}");
  const [analysis, setAnalysis] = useState<ManualSliceAnalysis | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const sortedSlices = useMemo(
    () => [...slices].sort((a, b) => a.orderIndex - b.orderIndex || a.y - b.y || a.x - b.x),
    [slices],
  );
  const selectedSlice = slices.find((slice) => slice.sliceId === selectedSliceId) ?? null;
  const outOfBoundsIds = useMemo(
    () => new Set(analysis?.outOfBoundsSliceIds ?? []),
    [analysis?.outOfBoundsSliceIds],
  );
  const importableCount = sortedSlices.filter(
    (slice) => slice.include && !isOutOfBounds(slice, imageSize),
  ).length;

  const updateAnalysis = async (nextSlices = slices) => {
    setErrorMessage(null);
    try {
      setAnalysis(await analyzeManualSlices(file, nextSlices));
    } catch (error) {
      setAnalysis(null);
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const updateSlices = (updater: (current: ManualSlice[]) => ManualSlice[]) => {
    setSlices((current) => {
      const next = updater(current);
      void updateAnalysis(next);
      return next;
    });
  };

  const addSlice = () => {
    if (!imageSize) {
      return;
    }
    const orderIndex = nextOrderIndex(slices);
    const width = Math.min(collection.defaultCellWidth, imageSize.width);
    const height = Math.min(collection.defaultCellHeight, imageSize.height);
    const next = makeSlice({
      orderIndex,
      x: Math.max(0, Math.round((imageSize.width - width) / 2)),
      y: Math.max(0, Math.round((imageSize.height - height) / 2)),
      w: width,
      h: height,
    });
    updateSlices((current) => [...current, next]);
    setSelectedSliceId(next.sliceId);
  };

  const duplicateSlice = (slice: ManualSlice) => {
    const next = {
      ...slice,
      sliceId: createSliceId(),
      name: uniqueSliceName(slices, `${slice.name || "slice"}_copy`),
      x: slice.x + 8,
      y: slice.y + 8,
      orderIndex: nextOrderIndex(slices),
    };
    updateSlices((current) => [...current, clampSlice(next, imageSize)]);
    setSelectedSliceId(next.sliceId);
  };

  const deleteSlice = (sliceId: string) => {
    updateSlices((current) => current.filter((slice) => slice.sliceId !== sliceId));
    setSelectedSliceId((current) => (current === sliceId ? null : current));
  };

  const patchSlice = (sliceId: string, patch: Partial<ManualSlice>) => {
    updateSlices((current) =>
      current.map((slice) =>
        slice.sliceId === sliceId ? clampSlice({ ...slice, ...patch }, imageSize) : slice,
      ),
    );
  };

  const pointerToImagePoint = (event: PointerEvent<SVGSVGElement>) => {
    const image = imageRef.current;
    if (!image || !imageSize) {
      return { x: 0, y: 0 };
    }
    const rect = image.getBoundingClientRect();
    const x = ((event.clientX - rect.left) / rect.width) * imageSize.width;
    const y = ((event.clientY - rect.top) / rect.height) * imageSize.height;
    return {
      x: Math.round(Math.max(0, Math.min(imageSize.width, x))),
      y: Math.round(Math.max(0, Math.min(imageSize.height, y))),
    };
  };

  const startCreate = (event: PointerEvent<SVGSVGElement>) => {
    if (!imageSize || event.button !== 0) {
      return;
    }
    const point = pointerToImagePoint(event);
    const next = makeSlice({
      orderIndex: nextOrderIndex(slices),
      x: point.x,
      y: point.y,
      w: 1,
      h: 1,
    });
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      mode: "create",
      sliceId: next.sliceId,
      startPoint: point,
      original: next,
    };
    setSelectedSliceId(next.sliceId);
    setSlices((current) => [...current, next]);
  };

  const startMove = (event: PointerEvent<SVGRectElement>, slice: ManualSlice) => {
    event.stopPropagation();
    event.currentTarget.ownerSVGElement?.setPointerCapture(event.pointerId);
    dragRef.current = {
      mode: "move",
      sliceId: slice.sliceId,
      startPoint: pointerToImagePoint(event as unknown as PointerEvent<SVGSVGElement>),
      original: slice,
    };
    setSelectedSliceId(slice.sliceId);
  };

  const startResize = (event: PointerEvent<SVGRectElement>, slice: ManualSlice) => {
    event.stopPropagation();
    event.currentTarget.ownerSVGElement?.setPointerCapture(event.pointerId);
    dragRef.current = {
      mode: "resize",
      sliceId: slice.sliceId,
      startPoint: pointerToImagePoint(event as unknown as PointerEvent<SVGSVGElement>),
      original: slice,
    };
    setSelectedSliceId(slice.sliceId);
  };

  const continueDrag = (event: PointerEvent<SVGSVGElement>) => {
    const drag = dragRef.current;
    if (!drag || !imageSize) {
      return;
    }
    const point = pointerToImagePoint(event);
    if (drag.mode === "create") {
      const x = Math.min(drag.startPoint.x, point.x);
      const y = Math.min(drag.startPoint.y, point.y);
      const w = Math.max(1, Math.abs(point.x - drag.startPoint.x));
      const h = Math.max(1, Math.abs(point.y - drag.startPoint.y));
      setSlices((current) =>
        current.map((slice) =>
          slice.sliceId === drag.sliceId ? clampSlice({ ...slice, x, y, w, h }, imageSize) : slice,
        ),
      );
      return;
    }

    if (drag.mode === "move") {
      const dx = point.x - drag.startPoint.x;
      const dy = point.y - drag.startPoint.y;
      setSlices((current) =>
        current.map((slice) =>
          slice.sliceId === drag.sliceId
            ? clampSlice({ ...drag.original, x: drag.original.x + dx, y: drag.original.y + dy }, imageSize)
            : slice,
        ),
      );
      return;
    }

    const w = Math.max(1, drag.original.w + point.x - drag.startPoint.x);
    const h = Math.max(1, drag.original.h + point.y - drag.startPoint.y);
    setSlices((current) =>
      current.map((slice) =>
        slice.sliceId === drag.sliceId ? clampSlice({ ...drag.original, w, h }, imageSize) : slice,
      ),
    );
  };

  const stopDrag = () => {
    if (!dragRef.current) {
      return;
    }
    const sliceId = dragRef.current.sliceId;
    dragRef.current = null;
    setSlices((current) => {
      const next = current.filter((slice) => slice.w >= 2 && slice.h >= 2);
      void updateAnalysis(next);
      if (!next.some((slice) => slice.sliceId === sliceId)) {
        setSelectedSliceId(null);
      }
      return next;
    });
  };

  const runImport = async () => {
    setIsRunning(true);
    setErrorMessage(null);
    setMessage(null);
    try {
      const result = await importManualSlices(
        collection.id,
        file,
        sortedSlices,
        displayNamePattern,
      );
      await onImported();
      setMessage(
        `${result.importedCount}개 Slice를 가져왔습니다. ${result.skippedSlices.length}개 Slice는 건너뛰었습니다.`,
      );
      if (result.warnings.length > 0) {
        setAnalysis((current) =>
          current ? { ...current, warnings: [...current.warnings, ...result.warnings] } : current,
        );
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsRunning(false);
    }
  };

  const runSaveMetadata = async () => {
    setIsRunning(true);
    setErrorMessage(null);
    setMessage(null);
    try {
      const result = await saveManualSlices(`${file.name}-${file.size}`, sortedSlices);
      setMessage(`Slice metadata ${result.savedCount}개를 저장했습니다: ${result.metadataPath}`);
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsRunning(false);
    }
  };

  return (
    <div className="grid min-h-0 gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
      <section className="min-h-0 rounded-md border border-border bg-white p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="text-sm font-semibold">직접 Slice 지정</h3>
            <p className="mt-1 text-xs text-muted">
              이미지 위에서 드래그해 Slice를 만들고, 선택한 Slice는 끌어서 이동하거나 오른쪽 아래 핸들로 크기를 조정합니다.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              className="inline-flex items-center gap-1 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:opacity-60"
              disabled={!imageSize}
              type="button"
              onClick={addSlice}
            >
              <Plus aria-hidden="true" className="size-4" />
              Slice 추가
            </button>
            <button
              className="inline-flex items-center gap-1 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:opacity-60"
              disabled={sortedSlices.length === 0 || isRunning}
              type="button"
              onClick={() => void runSaveMetadata()}
            >
              <Save aria-hidden="true" className="size-4" />
              metadata 저장
            </button>
          </div>
        </div>

        <div className="mt-4 overflow-auto rounded-md border border-border bg-preview p-3">
          <div className="relative inline-block max-w-full">
            {imageUrl ? (
              <img
                ref={imageRef}
                alt="직접 Slice 대상 시트"
                className="max-h-[560px] max-w-full select-none"
                draggable={false}
                src={imageUrl}
                onLoad={(event) => {
                  const nextSize = {
                    width: event.currentTarget.naturalWidth,
                    height: event.currentTarget.naturalHeight,
                  };
                  setImageSize(nextSize);
                  void updateAnalysis(slices);
                }}
              />
            ) : null}
            {imageSize ? (
              <svg
                className="absolute inset-0 size-full touch-none"
                data-testid="manual-slice-canvas"
                preserveAspectRatio="none"
                viewBox={`0 0 ${imageSize.width} ${imageSize.height}`}
                onPointerCancel={stopDrag}
                onPointerDown={startCreate}
                onPointerMove={continueDrag}
                onPointerUp={stopDrag}
              >
                {sortedSlices.map((slice) => {
                  const selected = selectedSliceId === slice.sliceId;
                  const invalid = outOfBoundsIds.has(slice.sliceId) || isOutOfBounds(slice, imageSize);
                  return (
                    <g key={slice.sliceId}>
                      <rect
                        className="cursor-move"
                        fill={selected ? "rgba(59,130,246,0.22)" : "rgba(14,165,233,0.12)"}
                        height={slice.h}
                        stroke={invalid ? "#dc2626" : selected ? "#2563eb" : "#0284c7"}
                        strokeDasharray={invalid ? "8 5" : undefined}
                        strokeWidth={selected ? 3 : 2}
                        width={slice.w}
                        x={slice.x}
                        y={slice.y}
                        onPointerDown={(event) => startMove(event, slice)}
                      />
                      <text
                        className="pointer-events-none select-none"
                        fill="#111827"
                        fontSize={14}
                        stroke="white"
                        strokeWidth={3}
                        x={slice.x + 6}
                        y={slice.y + 18}
                      >
                        {slice.orderIndex + 1}
                      </text>
                      <text
                        className="pointer-events-none select-none"
                        fill="#111827"
                        fontSize={14}
                        x={slice.x + 6}
                        y={slice.y + 18}
                      >
                        {slice.orderIndex + 1}
                      </text>
                      {selected ? (
                        <rect
                          className="cursor-nwse-resize"
                          fill="#2563eb"
                          height={14}
                          width={14}
                          x={slice.x + slice.w - 7}
                          y={slice.y + slice.h - 7}
                          onPointerDown={(event) => startResize(event, slice)}
                        />
                      ) : null}
                    </g>
                  );
                })}
              </svg>
            ) : null}
          </div>
        </div>
      </section>

      <aside className="flex min-h-0 flex-col gap-4">
        <section className="rounded-md border border-border bg-white p-4">
          <h3 className="text-sm font-semibold">가져오기 설정</h3>
          <label className="mt-3 flex flex-col gap-1 text-xs font-medium text-muted">
            이름 패턴
            <input
              className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
              value={displayNamePattern}
              onChange={(event) => setDisplayNamePattern(event.currentTarget.value)}
            />
          </label>
          <div className="mt-3 rounded-md bg-preview px-3 py-2 text-xs text-muted">
            포함 Slice {importableCount}개 / 전체 {sortedSlices.length}개
          </div>
          <button
            className="mt-3 w-full rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-60"
            disabled={importableCount === 0 || isRunning}
            type="button"
            onClick={() => void runImport()}
          >
            {isRunning ? "가져오는 중" : "포함 Slice 가져오기"}
          </button>
        </section>

        <section className="flex min-h-0 flex-col rounded-md border border-border bg-white p-4">
          <h3 className="text-sm font-semibold">Slice 목록</h3>
          <div className="mt-3 min-h-0 overflow-auto">
            {sortedSlices.length === 0 ? (
              <p className="rounded-md bg-preview p-3 text-sm text-muted">
                이미지 위에서 드래그하거나 `Slice 추가`를 눌러 영역을 만드세요.
              </p>
            ) : (
              <div className="grid gap-2">
                {sortedSlices.map((slice) => (
                  <SliceRow
                    invalid={outOfBoundsIds.has(slice.sliceId) || isOutOfBounds(slice, imageSize)}
                    isSelected={selectedSliceId === slice.sliceId}
                    key={slice.sliceId}
                    slice={slice}
                    onDelete={() => deleteSlice(slice.sliceId)}
                    onDuplicate={() => duplicateSlice(slice)}
                    onPatch={(patch) => patchSlice(slice.sliceId, patch)}
                    onSelect={() => setSelectedSliceId(slice.sliceId)}
                  />
                ))}
              </div>
            )}
          </div>
        </section>

        {selectedSlice ? (
          <section className="rounded-md border border-border bg-white p-4">
            <h3 className="text-sm font-semibold">선택 Slice 좌표</h3>
            <div className="mt-3 grid grid-cols-2 gap-2">
              <NumberField label="X" value={selectedSlice.x} onChange={(x) => patchSlice(selectedSlice.sliceId, { x })} />
              <NumberField label="Y" value={selectedSlice.y} onChange={(y) => patchSlice(selectedSlice.sliceId, { y })} />
              <NumberField label="W" value={selectedSlice.w} onChange={(w) => patchSlice(selectedSlice.sliceId, { w })} />
              <NumberField label="H" value={selectedSlice.h} onChange={(h) => patchSlice(selectedSlice.sliceId, { h })} />
            </div>
          </section>
        ) : null}

        {analysis?.warnings.length ? (
          <p className="rounded-md border border-border bg-white p-3 text-xs text-muted">
            {analysis.warnings.join(" / ")}
          </p>
        ) : null}
        {message ? <p className="rounded-md border border-border bg-white p-3 text-xs text-muted">{message}</p> : null}
        {errorMessage ? (
          <p className="rounded-md border border-border bg-white p-3 text-xs text-danger" role="alert">
            {errorMessage}
          </p>
        ) : null}
      </aside>
    </div>
  );
}

function SliceRow({
  slice,
  isSelected,
  invalid,
  onDelete,
  onDuplicate,
  onPatch,
  onSelect,
}: {
  slice: ManualSlice;
  isSelected: boolean;
  invalid: boolean;
  onDelete: () => void;
  onDuplicate: () => void;
  onPatch: (patch: Partial<ManualSlice>) => void;
  onSelect: () => void;
}) {
  return (
    <div
      className={cn(
        "rounded-md border p-3 text-sm",
        isSelected ? "border-focus bg-selected" : "border-border bg-white",
        invalid ? "border-danger" : "",
      )}
      data-testid="manual-slice-row"
    >
      <div className="flex items-start justify-between gap-2">
        <label className="flex min-w-0 flex-1 flex-col gap-1 text-xs font-medium text-muted">
          이름
          <input
            className="min-w-0 rounded-md border border-border bg-white px-2 py-1.5 text-sm text-foreground"
            value={slice.name}
            onFocus={onSelect}
            onChange={(event) => onPatch({ name: event.currentTarget.value })}
          />
        </label>
        <label className="mt-6 inline-flex items-center gap-1 text-xs text-muted">
          <input
            checked={slice.include}
            type="checkbox"
            onChange={(event) => onPatch({ include: event.currentTarget.checked })}
          />
          포함
        </label>
      </div>
      <button
        className="mt-2 text-left text-xs text-muted hover:text-foreground"
        type="button"
        onClick={onSelect}
      >
        #{slice.orderIndex + 1} · x {slice.x}, y {slice.y}, {slice.w}x{slice.h}
        {invalid ? " · 범위 확인 필요" : ""}
      </button>
      <div className="mt-2 flex flex-wrap gap-1">
        <button
          className="inline-flex items-center gap-1 rounded border border-border bg-white px-2 py-1 text-xs hover:bg-menu-hover"
          type="button"
          onClick={onDuplicate}
        >
          <Copy aria-hidden="true" className="size-3.5" />
          복제
        </button>
        <button
          className="inline-flex items-center gap-1 rounded border border-border bg-white px-2 py-1 text-xs text-danger hover:bg-menu-hover"
          type="button"
          onClick={onDelete}
        >
          <Trash2 aria-hidden="true" className="size-3.5" />
          삭제
        </button>
      </div>
    </div>
  );
}

function NumberField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <input
        className="rounded-md border border-border bg-white px-2 py-1.5 text-sm text-foreground"
        min={label === "X" || label === "Y" ? 0 : 1}
        type="number"
        value={value}
        onChange={(event) => onChange(Number.parseInt(event.currentTarget.value, 10) || 0)}
      />
    </label>
  );
}

function makeSlice({
  orderIndex,
  x,
  y,
  w,
  h,
}: {
  orderIndex: number;
  x: number;
  y: number;
  w: number;
  h: number;
}): ManualSlice {
  return {
    sliceId: createSliceId(),
    name: `slice_${String(orderIndex + 1).padStart(3, "0")}`,
    x,
    y,
    w,
    h,
    orderIndex,
    include: true,
    notes: null,
  };
}

function createSliceId() {
  return `slice_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

function nextOrderIndex(slices: ManualSlice[]) {
  return slices.reduce((max, slice) => Math.max(max, slice.orderIndex), -1) + 1;
}

function uniqueSliceName(slices: ManualSlice[], baseName: string) {
  const names = new Set(slices.map((slice) => slice.name));
  let name = baseName;
  let index = 2;
  while (names.has(name)) {
    name = `${baseName}_${index}`;
    index += 1;
  }
  return name;
}

function clampSlice(slice: ManualSlice, imageSize: ImageSize | null): ManualSlice {
  const w = Math.max(1, Math.round(slice.w));
  const h = Math.max(1, Math.round(slice.h));
  if (!imageSize) {
    return {
      ...slice,
      x: Math.max(0, Math.round(slice.x)),
      y: Math.max(0, Math.round(slice.y)),
      w,
      h,
    };
  }
  return {
    ...slice,
    x: Math.max(0, Math.min(imageSize.width - w, Math.round(slice.x))),
    y: Math.max(0, Math.min(imageSize.height - h, Math.round(slice.y))),
    w: Math.min(w, imageSize.width),
    h: Math.min(h, imageSize.height),
  };
}

function isOutOfBounds(slice: ManualSlice, imageSize: ImageSize | null) {
  if (!imageSize) {
    return false;
  }
  return (
    slice.x < 0 ||
    slice.y < 0 ||
    slice.w <= 0 ||
    slice.h <= 0 ||
    slice.x + slice.w > imageSize.width ||
    slice.y + slice.h > imageSize.height
  );
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import type { DragEndEvent } from "@dnd-kit/core";
import {
  horizontalListSortingStrategy,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  ArrowLeft,
  ArrowRight,
  Copy,
  Film,
  GripVertical,
  Pause,
  Play,
  RefreshCw,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";

import type { CollectionSummary } from "@/features/collections/types";
import { openExportPath } from "@/features/export/api";
import {
  analyzeSheetGrid,
  createFrameSheetGif,
  measureFrameSheetGif,
} from "@/features/sheets/api";
import { SheetGridOverlay } from "@/features/sheets/components/SheetGridOverlay";
import { SheetGridPresetSelect } from "@/features/sheets/components/SheetGridPresetSelect";
import { SheetGridSettingsPanel } from "@/features/sheets/components/SheetGridSettingsPanel";
import { SheetImagePicker } from "@/features/sheets/components/SheetImagePicker";
import {
  applyDurationToSelected,
  createFramesFromCells,
  deleteSelectedFrames,
  duplicateSelectedFrames,
  materializeFrames,
  moveFrame,
  reverseSelectedFrames,
  selectFrameIds,
  totalFrameDuration,
  updateFrameDuration,
} from "@/features/sheets/frame-sheet-gif-model";
import type { FrameStripItem } from "@/features/sheets/frame-sheet-gif-model";
import {
  applyPresetToImportSettings,
  defaultSheetGridSettings,
  nextSelectionAfterCellClick,
  presetInputFromImportSettings,
} from "@/features/sheets/sheet-ui-model";
import type {
  FrameSheetGifCreateResult,
  FrameSheetGifDirection,
  FrameSheetGifLoopMode,
  FrameSheetGifMeasurement,
  SheetCell,
  SheetGridAnalysis,
  SheetGridSettings,
} from "@/features/sheets/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import { getCommandErrorMessage } from "@/lib/tauri";
import { useModalFocus } from "@/lib/use-modal-focus";
import { usePrefersReducedMotion } from "@/lib/use-prefers-reduced-motion";
import { cn } from "@/lib/utils";

const DEFAULT_FRAME_DURATION_MS = 100;
const MAX_GIF_FRAMES = 500;

export function FrameSheetToGifDialog({
  collection,
  onClose,
  onCreated,
}: {
  collection: CollectionSummary;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const selectionAnchorRef = useRef<string | null>(null);
  useModalFocus(dialogRef, onClose);
  const [file, setFile] = useState<File | null>(null);
  const [settings, setSettings] = useState<SheetGridSettings>(() => defaultSheetGridSettings());
  const [analysis, setAnalysis] = useState<SheetGridAnalysis | null>(null);
  const [selectedCellIndexes, setSelectedCellIndexes] = useState<Set<number>>(new Set());
  const [frames, setFrames] = useState<FrameStripItem[]>([]);
  const [selectedFrameIds, setSelectedFrameIds] = useState<Set<string>>(new Set());
  const [direction, setDirection] = useState<FrameSheetGifDirection>("forward");
  const [loopMode, setLoopMode] = useState<FrameSheetGifLoopMode>("infinite");
  const [loopCount, setLoopCount] = useState(2);
  const [batchDurationMs, setBatchDurationMs] = useState(DEFAULT_FRAME_DURATION_MS);
  const [fps, setFps] = useState(10);
  const [displayName, setDisplayName] = useState("프레임 시트 애니메이션");
  const [busyAction, setBusyAction] = useState<"analyze" | "measure" | "create" | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [measurement, setMeasurement] = useState<FrameSheetGifMeasurement | null>(null);
  const [measuredSignature, setMeasuredSignature] = useState<string | null>(null);
  const [createdResult, setCreatedResult] = useState<FrameSheetGifCreateResult | null>(null);
  const imageUrl = useObjectUrl(file);
  const materializedFrames = useMemo(
    () => materializeFrames(frames, direction, loopMode),
    [direction, frames, loopMode],
  );
  const durationMs = useMemo(
    () => totalFrameDuration(materializedFrames),
    [materializedFrames],
  );
  const recipeSignature = useMemo(
    () =>
      JSON.stringify({
        file: file
          ? { name: file.name, size: file.size, lastModified: file.lastModified }
          : null,
        settings,
        frames: frames.map(({ sourceCellIndex, durationMs: frameDuration }) => ({
          sourceCellIndex,
          durationMs: frameDuration,
        })),
        direction,
        loopMode,
        loopCount: loopMode === "count" ? loopCount : null,
      }),
    [direction, file, frames, loopCount, loopMode, settings],
  );
  const measurementIsFresh =
    measurement !== null && measuredSignature !== null && measuredSignature === recipeSignature;
  const frameLimitExceeded = materializedFrames.length > MAX_GIF_FRAMES;
  const sourceFrameLimit = maxSourceFramesForRecipe(direction, loopMode);
  const duplicateWouldExceedLimit =
    frames.length + selectedFrameIds.size > sourceFrameLimit;
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const invalidateMeasurement = useCallback(() => {
    setMeasurement(null);
    setMeasuredSignature(null);
    setCreatedResult(null);
  }, []);

  const changeFile = (nextFile: File | null) => {
    setFile(nextFile);
    setAnalysis(null);
    setSelectedCellIndexes(new Set());
    setFrames([]);
    setSelectedFrameIds(new Set());
    selectionAnchorRef.current = null;
    invalidateMeasurement();
    setErrorMessage(null);
    setMessage(nextFile ? "분할 설정을 확인한 뒤 셀 분석을 실행하세요." : null);
    if (nextFile) {
      const stem = nextFile.name.replace(/\.[^.]+$/, "").trim();
      setDisplayName(stem ? `${stem} 애니메이션` : "프레임 시트 애니메이션");
    }
  };

  const changeGridSettings = (nextSettings: SheetGridSettings) => {
    setSettings(nextSettings);
    setAnalysis(null);
    setSelectedCellIndexes(new Set());
    setFrames([]);
    setSelectedFrameIds(new Set());
    selectionAnchorRef.current = null;
    invalidateMeasurement();
    setErrorMessage(null);
    setMessage("분할 설정이 바뀌었습니다. 셀 분석을 다시 실행하세요.");
  };

  const runAnalysis = async () => {
    if (!file) {
      setErrorMessage("먼저 PNG, JPG 또는 JPEG 프레임 시트를 선택하세요.");
      return;
    }
    setBusyAction("analyze");
    setErrorMessage(null);
    setMessage(null);
    try {
      const nextAnalysis = await analyzeSheetGrid(file, settings);
      const eligibleCells = nextAnalysis.cells.filter(
        (cell) => !cell.outOfBounds && !cell.emptyCandidate,
      );
      const sourceFrameLimit = maxSourceFramesForRecipe(direction, loopMode);
      const defaultCells = eligibleCells.slice(0, sourceFrameLimit);
      const defaultSelection = new Set(defaultCells.map((cell) => cell.index));
      const nextFrames = createFramesFromCells(
        defaultCells,
        DEFAULT_FRAME_DURATION_MS,
      );
      setAnalysis(nextAnalysis);
      setSelectedCellIndexes(defaultSelection);
      setFrames(nextFrames);
      setSelectedFrameIds(new Set());
      selectionAnchorRef.current = null;
      invalidateMeasurement();
      setMessage(
        eligibleCells.length > sourceFrameLimit
          ? `최종 GIF ${MAX_GIF_FRAMES}프레임 제한에 맞춰 앞의 ${nextFrames.length}개 셀만 기본 선택했습니다.`
          : `${nextFrames.length}개 셀을 기본 프레임으로 구성했습니다. 빈 셀 후보도 필요하면 직접 포함하세요.`,
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  };

  const rebuildFramesFromSelection = () => {
    if (!analysis) {
      return;
    }
    const selectedCells = analysis.cells.filter(
      (cell) => selectedCellIndexes.has(cell.index) && !cell.outOfBounds,
    );
    const sourceFrameLimit = maxSourceFramesForRecipe(direction, loopMode);
    if (selectedCells.length > sourceFrameLimit) {
      setErrorMessage(
        `현재 방향·반복 조합에서는 원본 프레임을 최대 ${sourceFrameLimit}개까지 사용할 수 있습니다.`,
      );
      return;
    }
    const nextFrames = createFramesFromCells(
      selectedCells,
      DEFAULT_FRAME_DURATION_MS,
    );
    setFrames(nextFrames);
    setSelectedFrameIds(new Set());
    selectionAnchorRef.current = null;
    invalidateMeasurement();
    setMessage(
      `선택한 ${nextFrames.length}개 셀로 프레임 스트립을 다시 만들었습니다. 이전 순서·시간 편집은 초기화되었습니다.`,
    );
  };

  const updateFrames = (updater: (current: FrameStripItem[]) => FrameStripItem[]) => {
    setFrames((current) => updater(current));
    invalidateMeasurement();
    setMessage(null);
    setErrorMessage(null);
  };

  const handleFrameClick = (
    frameId: string,
    event: MouseEvent<HTMLButtonElement>,
  ) => {
    const frameIds = frames.map((frame) => frame.id);
    const next = selectFrameIds(
      {
        selectedIds: [...selectedFrameIds],
        anchorId: selectionAnchorRef.current,
      },
      frameIds,
      frameId,
      {
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey,
      },
    );
    setSelectedFrameIds(new Set(next.selectedIds));
    selectionAnchorRef.current = next.anchorId;
  };

  const handleStripKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
      event.preventDefault();
      setSelectedFrameIds(new Set(frames.map((frame) => frame.id)));
      return;
    }
    if (event.key === "Delete" && selectedFrameIds.size > 0) {
      event.preventDefault();
      updateFrames((current) => deleteSelectedFrames(current, selectedFrameIds));
      setSelectedFrameIds(new Set());
      selectionAnchorRef.current = null;
    }
  };

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) {
      return;
    }
    updateFrames((current) => moveFrame(current, String(active.id), String(over.id)));
  };

  const runMeasurement = async () => {
    if (!file || !analysis || frames.length < 2) {
      setErrorMessage("분할 분석을 마치고 프레임을 2개 이상 구성하세요.");
      return;
    }
    if (frameLimitExceeded) {
      setErrorMessage(`생성 방향을 적용한 최종 프레임은 ${MAX_GIF_FRAMES}개 이하여야 합니다.`);
      return;
    }
    setBusyAction("measure");
    setErrorMessage(null);
    setMessage(null);
    try {
      const result = await measureFrameSheetGif(file, {
        targetCollectionId: collection.id,
        gridSettings: settings,
        frames: frames.map(({ sourceCellIndex, durationMs: frameDuration }) => ({
          sourceCellIndex,
          durationMs: frameDuration,
        })),
        direction,
        loopMode,
        loopCount: loopMode === "count" ? loopCount : null,
        displayName,
        expectedRenderHash: null,
      });
      setMeasurement(result);
      setMeasuredSignature(recipeSignature);
      setCreatedResult(null);
      setMessage(
        result.passesByteLimit
          ? `실제 GIF를 생성해 ${formatBytes(result.byteSize)}로 측정했습니다.`
          : `실제 GIF는 ${formatBytes(result.byteSize)}이며 모음 제한 ${formatBytes(result.maxBytes)}을 넘습니다.`,
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  };

  const runCreate = async () => {
    if (!file || !measurement || !measurementIsFresh || !displayName.trim()) {
      setErrorMessage("최신 설정으로 용량 측정을 마치고 아이콘 이름을 입력하세요.");
      return;
    }
    setBusyAction("create");
    setErrorMessage(null);
    setMessage(null);
    try {
      const result = await createFrameSheetGif(file, {
        targetCollectionId: collection.id,
        gridSettings: settings,
        frames: frames.map(({ sourceCellIndex, durationMs: frameDuration }) => ({
          sourceCellIndex,
          durationMs: frameDuration,
        })),
        direction,
        loopMode,
        loopCount: loopMode === "count" ? loopCount : null,
        displayName: displayName.trim(),
        expectedRenderHash: measurement.renderHash,
      });
      setCreatedResult(result);
      await onCreated();
      setMessage(
        `“${result.icon.displayName}” GIF 아이콘을 만들었습니다. 원본 프레임 시트도 별도로 보존했습니다.`,
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  };

  const moveSingleSelected = (offset: -1 | 1) => {
    if (selectedFrameIds.size !== 1) {
      return;
    }
    const selectedId = [...selectedFrameIds][0];
    const currentIndex = frames.findIndex((frame) => frame.id === selectedId);
    const target = frames[currentIndex + offset];
    if (!target) {
      return;
    }
    updateFrames((current) => moveFrame(current, selectedId, target.id));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/35 p-3 sm:p-5">
      <div
        ref={dialogRef}
        aria-labelledby="frame-sheet-gif-dialog-title"
        aria-modal="true"
        className="flex max-h-[94vh] w-full max-w-[1600px] flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-xl"
        role="dialog"
        tabIndex={-1}
      >
        <header className="flex items-center justify-between gap-3 border-b border-border px-5 py-4">
          <div>
            <h2
              className="flex items-center gap-2 text-base font-semibold"
              id="frame-sheet-gif-dialog-title"
            >
              <Film aria-hidden="true" />
              프레임 시트로 새 GIF 만들기
            </h2>
            <p className="mt-1 text-sm text-muted">
              {collection.name} · 시트를 나눈 뒤 순서, 시간, 반복을 편집해 새 애니메이션 아이콘으로 등록합니다.
            </p>
          </div>
          <button
            aria-label="닫기"
            className="flex size-9 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover"
            disabled={busyAction !== null}
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-auto p-4">
          <div className="grid items-start gap-4 xl:grid-cols-[320px_minmax(520px,1fr)_340px]">
            <section className="flex min-w-0 flex-col gap-3">
              <div>
                <h3 className="text-sm font-semibold">1. 파일과 분할</h3>
                <p className="mt-1 text-xs text-muted">
                  기존 시트 가져오기와 같은 행·열, 셀 크기, 여백, 간격, 읽기 순서를 사용합니다.
                </p>
              </div>
              <SheetImagePicker file={file} onFileChange={changeFile} />
              <SheetGridPresetSelect
                disabled={busyAction !== null}
                collectionId={collection.id}
                compatibleKinds={["static_import", "static_import_export", "static_export"]}
                currentSummary={`${settings.cellWidth ?? "-"}×${settings.cellHeight ?? "-"} · ${
                  settings.columns ?? "-"
                }열 · ${settings.readOrder === "row_major" ? "행 우선" : "열 우선"}`}
                saveKindLabel="가져오기/내보내기 공유"
                target="import"
                buildPresetInput={(name) =>
                  presetInputFromImportSettings(name, collection.id, settings)
                }
                onApplyPreset={(preset) =>
                  changeGridSettings(applyPresetToImportSettings(settings, preset))
                }
              />
              <SheetGridSettingsPanel
                disabled={busyAction !== null}
                settings={settings}
                onChange={changeGridSettings}
                onPreview={() => void runAnalysis()}
                onReset={() => changeGridSettings(defaultSheetGridSettings())}
              />
            </section>

            <section className="flex min-w-0 flex-col gap-3">
              <div className="flex flex-wrap items-end justify-between gap-2">
                <div>
                  <h3 className="text-sm font-semibold">2. 셀 검토</h3>
                  <p className="mt-1 text-xs text-muted">
                    빈 셀 후보는 기본 제외됩니다. 의도한 투명 프레임이라면 직접 다시 포함할 수 있습니다.
                  </p>
                </div>
                <button
                  className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
                  disabled={!analysis || busyAction !== null || selectedCellIndexes.size === 0}
                  type="button"
                  onClick={rebuildFramesFromSelection}
                >
                  선택 셀로 스트립 다시 만들기
                </button>
              </div>
              <div className="overflow-hidden rounded-md border border-border">
                <SheetGridOverlay
                  cells={analysis?.cells ?? []}
                  imageUrl={imageUrl}
                  selectedIndexes={selectedCellIndexes}
                  sheetHeight={analysis?.sheetHeight ?? 1}
                  sheetWidth={analysis?.sheetWidth ?? 1}
                  onToggleCell={(cellIndex, multi) => {
                    if (analysis?.cells.find((cell) => cell.index === cellIndex)?.outOfBounds) {
                      return;
                    }
                    setSelectedCellIndexes((current) =>
                      nextSelectionAfterCellClick(current, cellIndex, { multi }),
                    );
                  }}
                />
              </div>
              {analysis ? (
                <FrameSheetCellReview
                  analysis={analysis}
                  imageUrl={imageUrl}
                  selectedIndexes={selectedCellIndexes}
                  onSelectionChange={setSelectedCellIndexes}
                />
              ) : (
                <div className="rounded-md border border-dashed border-border bg-card p-5 text-center text-sm text-muted">
                  파일을 선택하고 ‘미리보기 갱신’을 실행하면 번호가 붙은 셀을 검토할 수 있습니다.
                </div>
              )}
            </section>

            <AnimationPreviewPanel
              analysis={analysis}
              direction={direction}
              durationMs={durationMs}
              frames={materializedFrames}
              imageUrl={imageUrl}
              loopCount={loopCount}
              loopMode={loopMode}
              measurement={measurementIsFresh ? measurement : null}
            />

          <section className="mt-5 flex min-w-0 flex-col gap-3 rounded-md border border-border bg-card p-4 xl:col-span-2">
            <div className="flex flex-wrap items-end justify-between gap-3">
              <div>
                <h3 className="text-sm font-semibold">3. 프레임 스트립</h3>
                <p className="mt-1 text-xs text-muted">
                  Ctrl/Shift로 선택하고 드래그해 정렬합니다. ‘스트립 뒤집기’는 현재 편집 순서를 바꾸고,
                  생성 방향은 아래 최종 출력에만 적용됩니다.
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <button
                  aria-label="선택 프레임 왼쪽으로 이동"
                  className={smallButtonClass}
                  disabled={selectedFrameIds.size !== 1}
                  title="키보드 사용자를 위한 한 칸 이동"
                  type="button"
                  onClick={() => moveSingleSelected(-1)}
                >
                  <ArrowLeft aria-hidden="true" />
                </button>
                <button
                  aria-label="선택 프레임 오른쪽으로 이동"
                  className={smallButtonClass}
                  disabled={selectedFrameIds.size !== 1}
                  title="키보드 사용자를 위한 한 칸 이동"
                  type="button"
                  onClick={() => moveSingleSelected(1)}
                >
                  <ArrowRight aria-hidden="true" />
                </button>
                <button
                  className={smallButtonClass}
                  disabled={selectedFrameIds.size === 0 || duplicateWouldExceedLimit}
                  title={
                    duplicateWouldExceedLimit
                      ? `현재 방향·반복에서는 원본 프레임을 최대 ${sourceFrameLimit}개까지 사용할 수 있습니다.`
                      : undefined
                  }
                  type="button"
                  onClick={() =>
                    updateFrames((current) =>
                      duplicateSelectedFrames(current, selectedFrameIds),
                    )
                  }
                >
                  <Copy aria-hidden="true" />
                  복제
                </button>
                <button
                  className={smallButtonClass}
                  disabled={selectedFrameIds.size < 2}
                  type="button"
                  onClick={() =>
                    updateFrames((current) =>
                      reverseSelectedFrames(current, selectedFrameIds),
                    )
                  }
                >
                  <RefreshCw aria-hidden="true" />
                  선택 순서 뒤집기
                </button>
                <button
                  className={smallButtonClass}
                  disabled={frames.length < 2}
                  type="button"
                  onClick={() => updateFrames((current) => [...current].reverse())}
                >
                  <RotateCcw aria-hidden="true" />
                  스트립 전체 뒤집기
                </button>
                <button
                  className={smallButtonClass}
                  disabled={selectedFrameIds.size === 0}
                  type="button"
                  onClick={() => {
                    updateFrames((current) =>
                      deleteSelectedFrames(current, selectedFrameIds),
                    );
                    setSelectedFrameIds(new Set());
                    selectionAnchorRef.current = null;
                  }}
                >
                  <Trash2 aria-hidden="true" />
                  삭제
                </button>
              </div>
            </div>

            <div className="flex flex-wrap items-end gap-2">
              <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                선택 프레임 시간
                <span className="flex items-center gap-2">
                  <input
                    className="w-28 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
                    max={60_000}
                    min={10}
                    step={10}
                    type="number"
                    value={batchDurationMs}
                    onChange={(event) =>
                      setBatchDurationMs(Number.parseInt(event.currentTarget.value, 10) || 10)
                    }
                  />
                  <span>ms</span>
                </span>
              </label>
              <button
                className={smallButtonClass}
                disabled={selectedFrameIds.size === 0}
                type="button"
                onClick={() =>
                  updateFrames((current) =>
                    applyDurationToSelected(current, selectedFrameIds, batchDurationMs),
                  )
                }
              >
                선택 시간 적용
              </button>
              <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                전체 FPS 편의 입력
                <input
                  className="w-24 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
                  max={100}
                  min={1}
                  type="number"
                  value={fps}
                  onChange={(event) =>
                    setFps(Math.min(100, Math.max(1, Number.parseInt(event.currentTarget.value, 10) || 1)))
                  }
                />
              </label>
              <button
                className={smallButtonClass}
                disabled={frames.length === 0}
                type="button"
                onClick={() =>
                  updateFrames((current) =>
                    applyDurationToSelected(
                      current,
                      new Set(current.map((frame) => frame.id)),
                      1_000 / fps,
                    ),
                  )
                }
              >
                전체 프레임을 {fps} FPS로 맞추기
              </button>
              <span className="text-xs text-muted">GIF 규격에 맞춰 실제 출력은 10ms 단위로 정규화됩니다.</span>
            </div>

            <div
              aria-label="선택 프레임 스트립"
              className="min-w-0 overflow-x-auto rounded-md border border-border bg-white p-3 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              role="listbox"
              tabIndex={0}
              onKeyDown={handleStripKeyDown}
            >
              {frames.length > 0 ? (
                <DndContext
                  collisionDetection={closestCenter}
                  sensors={sensors}
                  onDragEnd={handleDragEnd}
                >
                  <SortableContext
                    items={frames.map((frame) => frame.id)}
                    strategy={horizontalListSortingStrategy}
                  >
                    <div className="flex w-max gap-2">
                      {frames.map((frame, index) => (
                        <SortableFrameCard
                          key={frame.id}
                          analysis={analysis}
                          frame={frame}
                          imageUrl={imageUrl}
                          index={index}
                          selected={selectedFrameIds.has(frame.id)}
                          onClick={handleFrameClick}
                          onDurationChange={(duration) =>
                            updateFrames((current) =>
                              updateFrameDuration(current, frame.id, duration),
                            )
                          }
                        />
                      ))}
                    </div>
                  </SortableContext>
                </DndContext>
              ) : (
                <div className="flex h-32 items-center justify-center px-6 text-sm text-muted">
                  셀 분석 후 선택 셀로 프레임 스트립을 구성하세요.
                </div>
              )}
            </div>
          </section>

          <section className="mt-4 grid gap-4 rounded-md border border-border bg-white p-4 lg:grid-cols-[1fr_1fr_1fr_auto] xl:col-span-2">
            <label className="flex flex-col gap-1 text-xs font-medium text-muted">
              새 아이콘 이름
              <input
                className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
                maxLength={120}
                value={displayName}
                onChange={(event) => setDisplayName(event.currentTarget.value)}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs font-medium text-muted">
              GIF 생성 방향
              <select
                className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
                value={direction}
                onChange={(event) => {
                  setDirection(event.currentTarget.value as FrameSheetGifDirection);
                  invalidateMeasurement();
                }}
              >
                <option value="forward">정방향 — 스트립 그대로</option>
                <option value="reverse">역방향 — 출력만 반대로</option>
                <option value="pingpong">핑퐁 — 자연스러운 왕복 순서</option>
              </select>
            </label>
            <div className="grid grid-cols-[1fr_110px] gap-2">
              <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                반복
                <select
                  className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
                  value={loopMode}
                  onChange={(event) => {
                    setLoopMode(event.currentTarget.value as FrameSheetGifLoopMode);
                    invalidateMeasurement();
                  }}
                >
                  <option value="once">한 번</option>
                  <option value="infinite">무한 반복</option>
                  <option value="count">횟수 지정</option>
                </select>
              </label>
              <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                반복 횟수
                <input
                  className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground disabled:bg-card disabled:text-muted"
                  disabled={loopMode !== "count"}
                  max={65_535}
                  min={1}
                  type="number"
                  value={loopCount}
                  onChange={(event) => {
                    setLoopCount(Math.max(1, Number.parseInt(event.currentTarget.value, 10) || 1));
                    invalidateMeasurement();
                  }}
                />
              </label>
            </div>
            <div className="flex items-end">
              <button
                className="w-full rounded-md border border-accent bg-white px-4 py-2 text-sm font-semibold text-accent hover:bg-selected disabled:cursor-not-allowed disabled:border-border disabled:text-muted"
                disabled={
                  !analysis || frames.length < 2 || frameLimitExceeded || busyAction !== null
                }
                type="button"
                onClick={() => void runMeasurement()}
              >
                {busyAction === "measure" ? "GIF 생성·측정 중…" : "GIF 미리 생성 및 용량 측정"}
              </button>
            </div>
          </section>
          </div>

          {frameLimitExceeded ? (
            <p className="mt-3 rounded-md border border-danger bg-red-50 px-4 py-3 text-sm text-danger">
              현재 방향과 반복을 적용하면 {materializedFrames.length}프레임입니다. 최종 GIF는 최대{" "}
              {MAX_GIF_FRAMES}프레임이어야 하므로 일부 프레임을 삭제하거나 생성 방향을 바꾸세요.
            </p>
          ) : null}
          {measurement && !measurementIsFresh ? (
            <p className="mt-3 rounded-md border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
              프레임, 시간, 방향 또는 반복 설정이 바뀌어 이전 측정값이 만료되었습니다. 다시 측정해야 새
              GIF를 만들 수 있습니다.
            </p>
          ) : null}
          {measurementIsFresh && measurement ? (
            <MeasurementSummary collection={collection} measurement={measurement} />
          ) : null}
          {createdResult ? (
            <div className="mt-3 rounded-md border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-900">
              <p className="font-semibold">
                새 애니메이션 아이콘 ID: {createdResult.icon.id} · 원본 시트 보존 완료
              </p>
              <code className="mt-1 block break-all text-xs">
                {createdResult.preservedSheetPath}
              </code>
              <button
                className="mt-2 rounded-md border border-emerald-300 bg-white px-3 py-2 text-xs font-semibold hover:bg-emerald-100"
                type="button"
                onClick={() => {
                  void openExportPath(createdResult.preservedSheetPath).catch((error) =>
                    setErrorMessage(getCommandErrorMessage(error)),
                  );
                }}
              >
                원본 시트 위치 열기
              </button>
            </div>
          ) : null}
          {message ? (
            <p className="mt-3 rounded-md border border-border bg-card px-4 py-3 text-sm">{message}</p>
          ) : null}
          {errorMessage ? (
            <p className="mt-3 rounded-md border border-danger bg-red-50 px-4 py-3 text-sm text-danger">
              {errorMessage}
            </p>
          ) : null}
        </div>

        <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-border bg-card px-5 py-4">
          <p className="text-xs text-muted">
            원본 시트는 수정하지 않습니다. 생성 방향은 GIF 프레임에 굽고 별도 편집 상태로 중복 적용하지
            않습니다.
          </p>
          <div className="flex gap-2">
            <button
              className="rounded-md border border-border bg-white px-4 py-2 text-sm font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
              disabled={busyAction !== null}
              type="button"
              onClick={onClose}
            >
              닫기
            </button>
            <button
              className="rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:cursor-not-allowed disabled:opacity-60"
              disabled={
                !measurementIsFresh ||
                !displayName.trim() ||
                busyAction !== null ||
                createdResult !== null
              }
              type="button"
              onClick={() => void runCreate()}
            >
              {busyAction === "create" ? "새 GIF 등록 중…" : "새 GIF 아이콘 만들기"}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

function FrameSheetCellReview({
  analysis,
  imageUrl,
  selectedIndexes,
  onSelectionChange,
}: {
  analysis: SheetGridAnalysis;
  imageUrl: string | null;
  selectedIndexes: Set<number>;
  onSelectionChange: (next: Set<number>) => void;
}) {
  const setIncluded = (cell: SheetCell, included: boolean) => {
    const next = new Set(selectedIndexes);
    if (included) {
      next.add(cell.index);
    } else {
      next.delete(cell.index);
    }
    onSelectionChange(next);
  };

  return (
    <div className="rounded-md border border-border bg-card p-3">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs font-medium">
          {selectedIndexes.size}/{analysis.cells.length}개 셀 선택
        </span>
        <div className="flex gap-2">
          <button
            className={smallButtonClass}
            type="button"
            onClick={() =>
              onSelectionChange(
                new Set(
                  analysis.cells
                    .filter((cell) => !cell.outOfBounds)
                    .map((cell) => cell.index),
                ),
              )
            }
          >
            전체 포함
          </button>
          <button
            className={smallButtonClass}
            type="button"
            onClick={() => onSelectionChange(new Set())}
          >
            전체 제외
          </button>
        </div>
      </div>
      <div className="grid max-h-56 grid-cols-2 gap-2 overflow-auto sm:grid-cols-3 lg:grid-cols-4">
        {analysis.cells.map((cell) => (
          <label
            key={cell.index}
            className={cn(
              "flex min-w-0 cursor-pointer items-center gap-2 rounded-md border border-border bg-white p-2 text-xs",
              selectedIndexes.has(cell.index) ? "border-focus bg-selected" : "",
              cell.outOfBounds ? "cursor-not-allowed border-danger opacity-60" : "",
              cell.emptyCandidate ? "border-dashed" : "",
            )}
          >
            <input
              checked={selectedIndexes.has(cell.index)}
              disabled={cell.outOfBounds}
              type="checkbox"
              onChange={(event) => setIncluded(cell, event.currentTarget.checked)}
            />
            <FrameCellPreview
              cell={cell}
              imageUrl={imageUrl}
              sheetHeight={analysis.sheetHeight}
              sheetWidth={analysis.sheetWidth}
              size={42}
            />
            <span className="min-w-0">
              <span className="block font-semibold">#{cell.index + 1}</span>
              <span className="block truncate text-muted">
                {cell.emptyCandidate ? "빈 셀 후보" : `${cell.w}×${cell.h}`}
              </span>
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}

function SortableFrameCard({
  analysis,
  frame,
  imageUrl,
  index,
  selected,
  onClick,
  onDurationChange,
}: {
  analysis: SheetGridAnalysis | null;
  frame: FrameStripItem;
  imageUrl: string | null;
  index: number;
  selected: boolean;
  onClick: (frameId: string, event: MouseEvent<HTMLButtonElement>) => void;
  onDurationChange: (durationMs: number) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: frame.id,
  });
  const cell = analysis?.cells.find((candidate) => candidate.index === frame.sourceCellIndex) ?? null;

  return (
    <article
      ref={setNodeRef}
      aria-selected={selected}
      className={cn(
        "w-32 shrink-0 rounded-md border border-border bg-card p-2",
        selected ? "border-focus bg-selected" : "",
        isDragging ? "z-10 opacity-70 shadow-lg" : "",
      )}
      role="option"
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
      }}
    >
      <div className="mb-1 flex items-center justify-between gap-1">
        <span className="text-xs font-semibold">F{index + 1}</span>
        <button
          aria-label={`프레임 ${index + 1} 드래그 정렬`}
          className="flex size-7 cursor-grab items-center justify-center rounded hover:bg-menu-hover active:cursor-grabbing"
          type="button"
          {...attributes}
          {...listeners}
        >
          <GripVertical aria-hidden="true" />
        </button>
      </div>
      <button
        className="block w-full rounded border border-border bg-preview p-1 text-left"
        type="button"
        onClick={(event) => onClick(frame.id, event)}
      >
        {cell && analysis ? (
          <FrameCellPreview
            cell={cell}
            imageUrl={imageUrl}
            sheetHeight={analysis.sheetHeight}
            sheetWidth={analysis.sheetWidth}
            size={96}
          />
        ) : (
          <span className="flex aspect-square items-center justify-center text-xs text-muted">
            셀 없음
          </span>
        )}
        <span className="mt-1 block truncate text-center text-[11px] text-muted">
          원본 셀 #{frame.sourceCellIndex + 1}
        </span>
      </button>
      <label className="mt-2 flex items-center gap-1 text-[11px] text-muted">
        <input
          aria-label={`프레임 ${index + 1} 시간`}
          className="min-w-0 flex-1 rounded border border-border bg-white px-1 py-1 text-xs text-foreground"
          max={60_000}
          min={10}
          step={10}
          type="number"
          value={frame.durationMs}
          onClick={(event) => event.stopPropagation()}
          onChange={(event) =>
            onDurationChange(Number.parseInt(event.currentTarget.value, 10) || 10)
          }
        />
        ms
      </label>
    </article>
  );
}

export function AnimationPreviewPanel({
  analysis,
  direction,
  durationMs,
  frames,
  imageUrl,
  loopCount,
  loopMode,
  measurement,
}: {
  analysis: SheetGridAnalysis | null;
  direction: FrameSheetGifDirection;
  durationMs: number;
  frames: FrameStripItem[];
  imageUrl: string | null;
  loopCount: number;
  loopMode: FrameSheetGifLoopMode;
  measurement: FrameSheetGifMeasurement | null;
}) {
  const prefersReducedMotion = usePrefersReducedMotion();
  const [isPlaying, setIsPlaying] = useState(!prefersReducedMotion);
  const [frameIndex, setFrameIndex] = useState(0);
  const [completedCycles, setCompletedCycles] = useState(0);
  const playbackKey = frames
    .map((frame) => `${frame.id}:${frame.durationMs}`)
    .join("|");

  useEffect(() => {
    setFrameIndex(0);
    setCompletedCycles(0);
    setIsPlaying(!prefersReducedMotion);
  }, [direction, loopCount, loopMode, playbackKey, prefersReducedMotion]);

  useEffect(() => {
    if (!isPlaying || frames.length === 0) {
      return;
    }
    const frame = frames[Math.min(frameIndex, frames.length - 1)];
    const timeout = window.setTimeout(() => {
      if (frameIndex < frames.length - 1) {
        setFrameIndex((current) => current + 1);
        return;
      }
      const nextCycle = completedCycles + 1;
      const shouldRepeat =
        loopMode === "infinite" || (loopMode === "count" && nextCycle < Math.max(1, loopCount));
      if (shouldRepeat) {
        setCompletedCycles(nextCycle);
        setFrameIndex(0);
      } else {
        setIsPlaying(false);
      }
    }, Math.max(10, frame.durationMs));
    return () => window.clearTimeout(timeout);
  }, [completedCycles, frameIndex, frames, isPlaying, loopCount, loopMode]);

  const currentFrame = frames[Math.min(frameIndex, Math.max(0, frames.length - 1))] ?? null;
  const currentCell =
    currentFrame && analysis
      ? analysis.cells.find((cell) => cell.index === currentFrame.sourceCellIndex) ?? null
      : null;
  const measuredPreviewUrl = measurement
    ? filePathToAssetUrl(measurement.previewPath, measurement.renderHash)
    : null;

  const restart = () => {
    setFrameIndex(0);
    setCompletedCycles(0);
    setIsPlaying(true);
  };

  return (
    <aside className="sticky top-0 flex min-w-0 flex-col gap-3 self-start rounded-md border border-border bg-card p-4 xl:col-start-3 xl:row-span-3 xl:row-start-1">
      <div>
        <h3 className="text-sm font-semibold">동작 미리보기</h3>
        <p className="mt-1 text-xs text-muted">
          프레임 편집은 즉시 재생에 반영됩니다. 용량은 실제 GIF 인코딩 후 별도로 측정합니다.
        </p>
      </div>
      <div className="flex min-h-56 items-center justify-center rounded-md border border-border bg-preview p-4">
        {currentCell && analysis ? (
          <FrameCellPreview
            cell={currentCell}
            imageUrl={imageUrl}
            sheetHeight={analysis.sheetHeight}
            sheetWidth={analysis.sheetWidth}
            size={220}
          />
        ) : (
          <span className="text-sm text-muted">재생할 프레임이 없습니다.</span>
        )}
      </div>
      <div className="flex items-center justify-center gap-2">
        <button
          aria-label={isPlaying ? "일시정지" : "재생"}
          className={smallButtonClass}
          disabled={frames.length === 0}
          type="button"
          onClick={() => setIsPlaying((current) => !current)}
        >
          {isPlaying ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
          {isPlaying ? "일시정지" : "재생"}
        </button>
        <button
          className={smallButtonClass}
          disabled={frames.length === 0}
          type="button"
          onClick={restart}
        >
          <RotateCcw aria-hidden="true" />
          처음부터
        </button>
      </div>
      {prefersReducedMotion ? (
        <p className="text-xs text-muted">시스템의 동작 줄이기 설정 때문에 기본 재생을 멈췄습니다.</p>
      ) : null}
      <dl className="grid grid-cols-2 gap-2 text-xs">
        <PreviewStat label="현재 프레임" value={frames.length ? `${frameIndex + 1}/${frames.length}` : "-"} />
        <PreviewStat label="최종 프레임" value={`${frames.length}개`} />
        <PreviewStat label="총 시간" value={formatDuration(durationMs)} />
        <PreviewStat
          label="생성 방향"
          value={
            direction === "forward" ? "정방향" : direction === "reverse" ? "역방향" : "핑퐁"
          }
        />
      </dl>
      {measuredPreviewUrl ? (
        <div className="rounded-md border border-border bg-white p-3">
          <p className="mb-2 text-xs font-semibold">실제 인코딩 GIF</p>
          <img
            alt="실제 인코딩된 GIF 미리보기"
            className="mx-auto max-h-44 max-w-full object-contain [image-rendering:auto]"
            src={measuredPreviewUrl}
          />
        </div>
      ) : null}
    </aside>
  );
}

function FrameCellPreview({
  cell,
  imageUrl,
  sheetHeight,
  sheetWidth,
  size,
}: {
  cell: SheetCell;
  imageUrl: string | null;
  sheetHeight: number;
  sheetWidth: number;
  size: number;
}) {
  const width = Math.max(1, cell.w);
  const height = Math.max(1, cell.h);
  return (
    <span
      className="relative block shrink-0 overflow-hidden rounded bg-[linear-gradient(45deg,#e5e7eb_25%,transparent_25%),linear-gradient(-45deg,#e5e7eb_25%,transparent_25%),linear-gradient(45deg,transparent_75%,#e5e7eb_75%),linear-gradient(-45deg,transparent_75%,#e5e7eb_75%)] bg-[length:12px_12px] bg-[position:0_0,0_6px,6px_-6px,-6px_0]"
      style={{
        aspectRatio: `${width}/${height}`,
        width: size,
        maxWidth: "100%",
      }}
    >
      {imageUrl ? (
        <img
          alt=""
          className="pointer-events-none absolute max-w-none select-none"
          draggable={false}
          src={imageUrl}
          style={{
            height: `${(Math.max(1, sheetHeight) / height) * 100}%`,
            left: `${(-cell.x / width) * 100}%`,
            top: `${(-cell.y / height) * 100}%`,
            width: `${(Math.max(1, sheetWidth) / width) * 100}%`,
          }}
        />
      ) : null}
    </span>
  );
}

function MeasurementSummary({
  collection,
  measurement,
}: {
  collection: CollectionSummary;
  measurement: FrameSheetGifMeasurement;
}) {
  return (
    <div
      className={cn(
        "mt-3 rounded-md border px-4 py-3 text-sm",
        measurement.passesByteLimit
          ? "border-emerald-200 bg-emerald-50 text-emerald-950"
          : "border-amber-300 bg-amber-50 text-amber-950",
      )}
    >
      <p className="font-semibold">
        실제 GIF {formatBytes(measurement.byteSize)} / {formatBytes(measurement.maxBytes)}
        {measurement.passesByteLimit ? " · 모음 제한 이내" : " · 모음 제한 초과 경고"}
      </p>
      <p className="mt-1 text-xs">
        {measurement.width}×{measurement.height}px · {measurement.generatedFrameCount}프레임 ·{" "}
        {formatDuration(measurement.durationMs)} · {collection.name} 기준
      </p>
      {measurement.warnings.length > 0 ? (
        <ul className="mt-2 list-disc space-y-1 pl-5 text-xs">
          {measurement.warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function PreviewStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-white px-3 py-2">
      <dt className="text-muted">{label}</dt>
      <dd className="mt-1 font-semibold">{value}</dd>
    </div>
  );
}

function useObjectUrl(file: File | null) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!file) {
      setUrl(null);
      return;
    }
    const nextUrl = URL.createObjectURL(file);
    setUrl(nextUrl);
    return () => URL.revokeObjectURL(nextUrl);
  }, [file]);
  return url;
}

function formatDuration(durationMs: number) {
  if (durationMs < 1_000) {
    return `${durationMs}ms`;
  }
  return `${(durationMs / 1_000).toFixed(durationMs % 1_000 === 0 ? 0 : 2)}초`;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function maxSourceFramesForRecipe(
  direction: FrameSheetGifDirection,
  loopMode: FrameSheetGifLoopMode,
) {
  if (direction !== "pingpong") {
    return MAX_GIF_FRAMES;
  }
  return loopMode === "once" ? 250 : 251;
}

const smallButtonClass =
  "inline-flex items-center gap-1 rounded-md border border-border bg-white px-2.5 py-2 text-xs font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted";

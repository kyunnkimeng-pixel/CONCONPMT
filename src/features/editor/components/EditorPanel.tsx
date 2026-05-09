import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CircleDot,
  type LucideIcon,
  MoveDown,
  MoveDownLeft,
  MoveDownRight,
  MoveLeft,
  MoveRight,
  MoveUp,
  MoveUpLeft,
  MoveUpRight,
  RefreshCcw,
  RotateCcw,
  Save,
  X,
} from "lucide-react";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import { applyIconCrop, getIconEditorState } from "@/features/editor/api";
import {
  aspectRatioForShape,
  centeredFreeCrop,
  constrainCropToAspect,
  fixedCropForPreset,
} from "@/features/editor/crop-math";
import { CropCanvas } from "@/features/editor/components/CropCanvas";
import type {
  CropMode,
  CropRect,
  GifLoopMode,
  IconEditorState,
  IconShape,
  PresetPosition,
} from "@/features/editor/types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

interface EditorPanelProps {
  collection: CollectionSummary;
  iconId: string;
  onClose: () => void;
  onIconUpdated: (icon: IconSummary) => void;
}

interface EditorDraft {
  shape: IconShape;
  cropMode: CropMode;
  crop: CropRect;
  presetPosition: PresetPosition;
  cellWidth: number;
  cellHeight: number;
  gifLoopMode: GifLoopMode;
  gifLoopCount: number;
}

const SHAPE_OPTIONS: Array<{ value: IconShape; label: string }> = [
  { value: "single", label: "단일" },
  { value: "horizontal_double", label: "가로 2칸" },
  { value: "vertical_double", label: "세로 2칸" },
];

const CROP_MODE_OPTIONS: Array<{ value: CropMode; label: string }> = [
  { value: "free", label: "자유" },
  { value: "fixed", label: "고정" },
];

const GIF_LOOP_OPTIONS: Array<{ value: GifLoopMode; label: string }> = [
  { value: "preserve", label: "원본 유지" },
  { value: "infinite", label: "무한 반복" },
  { value: "once", label: "한 번" },
  { value: "count", label: "직접 입력" },
];

const PRESET_OPTIONS: Array<{
  value: PresetPosition;
  label: string;
  Icon: LucideIcon;
}> = [
  { value: "top_left", label: "왼쪽 위", Icon: MoveUpLeft },
  { value: "top", label: "위", Icon: MoveUp },
  { value: "top_right", label: "오른쪽 위", Icon: MoveUpRight },
  { value: "left", label: "왼쪽", Icon: MoveLeft },
  { value: "center", label: "가운데", Icon: CircleDot },
  { value: "right", label: "오른쪽", Icon: MoveRight },
  { value: "bottom_left", label: "왼쪽 아래", Icon: MoveDownLeft },
  { value: "bottom", label: "아래", Icon: MoveDown },
  { value: "bottom_right", label: "오른쪽 아래", Icon: MoveDownRight },
];

export function EditorPanel({
  collection,
  iconId,
  onClose,
  onIconUpdated,
}: EditorPanelProps) {
  const [editorState, setEditorState] = useState<IconEditorState | null>(null);
  const [draft, setDraft] = useState<EditorDraft | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isApplying, setIsApplying] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const loadEditorState = useCallback(async () => {
    setIsLoading(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const state = await getIconEditorState(collection.id, iconId);
      setEditorState(state);
      setDraft(draftFromState(state, collection));
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
      setEditorState(null);
      setDraft(null);
    } finally {
      setIsLoading(false);
    }
  }, [collection, iconId]);

  useEffect(() => {
    void loadEditorState();
  }, [loadEditorState]);

  const sourceDimensions = useMemo(() => {
    if (!editorState) {
      return null;
    }

    return {
      width: editorState.source.width,
      height: editorState.source.height,
    };
  }, [editorState]);

  const updateShape = (shape: IconShape) => {
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const crop =
        current.cropMode === "fixed"
          ? fixedCropForPreset(
              sourceDimensions,
              shape,
              current.cellWidth,
              current.cellHeight,
              current.presetPosition,
            )
          : constrainCropToAspect(
              current.crop,
              sourceDimensions,
              aspectRatioForShape(shape, current.cellWidth, current.cellHeight),
            );

      return {
        ...current,
        shape,
        crop,
        presetPosition: current.cropMode === "fixed" ? current.presetPosition : "custom",
      };
    });
  };

  const updateCropMode = (cropMode: CropMode) => {
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const presetPosition =
        cropMode === "fixed" && current.presetPosition === "custom"
          ? "center"
          : current.presetPosition;
      const crop =
        cropMode === "fixed"
          ? fixedCropForPreset(
              sourceDimensions,
              current.shape,
              current.cellWidth,
              current.cellHeight,
              presetPosition,
            )
          : constrainCropToAspect(
              current.crop,
              sourceDimensions,
              aspectRatioForShape(current.shape, current.cellWidth, current.cellHeight),
            );

      return {
        ...current,
        cropMode,
        crop,
        presetPosition,
      };
    });
  };

  const updateCellSize = (field: "cellWidth" | "cellHeight", value: number) => {
    if (!Number.isFinite(value) || value < 1) {
      return;
    }

    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const nextDraft = {
        ...current,
        [field]: Math.round(value),
      };
      const crop =
        nextDraft.cropMode === "fixed"
          ? fixedCropForPreset(
              sourceDimensions,
              nextDraft.shape,
              nextDraft.cellWidth,
              nextDraft.cellHeight,
              nextDraft.presetPosition,
            )
          : constrainCropToAspect(
              nextDraft.crop,
              sourceDimensions,
              aspectRatioForShape(
                nextDraft.shape,
                nextDraft.cellWidth,
                nextDraft.cellHeight,
              ),
            );

      return {
        ...nextDraft,
        crop,
      };
    });
  };

  const applyPreset = (presetPosition: PresetPosition) => {
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      return {
        ...current,
        presetPosition,
        crop: fixedCropForPreset(
          sourceDimensions,
          current.shape,
          current.cellWidth,
          current.cellHeight,
          presetPosition,
        ),
      };
    });
  };

  const handleCropChange = (crop: CropRect) => {
    setDraft((current) =>
      current
        ? {
            ...current,
            crop,
            presetPosition: "custom",
          }
        : current,
    );
  };

  const useCollectionDefaultSize = () => {
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const nextDraft = {
        ...current,
        cellWidth: collection.defaultCellWidth,
        cellHeight: collection.defaultCellHeight,
      };

      return {
        ...nextDraft,
        crop:
          nextDraft.cropMode === "fixed"
            ? fixedCropForPreset(
                sourceDimensions,
                nextDraft.shape,
                nextDraft.cellWidth,
                nextDraft.cellHeight,
                nextDraft.presetPosition,
              )
            : constrainCropToAspect(
                nextDraft.crop,
                sourceDimensions,
                aspectRatioForShape(
                  nextDraft.shape,
                  nextDraft.cellWidth,
                  nextDraft.cellHeight,
                ),
              ),
      };
    });
  };

  const resetCropToCenter = () => {
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      return {
        ...current,
        presetPosition: "center",
        crop:
          current.cropMode === "fixed"
            ? fixedCropForPreset(
                sourceDimensions,
                current.shape,
                current.cellWidth,
                current.cellHeight,
                "center",
              )
            : centeredFreeCrop(
                sourceDimensions,
                current.shape,
                current.cellWidth,
                current.cellHeight,
              ),
      };
    });
  };

  const revertSavedSettings = () => {
    if (!editorState) {
      return;
    }

    setDraft(draftFromState(editorState, collection));
    setStatusMessage("저장된 편집값으로 되돌렸습니다.");
    setErrorMessage(null);
  };

  const handleApply = async () => {
    if (!draft) {
      return;
    }

    setIsApplying(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const updatedIcon = await applyIconCrop(collection.id, {
        iconId,
        shape: draft.shape,
        cropMode: draft.cropMode,
        cropX: draft.crop.x,
        cropY: draft.crop.y,
        cropW: draft.crop.width,
        cropH: draft.crop.height,
        presetPosition: draft.presetPosition,
        cellWidth: draft.cellWidth,
        cellHeight: draft.cellHeight,
        gifLoopMode: draft.gifLoopMode,
        gifLoopCount: draft.gifLoopMode === "count" ? draft.gifLoopCount : null,
      });
      onIconUpdated(updatedIcon);

      const nextState = await getIconEditorState(collection.id, iconId);
      setEditorState(nextState);
      setDraft(draftFromState(nextState, collection));
      setStatusMessage("크롭을 적용했습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsApplying(false);
    }
  };

  return (
    <aside className="flex h-full w-[430px] shrink-0 flex-col border-l border-border bg-surface">
      <header className="flex items-center justify-between border-b border-border px-5 py-4">
        <div className="min-w-0">
          <h2 className="truncate text-base font-semibold tracking-normal">아이콘 편집</h2>
          <p className="mt-1 truncate text-xs text-muted">
            {editorState?.icon.displayName ?? "편집 정보"}
          </p>
        </div>
        <button
          aria-label="편집 패널 닫기"
          className="inline-flex size-9 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
          type="button"
          onClick={onClose}
        >
          <X aria-hidden="true" />
        </button>
      </header>

      <div className="flex-1 overflow-auto px-5 py-4">
        {isLoading ? (
          <p className="text-sm text-muted">편집 정보를 불러오는 중입니다.</p>
        ) : null}

        {!isLoading && errorMessage && !editorState ? (
          <p className="text-sm text-danger" role="alert">
            {errorMessage}
          </p>
        ) : null}

        {!isLoading && editorState && draft && sourceDimensions ? (
          <div className="flex flex-col gap-5">
            <section className="flex flex-col gap-3">
              <div>
                <h3 className="text-sm font-semibold tracking-normal">원본 이미지</h3>
                <p className="mt-1 truncate text-xs text-muted">
                  {editorState.source.originalFilename} · {editorState.source.width}×
                  {editorState.source.height}px · {formatBytes(editorState.source.byteSize)}
                </p>
              </div>
              <CropCanvas
                cellHeight={draft.cellHeight}
                cellWidth={draft.cellWidth}
                crop={draft.crop}
                cropMode={draft.cropMode}
                shape={draft.shape}
                sourceHeight={editorState.source.height}
                sourceUrl={editorState.source.originalImageUrl}
                sourceWidth={editorState.source.width}
                onCropChange={handleCropChange}
              />
            </section>

            <section className="flex flex-col gap-3 border-t border-border pt-4">
              <h3 className="text-sm font-semibold tracking-normal">모양</h3>
              <div className="grid grid-cols-3 gap-2">
                {SHAPE_OPTIONS.map((option) => (
                  <button
                    className={segmentedButtonClass(draft.shape === option.value)}
                    key={option.value}
                    type="button"
                    onClick={() => updateShape(option.value)}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </section>

            <section className="flex flex-col gap-3 border-t border-border pt-4">
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-semibold tracking-normal">셀 크기</h3>
                <button
                  className="rounded-md border border-border bg-white px-2.5 py-1.5 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                  type="button"
                  onClick={useCollectionDefaultSize}
                >
                  모음 기본값
                </button>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  너비
                  <input
                    className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    min={1}
                    type="number"
                    value={draft.cellWidth}
                    onChange={(event) =>
                      updateCellSize("cellWidth", event.currentTarget.valueAsNumber)
                    }
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  높이
                  <input
                    className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    min={1}
                    type="number"
                    value={draft.cellHeight}
                    onChange={(event) =>
                      updateCellSize("cellHeight", event.currentTarget.valueAsNumber)
                    }
                  />
                </label>
              </div>
            </section>

            <section className="flex flex-col gap-3 border-t border-border pt-4">
              <h3 className="text-sm font-semibold tracking-normal">크롭 모드</h3>
              <div className="grid grid-cols-2 gap-2">
                {CROP_MODE_OPTIONS.map((option) => (
                  <button
                    className={segmentedButtonClass(draft.cropMode === option.value)}
                    key={option.value}
                    type="button"
                    onClick={() => updateCropMode(option.value)}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </section>

            <section className="flex flex-col gap-3 border-t border-border pt-4">
              <h3 className="text-sm font-semibold tracking-normal">고정 위치</h3>
              <div className="grid grid-cols-3 gap-2">
                {PRESET_OPTIONS.map(({ value, label, Icon }) => (
                  <button
                    aria-label={label}
                    className={cn(
                      "inline-flex h-10 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted",
                      draft.presetPosition === value &&
                        draft.cropMode === "fixed" &&
                        "border-focus bg-selected",
                    )}
                    disabled={draft.cropMode !== "fixed"}
                    key={value}
                    title={label}
                    type="button"
                    onClick={() => applyPreset(value)}
                  >
                    <Icon aria-hidden={true} />
                  </button>
                ))}
              </div>
            </section>

            {editorState.source.isAnimated ? (
              <section className="flex flex-col gap-3 border-t border-border pt-4">
                <h3 className="text-sm font-semibold tracking-normal">GIF 반복</h3>
                <select
                  className="rounded-md border border-border bg-white px-3 py-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                  value={draft.gifLoopMode}
                  onChange={(event) =>
                    setDraft((current) =>
                      current
                        ? {
                            ...current,
                            gifLoopMode: event.currentTarget.value as GifLoopMode,
                          }
                        : current,
                    )
                  }
                >
                  {GIF_LOOP_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                {draft.gifLoopMode === "count" ? (
                  <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                    반복 횟수
                    <input
                      className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                      min={1}
                      type="number"
                      value={draft.gifLoopCount}
                      onChange={(event) =>
                        setDraft((current) =>
                          current
                            ? {
                                ...current,
                                gifLoopCount: Math.max(
                                  1,
                                  Math.round(event.currentTarget.valueAsNumber || 1),
                                ),
                              }
                            : current,
                        )
                      }
                    />
                  </label>
                ) : null}
              </section>
            ) : null}

            <section className="flex flex-col gap-3 border-t border-border pt-4">
              <h3 className="text-sm font-semibold tracking-normal">처리 미리보기</h3>
              <div
                className="flex items-center justify-center rounded-md border border-border bg-preview"
                style={{
                  minHeight: collection.previewHeight + 24,
                }}
              >
                {editorState.icon.currentPreviewUrl ? (
                  <img
                    alt=""
                    className="object-contain"
                    src={editorState.icon.currentPreviewUrl}
                    style={{
                      maxHeight: collection.previewHeight,
                      maxWidth: collection.previewWidth,
                    }}
                  />
                ) : (
                  <span className="text-sm text-muted">아직 적용된 미리보기가 없습니다.</span>
                )}
              </div>
            </section>

            {statusMessage ? (
              <p className="text-sm text-muted" role="status">
                {statusMessage}
              </p>
            ) : null}
            {errorMessage ? (
              <p className="text-sm text-danger" role="alert">
                {errorMessage}
              </p>
            ) : null}
          </div>
        ) : null}
      </div>

      <footer className="flex items-center justify-between gap-2 border-t border-border px-5 py-4">
        <button
          className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
          disabled={!draft || isApplying}
          type="button"
          onClick={resetCropToCenter}
        >
          <RefreshCcw aria-hidden="true" />
          초기화
        </button>
        <div className="flex items-center gap-2">
          <button
            className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
            disabled={!editorState || isApplying}
            type="button"
            onClick={revertSavedSettings}
          >
            <RotateCcw aria-hidden="true" />
            되돌리기
          </button>
          <button
            className="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
            disabled={!draft || isApplying}
            type="button"
            onClick={() => {
              void handleApply();
            }}
          >
            <Save aria-hidden="true" />
            {isApplying ? "적용 중" : "적용"}
          </button>
        </div>
      </footer>
    </aside>
  );
}

function draftFromState(
  state: IconEditorState,
  collection: CollectionSummary,
): EditorDraft {
  return {
    shape: state.icon.shape,
    cropMode: state.crop.cropMode,
    crop: {
      x: state.crop.cropX,
      y: state.crop.cropY,
      width: state.crop.cropW,
      height: state.crop.cropH,
    },
    presetPosition: state.crop.presetPosition,
    cellWidth: state.icon.cellWidthOverride ?? collection.defaultCellWidth,
    cellHeight: state.icon.cellHeightOverride ?? collection.defaultCellHeight,
    gifLoopMode: state.icon.gifLoopMode,
    gifLoopCount: state.icon.gifLoopCount ?? 1,
  };
}

function segmentedButtonClass(isSelected: boolean) {
  return cn(
    "rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
    isSelected && "border-focus bg-selected",
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

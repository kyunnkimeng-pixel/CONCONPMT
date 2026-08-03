import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type {
  CSSProperties,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent,
} from "react";
import {
  CircleDot,
  FlipHorizontal2,
  FlipVertical2,
  LoaderCircle,
  type LucideIcon,
  MoveDown,
  MoveDownLeft,
  MoveDownRight,
  MoveLeft,
  MoveRight,
  MoveUp,
  MoveUpLeft,
  MoveUpRight,
  Pencil,
  RefreshCcw,
  RotateCcw,
  RotateCw,
  Save,
  Undo2,
  X,
} from "lucide-react";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import {
  nextAdvancedEditorMode,
  type AdvancedEditorMode,
} from "@/features/editor/advanced-editor-tabs";
import {
  applyIconCrop,
  getIconEditorState,
  pickTextOverlayFont,
  previewIconEffects,
  repairAiToOriginal,
  updateIconEffects,
  updateIconTextOverlay,
} from "@/features/editor/api";
import {
  aspectRatioForShape,
  centeredFreeCrop,
  constrainCropToAspect,
  fixedCropForPreset,
} from "@/features/editor/crop-math";
import {
  effectPreviewRequestKey,
  hasUnsavedEditorChanges,
  isAiSourceRepairRequired,
  isEditorStateResponseCurrent,
  isRevisionConflict,
  isTextOverlayDraftDirty,
} from "@/features/editor/editor-state-guards";
import { AiReviewSection } from "@/features/editor/components/AiReviewSection";
import { CropCanvas } from "@/features/editor/components/CropCanvas";
import { EditorOutputPreview } from "@/features/editor/components/EditorOutputPreview";
import {
  effectRecipeSignature,
  effectRecipeStateSignature,
  normalizeEffectRecipe,
} from "@/features/editor/effect-recipe-model";
import {
  flipIconDraft,
  rotateIconDraft,
  sourceViewportGeometry,
  transformSummary,
} from "@/features/editor/transform-math";
import {
  applyGifOriginalPlaybackToPreview,
  applyOptimizationCandidate,
  applyOptimizationCandidateToPreview,
  clearOptimizationCandidate,
  generateGifOptimizationCandidates,
  generateStaticOptimizationCandidates,
  listExportProfiles,
  previewGifPlaybackFps,
} from "@/features/export/api";
import type {
  ExportProfile,
  OptimizationCandidate,
  OptimizationResult,
} from "@/features/export/types";
import type {
  CropMode,
  CropRect,
  EffectRecipeV1,
  GifLoopMode,
  IconEffectPreview,
  IconEditorState,
  IconShape,
  PresetPosition,
  TextOverlaySettings,
} from "@/features/editor/types";
import type { IconRevealAction } from "@/features/icons/icon-reveal";
import { filePathToAssetUrl } from "@/lib/asset-url";
import { getCommandErrorMessage } from "@/lib/tauri";
import { useModalFocus } from "@/lib/use-modal-focus";
import { cn } from "@/lib/utils";

const LazyEffectRecipeEditor = lazy(async () => {
  const module = await import(
    "@/features/editor/components/EffectRecipeEditor"
  );
  return { default: module.EffectRecipeEditor };
});

const LazyMotionEditorSection = lazy(async () => {
  const module = await import(
    "@/features/editor/components/MotionEditorSection"
  );
  return { default: module.MotionEditorSection };
});

const nonDraggableImageStyle: CSSProperties & { WebkitUserDrag: string } = {
  WebkitUserDrag: "none",
  userSelect: "none",
};

interface EditorPanelProps {
  collection: CollectionSummary;
  iconId: string;
  onDirtyChange?: (dirty: boolean) => void;
  onClose: () => void;
  onIconUpdated: (
    icon: IconSummary,
    options?: { silent?: boolean },
  ) => void;
  onRevealIcon: (
    iconId: string,
    action: IconRevealAction,
  ) => boolean | Promise<boolean>;
  onAiModalOpenChange?: (open: boolean) => void;
  onBusyChange?: (busy: boolean) => void;
  suppressBackgroundLiveRegions?: boolean;
}

interface EditorDraft {
  shape: IconShape;
  cropMode: CropMode;
  crop: CropRect;
  presetPosition: PresetPosition;
  cellWidth: number;
  cellHeight: number;
  transformQuarterTurns: 0 | 1 | 2 | 3;
  transformFlipHorizontal: boolean;
  transformFlipVertical: boolean;
  pieceIds: string[];
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

const EDITOR_PANEL_MIN_WIDTH = 420;
const EDITOR_PANEL_MAX_WIDTH = 760;
const EDITOR_PANEL_DEFAULT_WIDTH = 620;
const EDITOR_PANEL_WIDTH_STORAGE_KEY = "pmtconcon.editorPanelWidth";

const GIF_LOOP_OPTIONS: Array<{ value: GifLoopMode; label: string }> = [
  { value: "preserve", label: "원본 유지" },
  { value: "infinite", label: "무한 반복" },
  { value: "pingpong", label: "핑퐁 반복" },
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
  onRevealIcon,
  onDirtyChange,
  onAiModalOpenChange,
  onBusyChange,
  suppressBackgroundLiveRegions = false,
}: EditorPanelProps) {
  const [editorState, setEditorState] = useState<IconEditorState | null>(null);
  const [draft, setDraft] = useState<EditorDraft | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isApplying, setIsApplying] = useState(false);
  const [isAiMutationBusy, setIsAiMutationBusy] = useState(false);
  const [isAiDialogOpen, setIsAiDialogOpen] = useState(false);
  const suppressLiveRegions =
    isAiDialogOpen || suppressBackgroundLiveRegions;
  const [isAiRepairing, setIsAiRepairing] = useState(false);
  const [canRepairAiSource, setCanRepairAiSource] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isAdvancedOpen, setIsAdvancedOpen] = useState(false);
  const [isAdvancedDirty, setIsAdvancedDirty] = useState(false);
  const resizeStartRef = useRef<{ pointerX: number; width: number } | null>(null);
  const loadRequestIdRef = useRef(0);
  const activeIconIdRef = useRef(iconId);
  activeIconIdRef.current = iconId;
  const [panelWidth, setPanelWidth] = useState(() => {
    const savedWidth = window.localStorage.getItem(EDITOR_PANEL_WIDTH_STORAGE_KEY);
    const parsedWidth = savedWidth ? Number.parseInt(savedWidth, 10) : NaN;
    return clampPanelWidth(
      Number.isFinite(parsedWidth) ? parsedWidth : EDITOR_PANEL_DEFAULT_WIDTH,
    );
  });

  const loadEditorState = useCallback(async () => {
    const requestId = loadRequestIdRef.current + 1;
    loadRequestIdRef.current = requestId;
    setIsLoading(true);
    setCanRepairAiSource(false);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const state = await getIconEditorState(collection.id, iconId);
      if (loadRequestIdRef.current !== requestId) {
        return;
      }
      setEditorState(state);
      setDraft(draftFromState(state, collection));
      setCanRepairAiSource(false);
    } catch (error) {
      if (loadRequestIdRef.current !== requestId) {
        return;
      }
      setCanRepairAiSource(isAiSourceRepairRequired(error));
      setErrorMessage(getCommandErrorMessage(error));
      setEditorState(null);
      setDraft(null);
    } finally {
      if (loadRequestIdRef.current === requestId) {
        setIsLoading(false);
      }
    }
  }, [collection, iconId]);

  useEffect(() => {
    setIsAiRepairing(false);
    setIsAiMutationBusy(false);
    setCanRepairAiSource(false);
    void loadEditorState();
    return () => {
      loadRequestIdRef.current += 1;
    };
  }, [loadEditorState]);

  const repairToOriginalSource = async () => {
    if (!canRepairAiSource || isAiMutationBusy) {
      return;
    }

    const requestId = loadRequestIdRef.current + 1;
    loadRequestIdRef.current = requestId;
    const requestedIconId = iconId;
    const isCurrentRepair = () =>
      loadRequestIdRef.current === requestId &&
      isEditorStateResponseCurrent(activeIconIdRef.current, requestedIconId);

    setIsAiRepairing(true);
    setIsAiMutationBusy(true);
    setStatusMessage(null);
    try {
      await repairAiToOriginal(collection.id, requestedIconId);
      if (!isCurrentRepair()) {
        return;
      }

      const nextState = await getIconEditorState(collection.id, requestedIconId);
      if (
        !isCurrentRepair() ||
        !isEditorStateResponseCurrent(requestedIconId, nextState.icon.id)
      ) {
        return;
      }

      setEditorState(nextState);
      setDraft(draftFromState(nextState, collection));
      setCanRepairAiSource(false);
      setErrorMessage(null);
      setStatusMessage(
        "손상된 AI 소스 연결을 해제하고 보존된 원본 소스로 복구했습니다.",
      );
      onIconUpdated(nextState.icon);
    } catch (error) {
      if (isCurrentRepair()) {
        setErrorMessage(
          `원본 소스로 복구하지 못했습니다: ${getCommandErrorMessage(error)}`,
        );
      }
    } finally {
      if (isCurrentRepair()) {
        setIsAiRepairing(false);
        setIsAiMutationBusy(false);
      }
    }
  };

  useEffect(() => {
    window.localStorage.setItem(EDITOR_PANEL_WIDTH_STORAGE_KEY, String(panelWidth));
  }, [panelWidth]);

  const startResize = (event: PointerEvent<HTMLButtonElement>) => {
    event.preventDefault();
    resizeStartRef.current = {
      pointerX: event.clientX,
      width: panelWidth,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const updatePanelResize = (event: PointerEvent<HTMLButtonElement>) => {
    if (!resizeStartRef.current) {
      return;
    }

    const delta = resizeStartRef.current.pointerX - event.clientX;
    setPanelWidth(clampPanelWidth(resizeStartRef.current.width + delta));
  };

  const stopResize = (event: PointerEvent<HTMLButtonElement>) => {
    resizeStartRef.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
  };

  const sourceDimensions = useMemo(() => {
    if (!editorState) {
      return null;
    }

    return {
      width: editorState.source.width,
      height: editorState.source.height,
    };
  }, [editorState]);
  const hasUnsavedDraft = useMemo(() => {
    if (!editorState || !draft) {
      return false;
    }
    return JSON.stringify(draft) !== JSON.stringify(draftFromState(editorState, collection));
  }, [collection, draft, editorState]);
  const hasUnsavedChanges = hasUnsavedEditorChanges(
    hasUnsavedDraft,
    isAdvancedDirty,
  );
  const isEditorBusy = isApplying || isAiMutationBusy;

  useEffect(() => {
    onDirtyChange?.(hasUnsavedChanges);
  }, [hasUnsavedChanges, onDirtyChange]);

  useEffect(
    () => () => {
      onDirtyChange?.(false);
    },
    [onDirtyChange],
  );

  useEffect(() => {
    onBusyChange?.(isEditorBusy);
    return () => {
      if (isEditorBusy) {
        onBusyChange?.(false);
      }
    };
  }, [isEditorBusy, onBusyChange]);

  const handleAiModalOpenChange = useCallback(
    (open: boolean) => {
      setIsAiDialogOpen(open);
      onAiModalOpenChange?.(open);
    },
    [onAiModalOpenChange],
  );

  const requestPanelClose = useCallback(() => {
    if (isAiMutationBusy) {
      return;
    }
    if (
      hasUnsavedChanges &&
      !window.confirm("저장하지 않은 편집 변경을 버리고 패널을 닫을까요?")
    ) {
      return;
    }
    onClose();
  }, [hasUnsavedChanges, isAiMutationBusy, onClose]);


  const updateShape = (shape: IconShape) => {
    if (isAiMutationBusy) {
      return;
    }
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const nextDraft = { ...current, shape };
      const sourceGeometry = sourceViewportGeometry(nextDraft);
      const crop =
        current.cropMode === "fixed"
          ? fixedCropForPreset(
              sourceDimensions,
              sourceGeometry.shape,
              sourceGeometry.cellWidth,
              sourceGeometry.cellHeight,
              current.presetPosition,
            )
          : constrainCropToAspect(
              current.crop,
              sourceDimensions,
              aspectRatioForShape(
                sourceGeometry.shape,
                sourceGeometry.cellWidth,
                sourceGeometry.cellHeight,
              ),
            );

      return {
        ...nextDraft,
        crop,
        presetPosition: current.cropMode === "fixed" ? current.presetPosition : "custom",
      };
    });
  };

  const updateCropMode = (cropMode: CropMode) => {
    if (isAiMutationBusy) {
      return;
    }
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const presetPosition =
        cropMode === "fixed" && current.presetPosition === "custom"
          ? "center"
          : current.presetPosition;
      const sourceGeometry = sourceViewportGeometry(current);
      const crop =
        cropMode === "fixed"
          ? fixedCropForPreset(
              sourceDimensions,
              sourceGeometry.shape,
              sourceGeometry.cellWidth,
              sourceGeometry.cellHeight,
              presetPosition,
            )
          : centeredFreeCrop(
              sourceDimensions,
              sourceGeometry.shape,
              sourceGeometry.cellWidth,
              sourceGeometry.cellHeight,
              0.8,
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
    if (isAiMutationBusy) {
      return;
    }
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
      const sourceGeometry = sourceViewportGeometry(nextDraft);
      const crop =
        nextDraft.cropMode === "fixed"
          ? fixedCropForPreset(
              sourceDimensions,
              sourceGeometry.shape,
              sourceGeometry.cellWidth,
              sourceGeometry.cellHeight,
              nextDraft.presetPosition,
            )
          : constrainCropToAspect(
              nextDraft.crop,
              sourceDimensions,
              aspectRatioForShape(
                sourceGeometry.shape,
                sourceGeometry.cellWidth,
                sourceGeometry.cellHeight,
              ),
            );

      return {
        ...nextDraft,
        crop,
      };
    });
  };

  const applyPreset = (presetPosition: PresetPosition) => {
    if (isAiMutationBusy) {
      return;
    }
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const sourceGeometry = sourceViewportGeometry(current);
      return {
        ...current,
        presetPosition,
        crop: fixedCropForPreset(
          sourceDimensions,
          sourceGeometry.shape,
          sourceGeometry.cellWidth,
          sourceGeometry.cellHeight,
          presetPosition,
        ),
      };
    });
  };

  const handleCropChange = (crop: CropRect) => {
    if (isAiMutationBusy) {
      return;
    }
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

  const applyFlip = (axis: "horizontal" | "vertical") => {
    if (isAiMutationBusy) {
      return;
    }
    setDraft((current) => (current ? flipIconDraft(current, axis) : current));
    setStatusMessage(
      axis === "horizontal"
        ? "현재 결과를 좌우로 반전했습니다. 적용을 눌러 저장하세요."
        : "현재 결과를 상하로 반전했습니다. 적용을 눌러 저장하세요.",
    );
    setErrorMessage(null);
  };

  const applyQuarterTurn = (direction: "left" | "right") => {
    if (isAiMutationBusy) {
      return;
    }
    setDraft((current) => (current ? rotateIconDraft(current, direction) : current));
    setStatusMessage(
      direction === "left"
        ? "현재 결과를 왼쪽으로 90° 회전했습니다. 셀 크기와 2칸 방향도 함께 바뀝니다."
        : "현재 결과를 오른쪽으로 90° 회전했습니다. 셀 크기와 2칸 방향도 함께 바뀝니다.",
    );
    setErrorMessage(null);
  };

  const useCollectionDefaultSize = () => {
    if (isAiMutationBusy) {
      return;
    }
    setDraft((current) => {
      if (!current || !sourceDimensions) {
        return current;
      }

      const nextDraft = {
        ...current,
        cellWidth: collection.defaultCellWidth,
        cellHeight: collection.defaultCellHeight,
      };
      const sourceGeometry = sourceViewportGeometry(nextDraft);

      return {
        ...nextDraft,
        crop:
          nextDraft.cropMode === "fixed"
            ? fixedCropForPreset(
                sourceDimensions,
                sourceGeometry.shape,
                sourceGeometry.cellWidth,
                sourceGeometry.cellHeight,
                nextDraft.presetPosition,
              )
            : constrainCropToAspect(
                nextDraft.crop,
                sourceDimensions,
                aspectRatioForShape(
                  sourceGeometry.shape,
                  sourceGeometry.cellWidth,
                  sourceGeometry.cellHeight,
                ),
              ),
      };
    });
  };

  const resetCropToDefault = () => {
    if (isAiMutationBusy) {
      return;
    }
    if (!sourceDimensions) {
      return;
    }

    setDraft((current) => {
      if (!current) {
        return current;
      }

      const sourceGeometry = sourceViewportGeometry(current);
      return {
        ...current,
        presetPosition: "center",
        crop:
          current.cropMode === "fixed"
            ? fixedCropForPreset(
                sourceDimensions,
                sourceGeometry.shape,
                sourceGeometry.cellWidth,
                sourceGeometry.cellHeight,
                "center",
              )
            : centeredFreeCrop(
                sourceDimensions,
                sourceGeometry.shape,
                sourceGeometry.cellWidth,
                sourceGeometry.cellHeight,
                0.8,
              ),
      };
    });
    setStatusMessage("크롭을 기본 위치와 크기로 맞췄습니다.");
    setErrorMessage(null);
  };

  const revertSavedSettings = () => {
    if (isAiMutationBusy) {
      return;
    }
    if (!editorState) {
      return;
    }
    if (
      hasUnsavedDraft &&
      !window.confirm("저장하지 않은 크롭·변형 변경을 버리고 저장값으로 되돌릴까요?")
    ) {
      return;
    }

    setDraft(draftFromState(editorState, collection));
    setStatusMessage("저장된 편집값으로 되돌렸습니다.");
    setErrorMessage(null);
  };

  const handleApply = async () => {
    if (!draft || isAiMutationBusy) {
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
        transformQuarterTurns: draft.transformQuarterTurns,
        transformFlipHorizontal: draft.transformFlipHorizontal,
        transformFlipVertical: draft.transformFlipVertical,
        pieceIds: draft.pieceIds.slice(0, draft.shape === "single" ? 1 : 2),
        gifLoopMode: draft.gifLoopMode,
        gifLoopCount: draft.gifLoopMode === "count" ? draft.gifLoopCount : null,
      });
      onIconUpdated(updatedIcon);

      const nextState = await getIconEditorState(collection.id, iconId);
      setEditorState(nextState);
      setDraft(draftFromState(nextState, collection));
      setStatusMessage("크롭·변형 설정을 적용했습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsApplying(false);
    }
  };

  const handleAiEditorStateCommitted = async (
    nextState: IconEditorState,
    nextStatusMessage: string | null,
  ) => {
    if (
      !isEditorStateResponseCurrent(activeIconIdRef.current, nextState.icon.id)
    ) {
      return;
    }

    loadRequestIdRef.current += 1;
    setEditorState(nextState);
    setDraft(draftFromState(nextState, collection));
    setCanRepairAiSource(false);
    setErrorMessage(null);
    if (nextStatusMessage !== null) {
      setStatusMessage(nextStatusMessage);
    }
    onIconUpdated(nextState.icon, { silent: true });
  };

  const handleAiCreatedIconCommitted = async (createdIcon: IconSummary) => {
    onIconUpdated(createdIcon, { silent: true });
  };

  const sourceCropGeometry = draft ? sourceViewportGeometry(draft) : null;

  return (
    <aside
      aria-busy={isAiMutationBusy}
      className="relative flex h-full shrink-0 flex-col border-l border-border bg-surface"
      data-testid="editor-panel"
      style={{ width: panelWidth }}
    >
      <button
        aria-label="편집 패널 너비 조절"
        className="absolute -left-1 top-0 z-10 h-full w-2 cursor-col-resize bg-transparent hover:bg-focus/20 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        data-testid="editor-resize-handle"
        type="button"
        onPointerCancel={stopResize}
        onPointerDown={startResize}
        onPointerMove={updatePanelResize}
        onPointerUp={stopResize}
      />
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
          disabled={isAiMutationBusy}
          title={
            isAiMutationBusy ? "AI 소스 작업이 끝난 뒤 닫을 수 있습니다." : undefined
          }
          type="button"
          onClick={requestPanelClose}
        >
          <X aria-hidden="true" />
        </button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-5 py-4">
        {isLoading ? (
          <p className="text-sm text-muted">편집 정보를 불러오는 중입니다.</p>
        ) : null}

        {!isLoading && errorMessage && !editorState ? (
          canRepairAiSource ? (
            <AiSourceRepairNotice
              errorMessage={errorMessage}
              isRepairing={isAiRepairing}
              onRepair={() => {
                void repairToOriginalSource();
              }}
            />
          ) : (
            <p className="text-sm text-danger" role="alert">
              {errorMessage}
            </p>
          )
        ) : null}

        {!isLoading && editorState && draft && sourceDimensions ? (
          <div className="flex flex-col gap-5">
            <section className="flex flex-col gap-3">
              <div>
                <div className="flex items-center justify-between gap-2">
                  <h3 className="text-sm font-semibold tracking-normal">
                    {editorState.visualSource.activeVersionId
                      ? "현재 편집 소스 (AI)"
                      : "원본 이미지"}
                  </h3>
                  <button
                    aria-label="고급 편집 열기"
                    className="inline-flex h-8 items-center gap-1 rounded-md border border-border bg-white px-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
                    data-testid="editor-advanced-open"
                    disabled={hasUnsavedDraft || isAiMutationBusy}
                    title={
                      isAiMutationBusy
                        ? "AI 소스 작업이 끝난 뒤 고급 편집을 열 수 있습니다."
                        : hasUnsavedDraft
                          ? "크롭·변형 변경을 먼저 적용하거나 되돌리세요."
                          : "고급 편집"
                    }
                    type="button"
                    onClick={() => setIsAdvancedOpen(true)}
                  >
                    <Pencil aria-hidden="true" className="size-4" />
                    고급
                  </button>
                </div>
                <p className="mt-1 truncate text-xs text-muted">
                  {editorState.source.originalFilename} · {editorState.source.width}×
                  {editorState.source.height}px · {formatBytes(editorState.source.byteSize)}
                </p>
              </div>
              <CropCanvas
                disabled={isAiMutationBusy}
                cellHeight={sourceCropGeometry?.cellHeight ?? draft.cellHeight}
                cellWidth={sourceCropGeometry?.cellWidth ?? draft.cellWidth}
                crop={draft.crop}
                cropMode={draft.cropMode}
                shape={sourceCropGeometry?.shape ?? draft.shape}
                sourceHeight={editorState.source.height}
                sourceUrl={editorState.source.originalImageUrl}
                sourceWidth={editorState.source.width}
                textOverlay={editorState.textOverlay}
                onCropChange={handleCropChange}
              />
            </section>

            <EditorOutputPreview
              cellHeight={draft.cellHeight}
              cellWidth={draft.cellWidth}
              previewHeight={collection.previewHeight}
              previewWidth={collection.previewWidth}
              shape={draft.shape}
            >
              <LiveCropPreview
                cellHeight={draft.cellHeight}
                cellWidth={draft.cellWidth}
                crop={draft.crop}
                previewHeight={collection.previewHeight}
                previewWidth={collection.previewWidth}
                shape={draft.shape}
                sourceHeight={editorState.source.height}
                sourceUrl={editorState.source.originalImageUrl}
                sourceWidth={editorState.source.width}
                textOverlay={editorState.textOverlay}
                transformFlipHorizontal={draft.transformFlipHorizontal}
                transformFlipVertical={draft.transformFlipVertical}
                transformQuarterTurns={draft.transformQuarterTurns}
              />
            </EditorOutputPreview>

            <AiReviewSection
              collection={collection}
              hasUnsavedChanges={hasUnsavedChanges}
              icon={editorState.icon}
              visualSource={editorState.visualSource}
              onBusyChange={setIsAiMutationBusy}
              onCreatedIconCommitted={handleAiCreatedIconCommitted}
              onEditorStateCommitted={handleAiEditorStateCommitted}
              onModalOpenChange={handleAiModalOpenChange}
              onRevealIcon={onRevealIcon}
            />

            {isAiMutationBusy && !suppressLiveRegions ? (
              <p className="text-xs text-muted" role="status">
                AI 소스를 처리하는 동안 다른 편집 기능을 잠갔습니다.
              </p>
            ) : null}

            <section className="flex flex-col gap-3 border-t border-border pt-4">
              <h3 className="text-sm font-semibold tracking-normal">모양</h3>
              <div className="grid grid-cols-3 gap-2">
                {SHAPE_OPTIONS.map((option) => (
                  <button
                    className={segmentedButtonClass(draft.shape === option.value)}
                    data-testid={`editor-shape-${option.value}`}
                    disabled={isAiMutationBusy}
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
                  disabled={isAiMutationBusy}
                  type="button"
                  onClick={useCollectionDefaultSize}
                >
                  모음 기본값
                </button>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <DraftNumberInput
                  disabled={isAiMutationBusy}
                  label="너비"
                  value={draft.cellWidth}
                  onChange={(value) => updateCellSize("cellWidth", value)}
                />
                <DraftNumberInput
                  disabled={isAiMutationBusy}
                  label="높이"
                  value={draft.cellHeight}
                  onChange={(value) => updateCellSize("cellHeight", value)}
                />
              </div>
            </section>

            <section className="flex flex-col gap-3 border-t border-border pt-4">
              <h3 className="text-sm font-semibold tracking-normal">크롭 모드</h3>
              <div className="grid grid-cols-2 gap-2">
                {CROP_MODE_OPTIONS.map((option) => (
                  <button
                    className={segmentedButtonClass(draft.cropMode === option.value)}
                    data-testid={`editor-crop-mode-${option.value}`}
                    disabled={isAiMutationBusy}
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
                    disabled={draft.cropMode !== "fixed" || isAiMutationBusy}
                    data-testid={`editor-preset-${value}`}
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

            <section
              aria-label="아이콘 변형"
              className="flex flex-col gap-3 border-t border-border pt-4"
            >
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-semibold tracking-normal">변형</h3>
                <span
                  className="rounded-full border border-border bg-white px-2 py-1 text-[11px] text-muted"
                  data-testid="editor-transform-summary"
                >
                  현재: {transformSummary(draft)}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <button
                  className={transformButtonClass()}
                  data-testid="editor-flip-horizontal"
                  disabled={isApplying || isAiMutationBusy}
                  type="button"
                  onClick={() => applyFlip("horizontal")}
                >
                  <FlipHorizontal2 aria-hidden="true" className="size-4" />
                  좌우 반전
                </button>
                <button
                  className={transformButtonClass()}
                  data-testid="editor-flip-vertical"
                  disabled={isApplying || isAiMutationBusy}
                  type="button"
                  onClick={() => applyFlip("vertical")}
                >
                  <FlipVertical2 aria-hidden="true" className="size-4" />
                  상하 반전
                </button>
                <button
                  className={transformButtonClass()}
                  data-testid="editor-rotate-left"
                  disabled={isApplying || isAiMutationBusy}
                  type="button"
                  onClick={() => applyQuarterTurn("left")}
                >
                  <RotateCcw aria-hidden="true" className="size-4" />
                  왼쪽으로 90°
                </button>
                <button
                  className={transformButtonClass()}
                  data-testid="editor-rotate-right"
                  disabled={isApplying || isAiMutationBusy}
                  type="button"
                  onClick={() => applyQuarterTurn("right")}
                >
                  <RotateCw aria-hidden="true" className="size-4" />
                  오른쪽으로 90°
                </button>
              </div>
              <p className="text-xs leading-5 text-muted">
                {draft.shape === "single"
                  ? "원본은 보존되며 크롭한 전체 이미지를 변형합니다."
                  : "2개 조각을 한 이미지처럼 변형하며, 조각 위치와 내보내기 순서도 결과에 맞게 바뀝니다."}
                {editorState.source.isAnimated
                  ? " GIF의 모든 프레임에 같은 변형을 적용합니다."
                  : ""}
              </p>
            </section>

            {editorState.source.isAnimated ? (
              <section className="flex flex-col gap-3 border-t border-border pt-4">
                <h3 className="text-sm font-semibold tracking-normal">GIF 반복</h3>
                <select
                  className="rounded-md border border-border bg-white px-3 py-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                  data-testid="editor-gif-loop-mode"
                  disabled={isAiMutationBusy}
                  value={draft.gifLoopMode}
                  onChange={(event) => {
                    const gifLoopMode = event.currentTarget.value as GifLoopMode;
                    setDraft((current) =>
                      current
                        ? {
                            ...current,
                            gifLoopMode,
                          }
                        : current,
                    );
                  }}
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
                      data-testid="editor-gif-loop-count"
                      disabled={isAiMutationBusy}
                      min={1}
                      type="number"
                      value={draft.gifLoopCount}
                      onChange={(event) => {
                        const gifLoopCount = Math.max(
                          1,
                          Math.round(event.currentTarget.valueAsNumber || 1),
                        );
                        setDraft((current) =>
                          current
                            ? {
                                ...current,
                                gifLoopCount,
                              }
                            : current,
                        );
                      }}
                    />
                  </label>
                ) : null}
              </section>
            ) : null}

            {statusMessage && !suppressLiveRegions ? (
              <p className="text-sm text-muted" role="status">
                {statusMessage}
              </p>
            ) : null}
            {errorMessage && !suppressLiveRegions ? (
              <p className="text-sm text-danger" role="alert">
                {errorMessage}
              </p>
            ) : null}
          </div>
        ) : null}
      </div>

      <footer className="flex flex-wrap items-center justify-between gap-2 border-t border-border px-5 py-4">
        <button
          className="inline-flex items-center gap-2 whitespace-nowrap rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
          data-testid="editor-reset"
          disabled={!draft || !sourceDimensions || isApplying || isAiMutationBusy}
          title="크롭 위치와 크기만 기본값으로 되돌립니다. 다른 편집값은 유지됩니다."
          type="button"
          onClick={resetCropToDefault}
        >
          <RefreshCcw aria-hidden="true" />
          크롭 기본값
        </button>
        <div className="flex flex-wrap items-center justify-end gap-2">
          <button
            className="inline-flex items-center gap-2 whitespace-nowrap rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
            data-testid="editor-revert"
            disabled={!editorState || isApplying || isAiMutationBusy}
            title="저장된 편집값을 다시 불러오며 아직 적용하지 않은 변경만 버립니다."
            type="button"
            onClick={revertSavedSettings}
          >
            <Undo2 aria-hidden="true" />
            저장값으로 되돌리기
          </button>
          <button
            className="inline-flex items-center gap-2 whitespace-nowrap rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
            data-testid="editor-apply"
            disabled={!draft || isApplying || isAiMutationBusy}
            type="button"
            onClick={() => {
              void handleApply();
            }}
          >
            <Save aria-hidden="true" />
            {isApplying ? "크롭·변형 적용 중" : "크롭·변형 적용"}
          </button>
        </div>
      </footer>
      {isAdvancedOpen &&
      editorState &&
      editorState.icon.id === activeIconIdRef.current ? (
        <AdvancedEditPanel
          collection={collection}
          editorState={editorState}
          onClose={() => {
            setIsAdvancedOpen(false);
            setIsAdvancedDirty(false);
          }}
          onEditorStateUpdated={(nextState) => {
            if (
              !isEditorStateResponseCurrent(
                activeIconIdRef.current,
                nextState.icon.id,
              )
            ) {
              return;
            }
            setEditorState(nextState);
            setDraft(draftFromState(nextState, collection));
            onIconUpdated(nextState.icon);
          }}
          onDirtyChange={setIsAdvancedDirty}
          onStatus={setStatusMessage}
        />
      ) : null}
    </aside>
  );
}

export function AiSourceRepairNotice({
  errorMessage,
  isRepairing,
  onRepair,
}: {
  errorMessage: string;
  isRepairing: boolean;
  onRepair: () => void;
}) {
  return (
    <div className="flex flex-col gap-3 rounded-md border border-danger/30 bg-danger/5 p-3">
      <div>
        <p className="text-sm text-danger" role="alert">
          {errorMessage}
        </p>
        <p className="mt-1 text-xs leading-5 text-muted">
          손상된 AI 소스 연결을 해제하고 보존된 원본 소스로 편집을 다시 열 수
          있습니다.
        </p>
      </div>
      <button
        aria-busy={isRepairing}
        className="inline-flex w-fit items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-wait disabled:opacity-60"
        data-testid="editor-ai-repair-original"
        disabled={isRepairing}
        type="button"
        onClick={onRepair}
      >
        {isRepairing ? (
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
        ) : (
          <RefreshCcw aria-hidden="true" className="size-4" />
        )}
        {isRepairing ? "원본 소스로 복구 중" : "원본 소스로 복구"}
      </button>
    </div>
  );
}

function AdvancedEditPanel({
  collection,
  editorState,
  onClose,
  onEditorStateUpdated,
  onDirtyChange,
  onStatus,
}: {
  collection: CollectionSummary;
  editorState: IconEditorState;
  onClose: () => void;
  onEditorStateUpdated: (state: IconEditorState) => void;
  onStatus: (message: string | null) => void;
  onDirtyChange: (dirty: boolean) => void;
}) {
  const initialEffectRecipe = normalizeEffectRecipe(editorState.effectRecipe);
  const [advancedMode, setAdvancedMode] = useState<AdvancedEditorMode>("effects");
  const [profiles, setProfiles] = useState<ExportProfile[]>([]);
  const [profileId, setProfileId] = useState("");
  const [pieceId, setPieceId] = useState(editorState.icon.pieces[0]?.id ?? "");
  const [fpsLimit, setFpsLimit] = useState(15);
  const [playbackFps, setPlaybackFps] = useState<number | null>(null);
  const [colorLimit, setColorLimit] = useState<number | null>(128);
  const [jpegQuality, setJpegQuality] = useState(82);
  const [textEnabled, setTextEnabled] = useState(editorState.textOverlay.enabled);
  const [textValue, setTextValue] = useState(editorState.textOverlay.text);
  const [textFontPath, setTextFontPath] = useState(editorState.textOverlay.fontPath ?? "");
  const [textFontSize, setTextFontSize] = useState(editorState.textOverlay.fontSize);
  const [textX, setTextX] = useState(Math.round(editorState.textOverlay.x * 100));
  const [textY, setTextY] = useState(Math.round(editorState.textOverlay.y * 100));
  const [textColor, setTextColor] = useState(editorState.textOverlay.color);
  const [textStrokeColor, setTextStrokeColor] = useState(
    editorState.textOverlay.strokeColor,
  );
  const [textStrokeWidth, setTextStrokeWidth] = useState(
    editorState.textOverlay.strokeWidth,
  );
  const [result, setResult] = useState<OptimizationResult | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const playbackPreviewRequestRef = useRef(0);
  const [playbackPreviewPath, setPlaybackPreviewPath] = useState<string | null>(null);
  const [playbackPreviewCacheKey, setPlaybackPreviewCacheKey] = useState<string | null>(
    null,
  );
  const [isPlaybackPreviewLoading, setIsPlaybackPreviewLoading] = useState(false);
  const [playbackPreviewError, setPlaybackPreviewError] = useState<string | null>(null);
  const effectPreviewRequestRef = useRef(0);
  const advancedDialogRef = useRef<HTMLElement>(null);
  const [savedEffectRecipe, setSavedEffectRecipe] =
    useState<EffectRecipeV1>(initialEffectRecipe);
  const [effectDraft, setEffectDraft] =
    useState<EffectRecipeV1>(initialEffectRecipe);
  const [effectRevision, setEffectRevision] = useState(
    editorState.effectRevision,
  );
  const [effectPreview, setEffectPreview] =
    useState<IconEffectPreview | null>(null);
  const [effectPreviewResolvedKey, setEffectPreviewResolvedKey] =
    useState<string | null>(null);
  const [isEffectPreviewLoading, setIsEffectPreviewLoading] = useState(false);
  const [isEffectSaving, setIsEffectSaving] = useState(false);
  const [isEffectReverting, setIsEffectReverting] = useState(false);
  const [effectPreviewError, setEffectPreviewError] = useState<string | null>(
    null,
  );
  const [effectActionError, setEffectActionError] = useState<string | null>(
    null,
  );
  const [hasEffectConflict, setHasEffectConflict] = useState(false);
  const [isMotionDirty, setIsMotionDirty] = useState(false);
  const [isMotionWorking, setIsMotionWorking] = useState(false);

  const effectDraftSignature = useMemo(
    () => effectRecipeSignature(effectDraft),
    [effectDraft],
  );
  const currentEffectPreviewRequestKey = useMemo(
    () =>
      effectPreviewRequestKey({
        iconId: editorState.icon.id,
        iconUpdatedAt: editorState.icon.updatedAt,
        effectRevision: editorState.effectRevision,
        draftSignature: effectDraftSignature,
      }),
    [
      editorState.effectRevision,
      editorState.icon.id,
      editorState.icon.updatedAt,
      effectDraftSignature,
    ],
  );
  const isEffectDirty =
    effectRecipeStateSignature(effectDraft) !==
    effectRecipeStateSignature(savedEffectRecipe);
  const isTextDirty = isTextOverlayDraftDirty(
    {
      enabled: textEnabled,
      text: textValue,
      fontPath: textFontPath,
      fontSize: textFontSize,
      xPercent: textX,
      yPercent: textY,
      color: textColor,
      strokeColor: textStrokeColor,
      strokeWidth: textStrokeWidth,
    },
    editorState.textOverlay,
  );
  const isEffectPreviewStale =
    effectPreview !== null &&
    effectPreviewResolvedKey !== currentEffectPreviewRequestKey;
  const isEffectRevisionStale =
    hasEffectConflict || editorState.effectRevision !== effectRevision;
  const isEffectWorking = isEffectSaving || isEffectReverting;
  const isBlockingBusy = isBusy || isEffectWorking || isMotionWorking;
  const isAdvancedDirty = isEffectDirty || isMotionDirty || isTextDirty;

  useEffect(() => {
    onDirtyChange(isAdvancedDirty);
  }, [isAdvancedDirty, onDirtyChange]);

  useEffect(
    () => () => {
      onDirtyChange(false);
    },
    [onDirtyChange],
  );

  const requestClose = useCallback(() => {
    if (isBlockingBusy) {
      return;
    }
    if (
      isAdvancedDirty &&
      !window.confirm(
        "저장하지 않은 정적 효과, 모션 또는 텍스트 변경을 버리고 고급 편집을 닫을까요?",
      )
    ) {
      return;
    }
    onClose();
  }, [isAdvancedDirty, isBlockingBusy, onClose]);

  useModalFocus(advancedDialogRef, requestClose);

  useEffect(() => {
    let isActive = true;
    setIsBusy(true);
    setErrorMessage(null);

    listExportProfiles(collection.id)
      .then((nextProfiles) => {
        if (!isActive) {
          return;
        }
        setProfiles(nextProfiles);
        const preferred =
          nextProfiles.find((profile) => profile.profileType === "dcinside") ??
          nextProfiles[0];
        setProfileId(preferred?.id ?? "");
      })
      .catch((error) => {
        if (isActive) {
          setErrorMessage(getCommandErrorMessage(error));
        }
      })
      .finally(() => {
        if (isActive) {
          setIsBusy(false);
        }
      });

    return () => {
      isActive = false;
    };
  }, [collection.id]);

  const selectedProfile = profiles.find((profile) => profile.id === profileId) ?? null;
  const selectedPiece = editorState.icon.pieces.find((piece) => piece.id === pieceId);
  const usesJpegQuality =
    !editorState.source.isAnimated && selectedProfile?.targetFormat === "jpg";

  useEffect(() => {
    const requestId = playbackPreviewRequestRef.current + 1;
    playbackPreviewRequestRef.current = requestId;
    setPlaybackPreviewError(null);

    if (!editorState.source.isAnimated || playbackFps === null) {
      setPlaybackPreviewPath(null);
      setPlaybackPreviewCacheKey(null);
      setIsPlaybackPreviewLoading(false);
      return;
    }

    let isActive = true;
    setIsPlaybackPreviewLoading(true);
    const timer = window.setTimeout(() => {
      previewGifPlaybackFps(editorState.icon.id, playbackFps)
        .then((preview) => {
          if (!isActive || playbackPreviewRequestRef.current !== requestId) {
            return;
          }
          setPlaybackPreviewPath(preview.previewPath);
          setPlaybackPreviewCacheKey(preview.generatedAt);
        })
        .catch((error) => {
          if (!isActive || playbackPreviewRequestRef.current !== requestId) {
            return;
          }
          setPlaybackPreviewPath(null);
          setPlaybackPreviewCacheKey(null);
          setPlaybackPreviewError(getCommandErrorMessage(error));
        })
        .finally(() => {
          if (isActive && playbackPreviewRequestRef.current === requestId) {
            setIsPlaybackPreviewLoading(false);
          }
        });
    }, 450);

    return () => {
      isActive = false;
      window.clearTimeout(timer);
    };
  }, [editorState.icon.id, editorState.source.isAnimated, playbackFps]);

  useEffect(() => {
    const requestId = effectPreviewRequestRef.current + 1;
    effectPreviewRequestRef.current = requestId;
    const requestedKey = currentEffectPreviewRequestKey;
    let isActive = true;

    setEffectPreviewError(null);
    setIsEffectPreviewLoading(true);
    const timer = window.setTimeout(() => {
      previewIconEffects(collection.id, {
        iconId: editorState.icon.id,
        recipe: effectDraft,
      })
        .then((preview) => {
          if (!isActive || effectPreviewRequestRef.current !== requestId) {
            return;
          }
          setEffectPreview(preview);
          setEffectPreviewResolvedKey(requestedKey);
        })
        .catch((error) => {
          if (!isActive || effectPreviewRequestRef.current !== requestId) {
            return;
          }
          setEffectPreviewError(getCommandErrorMessage(error));
        })
        .finally(() => {
          if (isActive && effectPreviewRequestRef.current === requestId) {
            setIsEffectPreviewLoading(false);
          }
        });
    }, 450);

    return () => {
      isActive = false;
      window.clearTimeout(timer);
    };
  }, [
    collection.id,
    currentEffectPreviewRequestKey,
    editorState.icon.id,
    effectDraft,
  ]);

  const handleGenerate = async () => {
    if (!selectedProfile || !selectedPiece) {
      return;
    }

    setIsBusy(true);
    setErrorMessage(null);
    setResult(null);

    try {
      const nextResult = editorState.source.isAnimated
        ? await generateGifOptimizationCandidates(editorState.icon.id, profileId, pieceId, {
            colorLimit,
            fpsLimit,
            playbackFps: null,
          })
        : await generateStaticOptimizationCandidates(editorState.icon.id, profileId, pieceId, {
            jpegQuality: usesJpegQuality ? jpegQuality : null,
          });
      setResult(nextResult);
      onStatus(nextResult.message);
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsBusy(false);
    }
  };

  const handleApplyPlaybackFps = async () => {
    if (!selectedProfile || !selectedPiece || !editorState.source.isAnimated) {
      return;
    }

    setIsBusy(true);
    setErrorMessage(null);
    setResult(null);

    try {
      if (playbackFps === null) {
        await applyGifOriginalPlaybackToPreview(
          editorState.icon.id,
          selectedProfile.id,
          selectedPiece.id,
        );
        const nextState = await getIconEditorState(collection.id, editorState.icon.id);
        onEditorStateUpdated(nextState);
        onStatus("원본 GIF 프레임 지연 시간을 실제 미리보기와 내보내기 결과에 적용했습니다.");
        return;
      }

      const nextResult = await generateGifOptimizationCandidates(
        editorState.icon.id,
        profileId,
        pieceId,
        {
          colorLimit: null,
          fpsLimit: null,
          frameStep: null,
          playbackFps,
        },
      );
      const candidate =
        nextResult.candidates.find((next) => next.preset === "quality") ??
        nextResult.candidates[0] ??
        null;

      if (!candidate) {
        setErrorMessage("GIF 재생 FPS를 적용할 후보를 만들 수 없습니다.");
        return;
      }

      await applyOptimizationCandidateToPreview(candidate.id);
      const nextState = await getIconEditorState(collection.id, editorState.icon.id);
      onEditorStateUpdated(nextState);
      setResult(null);
      onStatus(`GIF 재생 FPS ${playbackFps}를 실제 미리보기와 내보내기 결과에 적용했습니다.`);
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsBusy(false);
    }
  };

  const handleApplyCandidate = async (candidate: OptimizationCandidate) => {
    setIsBusy(true);
    setErrorMessage(null);
    try {
      const applied = await applyOptimizationCandidate(candidate.id);
      setResult((current) =>
        current
          ? {
              ...current,
              candidates: current.candidates.map((next) => ({
                ...next,
                isActiveForExport: next.id === candidate.id,
              })),
            }
          : current,
      );
      onStatus(applied.message);
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsBusy(false);
    }
  };

  const handleClear = async () => {
    if (!selectedProfile || !selectedPiece) {
      return;
    }

    setIsBusy(true);
    setErrorMessage(null);
    try {
      const cleared = await clearOptimizationCandidate(
        editorState.icon.id,
        selectedProfile.id,
        selectedPiece.id,
      );
      setResult(null);
      onStatus(cleared.message);
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsBusy(false);
    }
  };

  const handlePickFont = async () => {
    setIsBusy(true);
    setErrorMessage(null);
    try {
      const selected = await pickTextOverlayFont(textFontPath || null);
      if (selected) {
        setTextFontPath(selected);
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsBusy(false);
    }
  };

  const handleApplyText = async () => {
    setIsBusy(true);
    setErrorMessage(null);
    try {
      const nextState = await updateIconTextOverlay(collection.id, {
        iconId: editorState.icon.id,
        enabled: textEnabled,
        text: textValue,
        fontPath: textFontPath.trim() ? textFontPath.trim() : null,
        fontSize: textFontSize,
        x: textX / 100,
        y: textY / 100,
        color: textColor,
        strokeColor: textStrokeColor,
        strokeWidth: textStrokeWidth,
      });
      onEditorStateUpdated(nextState);
      onStatus("텍스트 설정을 적용했습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsBusy(false);
    }
  };

  const handleSaveEffects = async () => {
    if (!isEffectDirty || isEffectRevisionStale) {
      return;
    }

    setIsEffectSaving(true);
    setEffectActionError(null);
    try {
      const nextState = await updateIconEffects(collection.id, {
        iconId: editorState.icon.id,
        expectedRevision: effectRevision,
        recipe: effectDraft,
      });
      const nextRecipe = normalizeEffectRecipe(nextState.effectRecipe);
      setSavedEffectRecipe(nextRecipe);
      setEffectDraft(nextRecipe);
      setEffectRevision(nextState.effectRevision);
      setHasEffectConflict(false);
      onEditorStateUpdated(nextState);
      onStatus("효과 설정을 저장하고 실제 미리보기와 내보내기에 적용했습니다.");
    } catch (error) {
      if (isRevisionConflict(error)) {
        setHasEffectConflict(true);
      }
      setEffectActionError(
        `효과를 저장하지 못했습니다: ${getCommandErrorMessage(error)}`,
      );
    } finally {
      setIsEffectSaving(false);
    }
  };

  const handleRevertEffects = async () => {
    if (
      isEffectDirty &&
      !window.confirm("저장하지 않은 효과 변경을 버리고 최신 저장값을 불러올까요?")
    ) {
      return;
    }

    setIsEffectReverting(true);
    setEffectActionError(null);
    try {
      const latestState = await getIconEditorState(
        collection.id,
        editorState.icon.id,
      );
      const latestRecipe = normalizeEffectRecipe(latestState.effectRecipe);
      setSavedEffectRecipe(latestRecipe);
      setEffectDraft(latestRecipe);
      setEffectRevision(latestState.effectRevision);
      setHasEffectConflict(false);
      onEditorStateUpdated(latestState);
      onStatus("저장된 효과 설정으로 되돌렸습니다.");
    } catch (error) {
      setEffectActionError(
        `저장된 효과를 불러오지 못했습니다: ${getCommandErrorMessage(error)}`,
      );
    } finally {
      setIsEffectReverting(false);
    }
  };

  const handleAdvancedTabKeyDown = (
    event: ReactKeyboardEvent<HTMLButtonElement>,
  ) => {
    const nextMode = nextAdvancedEditorMode(advancedMode, event.key);
    if (nextMode === null) {
      return;
    }

    event.preventDefault();
    setAdvancedMode(nextMode);
    window.requestAnimationFrame(() => {
      document.getElementById(`advanced-${nextMode}-tab`)?.focus();
    });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4">
      <section
        aria-labelledby="advanced-edit-heading"
        aria-modal="true"
        className="relative flex max-h-full w-full max-w-5xl flex-col rounded-md border border-border bg-surface shadow-xl"
        ref={advancedDialogRef}
        role="dialog"
        tabIndex={-1}
      >
        <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
          <div className="min-w-0">
            <h3
              className="truncate text-base font-semibold tracking-normal"
              id="advanced-edit-heading"
            >
              고급 편집
            </h3>
            <p className="mt-1 truncate text-xs text-muted">
              {advancedMode === "effects"
                ? "정적 효과 순서를 편집하고 실제 네이티브 출력으로 결과를 확인합니다."
                : advancedMode === "motion"
                  ? "움직임과 일렁임을 조합하고 실제 GIF로 재생·용량을 확인합니다."
                  : "실제 export 후보를 생성해서 용량을 측정하고 적용합니다."}
            </p>
          </div>
          <button
            aria-label="고급 편집 닫기"
            className="inline-flex size-8 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            disabled={isBlockingBusy}
            type="button"
            onClick={requestClose}
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </header>

        {isBlockingBusy ? (
          <div className="absolute inset-x-0 top-[57px] z-10 border-b border-border bg-surface/95 px-4 py-2">
            <div className="flex items-center justify-between text-xs text-muted">
              <span>
                {isMotionWorking
                  ? "모션 GIF를 생성하거나 설정을 저장하는 중입니다."
                  : isEffectSaving
                  ? "효과 설정을 저장하는 중입니다."
                  : isEffectReverting
                    ? "저장된 효과 설정을 불러오는 중입니다."
                  : "고급 편집 작업을 처리하는 중입니다."}
              </span>
              <span>
                {isEffectSaving
                  ? "미리보기와 내보내기 결과 갱신"
                  : isEffectReverting
                    ? "최신 저장 버전 확인"
                  : "실제 파일 생성 및 용량 측정"}
              </span>
            </div>
            <div
              aria-label="고급 편집 처리 중"
              className="mt-2 h-2 overflow-hidden rounded-full bg-preview"
              role="progressbar"
            >
              <div className="h-full w-1/2 animate-pulse rounded-full bg-accent motion-reduce:animate-none" />
            </div>
          </div>
        ) : null}

        <div
          aria-label="고급 편집 기능"
          className="flex gap-1 border-b border-border bg-canvas px-4 pt-2"
          role="tablist"
        >
          <button
            aria-controls="advanced-effects-panel"
            aria-selected={advancedMode === "effects"}
            className={cn(
              "rounded-t-md border border-b-0 border-transparent px-3 py-2 text-sm font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
              advancedMode === "effects"
                ? "border-border bg-surface text-foreground"
                : "text-muted hover:bg-menu-hover",
            )}
            disabled={isBlockingBusy}
            id="advanced-effects-tab"
            role="tab"
            tabIndex={advancedMode === "effects" ? 0 : -1}
            type="button"
            onClick={() => setAdvancedMode("effects")}
            onKeyDown={handleAdvancedTabKeyDown}
          >
            정적 효과
          </button>
          <button
            aria-controls="advanced-motion-panel"
            aria-selected={advancedMode === "motion"}
            className={cn(
              "rounded-t-md border border-b-0 border-transparent px-3 py-2 text-sm font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
              advancedMode === "motion"
                ? "border-border bg-surface text-foreground"
                : "text-muted hover:bg-menu-hover",
            )}
            disabled={isBlockingBusy}
            id="advanced-motion-tab"
            role="tab"
            tabIndex={advancedMode === "motion" ? 0 : -1}
            type="button"
            onClick={() => setAdvancedMode("motion")}
            onKeyDown={handleAdvancedTabKeyDown}
          >
            모션
          </button>
          <button
            aria-controls="advanced-tools-panel"
            aria-selected={advancedMode === "tools"}
            className={cn(
              "rounded-t-md border border-b-0 border-transparent px-3 py-2 text-sm font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
              advancedMode === "tools"
                ? "border-border bg-surface text-foreground"
                : "text-muted hover:bg-menu-hover",
            )}
            disabled={isBlockingBusy}
            id="advanced-tools-tab"
            role="tab"
            tabIndex={advancedMode === "tools" ? 0 : -1}
            type="button"
            onClick={() => setAdvancedMode("tools")}
            onKeyDown={handleAdvancedTabKeyDown}
          >
            텍스트·용량
          </button>
        </div>

        <div
          aria-labelledby="advanced-effects-tab"
          className={cn(
            "grid min-h-0 gap-4 overflow-auto p-4 md:grid-cols-[minmax(360px,1.1fr)_minmax(280px,0.9fr)]",
            advancedMode !== "effects" && "hidden",
          )}
          hidden={advancedMode !== "effects"}
            id="advanced-effects-panel"
            role="tabpanel"
          >
            <Suspense
              fallback={
                <div
                  aria-live="polite"
                  className="rounded-md border border-border bg-canvas p-4 text-sm text-muted"
                >
                  효과 편집기를 불러오는 중입니다.
                </div>
              }
            >
              <LazyEffectRecipeEditor
                disabled={isEffectWorking || isEffectRevisionStale}
                recipe={effectDraft}
                onChange={(recipe) => {
                  setEffectDraft(normalizeEffectRecipe(recipe));
                  setEffectActionError(null);
                }}
              />
            </Suspense>

            <div className="flex min-h-0 flex-col gap-3">
              <section
                aria-labelledby="effect-preview-heading"
                className="rounded-md border border-border bg-white p-3"
              >
                <div className="mb-3 flex flex-wrap items-start justify-between gap-2">
                  <div>
                    <h4
                      className="text-sm font-semibold tracking-normal"
                      id="effect-preview-heading"
                    >
                      현재 셀 네이티브 미리보기
                    </h4>
                    <p className="mt-1 text-[11px] text-muted">
                      효과 합성은 export recipe와 같습니다. 최종 형식·리사이즈
                      필터·최적화에 따라 색, 용량, 프레임 수와 타이밍은 달라질 수
                      있습니다.
                    </p>
                  </div>
                  <div className="flex flex-wrap justify-end gap-1 text-[11px]">
                    <span
                      className={cn(
                        "rounded-full border px-2 py-0.5",
                        isEffectDirty
                          ? "border-amber-300 bg-amber-50 text-amber-800"
                          : "border-border bg-canvas text-muted",
                      )}
                    >
                      {isEffectDirty ? "저장 전 변경 있음" : "저장됨"}
                    </span>
                    <span
                      className={cn(
                        "rounded-full border px-2 py-0.5",
                        isEffectPreviewLoading || isEffectPreviewStale
                          ? "border-blue-200 bg-blue-50 text-blue-800"
                          : "border-border bg-canvas text-muted",
                      )}
                    >
                      {isEffectPreviewLoading
                        ? "미리보기 생성 중"
                        : isEffectPreviewStale
                          ? "이전 미리보기"
                          : effectPreview
                            ? "최신 미리보기"
                            : "미리보기 대기"}
                    </span>
                  </div>
                </div>

                <div className="relative mx-auto flex aspect-square w-full max-w-96 items-center justify-center overflow-hidden rounded-md border border-border bg-preview p-3">
                  {effectPreview?.previewPath ? (
                    <img
                      alt="효과가 적용된 현재 셀 미리보기"
                      className={cn(
                        "max-h-full max-w-full object-contain",
                        isEffectPreviewStale && "opacity-60",
                      )}
                      draggable={false}
                      src={effectPreview.previewPath}
                      style={nonDraggableImageStyle}
                    />
                  ) : (
                    <p className="px-4 text-center text-sm text-muted">
                      현재 셀 미리보기를 준비하고 있습니다.
                    </p>
                  )}
                  {isEffectPreviewLoading ? (
                    <div className="absolute inset-x-3 bottom-3 rounded-md bg-surface/90 px-3 py-2 text-center text-xs text-muted shadow">
                      변경을 반영해 다시 렌더링하는 중입니다.
                    </div>
                  ) : null}
                </div>

                <p
                  aria-live="polite"
                  className="mt-2 text-xs text-muted"
                  role="status"
                >
                  {isEffectPreviewLoading
                    ? "효과 변경이 멈춘 뒤 네이티브 미리보기를 생성합니다."
                    : isEffectPreviewStale
                      ? "설정이 바뀌어 이전 미리보기를 표시하고 있습니다."
                      : effectPreview
                        ? `미리보기 생성 완료 · 저장 버전 ${effectRevision}`
                        : "미리보기 요청을 기다리고 있습니다."}
                </p>
              </section>

              {effectPreview ? (
                <dl className="grid grid-cols-2 gap-2 rounded-md border border-border bg-canvas p-3 text-xs">
                  <div>
                    <dt className="text-muted">현재 미리보기 파일</dt>
                    <dd className="mt-0.5 font-semibold">
                      {formatBytes(effectPreview.byteSize)}
                    </dd>
                  </div>
                  <div>
                    <dt className="text-muted">가장 큰 조각</dt>
                    <dd
                      className={cn(
                        "mt-0.5 font-semibold",
                        effectPreview.maxPieceByteSize > effectPreview.maxBytes &&
                          "text-danger",
                      )}
                    >
                      {formatBytes(effectPreview.maxPieceByteSize)} /{" "}
                      {formatBytes(effectPreview.maxBytes)}
                    </dd>
                  </div>
                  <div>
                    <dt className="text-muted">프레임</dt>
                    <dd className="mt-0.5 font-semibold">
                      {effectPreview.frameCount}개
                    </dd>
                  </div>
                  <div>
                    <dt className="text-muted">처리 시간</dt>
                    <dd className="mt-0.5 font-semibold">
                      {effectPreview.processingMs}ms
                    </dd>
                  </div>
                </dl>
              ) : null}

              {effectPreview?.warnings.length ? (
                <section
                  aria-labelledby="effect-warning-heading"
                  className="rounded-md border border-amber-300 bg-amber-50 p-3"
                >
                  <h4
                    className="text-xs font-semibold text-amber-900"
                    id="effect-warning-heading"
                  >
                    확인할 점
                  </h4>
                  <ul className="mt-1 list-disc space-y-1 pl-4 text-xs text-amber-900">
                    {effectPreview.warnings.map((warning, index) => (
                      <li key={`${index}-${warning}`}>{warning}</li>
                    ))}
                  </ul>
                </section>
              ) : null}

              {isEffectRevisionStale ? (
                <p className="rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-900">
                  다른 작업에서 저장된 효과 버전이 바뀌었습니다. 최신 저장값으로
                  되돌린 뒤 다시 편집하세요.
                </p>
              ) : null}
              {effectPreviewError ? (
                <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-danger">
                  미리보기를 만들지 못했습니다: {effectPreviewError}
                </p>
              ) : null}
              {effectActionError ? (
                <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-danger">
                  {effectActionError}
                </p>
              ) : null}

              <div className="mt-auto flex flex-wrap justify-end gap-2 border-t border-border pt-3">
                <button
                  className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                  disabled={
                    isEffectWorking ||
                    (!isEffectDirty && !isEffectRevisionStale)
                  }
                  type="button"
                  onClick={() => {
                    void handleRevertEffects();
                  }}
                >
                  {isEffectReverting ? "저장값 불러오는 중" : "저장값으로 되돌리기"}
                </button>
                <button
                  className="rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
                  disabled={
                    isEffectWorking ||
                    !isEffectDirty ||
                    isEffectRevisionStale
                  }
                  type="button"
                  onClick={() => {
                    void handleSaveEffects();
                  }}
                >
                  {isEffectSaving ? "효과 저장 중" : "효과 저장"}
                </button>
              </div>
            </div>
        </div>

        <div
          aria-labelledby="advanced-motion-tab"
          className={cn(advancedMode !== "motion" && "hidden")}
          hidden={advancedMode !== "motion"}
          id="advanced-motion-panel"
          role="tabpanel"
        >
          <Suspense
            fallback={
              advancedMode === "motion" ? (
                <div className="p-4 text-sm text-muted" aria-live="polite">
                  모션 편집기를 불러오는 중입니다.
                </div>
              ) : null
            }
          >
            <LazyMotionEditorSection
              key={editorState.icon.id}
              collection={collection}
              editorState={editorState}
              staticEffectDirty={isEffectDirty}
              staticEffectRevisionStale={isEffectRevisionStale}
              onBusyChange={setIsMotionWorking}
              onDirtyChange={setIsMotionDirty}
              onEditorStateUpdated={onEditorStateUpdated}
              onStatus={onStatus}
            />
          </Suspense>
        </div>

        <div
          aria-labelledby="advanced-tools-tab"
          className={cn(
            "grid min-h-0 gap-4 overflow-auto p-4 md:grid-cols-[260px_minmax(0,1fr)]",
            advancedMode !== "tools" && "hidden",
          )}
          hidden={advancedMode !== "tools"}
          id="advanced-tools-panel"
          role="tabpanel"
        >
          <aside className="flex flex-col gap-3">
            <label className="flex flex-col gap-1 text-xs font-medium text-muted">
              Export 프로필
              <select
                className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                disabled={isBusy}
                value={profileId}
                onChange={(event) => setProfileId(event.currentTarget.value)}
              >
                {profiles.map((profile) => (
                  <option key={profile.id} value={profile.id}>
                    {profile.name}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex flex-col gap-1 text-xs font-medium text-muted">
              조각
              <select
                className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                disabled={isBusy}
                value={pieceId}
                onChange={(event) => setPieceId(event.currentTarget.value)}
              >
                {editorState.icon.pieces.map((piece) => (
                  <option key={piece.id} value={piece.id}>
                    {piece.pieceRole} · {piece.altText || "alt 없음"}
                  </option>
                ))}
              </select>
            </label>

            {editorState.source.isAnimated ? (
              <>
                <div className="flex flex-col gap-2 rounded-md border border-border bg-white p-3">
                  <div className="flex items-center justify-between gap-2 text-xs font-medium text-muted">
                    <span>GIF 프레임 줄이기 FPS 상한</span>
                    <ClearableNumberInput
                      disabled={isBusy}
                      max={60}
                      min={1}
                      value={fpsLimit}
                      onCommit={setFpsLimit}
                    />
                  </div>
                  <input
                    disabled={isBusy}
                    max={60}
                    min={1}
                    type="range"
                    value={fpsLimit}
                    onChange={(event) => setFpsLimit(Number(event.currentTarget.value))}
                  />
                  <p className="text-[11px] text-muted">
                    높은 FPS 제한은 프레임 손실을 줄이지만 용량이 거의 줄지 않을 수 있습니다.
                  </p>
                </div>
                <div className="flex flex-col gap-2 rounded-md border border-border bg-white p-3">
                  <div className="flex items-center justify-between gap-2 text-xs font-medium text-muted">
                    <span>GIF 재생 FPS</span>
                    <div className="flex items-center gap-2">
                      <ClearableNumberInput
                        disabled={isBusy}
                        max={60}
                        min={1}
                        value={playbackFps ?? 24}
                        onCommit={(value) => setPlaybackFps(Math.round(value))}
                      />
                      <button
                        className="rounded border border-border px-2 py-1 text-[11px] font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
                        data-testid="advanced-playback-original-fps"
                        disabled={isBusy}
                        type="button"
                        onClick={() => setPlaybackFps(null)}
                      >
                        원본 FPS
                      </button>
                    </div>
                  </div>
                  <input
                    disabled={isBusy}
                    max={60}
                    min={1}
                    type="range"
                    value={playbackFps ?? 24}
                    onChange={(event) =>
                      setPlaybackFps(Number(event.currentTarget.value))
                    }
                  />
                  <p className="text-[11px] text-muted">
                    원본 FPS는 GIF의 기존 프레임 지연 시간을 그대로 씁니다. 값을 지정하면
                    후보 GIF의 재생 속도가 해당 FPS 기준으로 다시 인코딩됩니다.
                  </p>
                  <button
                    className="rounded-md border border-accent bg-white px-3 py-2 text-xs font-semibold text-accent hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
                    data-testid="advanced-playback-fps-apply"
                    disabled={isBusy || !selectedProfile || !selectedPiece}
                    type="button"
                    onClick={() => {
                      void handleApplyPlaybackFps();
                    }}
                  >
                    GIF 재생 FPS 적용
                  </button>
                </div>
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  최대 색상 수
                  <select
                    className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    disabled={isBusy}
                    value={colorLimit ?? "auto"}
                    onChange={(event) =>
                      setColorLimit(
                        event.currentTarget.value === "auto"
                          ? null
                          : Number(event.currentTarget.value),
                      )
                    }
                  >
                    <option value="auto">자동</option>
                    {[256, 224, 192, 160, 128, 96, 64, 48, 32].map((value) => (
                      <option key={value} value={value}>
                        {value}색
                      </option>
                    ))}
                  </select>
                </label>
              </>
            ) : usesJpegQuality ? (
              <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                JPG 품질
                <ClearableNumberInput
                  disabled={isBusy}
                  max={100}
                  min={1}
                  value={jpegQuality}
                  onCommit={setJpegQuality}
                />
              </label>
            ) : (
              <div className="rounded-md border border-border bg-white p-3 text-xs text-muted">
                PNG/원본 형식은 JPG 품질 값을 사용하지 않습니다. 현재 후보는 목표 크기로
                리사이즈하고 투명도를 보존한 실제 파일 크기를 측정합니다.
              </div>
            )}

            <button
              className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
              disabled={isBusy || !selectedProfile || !selectedPiece}
              type="button"
              onClick={() => {
                void handleGenerate();
              }}
            >
              {isBusy ? "처리 중" : "용량 후보 생성"}
            </button>
            <button
              className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={isBusy || !selectedProfile || !selectedPiece}
              type="button"
              onClick={() => {
                void handleClear();
              }}
            >
              원본 export 사용
            </button>

            <section className="rounded-md border border-border bg-canvas p-3">
              <div className="flex items-center justify-between gap-2">
                <h4 className="text-xs font-semibold tracking-normal text-foreground">
                  텍스트 추가
                </h4>
                <label className="flex items-center gap-1 text-xs text-muted">
                  <input
                    checked={textEnabled}
                    disabled={isBusy}
                    type="checkbox"
                    onChange={(event) => setTextEnabled(event.currentTarget.checked)}
                  />
                  사용
                </label>
              </div>
              <textarea
                className="mt-2 min-h-16 w-full resize-y rounded-md border border-border bg-white px-2 py-1.5 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                disabled={isBusy}
                placeholder="넣을 글자"
                value={textValue}
                onChange={(event) => setTextValue(event.currentTarget.value)}
              />
              <div className="mt-2 grid grid-cols-2 gap-2">
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  크기
                  <ClearableNumberInput
                    disabled={isBusy}
                    min={1}
                    value={textFontSize}
                    onCommit={setTextFontSize}
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  외곽선
                  <ClearableNumberInput
                    disabled={isBusy}
                    min={0}
                    value={textStrokeWidth}
                    onCommit={setTextStrokeWidth}
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  X 위치
                  <input
                    disabled={isBusy}
                    max={100}
                    min={0}
                    type="range"
                    value={textX}
                    onChange={(event) => setTextX(Number(event.currentTarget.value))}
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  Y 위치
                  <input
                    disabled={isBusy}
                    max={100}
                    min={0}
                    type="range"
                    value={textY}
                    onChange={(event) => setTextY(Number(event.currentTarget.value))}
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  글자색
                  <input
                    className="h-9 rounded-md border border-border bg-white"
                    disabled={isBusy}
                    type="color"
                    value={toColorInputValue(textColor)}
                    onChange={(event) => setTextColor(event.currentTarget.value)}
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  외곽선색
                  <input
                    className="h-9 rounded-md border border-border bg-white"
                    disabled={isBusy}
                    type="color"
                    value={toColorInputValue(textStrokeColor)}
                    onChange={(event) => setTextStrokeColor(event.currentTarget.value)}
                  />
                </label>
              </div>
              <div className="mt-2 flex gap-2">
                <input
                  className="min-w-0 flex-1 rounded-md border border-border bg-white px-2 py-1.5 text-xs text-foreground"
                  disabled={isBusy}
                  placeholder="폰트 파일 경로"
                  value={textFontPath}
                  onChange={(event) => setTextFontPath(event.currentTarget.value)}
                />
                <button
                  className="shrink-0 rounded-md border border-border bg-white px-2 py-1.5 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                  disabled={isBusy}
                  type="button"
                  onClick={() => {
                    void handlePickFont();
                  }}
                >
                  폰트 선택
                </button>
              </div>
              <p className="mt-2 text-[11px] text-muted">
                기본 폰트는 설치된 Noto Sans KR/Nanum/D2Coding 같은 상용 무료 폰트를
                우선 사용합니다. 없으면 사용 가능한 ttf/otf 파일을 선택해야 합니다.
              </p>
              <button
                className="mt-2 w-full rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
                disabled={isBusy}
                type="button"
                onClick={() => {
                  void handleApplyText();
                }}
              >
                텍스트 적용
              </button>
            </section>
          </aside>

          <div className="flex min-h-0 flex-col gap-3">
            {editorState.icon.transformQuarterTurns !== 0 ||
            editorState.icon.transformFlipHorizontal ||
            editorState.icon.transformFlipVertical ? (
              <p className="rounded-md border border-border bg-canvas px-3 py-2 text-xs text-muted">
                텍스트 위치는 변형 전 원본 좌표에서 편집하며, 저장 결과에는 현재
                회전·반전이 적용됩니다.
              </p>
            ) : null}
            <AdvancedLivePreview
              color={textColor}
              disabled={isBusy}
              enabled={textEnabled}
              fontSize={textFontSize}
              isPlaybackPreviewLoading={isPlaybackPreviewLoading}
              playbackPreviewError={playbackPreviewError}
              playbackPreviewLabel={
                editorState.source.isAnimated
                  ? playbackFps === null
                    ? "원본 FPS 미리보기"
                    : `${playbackFps} FPS 미리보기`
                  : null
              }
              sourceCacheKey={playbackPreviewCacheKey}
              sourceHeight={editorState.source.height}
              sourceUrl={playbackPreviewPath ?? editorState.source.originalImageUrl}
              sourceWidth={editorState.source.width}
              strokeColor={textStrokeColor}
              strokeWidth={textStrokeWidth}
              text={textValue}
              x={textX}
              y={textY}
              onMove={(nextX, nextY) => {
                setTextX(nextX);
                setTextY(nextY);
              }}
              onResize={setTextFontSize}
            />
            {selectedProfile ? (
              <div className="rounded-md border border-border bg-canvas px-3 py-2 text-xs text-muted">
                제한: {formatBytes(selectedProfile.maxBytes)} · 출력 크기{" "}
                {selectedProfile.targetCellWidth}×{selectedProfile.targetCellHeight}
              </div>
            ) : null}

            {errorMessage ? (
              <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-danger">
                {errorMessage}
              </p>
            ) : null}

            {result ? (
              <div className="rounded-md border border-border bg-white p-3">
                <p className="text-sm font-medium">{result.message}</p>
                <p className="mt-1 text-xs text-muted">
                  기준 파일: {formatBytes(result.analysis.baselineBytes)} /{" "}
                  {formatBytes(result.analysis.targetMaxBytes)}
                </p>
              </div>
            ) : null}

            <div className="grid gap-3 md:grid-cols-3">
              {(result?.candidates ?? []).map((candidate) => (
                <AdvancedCandidateCard
                  candidate={candidate}
                  disabled={isBusy}
                  key={candidate.id}
                  onApply={(nextCandidate) => {
                    void handleApplyCandidate(nextCandidate);
                  }}
                />
              ))}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

function AdvancedLivePreview({
  color,
  disabled,
  enabled,
  fontSize,
  isPlaybackPreviewLoading,
  playbackPreviewError,
  playbackPreviewLabel,
  sourceCacheKey,
  sourceHeight,
  sourceUrl,
  sourceWidth,
  strokeColor,
  strokeWidth,
  text,
  x,
  y,
  onMove,
  onResize,
}: {
  color: string;
  disabled: boolean;
  enabled: boolean;
  fontSize: number;
  isPlaybackPreviewLoading: boolean;
  playbackPreviewError: string | null;
  playbackPreviewLabel: string | null;
  sourceCacheKey: string | null;
  sourceHeight: number;
  sourceUrl: string | null;
  sourceWidth: number;
  strokeColor: string;
  strokeWidth: number;
  text: string;
  x: number;
  y: number;
  onMove: (x: number, y: number) => void;
  onResize: (fontSize: number) => void;
}) {
  const previewRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<{
    kind: "move" | "resize";
    startClientX: number;
    startClientY: number;
    startFontSize: number;
  } | null>(null);
  const [previewWidth, setPreviewWidth] = useState(0);
  const assetUrl = filePathToAssetUrl(sourceUrl, sourceCacheKey);
  const displayScale =
    previewWidth > 0 ? previewWidth / Math.max(1, sourceWidth) : 1;

  useEffect(() => {
    const element = previewRef.current;
    if (!element) {
      return;
    }
    const update = () => setPreviewWidth(element.getBoundingClientRect().width);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const startDrag = (event: PointerEvent<HTMLElement>, kind: "move" | "resize") => {
    if (disabled || !enabled) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      kind,
      startClientX: event.clientX,
      startClientY: event.clientY,
      startFontSize: fontSize,
    };
  };

  const updateDrag = (event: PointerEvent<HTMLElement>) => {
    const dragState = dragRef.current;
    if (!dragState) {
      return;
    }
    event.preventDefault();

    if (dragState.kind === "resize") {
      const delta = Math.max(
        event.clientX - dragState.startClientX,
        event.clientY - dragState.startClientY,
      );
      onResize(
        Math.max(
          1,
          Math.round(dragState.startFontSize + delta / Math.max(0.01, displayScale) / 2),
        ),
      );
      return;
    }

    const rect = previewRef.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) {
      return;
    }
    onMove(
      clampPercent(((event.clientX - rect.left) / rect.width) * 100),
      clampPercent(((event.clientY - rect.top) / rect.height) * 100),
    );
  };

  const endDrag = (event: PointerEvent<HTMLElement>) => {
    dragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  return (
    <section className="rounded-md border border-border bg-white p-3">
      <div className="mb-2 flex items-center justify-between gap-2">
        <h4 className="text-xs font-semibold tracking-normal">실시간 편집 미리보기</h4>
        <div className="flex items-center gap-2 text-[11px] text-muted">
          {playbackPreviewLabel ? (
            <span data-testid="advanced-live-preview-fps-status">
              {isPlaybackPreviewLoading
                ? `${playbackPreviewLabel} 생성 중`
                : playbackPreviewLabel}
            </span>
          ) : null}
          <span>{fontSize}px</span>
        </div>
      </div>
      <div
        className="relative w-full overflow-hidden rounded-md border border-border bg-preview"
        ref={previewRef}
        style={{
          aspectRatio: `${Math.max(1, sourceWidth)} / ${Math.max(1, sourceHeight)}`,
        }}
      >
        {assetUrl ? (
          <img
            alt=""
            className="size-full object-fill"
            data-testid="advanced-live-preview-image"
            draggable={false}
            key={assetUrl}
            src={assetUrl}
            onDragStart={(event) => event.preventDefault()}
          />
        ) : (
          <div className="flex size-full items-center justify-center text-xs text-muted">
            미리보기 없음
          </div>
        )}
        {isPlaybackPreviewLoading ? (
          <div className="pointer-events-none absolute inset-x-2 bottom-2 rounded bg-white/85 px-2 py-1 text-center text-[11px] font-medium text-muted shadow-sm">
            GIF 재생 속도 미리보기 생성 중
          </div>
        ) : null}
        {enabled ? (
          <div
            className={cn(
              "absolute min-h-8 min-w-12 cursor-move select-none rounded border border-focus/60 bg-white/10 px-2 py-1 text-center font-semibold leading-tight",
              disabled && "cursor-not-allowed opacity-70",
            )}
            style={{
              color,
              fontSize: Math.max(1, fontSize * displayScale),
              left: `${x}%`,
              textShadow: textStrokeShadow(strokeColor, strokeWidth * displayScale),
              top: `${y}%`,
              transform: "translate(-50%, -50%)",
            }}
            onPointerCancel={endDrag}
            onPointerDown={(event) => startDrag(event, "move")}
            onPointerMove={updateDrag}
            onPointerUp={endDrag}
          >
            {text.trim() || "텍스트"}
            <span
              aria-hidden="true"
              className="absolute -bottom-1.5 -right-1.5 size-3 cursor-nwse-resize rounded-sm border border-focus bg-white"
              onPointerCancel={endDrag}
              onPointerDown={(event) => startDrag(event, "resize")}
              onPointerMove={updateDrag}
              onPointerUp={endDrag}
            />
          </div>
        ) : null}
      </div>
      {playbackPreviewError ? (
        <p className="mt-2 rounded border border-yellow-200 bg-yellow-50 px-2 py-1 text-[11px] text-muted">
          FPS 미리보기를 만들 수 없습니다. 적용 버튼을 누르면 동일한 검증을 다시 수행합니다.
        </p>
      ) : null}
    </section>
  );
}

function ClearableNumberInput({
  disabled,
  max,
  min,
  value,
  onCommit,
}: {
  disabled?: boolean;
  max?: number;
  min: number;
  value: number;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  return (
    <input
      className="rounded-md border border-border bg-white px-2 py-1.5 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
      disabled={disabled}
      max={max}
      min={min}
      type="number"
      value={draft}
      onBlur={() => {
        if (draft.trim() === "") {
          setDraft(String(value));
        }
      }}
      onChange={(event) => {
        const nextDraft = event.currentTarget.value;
        setDraft(nextDraft);
        if (nextDraft.trim() === "") {
          return;
        }
        const parsed = Number(nextDraft);
        if (!Number.isFinite(parsed)) {
          return;
        }
        const clamped = Math.max(min, max === undefined ? parsed : Math.min(max, parsed));
        onCommit(clamped);
      }}
    />
  );
}

function clampPercent(value: number) {
  return Math.min(100, Math.max(0, Math.round(value)));
}

function AdvancedCandidateCard({
  candidate,
  disabled,
  onApply,
}: {
  candidate: OptimizationCandidate;
  disabled: boolean;
  onApply: (candidate: OptimizationCandidate) => void;
}) {
  const previewUrl = filePathToAssetUrl(candidate.previewUrl || candidate.path);

  return (
    <article className="flex min-w-0 flex-col gap-2 rounded-md border border-border bg-white p-3">
      {previewUrl ? (
        <img
          alt=""
          className="aspect-square w-full rounded border border-border bg-preview object-contain"
          draggable={false}
          src={previewUrl}
          onDragStart={(event) => event.preventDefault()}
        />
      ) : (
        <div className="flex aspect-square w-full items-center justify-center rounded border border-border bg-preview text-xs text-muted">
          미리보기 없음
        </div>
      )}
      <div className="flex items-center justify-between gap-2">
        <h4 className="truncate text-sm font-semibold">{candidatePresetLabel(candidate.preset)}</h4>
        <span className={candidate.passes ? "text-xs text-foreground" : "text-xs text-danger"}>
          {candidate.passes ? "통과" : "초과"}
        </span>
      </div>
      <p className="text-xs text-muted">
        {formatBytes(candidate.measuredByteSize)} / {formatBytes(candidate.targetMaxBytes)}
      </p>
      {candidate.frameCount !== null ? (
        <p className="text-xs text-muted">
          프레임 {candidate.originalFrameCount ?? "-"} → {candidate.frameCount}
        </p>
      ) : null}
      {candidate.quality !== null ? (
        <p className="text-xs text-muted">품질 {candidate.quality}</p>
      ) : null}
      <button
        className="mt-auto rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
        disabled={disabled}
        type="button"
        onClick={() => onApply(candidate)}
      >
        {candidate.isActiveForExport ? "적용됨" : "적용"}
      </button>
    </article>
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
    transformQuarterTurns: state.icon.transformQuarterTurns,
    transformFlipHorizontal: state.icon.transformFlipHorizontal,
    transformFlipVertical: state.icon.transformFlipVertical,
    pieceIds: [...state.icon.pieces]
      .sort((left, right) => left.pieceIndex - right.pieceIndex)
      .map((piece) => piece.id),
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

function transformButtonClass() {
  return "inline-flex min-h-10 items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60";
}

function clampPanelWidth(width: number) {
  return Math.min(EDITOR_PANEL_MAX_WIDTH, Math.max(EDITOR_PANEL_MIN_WIDTH, width));
}

function DraftNumberInput({
  label,
  value,
  disabled = false,
  onChange,
}: {
  label: string;
  value: number;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <input
        className="select-text rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        disabled={disabled}
        inputMode="numeric"
        min={1}
        type="number"
        value={draft}
        onBlur={() => {
          const parsed = Number.parseInt(draft, 10);
          if (!Number.isFinite(parsed) || parsed < 1) {
            setDraft(String(value));
          }
        }}
        onChange={(event) => {
          const nextValue = event.currentTarget.value;
          setDraft(nextValue);

          if (nextValue.trim() === "") {
            return;
          }

          const parsed = Number.parseInt(nextValue, 10);
          if (Number.isFinite(parsed) && parsed >= 1) {
            onChange(parsed);
          }
        }}
      />
    </label>
  );
}

function LiveCropPreview({
  sourceUrl,
  sourceWidth,
  sourceHeight,
  crop,
  shape,
  previewWidth,
  previewHeight,
  textOverlay,
  transformQuarterTurns,
  transformFlipHorizontal,
  transformFlipVertical,
}: {
  sourceUrl: string;
  sourceWidth: number;
  sourceHeight: number;
  crop: CropRect;
  shape: IconShape;
  cellWidth: number;
  cellHeight: number;
  previewWidth: number;
  previewHeight: number;
  textOverlay?: TextOverlaySettings | null;
  transformQuarterTurns: 0 | 1 | 2 | 3;
  transformFlipHorizontal: boolean;
  transformFlipVertical: boolean;
}) {
  const viewport = previewViewportSize(previewWidth, previewHeight, shape);
  const sourceGeometry = sourceViewportGeometry({
    shape,
    cellWidth: previewWidth,
    cellHeight: previewHeight,
    transformQuarterTurns,
    transformFlipHorizontal,
    transformFlipVertical,
    pieceIds: [],
  });
  const sourceViewport = previewViewportSize(
    sourceGeometry.cellWidth,
    sourceGeometry.cellHeight,
    sourceGeometry.shape,
  );
  const scale = sourceViewport.width / Math.max(1, crop.width);

  return (
    <div
      className="relative overflow-hidden border border-border bg-white"
      data-testid="editor-live-preview"
      style={{
        height: viewport.height,
        width: viewport.width,
      }}
      onDragStart={(event) => event.preventDefault()}
    >
      <div
        className="absolute left-1/2 top-1/2"
        style={{
          height: sourceViewport.height,
          transform: "translate(-50%, -50%)",
          width: sourceViewport.width,
        }}
      >
        <div
          className="relative h-full w-full overflow-hidden"
          data-testid="editor-live-preview-transform"
          style={{
            transform: `scaleX(${transformFlipHorizontal ? -1 : 1}) scaleY(${
              transformFlipVertical ? -1 : 1
            }) rotate(${transformQuarterTurns * 90}deg)`,
            transformOrigin: "center",
          }}
        >
          <img
            alt=""
            className="pointer-events-none absolute left-0 top-0 max-w-none select-none"
            draggable={false}
            src={sourceUrl}
            style={{
              ...nonDraggableImageStyle,
              height: sourceHeight * scale,
              transform: `translate(${-crop.x * scale}px, ${-crop.y * scale}px)`,
              width: sourceWidth * scale,
            }}
            onDragStart={(event) => event.preventDefault()}
          />
          {textOverlay?.enabled && textOverlay.text.trim() ? (
            <div
              className="pointer-events-none absolute select-none whitespace-pre-line text-center font-semibold leading-[1.2]"
              data-testid="editor-live-preview-text-overlay"
              style={{
                color: textOverlay.color,
                fontSize: Math.max(1, textOverlay.fontSize * scale),
                left: (sourceWidth * textOverlay.x - crop.x) * scale,
                textShadow: textStrokeShadow(
                  textOverlay.strokeColor,
                  textOverlay.strokeWidth * scale,
                ),
                top: (sourceHeight * textOverlay.y - crop.y) * scale,
                transform: "translate(-50%, -50%)",
              }}
            >
              {textOverlay.text}
            </div>
          ) : null}
        </div>
      </div>
      {shape === "horizontal_double" ? (
        <span className="absolute bottom-0 left-1/2 top-0 border-l border-dashed border-focus" />
      ) : null}
      {shape === "vertical_double" ? (
        <span className="absolute left-0 right-0 top-1/2 border-t border-dashed border-focus" />
      ) : null}
    </div>
  );
}

function previewViewportSize(width: number, height: number, shape: IconShape) {
  return {
    width: shape === "horizontal_double" ? width * 2 : width,
    height: shape === "vertical_double" ? height * 2 : height,
  };
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

function textStrokeShadow(color: string, width: number) {
  if (!Number.isFinite(width) || width <= 0) {
    return undefined;
  }

  const radius = Math.min(12, Math.max(1, Math.round(width)));
  const shadows: string[] = [];
  for (let y = -radius; y <= radius; y += 1) {
    for (let x = -radius; x <= radius; x += 1) {
      if (x === 0 && y === 0) {
        continue;
      }
      if (x * x + y * y <= radius * radius) {
        shadows.push(`${x}px ${y}px 0 ${color}`);
      }
    }
  }
  return shadows.join(", ");
}

function candidatePresetLabel(preset: string) {
  switch (preset) {
    case "quality":
      return "화질 우선";
    case "balanced":
      return "균형";
    case "smallest":
      return "용량 우선";
    case "baseline":
      return "기준 파일";
    case "custom":
      return "사용자 설정";
    default:
      return "최적화 후보";
  }
}

function toColorInputValue(value: string) {
  const normalized = value.trim();
  if (/^#[0-9a-fA-F]{6}$/.test(normalized)) {
    return normalized;
  }
  if (/^#[0-9a-fA-F]{8}$/.test(normalized)) {
    return normalized.slice(0, 7);
  }
  return "#ffffff";
}

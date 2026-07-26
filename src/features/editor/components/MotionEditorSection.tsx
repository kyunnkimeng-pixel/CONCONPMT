import { useEffect, useMemo, useRef, useState } from "react";
import { Save, Undo2 } from "lucide-react";

import type { CollectionSummary } from "@/features/collections/types";
import {
  getIconEditorState,
  previewIconMotion,
  updateIconMotion,
} from "@/features/editor/api";
import { MotionPreviewPanel } from "@/features/editor/components/MotionPreviewPanel";
import { MotionRecipeEditor } from "@/features/editor/components/MotionRecipeEditor";
import {
  activateIconRequestLifecycle,
  captureIconRequest,
  createIconRequestLifecycle,
  invalidateIconRequestLifecycle,
  isIconRequestCurrent,
  isRevisionConflict,
  isStaleMeasurement,
} from "@/features/editor/editor-state-guards";
import {
  emptyMotionRecipe,
  hasEnabledMotion,
  motionPreviewRequestKey,
  motionRecipeSignature,
  motionRecipeStateSignature,
  normalizeMotionRecipe,
} from "@/features/editor/motion-recipe-model";
import type {
  IconEditorState,
  MotionPreviewDto,
  MotionRecipeV1,
} from "@/features/editor/types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

export function MotionEditorSection({
  collection,
  editorState,
  staticEffectDirty,
  staticEffectRevisionStale,
  onBusyChange,
  onDirtyChange,
  onEditorStateUpdated,
  onStatus,
}: {
  collection: CollectionSummary;
  editorState: IconEditorState;
  staticEffectDirty: boolean;
  staticEffectRevisionStale: boolean;
  onBusyChange: (busy: boolean) => void;
  onDirtyChange: (dirty: boolean) => void;
  onEditorStateUpdated: (state: IconEditorState) => void;
  onStatus: (message: string | null) => void;
}) {
  const initialRecipe = normalizeMotionRecipe(
    editorState.motionRecipe ?? emptyMotionRecipe(),
  );
  const [savedRecipe, setSavedRecipe] =
    useState<MotionRecipeV1>(initialRecipe);
  const [draft, setDraft] = useState<MotionRecipeV1>(initialRecipe);
  const [motionRevision, setMotionRevision] = useState(
    editorState.motionRevision ?? 0,
  );
  const [measurement, setMeasurement] =
    useState<MotionPreviewDto | null>(null);
  const [measuredRequestKey, setMeasuredRequestKey] = useState<string | null>(
    null,
  );
  const [isMeasuring, setIsMeasuring] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isReverting, setIsReverting] = useState(false);
  const [hasConflict, setHasConflict] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const requestIdRef = useRef(0);
  const lifecycleRef = useRef(
    createIconRequestLifecycle(editorState.icon.id),
  );
  const onBusyChangeRef = useRef(onBusyChange);
  const onDirtyChangeRef = useRef(onDirtyChange);
  onBusyChangeRef.current = onBusyChange;
  onDirtyChangeRef.current = onDirtyChange;
  activateIconRequestLifecycle(lifecycleRef.current, editorState.icon.id);

  const draftSignature = useMemo(
    () => motionRecipeSignature(draft),
    [draft],
  );
  const currentRequestKey = useMemo(
    () =>
      motionPreviewRequestKey({
        iconId: editorState.icon.id,
        iconUpdatedAt: editorState.icon.updatedAt,
        effectRevision: editorState.effectRevision,
        motionRevision,
        draftSignature,
        maxBytes: collection.maxBytes,
      }),
    [
      collection.maxBytes,
      draftSignature,
      editorState.effectRevision,
      editorState.icon.id,
      editorState.icon.updatedAt,
      motionRevision,
    ],
  );
  const isDirty =
    motionRecipeStateSignature(draft) !==
    motionRecipeStateSignature(savedRecipe);
  const revisionStale =
    hasConflict || (editorState.motionRevision ?? 0) !== motionRevision;
  const measurementIsFresh =
    measurement !== null && measuredRequestKey === currentRequestKey;
  const isWorking = isMeasuring || isSaving || isReverting;
  const hasActiveMotion = hasEnabledMotion(draft);
  const outputIsAnimated = editorState.source.isAnimated || hasActiveMotion;
  const staticBaseBlocked = staticEffectDirty || staticEffectRevisionStale;

  useEffect(() => {
    onDirtyChange(isDirty);
  }, [isDirty, onDirtyChange]);

  useEffect(() => {
    onBusyChange(isWorking);
  }, [isWorking, onBusyChange]);

  useEffect(() => {
    activateIconRequestLifecycle(lifecycleRef.current, editorState.icon.id);
    return () => {
      invalidateIconRequestLifecycle(lifecycleRef.current);
      requestIdRef.current += 1;
      onBusyChangeRef.current(false);
      onDirtyChangeRef.current(false);
    };
  }, [editorState.icon.id]);

  const handleMeasure = async () => {
    if (staticBaseBlocked || revisionStale || isWorking) {
      if (staticBaseBlocked) {
        setActionError(
          "정적 효과의 저장 전 변경을 먼저 저장하거나 되돌린 뒤 측정하세요.",
        );
      }
      return;
    }

    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    const requestToken = captureIconRequest(lifecycleRef.current);
    const requestedKey = currentRequestKey;
    setIsMeasuring(true);
    setPreviewError(null);
    setActionError(null);
    try {
      const nextMeasurement = await previewIconMotion(collection.id, {
        iconId: editorState.icon.id,
        recipe: draft,
      });
      if (
        requestIdRef.current !== requestId ||
        !isIconRequestCurrent(lifecycleRef.current, requestToken)
      ) {
        return;
      }
      setMeasurement(nextMeasurement);
      setMeasuredRequestKey(requestedKey);
      onStatus(
        nextMeasurement.passesByteLimit
          ? outputIsAnimated
            ? `현재 편집 미리보기 기준 모션 GIF의 가장 큰 조각을 ${formatBytes(nextMeasurement.maxPieceByteSize)}로 측정했습니다.`
            : `활성 모션이 없어 정적 미리보기의 가장 큰 조각을 ${formatBytes(nextMeasurement.maxPieceByteSize)}로 측정했습니다.`
          : `현재 편집 미리보기의 가장 큰 조각이 ${formatBytes(
              nextMeasurement.maxPieceByteSize,
            )}로 모음 제한을 넘습니다.`,
      );
    } catch (error) {
      if (
        requestIdRef.current === requestId &&
        isIconRequestCurrent(lifecycleRef.current, requestToken)
      ) {
        setPreviewError(getCommandErrorMessage(error));
      }
    } finally {
      if (
        requestIdRef.current === requestId &&
        isIconRequestCurrent(lifecycleRef.current, requestToken)
      ) {
        setIsMeasuring(false);
      }
    }
  };

  const handleSave = async () => {
    if (
      !isDirty ||
      !measurement ||
      !measurementIsFresh ||
      revisionStale ||
      staticBaseBlocked ||
      isWorking
    ) {
      return;
    }

    const requestToken = captureIconRequest(lifecycleRef.current);
    setIsSaving(true);
    setActionError(null);
    try {
      const nextState = await updateIconMotion(collection.id, {
        iconId: editorState.icon.id,
        expectedRevision: motionRevision,
        expectedRenderSignature: measurement.renderSignature,
        recipe: draft,
      });
      if (
        !isIconRequestCurrent(lifecycleRef.current, requestToken) ||
        nextState.icon.id !== requestToken.iconId
      ) {
        return;
      }
      const nextRecipe = normalizeMotionRecipe(
        nextState.motionRecipe ?? emptyMotionRecipe(),
      );
      const nextRevision = nextState.motionRevision ?? motionRevision + 1;
      const nextSignature = motionRecipeSignature(nextRecipe);
      const nextRequestKey = motionPreviewRequestKey({
        iconId: nextState.icon.id,
        iconUpdatedAt: nextState.icon.updatedAt,
        effectRevision: nextState.effectRevision,
        motionRevision: nextRevision,
        draftSignature: nextSignature,
        maxBytes: collection.maxBytes,
      });
      setSavedRecipe(nextRecipe);
      setDraft(nextRecipe);
      setMotionRevision(nextRevision);
      setMeasuredRequestKey(nextRequestKey);
      setHasConflict(false);
      onEditorStateUpdated(nextState);
      onStatus(
        "모션 설정을 저장하고 실제 미리보기와 내보내기에 적용했습니다.",
      );
    } catch (error) {
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      const revisionConflict = isRevisionConflict(error);
      if (revisionConflict) {
        setHasConflict(true);
      }
      if (revisionConflict || isStaleMeasurement(error)) {
        setMeasurement(null);
        setMeasuredRequestKey(null);
      }
      setActionError(
        `모션을 저장하지 못했습니다: ${getCommandErrorMessage(error)}`,
      );
    } finally {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        setIsSaving(false);
      }
    }
  };

  const handleRevert = async () => {
    if (
      isDirty &&
      !window.confirm(
        "저장하지 않은 모션 변경을 버리고 최신 저장값을 불러올까요?",
      )
    ) {
      return;
    }

    const requestToken = captureIconRequest(lifecycleRef.current);
    setIsReverting(true);
    setActionError(null);
    try {
      const latestState = await getIconEditorState(
        collection.id,
        editorState.icon.id,
      );
      if (
        !isIconRequestCurrent(lifecycleRef.current, requestToken) ||
        latestState.icon.id !== requestToken.iconId
      ) {
        return;
      }
      const latestRecipe = normalizeMotionRecipe(
        latestState.motionRecipe ?? emptyMotionRecipe(),
      );
      setSavedRecipe(latestRecipe);
      setDraft(latestRecipe);
      setMotionRevision(latestState.motionRevision ?? 0);
      setMeasurement(null);
      setMeasuredRequestKey(null);
      setHasConflict(false);
      setPreviewError(null);
      onEditorStateUpdated(latestState);
      onStatus("저장된 모션 설정으로 되돌렸습니다.");
    } catch (error) {
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      setActionError(
        `저장된 모션을 불러오지 못했습니다: ${getCommandErrorMessage(error)}`,
      );
    } finally {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        setIsReverting(false);
      }
    }
  };

  return (
    <div
      className="grid min-h-0 gap-4 overflow-auto p-4 md:grid-cols-[minmax(360px,1.1fr)_minmax(280px,0.9fr)]"
    >
      <MotionRecipeEditor
        disabled={isWorking || revisionStale}
        isAnimatedSource={editorState.source.isAnimated}
        measuredDurationMs={
          measurementIsFresh ? measurement?.durationMs ?? null : null
        }
        measuredFps={
          measurementIsFresh ? measurement?.effectiveFps ?? null : null
        }
        recipe={draft}
        sourceFrameCount={editorState.source.frameCount}
        onChange={(recipe) => {
          requestIdRef.current += 1;
          setDraft(normalizeMotionRecipe(recipe));
          setMeasurement(null);
          setMeasuredRequestKey(null);
          setPreviewError(null);
          setActionError(null);
          onStatus(null);
        }}
      />

      <div className="flex min-h-0 flex-col gap-3">
        <div className="flex flex-wrap justify-end gap-1 text-[11px]">
          <span
            className={cn(
              "rounded-full border px-2 py-0.5",
              isDirty
                ? "border-amber-300 bg-amber-50 text-amber-800"
                : "border-border bg-canvas text-muted",
            )}
          >
            {isDirty ? "저장 전 변경 있음" : "저장됨"}
          </span>
          <span
            className={cn(
              "rounded-full border px-2 py-0.5",
              measurementIsFresh
                ? "border-emerald-200 bg-emerald-50 text-emerald-800"
                : "border-blue-200 bg-blue-50 text-blue-800",
            )}
          >
            {measurementIsFresh ? "저장 가능한 최신 측정" : "측정 필요"}
          </span>
        </div>

        <MotionPreviewPanel
          hasActiveMotion={hasActiveMotion}
          isAnimatedSource={editorState.source.isAnimated}
          isFresh={measurementIsFresh}
          isMeasuring={isMeasuring}
          measurement={measurement}
        />

        {staticBaseBlocked ? (
          <p className="rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-900">
            모션은 저장된 정적 효과 결과를 기준으로 만듭니다. 정적 효과 탭의
            변경을 먼저 저장하거나 되돌리세요.
          </p>
        ) : null}
        {revisionStale ? (
          <p className="rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-900">
            다른 작업에서 저장된 모션 버전이 바뀌었습니다. 최신 저장값으로
            되돌린 뒤 다시 편집하세요.
          </p>
        ) : null}
        {previewError ? (
          <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-danger" role="alert">
            {outputIsAnimated ? "모션 GIF" : "정적 미리보기"}를 만들지
            못했습니다: {previewError}
          </p>
        ) : null}
        {actionError ? (
          <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-danger" role="alert">
            {actionError}
          </p>
        ) : null}

        <div className="mt-auto flex flex-wrap justify-end gap-2 border-t border-border pt-3">
          <button
            className={secondaryButtonClass}
            disabled={isWorking || (!isDirty && !revisionStale)}
            type="button"
            onClick={() => {
              void handleRevert();
            }}
          >
            <Undo2 aria-hidden="true" className="size-4" />
            {isReverting ? "저장값 불러오는 중" : "저장값으로 되돌리기"}
          </button>
          <button
            className={secondaryButtonClass}
            disabled={isWorking || revisionStale || staticBaseBlocked}
            type="button"
            onClick={() => {
              void handleMeasure();
            }}
          >
            {isMeasuring ? (
              outputIsAnimated ? "GIF 생성·측정 중" : "정적 미리보기 측정 중"
            ) : outputIsAnimated ? (
              "GIF 미리보기·용량 측정"
            ) : "정적 미리보기·용량 측정"}
          </button>
          <button
            className={primaryButtonClass}
            disabled={
              isWorking ||
              !isDirty ||
              !measurementIsFresh ||
              revisionStale ||
              staticBaseBlocked
            }
            title={
              measurementIsFresh
                ? "최신 측정과 같은 모션 recipe를 저장합니다."
                : outputIsAnimated
                  ? "먼저 현재 설정으로 GIF 미리보기·용량 측정을 실행하세요."
                  : "먼저 현재 설정으로 정적 미리보기·용량 측정을 실행하세요."
            }
            type="button"
            onClick={() => {
              void handleSave();
            }}
          >
            <Save aria-hidden="true" className="size-4" />
            {isSaving ? "모션 저장 중" : "모션 저장"}
          </button>
        </div>
      </div>
    </div>
  );
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

const secondaryButtonClass =
  "inline-flex items-center gap-1 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
const primaryButtonClass =
  "inline-flex items-center gap-1 rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60";

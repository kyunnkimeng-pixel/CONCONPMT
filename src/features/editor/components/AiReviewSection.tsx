import {
  Check,
  Eye,
  FileImage,
  History,
  ImageOff,
  Layers3,
  LoaderCircle,
  Maximize2,
  Plus,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  Sparkles,
  Upload,
  X,
} from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import type { ReactNode, RefObject } from "react";

import {
  activateAiCandidate,
  createAiIconRoot,
  getAiReviewState,
  importLocalAiCandidate,
  previewAiCandidateNormalization,
  restoreAiVersion,
} from "@/features/editor/api";
import {
  AI_NORMALIZATION_ALIGNMENT_OPTIONS,
  AI_NORMALIZATION_MODE_OPTIONS,
  AI_NORMALIZATION_RESIZE_FILTER_OPTIONS,
  createAiNormalizationPreviewRequestKey,
  createDefaultAiNormalizationOptions,
  deriveAiNormalizationPreviewStatus,
  deriveAiNormalizationWarnings,
} from "@/features/editor/ai-normalization-model";
import type {
  AiNormalizationOptions,
  AiNormalizationPreviewStatus,
  AiNormalizationWarning,
} from "@/features/editor/ai-normalization-model";
import {
  AI_CANDIDATE_IMAGE_ACCEPT,
  AI_MANUAL_SERVICE_OPTIONS,
  activeAiSourceLabel,
  aiCandidateActionState,
  aiCandidateFileFormatError,
  aiCandidateFileSizeError,
  aiServiceSurfaceLabel,
  aiSourceActionLockReason,
  formatAiRecordedAt,
} from "@/features/editor/ai-review-model";
import {
  AI_COMPARE_VIEWS,
  AI_WORKSPACE_TABS,
  aiWorkspaceLayoutForWidth,
  aiWorkspaceUiReducer,
  createInitialAiWorkspaceUiState,
  nextAiCandidateIndex,
  nextAiWorkspaceTab,
} from "@/features/editor/ai-workspace-model";
import type {
  AiCompareView,
  AiCompareZoom,
  AiWorkspaceView,
} from "@/features/editor/ai-workspace-model";
import {
  activateIconRequestLifecycle,
  captureIconRequest,
  createIconRequestLifecycle,
  invalidateIconRequestLifecycle,
  isIconRequestCurrent,
} from "@/features/editor/editor-state-guards";
import type {
  AiCandidate,
  AiCandidateUsageSummary,
  AiManualServiceSurface,
  AiNormalizationCompatibility,
  AiNormalizationPreview,
  AiNormalizationPreviewWarning,
  AiReviewState,
  AiVersion,
  EffectiveVisualSource,
  IconEditorState,
  SourceFileSummary,
} from "@/features/editor/types";
import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import { AiProviderPanel } from "@/features/editor/components/AiProviderPanel";
import { newestGeneratedCandidateId } from "@/features/editor/ai-provider-model";
import type { IconRevealAction } from "@/features/icons/icon-reveal";
import { getCommandErrorMessage } from "@/lib/tauri";
import { useModalFocus } from "@/lib/use-modal-focus";
import { cn } from "@/lib/utils";

interface AiReviewSectionProps {
  collection: CollectionSummary;
  icon: IconSummary;
  visualSource: EffectiveVisualSource;
  hasUnsavedChanges: boolean;
  onBusyChange: (busy: boolean) => void;
  onCreatedIconCommitted: (createdIcon: IconSummary) => Promise<void>;
  onEditorStateCommitted: (
    editorState: IconEditorState,
    statusMessage: string | null,
  ) => Promise<void>;
  onRevealIcon: (
    iconId: string,
    action: IconRevealAction,
  ) => boolean | Promise<boolean>;
  onModalOpenChange?: (open: boolean) => void;
}

type BusyAction =
  | "import"
  | "provider"
  | "sync-created-icon"
  | "sync-editor"
  | "restore-original"
  | `preview:${string}`
  | `create:${string}`
  | `activate:${string}`
  | `restore:${string}`;

interface NormalizationPreviewState {
  requestKey: string;
  preview: AiNormalizationPreview;
}

interface CachedEditorSyncFailure {
  detail: string;
  editorState: IconEditorState;
  statusMessage: string | null;
}

interface CachedCreatedIconSyncFailure {
  detail: string;
  createdIcon: IconSummary;
}

export type AiMutationOutcome =
  | {
      kind: "create";
      candidateId: string;
      createdIcon: IconSummary;
      createdIconUsage: AiCandidateUsageSummary;
      syncError: string | null;
    }
  | {
      kind: "activate";
      editorState: IconEditorState;
      syncError: string | null;
    };

type AiAnnouncementTone = "error" | "status";
type AiExternalHandoff = (
  handoff: () => boolean | Promise<boolean>,
) => Promise<boolean>;

const AiExternalHandoffContext = createContext<AiExternalHandoff>(async (handoff) =>
  handoff(),
);

export function AiReviewSection({
  collection,
  icon,
  visualSource,
  hasUnsavedChanges,
  onBusyChange,
  onCreatedIconCommitted,
  onEditorStateCommitted,
  onRevealIcon,
  onModalOpenChange,
}: AiReviewSectionProps) {
  const collectionId = collection.id;
  const iconId = icon.id;
  const iconName = icon.displayName;
  const [isWorkspaceOpen, setIsWorkspaceOpen] = useState(false);
  const [isInspectorExpanded, setIsInspectorExpanded] = useState(true);
  const [workspaceUi, dispatchWorkspaceUi] = useReducer(
    aiWorkspaceUiReducer,
    undefined,
    createInitialAiWorkspaceUiState,
  );
  const [workspaceLayout, setWorkspaceLayout] = useState(() =>
    aiWorkspaceLayoutForWidth(
      typeof window === "undefined" ? 1200 : window.innerWidth,
    ),
  );
  const [reviewState, setReviewState] = useState<AiReviewState | null>(null);
  const [serviceSurface, setServiceSurface] =
    useState<AiManualServiceSurface>("other_manual");
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(
    null,
  );
  const [normalizationOptions, setNormalizationOptions] =
    useState<AiNormalizationOptions>(createDefaultAiNormalizationOptions);
  const [normalizationPreview, setNormalizationPreview] =
    useState<NormalizationPreviewState | null>(null);
  const [normalizationErrorMessage, setNormalizationErrorMessage] =
    useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<BusyAction | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [mutationOutcome, setMutationOutcome] =
    useState<AiMutationOutcome | null>(null);
  const [pendingWorkspaceFocusTestId, setPendingWorkspaceFocusTestId] =
    useState<string | null>(null);
  const [fileErrorMessage, setFileErrorMessage] = useState<string | null>(null);
  const [editorSyncFailure, setEditorSyncFailure] =
    useState<CachedEditorSyncFailure | null>(null);
  const [createdIconSyncFailure, setCreatedIconSyncFailure] =
    useState<CachedCreatedIconSyncFailure | null>(null);
  const busyActionRef = useRef<BusyAction | null>(null);
  const requestIdRef = useRef(0);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const lifecycleRef = useRef(createIconRequestLifecycle(iconId));
  activateIconRequestLifecycle(lifecycleRef.current, iconId);
  const announceProvider = useCallback(
    (message: string, tone: "status" | "error") => {
      setErrorMessage(tone === "error" ? message : null);
      setStatusMessage(tone === "status" ? message : null);
    },
    [],
  );

  const acceptReviewState = useCallback(
    (nextState: AiReviewState, preferredCandidateId?: string | null) => {
      setNormalizationPreview(null);
      setNormalizationErrorMessage(null);
      setReviewState(nextState);
      setSelectedCandidateId((currentCandidateId) => {
        const preferredCandidate =
          preferredCandidateId &&
          nextState.candidates.some(
            (candidate) => candidate.id === preferredCandidateId,
          )
            ? preferredCandidateId
            : null;
        if (preferredCandidate) {
          return preferredCandidate;
        }
        if (
          currentCandidateId &&
          nextState.candidates.some(
            (candidate) => candidate.id === currentCandidateId,
          )
        ) {
          return currentCandidateId;
        }
        return nextState.candidates[0]?.id ?? null;
      });
    },
    [],
  );

  const loadReviewState = useCallback(async (successMessage?: string) => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    const requestToken = captureIconRequest(lifecycleRef.current);
    const isCurrentRequest = () =>
      requestIdRef.current === requestId &&
      isIconRequestCurrent(lifecycleRef.current, requestToken);
    setIsLoading(true);
    setErrorMessage(null);

    try {
      const nextState = await getAiReviewState(collectionId, iconId);
      if (isCurrentRequest()) {
        acceptReviewState(nextState);
        if (successMessage) {
          setStatusMessage(successMessage);
        }
      }
    } catch (error) {
      if (isCurrentRequest()) {
        setReviewState(null);
        setErrorMessage(getCommandErrorMessage(error));
      }
    } finally {
      if (isCurrentRequest()) {
        setIsLoading(false);
      }
    }
  }, [acceptReviewState, collectionId, iconId]);

  useEffect(() => {
    activateIconRequestLifecycle(lifecycleRef.current, iconId);
    setReviewState(null);
    setSelectedFile(null);
    setIsWorkspaceOpen(false);
    setIsInspectorExpanded(true);
    dispatchWorkspaceUi({ type: "reset" });
    busyActionRef.current = null;
    setBusyAction(null);
    setStatusMessage(null);
    setMutationOutcome(null);
    setPendingWorkspaceFocusTestId(null);
    setFileErrorMessage(null);
    setEditorSyncFailure(null);
    setCreatedIconSyncFailure(null);
    setNormalizationErrorMessage(null);
    setErrorMessage(null);
    setSelectedCandidateId(null);
    setNormalizationOptions(createDefaultAiNormalizationOptions());
    setNormalizationPreview(null);
    onBusyChange(false);
    if (fileInputRef.current) {
      fileInputRef.current.value = "";
    }
    void loadReviewState();
    return () => {
      requestIdRef.current += 1;
      busyActionRef.current = null;
      invalidateIconRequestLifecycle(lifecycleRef.current);
      onBusyChange(false);
    };
  }, [iconId, loadReviewState, onBusyChange]);

  useEffect(() => {
    onModalOpenChange?.(isWorkspaceOpen);
    return () => {
      if (isWorkspaceOpen) {
        onModalOpenChange?.(false);
      }
    };
  }, [isWorkspaceOpen, onModalOpenChange]);

  useEffect(() => {
    const updateLayout = () => {
      setWorkspaceLayout(aiWorkspaceLayoutForWidth(window.innerWidth));
    };
    updateLayout();
    window.addEventListener("resize", updateLayout);
    return () => window.removeEventListener("resize", updateLayout);
  }, []);

  const currentVisualSource = reviewState?.visualSource ?? visualSource;
  const selectedCandidate =
    reviewState?.candidates.find(
      (candidate) => candidate.id === selectedCandidateId,
    ) ?? null;
  const expectedPreviewRequestKey = useMemo(() => {
    if (!reviewState || !selectedCandidate) {
      return null;
    }
    return createAiNormalizationPreviewRequestKey({
      candidateId: selectedCandidate.id,
      rawSourceFileId: selectedCandidate.source.id,
      rawSourceSha256: selectedCandidate.source.sha256,
      providerNativeWidth: selectedCandidate.source.width,
      providerNativeHeight: selectedCandidate.source.height,
      targetCanvasWidth:
        reviewState.visualSource.effectiveRenderSource.width,
      targetCanvasHeight:
        reviewState.visualSource.effectiveRenderSource.height,
      originalLineageId: reviewState.visualSource.originalLineageId,
      originalLineageGeneration:
        reviewState.visualSource.originalLineageGeneration,
      activationRevision: reviewState.visualSource.activationRevision,
      nativeRecipeSignature: reviewState.nativeRecipeSignature,
      options: normalizationOptions,
    });
  }, [normalizationOptions, reviewState, selectedCandidate]);
  const latestExpectedPreviewRequestKeyRef = useRef<string | null>(null);
  latestExpectedPreviewRequestKeyRef.current = expectedPreviewRequestKey;
  const matchingNormalizationPreview =
    expectedPreviewRequestKey !== null &&
    normalizationPreview?.requestKey === expectedPreviewRequestKey
      ? normalizationPreview.preview
      : null;
  const previewAction = selectedCandidate
    ? (`preview:${selectedCandidate.id}` as const)
    : null;
  const normalizationStatus: AiNormalizationPreviewStatus =
    selectedCandidate && !selectedCandidate.isAvailable
      ? {
          code: "error",
          tone: "error",
          label: "사용할 수 없는 후보",
          message:
            selectedCandidate.unavailableReason ??
            "저장된 후보 이미지를 찾거나 읽을 수 없습니다.",
          canCommit: false,
        }
      : deriveAiNormalizationPreviewStatus({
          hasSelectedCandidate: selectedCandidate !== null,
          expectedRequestKey: expectedPreviewRequestKey,
          previewRequestKey: normalizationPreview?.requestKey ?? null,
          isPreviewing: previewAction !== null && busyAction === previewAction,
          errorMessage: normalizationErrorMessage,
        });
  const clientNormalizationWarnings = useMemo(
    () =>
      selectedCandidate && reviewState
        ? deriveAiNormalizationWarnings({
            sourceWidth: selectedCandidate.source.width,
            sourceHeight: selectedCandidate.source.height,
            sourceHasAlpha: selectedCandidate.source.hasAlpha,
            sourceIsAnimated: selectedCandidate.source.isAnimated,
            targetCanvasWidth:
              reviewState.visualSource.effectiveRenderSource.width,
            targetCanvasHeight:
              reviewState.visualSource.effectiveRenderSource.height,
            options: normalizationOptions,
          })
        : [],
    [normalizationOptions, reviewState, selectedCandidate],
  );
  const actionLockReason = aiSourceActionLockReason(hasUnsavedChanges);
  const displaySyncLockReason = createdIconSyncFailure
    ? "새 아이콘 목록 반영을 다시 시도한 뒤 다른 AI 작업을 진행해 주세요."
    : editorSyncFailure
      ? "편집기 표시를 다시 적용한 뒤 다른 AI 작업을 진행해 주세요."
      : null;
  const mutationLockReason = actionLockReason ?? displaySyncLockReason;
  const summaryRestoreLockReason = deriveAiSummaryRestoreLockReason({
    actionLockReason,
    errorMessage,
    hasReviewState: reviewState !== null,
    isLoading,
  });
  const viewingDisabled = isLoading || busyAction !== null;
  const mutationDisabled = viewingDisabled || mutationLockReason !== null;
  const beginBusyAction = (action: BusyAction) => {
    if (busyActionRef.current !== null) {
      return false;
    }
    busyActionRef.current = action;
    setBusyAction(action);
    onBusyChange(true);
    return true;
  };

  const finishBusyAction = (action: BusyAction) => {
    if (busyActionRef.current === action) {
      busyActionRef.current = null;
    }
    setBusyAction((current) => (current === action ? null : current));
    onBusyChange(false);
  };



  const previewCandidateNormalization = async () => {
    if (
      !reviewState ||
      !selectedCandidate ||
      !selectedCandidate.isAvailable ||
      !expectedPreviewRequestKey ||
      viewingDisabled
    ) {
      return;
    }

    const action: BusyAction = `preview:${selectedCandidate.id}`;
    if (!beginBusyAction(action)) {
      return;
    }
    const requestToken = captureIconRequest(lifecycleRef.current);
    const requestKey = expectedPreviewRequestKey;
    const optionsAtRequest: AiNormalizationOptions = {
      ...normalizationOptions,
      padRgba: [...normalizationOptions.padRgba],
    };
    setNormalizationErrorMessage(null);
    setErrorMessage(null);
    setStatusMessage(null);
    try {
      const preview = await previewAiCandidateNormalization(collectionId, {
        iconId,
        candidateId: selectedCandidate.id,
        expectedRevision: reviewState.visualSource.activationRevision,
        normalization: optionsAtRequest,
      });
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      if (latestExpectedPreviewRequestKeyRef.current !== requestKey) {
        setNormalizationErrorMessage(
          "후보 또는 편집 설정이 바뀌었습니다. 최신 설정으로 미리보기를 다시 만들어 주세요.",
        );
        return;
      }
      if (
        preview.candidateId !== selectedCandidate.id ||
        preview.nativeRecipeSignature !== reviewState.nativeRecipeSignature
      ) {
        setNormalizationErrorMessage(
          "미리보기 결과가 현재 후보 또는 편집 상태와 맞지 않습니다. 다시 만들어 주세요.",
        );
        return;
      }
      setNormalizationPreview({ requestKey, preview });
      dispatchWorkspaceUi({ type: "set_compare_view", view: "normalized" });
    } catch (error) {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        setNormalizationErrorMessage(getCommandErrorMessage(error));
      }
    } finally {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        finishBusyAction(action);
      }
    }
  };
  const importCandidate = async () => {
    if (!selectedFile) {
      const message = "가져올 이미지 파일을 먼저 선택해 주세요.";
      setFileErrorMessage(message);
      setErrorMessage(message);
      return;
    }
    if (viewingDisabled) {
      return;
    }

    const action: BusyAction = "import";
    if (!beginBusyAction(action)) {
      return;
    }
    const requestToken = captureIconRequest(lifecycleRef.current);
    setErrorMessage(null);
    setStatusMessage(null);
    const knownCandidateIds = new Set(
      reviewState?.candidates.map((candidate) => candidate.id) ?? [],
    );
    setNormalizationErrorMessage(null);
    try {
      const nextState = await importLocalAiCandidate(
        collectionId,
        iconId,
        serviceSurface,
        selectedFile,
      );
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      const importedCandidate = nextState.candidates.find(
        (candidate) => !knownCandidateIds.has(candidate.id),
      );
      acceptReviewState(nextState, importedCandidate?.id);
      setSelectedFile(null);
      setFileErrorMessage(null);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
      setStatusMessage(
        "로컬 결과를 비활성 후보로 보관했습니다. 현재 아이콘은 아직 바뀌지 않았습니다.",
      );
      dispatchWorkspaceUi({ type: "set_view", view: "review" });
      dispatchWorkspaceUi({ type: "set_compare_view", view: "raw" });
    } catch (error) {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        const message = getCommandErrorMessage(error);
        setFileErrorMessage(message);
        setErrorMessage(message);
      }
    } finally {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        finishBusyAction(action);
      }
    }
  };

  const activateCandidate = async (candidateId: string) => {
    const preview = matchingNormalizationPreview;
    if (
      !reviewState ||
      !selectedCandidate?.isAvailable ||
      selectedCandidate.id !== candidateId ||
      !preview ||
      preview.candidateId !== candidateId ||
      mutationDisabled
    ) {
      return;
    }

    const action: BusyAction = `activate:${candidateId}`;
    if (!beginBusyAction(action)) {
      return;
    }
    setNormalizationErrorMessage(null);
    const requestToken = captureIconRequest(lifecycleRef.current);
    setErrorMessage(null);
    setStatusMessage(null);
    setEditorSyncFailure(null);
    try {
      const result = await activateAiCandidate(collectionId, {
        iconId,
        candidateId,
        normalization: normalizationOptions,
        expectedPreviewSignature: preview.previewSignature,
        expectedRevision: reviewState.visualSource.activationRevision,
      });
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      acceptReviewState(result.reviewState, candidateId);
      let syncError: string | null = null;
      try {
        await onEditorStateCommitted(result.editorState, null);
      } catch (error) {
        syncError = getCommandErrorMessage(error);
      }
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      setMutationOutcome({
        kind: "activate",
        editorState: result.editorState,
        syncError,
      });
    } catch (error) {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        setErrorMessage(getCommandErrorMessage(error));
      }
    } finally {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        finishBusyAction(action);
      }
    }
  };

  const createIconRoot = async (candidateId: string) => {
    const preview = matchingNormalizationPreview;
    if (
      !reviewState ||
      !selectedCandidate?.isAvailable ||
      selectedCandidate.id !== candidateId ||
      !preview ||
      preview.candidateId !== candidateId ||
      mutationDisabled
    ) {
      return;
    }

    const action: BusyAction = `create:${candidateId}`;
    if (!beginBusyAction(action)) {
      return;
    }
    const requestToken = captureIconRequest(lifecycleRef.current);
    setNormalizationErrorMessage(null);
    setErrorMessage(null);
    setStatusMessage(null);
    setCreatedIconSyncFailure(null);
    try {
      const result = await createAiIconRoot(collectionId, {
        iconId,
        candidateId,
        normalization: normalizationOptions,
        expectedPreviewSignature: preview.previewSignature,
        expectedRevision: reviewState.visualSource.activationRevision,
      });
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }

      setReviewState(result.sourceReviewState);
      setSelectedCandidateId(candidateId);
      let syncError: string | null = null;
      try {
        await onCreatedIconCommitted(result.createdIcon);
      } catch (error) {
        syncError = getCommandErrorMessage(error);
      }
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      setMutationOutcome({
        kind: "create",
        candidateId,
        createdIcon: result.createdIcon,
        createdIconUsage: result.createdIconUsage,
        syncError,
      });
    } catch (error) {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        setErrorMessage(getCommandErrorMessage(error));
      }
    } finally {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        finishBusyAction(action);
      }
    }
  };

  const restoreVersion = async (versionId: string | null) => {
    const targetVersion = versionId
      ? reviewState?.versions.find((version) => version.id === versionId)
      : null;
    if (
      !reviewState ||
      mutationDisabled ||
      (versionId !== null && targetVersion?.isAvailable !== true)
    ) {
      return;
    }

    const action: BusyAction = versionId
      ? `restore:${versionId}`
      : "restore-original";
    if (!beginBusyAction(action)) {
      return;
    }
    const requestToken = captureIconRequest(lifecycleRef.current);
    const restoringFromOutcome =
      versionId === null && mutationOutcome?.kind === "activate";
    setErrorMessage(null);
    setNormalizationErrorMessage(null);
    setStatusMessage(null);
    setEditorSyncFailure(null);
    const successMessage = versionId
      ? "저장된 AI 소스로 전환했습니다. 공급자를 다시 호출하지 않았습니다."
      : "원본 소스로 전환했습니다. AI 후보와 소스 이력은 유지됩니다.";
    try {
      const result = await restoreAiVersion(collectionId, {
        iconId,
        versionId,
        expectedRevision: reviewState.visualSource.activationRevision,
      });
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      acceptReviewState(result.reviewState);
      try {
        await onEditorStateCommitted(
          result.editorState,
          isWorkspaceOpen && !restoringFromOutcome ? null : successMessage,
        );
      } catch (error) {
        setEditorSyncFailure({
          detail: getCommandErrorMessage(error),
          editorState: result.editorState,
          statusMessage:
            isWorkspaceOpen && !restoringFromOutcome ? null : successMessage,
        });
      }
      if (!isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        return;
      }
      setStatusMessage(successMessage);
      if (restoringFromOutcome) {
        setMutationOutcome(null);
        setIsWorkspaceOpen(false);
      }
    } catch (error) {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        setErrorMessage(getCommandErrorMessage(error));
      }
    } finally {
      if (isIconRequestCurrent(lifecycleRef.current, requestToken)) {
        finishBusyAction(action);
      }
    }
  };

  const retryMutationOutcomeSync = async () => {
    if (!mutationOutcome?.syncError || busyAction !== null) {
      return;
    }
    const outcome = mutationOutcome;
    const action: BusyAction =
      outcome.kind === "create" ? "sync-created-icon" : "sync-editor";
    if (!beginBusyAction(action)) {
      return;
    }
    try {
      if (outcome.kind === "create") {
        await onCreatedIconCommitted(outcome.createdIcon);
        setCreatedIconSyncFailure(null);
      } else {
        await onEditorStateCommitted(outcome.editorState, null);
        setEditorSyncFailure(null);
      }
      setMutationOutcome((current) =>
        current ? { ...current, syncError: null } : current,
      );
    } catch (error) {
      const detail = getCommandErrorMessage(error);
      setMutationOutcome((current) =>
        current ? { ...current, syncError: detail } : current,
      );
    } finally {
      finishBusyAction(action);
    }
  };

  const retryCachedEditorSync = async () => {
    if (!editorSyncFailure || busyAction !== null) {
      return;
    }
    const action: BusyAction = "sync-editor";
    if (!beginBusyAction(action)) {
      return;
    }
    try {
      await onEditorStateCommitted(
        editorSyncFailure.editorState,
        editorSyncFailure.statusMessage,
      );
      setEditorSyncFailure(null);
      setStatusMessage("편집기 표시를 저장된 결과와 맞췄습니다.");
    } catch (error) {
      setEditorSyncFailure((current) =>
        current ? { ...current, detail: getCommandErrorMessage(error) } : current,
      );
    } finally {
      finishBusyAction(action);
    }
  };

  const retryCachedCreatedIconSync = async () => {
    if (!createdIconSyncFailure || busyAction !== null) {
      return;
    }
    const action: BusyAction = "sync-created-icon";
    if (!beginBusyAction(action)) {
      return;
    }
    try {
      await onCreatedIconCommitted(createdIconSyncFailure.createdIcon);
      setCreatedIconSyncFailure(null);
      setStatusMessage("아이콘 목록을 저장된 결과와 맞췄습니다.");
    } catch (error) {
      setCreatedIconSyncFailure((current) =>
        current ? { ...current, detail: getCommandErrorMessage(error) } : current,
      );
    } finally {
      finishBusyAction(action);
    }
  };

  const selectFile = (file: File | null) => {
    setErrorMessage(null);
    setFileErrorMessage(null);
    setStatusMessage(null);
    if (!file) {
      setSelectedFile(null);
      return;
    }
    const formatError = aiCandidateFileFormatError(file);
    if (formatError) {
      setSelectedFile(null);
      setFileErrorMessage(formatError);
      setErrorMessage(formatError);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
      return;
    }
    const sizeError = aiCandidateFileSizeError(file);
    if (sizeError) {
      setSelectedFile(null);
      setFileErrorMessage(sizeError);
      setErrorMessage(sizeError);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
      return;
    }
    setSelectedFile(file);
  };

  const selectCandidateForReview = (candidateId: string) => {
    setSelectedCandidateId(candidateId);
    setNormalizationPreview(null);
    setNormalizationErrorMessage(null);
    setErrorMessage(null);
    setStatusMessage(null);
    dispatchWorkspaceUi({ type: "set_compare_view", view: "raw" });
  };

  const closeWorkspace = () => {
    if (busyActionRef.current === null) {
      setPendingWorkspaceFocusTestId(null);
      setIsWorkspaceOpen(false);
    }
  };

  const workspaceAnnouncementTone: AiAnnouncementTone =
    errorMessage ||
    normalizationErrorMessage ||
    editorSyncFailure ||
    createdIconSyncFailure
      ? "error"
      : "status";
  const workspaceAnnouncement = (
    <>
      {errorMessage ? (
        <p className="text-xs leading-5 text-danger">{errorMessage}</p>
      ) : null}
      {normalizationErrorMessage ? (
        <p className="text-xs leading-5 text-danger">
          {normalizationErrorMessage}
        </p>
      ) : null}
      {statusMessage ? (
        <p className="text-xs leading-5 text-muted">{statusMessage}</p>
      ) : null}
      {actionLockReason ? (
        <p
          className="text-xs leading-5 text-foreground"
          data-testid="ai-mutation-lock-reason"
        >
          {actionLockReason}
        </p>
      ) : null}
      {editorSyncFailure ? (
        <AiEditorRefreshWarning
          busy={busyAction === "sync-editor"}
          detail={editorSyncFailure.detail}
          disabled={isLoading || busyAction !== null}
          onRetry={() => {
            void retryCachedEditorSync();
          }}
        />
      ) : null}
      {createdIconSyncFailure ? (
        <AiCreatedIconRefreshWarning
          busy={busyAction === "sync-created-icon"}
          detail={createdIconSyncFailure.detail}
          disabled={isLoading || busyAction !== null}
          onRetry={() => {
            void retryCachedCreatedIconSync();
          }}
        />
      ) : null}
      {!errorMessage &&
      !normalizationErrorMessage &&
      !statusMessage &&
      !actionLockReason &&
      !editorSyncFailure &&
      !createdIconSyncFailure ? (
        <p className="text-xs leading-5 text-muted">
          원본은 항상 보존됩니다. 후보를 미리 본 뒤 사용할 위치를 선택하세요.
        </p>
      ) : null}
    </>
  );

  const workspaceFooter = selectedCandidate ? (
    <AiCandidateActionButtons
      actionLockReason={mutationLockReason}
      candidate={selectedCandidate}
      currentCompatibility={
        matchingNormalizationPreview?.currentIconCompatibility ?? null
      }
      disabled={mutationDisabled}
      isCurrentRecipe={matchingNormalizationPreview?.isCurrentRecipe ?? false}
      isActivating={busyAction === `activate:${selectedCandidate.id}`}
      isCreating={busyAction === `create:${selectedCandidate.id}`}
      newIconCompatibility={
        matchingNormalizationPreview?.newIconCompatibility ?? null
      }
      previewReady={
        matchingNormalizationPreview !== null && normalizationStatus.canCommit
      }
      onActivate={() => {
        void activateCandidate(selectedCandidate.id);
      }}
      onCreate={() => {
        void createIconRoot(selectedCandidate.id);
      }}
      onRevealLatestCreatedIcon={(createdIcon) =>
        onRevealIcon(createdIcon.id, "focus_tile")
      }
    />
  ) : (
    <p className="text-xs text-muted">
      후보를 가져오고 선택하면 사용 동작이 활성화됩니다.
    </p>
  );

  const continueCandidateComparison = () => {
    const candidateId =
      mutationOutcome?.kind === "create"
        ? mutationOutcome.candidateId
        : selectedCandidateId;
    if (mutationOutcome?.kind === "create" && mutationOutcome.syncError) {
      setCreatedIconSyncFailure({
        detail: mutationOutcome.syncError,
        createdIcon: mutationOutcome.createdIcon,
      });
    }
    dispatchWorkspaceUi({ type: "set_view", view: "review" });
    setPendingWorkspaceFocusTestId(
      candidateId
        ? `ai-select-candidate-${candidateId}`
        : "ai-workspace-tab-review",
    );
    setMutationOutcome(null);
  };

  const preserveActivationSyncFailure = () => {
    if (mutationOutcome?.kind === "activate" && mutationOutcome.syncError) {
      setEditorSyncFailure({
        detail: mutationOutcome.syncError,
        editorState: mutationOutcome.editorState,
        statusMessage: null,
      });
    }
  };

  const returnToEditor = () => {
    preserveActivationSyncFailure();
    setPendingWorkspaceFocusTestId(null);
    setMutationOutcome(null);
    setIsWorkspaceOpen(false);
  };

  const showSourceHistory = () => {
    preserveActivationSyncFailure();
    dispatchWorkspaceUi({ type: "set_view", view: "history" });
    setPendingWorkspaceFocusTestId("ai-workspace-tab-history");
    setMutationOutcome(null);
  };

  const closeWorkspaceAfterExternalHandoff = () => {
    setPendingWorkspaceFocusTestId(null);
    setMutationOutcome(null);
    setIsWorkspaceOpen(false);
  };

  return (
    <div
      className="border-t border-border pt-4"
      data-testid="ai-review-section"
    >
      <AiSourceSummary
        busy={busyAction === "restore-original"}
        iconName={iconName}
        isLoading={isLoading}
        mutationLockReason={summaryRestoreLockReason}
        visualSource={currentVisualSource}
        onOpen={() => {
          setPendingWorkspaceFocusTestId(null);
          setIsWorkspaceOpen(true);
        }}
        onRestoreOriginal={() => {
          void restoreVersion(null);
        }}
      />

      {editorSyncFailure && !isWorkspaceOpen ? (
        <div className="mt-3">
          <AiEditorRefreshWarning
            busy={busyAction === "sync-editor"}
            detail={editorSyncFailure.detail}
            disabled={isLoading || busyAction !== null}
            onRetry={() => {
              void retryCachedEditorSync();
            }}
          />
        </div>
      ) : null}

      {createdIconSyncFailure && !isWorkspaceOpen ? (
        <div className="mt-3">
          <AiCreatedIconRefreshWarning
            busy={busyAction === "sync-created-icon"}
            detail={createdIconSyncFailure.detail}
            disabled={isLoading || busyAction !== null}
            onRetry={() => {
              void retryCachedCreatedIconSync();
            }}
          />
        </div>
      ) : null}

      {isWorkspaceOpen ? (
        mutationOutcome ? (
          <AiMutationOutcomeDialog
            busy={busyAction !== null}
            outcome={mutationOutcome}
            onClose={
              mutationOutcome.kind === "create"
                ? continueCandidateComparison
                : returnToEditor
            }
            onContinueComparing={continueCandidateComparison}
            onExternalHandoffComplete={closeWorkspaceAfterExternalHandoff}
            onOpenCreatedIcon={(action) =>
              mutationOutcome.kind === "create"
                ? onRevealIcon(mutationOutcome.createdIcon.id, action)
                : false
            }
            onRestoreOriginal={() => {
              void restoreVersion(null);
            }}
            onRetrySync={() => {
              void retryMutationOutcomeSync();
            }}
            onReturnToEditor={returnToEditor}
            onShowHistory={showSourceHistory}
          />
        ) : (
        <AiWorkspaceDialog
          activeSourceLabel={activeAiSourceLabel(currentVisualSource)}
          activeView={workspaceUi.view}
          announcement={workspaceAnnouncement}
          announcementTone={workspaceAnnouncementTone}
          busy={busyAction !== null}
          footer={workspaceFooter}
          iconName={iconName}
          initialFocusTestId={pendingWorkspaceFocusTestId}
          layoutMode={workspaceLayout.mode}
          onClose={closeWorkspace}
          onInitialFocusApplied={() => {
            setPendingWorkspaceFocusTestId(null);
          }}
          onViewChange={(view) => {
            dispatchWorkspaceUi({ type: "set_view", view });
          }}
        >
          {workspaceUi.view === "import" ? (
            <AiImportResultPanel
              currentVisualSource={currentVisualSource}
              disabled={viewingDisabled}
              fileErrorMessage={fileErrorMessage}
              fileInputRef={fileInputRef}
              isImporting={busyAction === "import"}
              selectedFile={selectedFile}
              serviceSurface={serviceSurface}
              providerPanel={
                <AiProviderPanel
                  collection={collection}
                  disabled={viewingDisabled && busyAction !== "provider"}
                  hasUnsavedChanges={hasUnsavedChanges}
                  icon={icon}
                  source={currentVisualSource.effectiveRenderSource}
                  onAnnouncement={announceProvider}

                  onBusyEnd={() => finishBusyAction("provider")}
                  onBusyStart={() => beginBusyAction("provider")}
                  onGenerated={(nextState) => {
                    const newestCandidateId = newestGeneratedCandidateId(
                      reviewState?.candidates ?? [],
                      nextState.candidates,
                    );
                    acceptReviewState(nextState, newestCandidateId);
                    dispatchWorkspaceUi({ type: "set_view", view: "review" });
                    dispatchWorkspaceUi({
                      type: "set_compare_view",
                      view: "raw",
                    });
                  }}
                />
              }
              onFileChange={selectFile}
              onImport={() => {
                void importCandidate();
              }}
              onServiceSurfaceChange={setServiceSurface}
            />
          ) : null}

          {workspaceUi.view === "review" ? (
            isLoading && !reviewState ? (
              <AiWorkspaceLoading />
            ) : reviewState ? (
              <AiReviewWorkspaceBody
                candidateRailOrientation={
                  workspaceLayout.candidateRailOrientation
                }
                checkerboardEnabled={workspaceUi.checkerboardEnabled}
                clientWarnings={clientNormalizationWarnings}
                compareView={workspaceUi.compareView}
                compareZoom={workspaceUi.compareZoom}
                inspectorExpanded={isInspectorExpanded}
                inspectorPlacement={workspaceLayout.inspectorPlacement}
                isPreviewing={
                  previewAction !== null && busyAction === previewAction
                }
                normalizationOptions={normalizationOptions}
                normalizationPreview={matchingNormalizationPreview}
                normalizationStatus={normalizationStatus}
                reviewState={reviewState}
                selectedCandidate={selectedCandidate}
                selectedCandidateId={selectedCandidateId}
                viewingDisabled={viewingDisabled}
                onCheckerboardChange={(enabled) => {
                  dispatchWorkspaceUi({
                    type: "set_checkerboard",
                    enabled,
                  });
                }}
                onCompareViewChange={(view) => {
                  dispatchWorkspaceUi({ type: "set_compare_view", view });
                }}
                onCompareZoomChange={(zoom) => {
                  dispatchWorkspaceUi({ type: "set_compare_zoom", zoom });
                }}
                onInspectorExpandedChange={setIsInspectorExpanded}
                onNormalizationOptionsChange={(nextOptions) => {
                  setNormalizationOptions(nextOptions);
                  setNormalizationPreview(null);
                  setNormalizationErrorMessage(null);
                  setErrorMessage(null);
                  setStatusMessage(null);
                }}
                onPreview={() => {
                  void previewCandidateNormalization();
                }}
                onSelectCandidate={selectCandidateForReview}
              />
            ) : (
              <AiWorkspaceLoadError
                message={
                  errorMessage ??
                  "저장된 AI 후보와 소스 이력을 불러오지 못했습니다."
                }
                onRetry={() => {
                  void loadReviewState();
                }}
              />
            )
          ) : null}

          {workspaceUi.view === "history" ? (
            isLoading && !reviewState ? (
              <AiWorkspaceLoading />
            ) : reviewState ? (
              <AiVersionHistory
                busyAction={busyAction}
                mutationDisabled={mutationDisabled}
                mutationLockReason={actionLockReason}
                reviewState={reviewState}
                viewingDisabled={viewingDisabled}
                onRefresh={() => {
                  void loadReviewState(
                    "AI 소스 이력을 최신 상태로 불러왔습니다.",
                  );
                }}
                onRestore={restoreVersion}
              />
            ) : (
              <AiWorkspaceLoadError
                message={
                  errorMessage ??
                  "저장된 AI 후보와 소스 이력을 불러오지 못했습니다."
                }
                onRetry={() => {
                  void loadReviewState();
                }}
              />
            )
          ) : null}
        </AiWorkspaceDialog>
        )
      ) : null}
    </div>
  );
}


export function deriveAiSummaryRestoreLockReason({
  actionLockReason,
  errorMessage,
  hasReviewState,
  isLoading,
}: {
  actionLockReason: string | null;
  errorMessage: string | null;
  hasReviewState: boolean;
  isLoading: boolean;
}) {
  if (actionLockReason) {
    return actionLockReason;
  }
  if (!isLoading && !hasReviewState) {
    return (
      errorMessage?.trim() ||
      "AI 소스 이력을 불러오지 못했습니다. AI 작업공간에서 다시 시도해 주세요."
    );
  }
  return null;
}
export function AiSourceSummary({
  busy,
  iconName,
  isLoading,
  mutationLockReason,
  visualSource,
  onOpen,
  onRestoreOriginal,
}: {
  busy: boolean;
  iconName: string;
  isLoading: boolean;
  mutationLockReason: string | null;
  visualSource: EffectiveVisualSource;
  onOpen: () => void;
  onRestoreOriginal: () => void;
}) {
  const isAiActive = visualSource.activeVersionId !== null;
  return (
    <section
      aria-labelledby="ai-source-summary-title"
      className="rounded-md border border-border bg-white p-3"
      data-testid="ai-source-summary"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3
            className="inline-flex items-center gap-2 text-sm font-semibold"
            id="ai-source-summary-title"
          >
            <Sparkles aria-hidden="true" className="size-4" />
            이미지 소스
          </h3>
          <p className="mt-1 truncate text-xs text-muted" title={iconName}>
            원본: {visualSource.originalSource.originalFilename}
          </p>
          {isAiActive ? (
            <p className="mt-1 text-[11px] leading-4 text-muted">
              원본은 보존되어 있습니다.
            </p>
          ) : null}
        </div>
        <span
          className={cn(
            "shrink-0 rounded-full border px-2 py-1 text-[11px]",
            isAiActive
              ? "border-focus/40 bg-selected text-foreground"
              : "border-border bg-white text-muted",
          )}
          data-testid="ai-active-source-status"
        >
          {activeAiSourceLabel(visualSource)}
        </span>
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          className="inline-flex min-h-10 items-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
          data-testid="ai-open-workspace"
          type="button"
          onClick={onOpen}
        >
          <Sparkles aria-hidden="true" className="size-4" />
          {isAiActive ? "AI 작업공간 열기" : "AI로 수정"}
        </button>
        {isAiActive ? (
          <button
            aria-busy={busy}
            aria-describedby={
              mutationLockReason ? "ai-summary-restore-reason" : undefined
            }
            className="inline-flex min-h-10 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
            data-testid="ai-summary-restore-original"
            disabled={isLoading || busy || mutationLockReason !== null}
            title={
              mutationLockReason ??
              "AI 소스 이력을 유지한 채 보존된 원본으로 전환합니다."
            }
            type="button"
            onClick={onRestoreOriginal}
          >
            {busy ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin motion-reduce:animate-none" />
            ) : (
              <RotateCcw aria-hidden="true" className="size-4" />
            )}
            원본으로 돌아가기
          </button>
        ) : null}
      {isAiActive && mutationLockReason ? (
        <p
          className="basis-full text-[11px] leading-4 text-danger"
          id="ai-summary-restore-reason"
        >
          {mutationLockReason}
        </p>
      ) : null}
      </div>
    </section>
  );
}

export function AiWorkspaceDialog({
  activeSourceLabel: sourceLabel,
  activeView,
  announcement,
  announcementTone,
  busy,
  children,
  footer,
  iconName,
  initialFocusTestId,
  layoutMode,
  onClose,
  onInitialFocusApplied,
  onViewChange,
}: {
  activeSourceLabel: string;
  activeView: AiWorkspaceView;
  announcement: ReactNode;
  announcementTone: AiAnnouncementTone;
  busy: boolean;
  children: ReactNode;
  footer: ReactNode;
  iconName: string;
  initialFocusTestId?: string | null;
  layoutMode: "wide" | "narrow";
  onClose: () => void;
  onInitialFocusApplied?: () => void;
  onViewChange: (view: AiWorkspaceView) => void;
}) {
  const dialogRef = useRef<HTMLElement>(null);
  const { suppressFocusRestore, resumeFocusRestore } = useModalFocus(
    dialogRef,
    onClose,
    {
      initialFocus: initialFocusTestId
        ? (dialog) =>
            Array.from(
              dialog.querySelectorAll<HTMLElement>("[data-testid]"),
            ).find((element) => element.dataset.testid === initialFocusTestId) ??
            null
        : undefined,
      onInitialFocusApplied,
    },
  );
  const runExternalHandoff = useCallback(
    async (handoff: () => boolean | Promise<boolean>) => {
      suppressFocusRestore();
      try {
        const approved = await handoff();
        if (!approved) {
          resumeFocusRestore();
          return false;
        }
        onClose();
        return true;
      } catch {
        resumeFocusRestore();
        return false;
      }
    },
    [onClose, resumeFocusRestore, suppressFocusRestore],
  );

  return (
    <AiExternalHandoffContext.Provider value={runExternalHandoff}>
      <div
        className="fixed inset-0 z-[90] flex items-center justify-center bg-black/45 p-4"
        data-testid="ai-workspace-overlay"
      >
        <section
          aria-describedby="ai-workspace-description"
          aria-labelledby="ai-workspace-title"
          aria-modal="true"
          className="grid h-full max-h-[728px] w-full max-w-[1168px] grid-rows-[auto_auto_minmax(0,1fr)_auto_auto] overflow-hidden rounded-xl border border-border bg-panel shadow-2xl"
          data-layout={layoutMode}
          data-testid="ai-workspace-dialog"
          ref={dialogRef}
          role="dialog"
          tabIndex={-1}
        >
          <header
            className="flex items-start justify-between gap-4 border-b border-border bg-white px-4 py-3"
            data-testid="ai-workspace-header"
          >
            <div className="min-w-0">
              <h2
                className="truncate text-base font-semibold"
                id="ai-workspace-title"
              >
                AI로 이미지 수정 · {iconName} · {sourceLabel}
              </h2>
              <p
                className="mt-1 text-xs leading-5 text-muted"
                id="ai-workspace-description"
              >
                저장한 JPG·PNG 결과를 가져와 원본을 보존한 채 비교하고 적용합니다.
              </p>
            </div>
            <button
              aria-label="AI 작업공간 닫기"
              className="inline-flex size-9 shrink-0 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-wait disabled:opacity-50"
              data-testid="ai-workspace-close"
              disabled={busy}
              type="button"
              onClick={onClose}
            >
              <X aria-hidden="true" className="size-4" />
            </button>
          </header>
          <div
            aria-label="AI 작업공간 보기"
            className="flex gap-1 border-b border-border bg-white px-4 pt-2"
            data-testid="ai-workspace-tabs"
            role="tablist"
          >
            {AI_WORKSPACE_TABS.map((tab) => (
              <button
                aria-controls="ai-workspace-panel"
                aria-selected={activeView === tab.value}
                className={cn(
                  "min-h-10 rounded-t-md border-x border-t px-3 py-2 text-xs font-semibold focus-visible:z-10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
                  activeView === tab.value
                    ? "border-border bg-panel text-foreground"
                    : "border-transparent bg-white text-muted hover:bg-menu-hover",
                )}
                data-testid={`ai-workspace-tab-${tab.value}`}
                id={`ai-workspace-tab-${tab.value}`}
                key={tab.value}
                role="tab"
                tabIndex={activeView === tab.value ? 0 : -1}
                type="button"
                onClick={() => onViewChange(tab.value)}
                onKeyDown={(event) => {
                  const nextView = nextAiWorkspaceTab(tab.value, event.key);
                  if (!nextView) return;
                  event.preventDefault();
                  onViewChange(nextView);
                  window.requestAnimationFrame(() => {
                    document.getElementById(`ai-workspace-tab-${nextView}`)?.focus();
                  });
                }}
              >
                {tab.label}
              </button>
            ))}
          </div>
          <div
            aria-labelledby={`ai-workspace-tab-${activeView}`}
            className="min-h-0 overflow-hidden bg-panel"
            data-testid="ai-workspace-body"
            id="ai-workspace-panel"
            role="tabpanel"
          >
            {children}
          </div>
          <div
            aria-atomic="true"
            aria-live={announcementTone === "error" ? "assertive" : "polite"}
            className="max-h-32 overflow-y-auto border-t border-border bg-preview px-4 py-2"
            data-testid="ai-workspace-announcement"
            role={announcementTone === "error" ? "alert" : "status"}
          >
            {announcement}
          </div>
          <footer
            className="flex flex-wrap items-start justify-between gap-3 border-t border-border bg-white px-4 py-3"
            data-testid="ai-workspace-footer"
          >
            <button
              className="inline-flex min-h-10 items-center justify-center rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-wait disabled:opacity-50"
              disabled={busy}
              type="button"
              onClick={onClose}
            >
              취소
            </button>
            <div className="min-w-0 flex-1">{footer}</div>
          </footer>
        </section>
      </div>
    </AiExternalHandoffContext.Provider>
  );
}

export function AiMutationOutcomeDialog({
  busy,
  outcome,
  onClose,
  onContinueComparing,
  onExternalHandoffComplete,
  onOpenCreatedIcon,
  onRestoreOriginal,
  onRetrySync,
  onReturnToEditor,
  onShowHistory,
}: {
  busy: boolean;
  outcome: AiMutationOutcome;
  onClose: () => void;
  onContinueComparing: () => void;
  onExternalHandoffComplete: () => void;
  onOpenCreatedIcon: (
    action: IconRevealAction,
  ) => boolean | Promise<boolean>;
  onRestoreOriginal: () => void;
  onRetrySync: () => void;
  onReturnToEditor: () => void;
  onShowHistory: () => void;
}) {
  const dialogRef = useRef<HTMLElement>(null);
  const { suppressFocusRestore, resumeFocusRestore } = useModalFocus(
    dialogRef,
    () => {
      if (!busy) {
        onClose();
      }
    },
  );
  const syncFailed = outcome.syncError !== null;
  const title = outcome.kind === "create"
    ? "새 아이콘을 추가했습니다."
    : "현재 아이콘이 AI 소스를 사용 중입니다.";
  const runCreatedIconHandoff = async (action: IconRevealAction) => {
    suppressFocusRestore();
    try {
      const approved = await onOpenCreatedIcon(action);
      if (!approved) {
        resumeFocusRestore();
        return;
      }
      onExternalHandoffComplete();
    } catch {
      resumeFocusRestore();
    }
  };
  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 p-4"
      data-testid="ai-mutation-outcome-overlay"
    >
      <section
        aria-describedby="ai-mutation-outcome-description"
        aria-labelledby="ai-mutation-outcome-title"
        aria-modal="true"
        className="w-full max-w-lg rounded-xl border border-border bg-white p-5 shadow-2xl"
        data-kind={outcome.kind}
        data-testid="ai-mutation-outcome-dialog"
        ref={dialogRef}
        role="dialog"
        tabIndex={-1}
      >
        <h2 className="text-base font-semibold" id="ai-mutation-outcome-title">{title}</h2>
        <p className="mt-2 text-xs leading-5 text-muted" id="ai-mutation-outcome-description">
          {outcome.kind === "create"
            ? "새 아이콘은 작업 중 상태로 추가됩니다. 이름과 alt 값, crop·효과를 확인한 뒤 내보내세요."
            : "crop·효과는 유지되며 원본이나 저장된 AI 소스로 언제든 돌아갈 수 있습니다."}
        </p>
        <div
          aria-atomic="true"
          aria-live={syncFailed ? "assertive" : "polite"}
          className={cn(
            "mt-4 rounded-md border px-3 py-2 text-xs leading-5",
            syncFailed
              ? "border-danger/30 bg-danger/5 text-danger"
              : "border-focus/30 bg-selected/40 text-foreground",
          )}
          data-testid="ai-mutation-outcome-status"
          role={syncFailed ? "alert" : "status"}
        >
          {syncFailed ? (
            <>
              <p className="font-semibold">
                {outcome.kind === "create"
                  ? "저장은 완료됐지만 아이콘 목록에 표시하지 못했습니다."
                  : "저장은 완료됐지만 편집기 표시를 적용하지 못했습니다."}
              </p>
              <p className="mt-1 text-muted">
                서버에 다시 저장하지 않고 방금 받은 결과만 다시 반영합니다. {outcome.syncError}
              </p>
            </>
          ) : (
            <p>
              {outcome.kind === "create"
                ? `목록에 반영했습니다. 이 후보로 만든 아이콘 ${outcome.createdIconUsage.createdIconCount}개`
                : "편집기와 AI 소스 이력을 같은 응답으로 반영했습니다."}
            </p>
          )}
        </div>
        {syncFailed ? (
          <button
            aria-busy={busy}
            className="mt-3 inline-flex min-h-10 items-center justify-center rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-wait disabled:opacity-50"
            data-testid="ai-retry-outcome-sync"
            disabled={busy}
            type="button"
            onClick={onRetrySync}
          >
            {outcome.kind === "create" ? "목록 새로고침" : "편집기 표시 다시 적용"}
          </button>
        ) : null}
        <div className="mt-5 grid gap-2 sm:grid-cols-3">
          {outcome.kind === "create" ? (
            <>
              <button autoFocus className="min-h-10 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50" data-testid="ai-outcome-open-created-icon" disabled={busy || syncFailed} type="button" onClick={() => {
                void runCreatedIconHandoff("open_editor");
              }}>
                새 아이콘 열기
              </button>
              <button className="min-h-10 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50" data-testid="ai-outcome-reveal-created-icon" disabled={busy || syncFailed} type="button" onClick={() => {
                void runCreatedIconHandoff("focus_tile");
              }}>
                목록에서 보기
              </button>
              <button className="min-h-10 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50" data-testid="ai-outcome-continue-comparing" disabled={busy} type="button" onClick={onContinueComparing}>
                계속 후보 비교
              </button>
            </>
          ) : (
            <>
              <button autoFocus className="min-h-10 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50" data-testid="ai-outcome-return-editor" disabled={busy} type="button" onClick={onReturnToEditor}>
                편집기로 돌아가기
              </button>
              <button className="min-h-10 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50" data-testid="ai-outcome-restore-original" disabled={busy} type="button" onClick={onRestoreOriginal}>
                원본으로 돌아가기
              </button>
              <button className="min-h-10 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50" data-testid="ai-outcome-show-history" disabled={busy} type="button" onClick={onShowHistory}>
                소스 이력 보기
              </button>
            </>
          )}
        </div>
      </section>
    </div>
  );
}

export function AiImportResultPanel({
  currentVisualSource,
  disabled,
  fileErrorMessage,
  fileInputRef,
  isImporting,
  selectedFile,
  serviceSurface,
  providerPanel,
  onFileChange,
  onImport,
  onServiceSurfaceChange,
}: {
  currentVisualSource: EffectiveVisualSource;
  disabled: boolean;
  fileErrorMessage: string | null;
  fileInputRef: RefObject<HTMLInputElement | null>;
  isImporting: boolean;
  selectedFile: File | null;
  serviceSurface: AiManualServiceSurface;
  providerPanel?: ReactNode;
  onFileChange: (file: File | null) => void;
  onImport: () => void;
  onServiceSurfaceChange: (service: AiManualServiceSurface) => void;
}) {
  const fileDescriptionIds = [
    "ai-candidate-file-help",
    "ai-candidate-file-preservation",
    fileErrorMessage ? "ai-candidate-file-error" : null,
  ].filter(Boolean).join(" ");
  return (
    <div className="h-full overflow-y-auto p-4" data-testid="ai-import-panel">
      <div className="mx-auto flex w-full max-w-3xl flex-col gap-4">
        <div className="flex gap-2 rounded-md border border-focus/25 bg-selected/50 p-3 text-xs leading-5" role="note">
          <ShieldCheck aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-focus" />
          <p>
            원본은 항상 보존됩니다. API는 아래에서 키를 세션에 연결하고 결과 1장 요청을
            직접 눌렀을 때만 한 번 호출합니다. 웹 전달과 로컬 가져오기는 파일을 자동으로
            전송하지 않습니다.
          </p>
        </div>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <VisualSourceCard badge="영구 보존" label="원본" source={currentVisualSource.originalSource} />
          <VisualSourceCard
            badge={currentVisualSource.activeVersionId ? "AI 활성" : "원본과 같음"}
            label="현재 편집 소스"
            source={currentVisualSource.effectiveRenderSource}
          />
        </div>
        {providerPanel}
        <details className="rounded-md border border-border bg-white p-4">
          <summary className="cursor-pointer text-sm font-semibold">
            다른 이미지 직접 가져오기 (고급)
          </summary>
          <section className="mt-3 flex flex-col gap-3">
          <div>
            <h3 className="text-sm font-semibold">로컬 결과를 후보로 가져오기</h3>
            <p className="mt-1 text-xs leading-5 text-muted">
              출처는 이력 표시용입니다. 사이트를 열거나 파일을 자동으로 전송하지 않습니다.
            </p>
          </div>
          <div className="flex flex-col gap-1">
            <label className="text-xs font-medium text-muted" htmlFor="ai-service-surface">
              수동 작업 출처
            </label>
            <select
              aria-describedby="ai-service-surface-help"
              className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              data-testid="ai-service-surface"
              disabled={disabled}
              id="ai-service-surface"
              value={serviceSurface}
              onChange={(event) => onServiceSurfaceChange(event.currentTarget.value as AiManualServiceSurface)}
            >
              {AI_MANUAL_SERVICE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
            <p className="text-[11px] font-normal leading-4 text-muted" id="ai-service-surface-help">
              선택한 출처는 후보 이력에만 기록되며 외부 서비스를 호출하지 않습니다.
            </p>
          </div>
          <div className="flex flex-col gap-1">
            <label className="text-xs font-medium text-muted" htmlFor="ai-candidate-file">
              결과 이미지
            </label>
            <input
              accept={AI_CANDIDATE_IMAGE_ACCEPT}
              aria-describedby={fileDescriptionIds}
              aria-invalid={fileErrorMessage ? true : undefined}
              className="rounded-md border border-border bg-white px-2 py-2 text-xs text-foreground file:mr-2 file:rounded file:border-0 file:bg-menu-hover file:px-2 file:py-1 file:text-xs file:font-medium"
              data-testid="ai-candidate-file"
              disabled={disabled}
              id="ai-candidate-file"
              ref={fileInputRef}
              type="file"
              onChange={(event) => onFileChange(event.currentTarget.files?.item(0) ?? null)}
            />
            <p className="text-xs font-normal text-muted" id="ai-candidate-file-help">
              JPG·PNG 정적 이미지, 최대 16MB · 현재 소스와 다른 크기도 가능
            </p>
            <p className="text-xs font-normal leading-5 text-muted" id="ai-candidate-file-preservation">
              가져온 AI 원본은 크기와 비율 그대로 보존됩니다. GIF AI 편집은
              프레임·스프라이트 실험 단계에서 추가할 예정입니다.
            </p>
            {fileErrorMessage ? (
              <p className="text-xs font-medium leading-5 text-danger" id="ai-candidate-file-error">
                {fileErrorMessage}
              </p>
            ) : null}
          </div>
          {selectedFile ? (
            <p className="truncate text-xs text-muted">
              선택: {selectedFile.name} · {formatBytes(selectedFile.size)}
            </p>
          ) : (
            <p className="text-xs leading-5 text-muted" id="ai-import-file-required">
              파일을 선택하면 후보로 가져오기를 사용할 수 있습니다.
            </p>
          )}
          <button
            aria-busy={isImporting}
            aria-describedby={!selectedFile ? "ai-import-file-required" : undefined}
            className="inline-flex min-h-10 items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
            data-testid="ai-import-candidate"
            disabled={disabled || !selectedFile}
            title="파일을 비활성 AI 후보로 앱 라이브러리에 복사합니다."
            type="button"
            onClick={onImport}
          >
            {isImporting ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin motion-reduce:animate-none" />
            ) : (
              <Upload aria-hidden="true" className="size-4" />
            )}
            {isImporting ? "후보 가져오는 중" : "후보로 가져오기"}
          </button>
          </section>
        </details>
      </div>
    </div>
  );
}

function AiReviewWorkspaceBody({
  candidateRailOrientation,
  checkerboardEnabled,
  clientWarnings,
  compareView,
  compareZoom,
  inspectorExpanded,
  inspectorPlacement,
  isPreviewing,
  normalizationOptions,
  normalizationPreview,
  normalizationStatus,
  reviewState,
  selectedCandidate,
  selectedCandidateId,
  viewingDisabled,
  onCheckerboardChange,
  onCompareViewChange,
  onCompareZoomChange,
  onInspectorExpandedChange,
  onNormalizationOptionsChange,
  onPreview,
  onSelectCandidate,
}: {
  candidateRailOrientation: "vertical" | "horizontal";
  checkerboardEnabled: boolean;
  clientWarnings: ReadonlyArray<AiNormalizationWarning>;
  compareView: AiCompareView;
  compareZoom: AiCompareZoom;
  inspectorExpanded: boolean;
  inspectorPlacement: "right" | "bottom";
  isPreviewing: boolean;
  normalizationOptions: AiNormalizationOptions;
  normalizationPreview: AiNormalizationPreview | null;
  normalizationStatus: AiNormalizationPreviewStatus;
  reviewState: AiReviewState;
  selectedCandidate: AiCandidate | null;
  selectedCandidateId: string | null;
  viewingDisabled: boolean;
  onCheckerboardChange: (enabled: boolean) => void;
  onCompareViewChange: (view: AiCompareView) => void;
  onCompareZoomChange: (zoom: AiCompareZoom) => void;
  onInspectorExpandedChange: (expanded: boolean) => void;
  onNormalizationOptionsChange: (options: AiNormalizationOptions) => void;
  onPreview: () => void;
  onSelectCandidate: (candidateId: string) => void;
}) {
  const warnings = normalizationPreview
    ? mergeNormalizationWarnings(
        clientWarnings,
        normalizationPreview.warnings,
      )
    : clientWarnings;
  return (
    <div
      className={cn(
        "h-full min-h-0 gap-3 p-3",
        inspectorPlacement === "right"
          ? "grid grid-cols-[190px_minmax(0,1fr)_280px]"
          : "flex flex-col overflow-y-auto",
      )}
      data-testid="ai-review-workspace"
    >
      <AiCandidateRail
        activeCandidateId={reviewState.visualSource.activeCandidateId}
        candidates={reviewState.candidates}
        disabled={viewingDisabled}
        orientation={candidateRailOrientation}
        selectedCandidateId={selectedCandidateId}
        onSelect={onSelectCandidate}
      />
      <AiCandidateCompareStage
        checkerboardEnabled={checkerboardEnabled}
        compareView={compareView}
        compareZoom={compareZoom}
        normalizationPreview={normalizationPreview}
        selectedCandidate={selectedCandidate}
        visualSource={reviewState.visualSource}
        warnings={warnings}
        onCheckerboardChange={onCheckerboardChange}
        onCompareViewChange={onCompareViewChange}
        onCompareZoomChange={onCompareZoomChange}
      />
      <AiNormalizationInspector
        expanded={inspectorExpanded}
        isPreviewing={isPreviewing}
        normalizationOptions={normalizationOptions}
        normalizationStatus={normalizationStatus}
        placement={inspectorPlacement}
        reviewState={reviewState}
        selectedCandidate={selectedCandidate}
        viewingDisabled={viewingDisabled}
        onExpandedChange={onInspectorExpandedChange}
        onNormalizationOptionsChange={onNormalizationOptionsChange}
        onPreview={onPreview}
      />
    </div>
  );
}

function AiCandidateRail({
  activeCandidateId,
  candidates,
  disabled,
  orientation,
  selectedCandidateId,
  onSelect,
}: {
  activeCandidateId: string | null;
  candidates: AiCandidate[];
  disabled: boolean;
  orientation: "vertical" | "horizontal";
  selectedCandidateId: string | null;
  onSelect: (candidateId: string) => void;
}) {
  return (
    <aside
      className={cn(
        "min-h-0 rounded-md border border-border bg-white p-2",
        orientation === "vertical"
          ? "overflow-y-auto"
          : "shrink-0 overflow-x-auto",
      )}
      data-orientation={orientation}
      data-testid="ai-candidate-rail"
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <h3 className="inline-flex items-center gap-1 text-xs font-semibold">
          <FileImage aria-hidden="true" className="size-4" />
          가져온 후보
        </h3>
        <span className="text-[11px] text-muted">{candidates.length}개</span>
      </div>
      {candidates.length === 0 ? (
        <p className="rounded-md border border-dashed border-border p-3 text-xs leading-5 text-muted">
          아직 후보가 없습니다. 결과 가져오기 탭에서 이미지를 추가하세요.
        </p>
      ) : (
        <div
          aria-label="AI 후보 선택"
          aria-orientation={orientation}
          className={cn(
            "gap-2",
            orientation === "vertical"
              ? "flex flex-col"
              : "grid auto-cols-[190px] grid-flow-col",
          )}
          role="radiogroup"
        >
          {candidates.map((candidate, index) => {
            const isActive = activeCandidateId === candidate.id;
            const isSelected = selectedCandidateId === candidate.id;
            const actionState = aiCandidateActionState(candidate, isActive);
            return (
              <article
                className={cn(
                  "min-w-0 rounded-md border p-2",
                  isSelected
                    ? "border-focus bg-selected/50"
                    : isActive
                      ? "border-focus/50 bg-selected/20"
                      : "border-border bg-white",
                )}
                data-testid={`ai-candidate-${candidate.id}`}
                key={candidate.id}
              >
                <button
                  aria-checked={isSelected}
                  aria-label={`후보 ${index + 1}, ${candidate.source.originalFilename}, ${aiServiceSurfaceLabel(candidate.serviceSurface)}, ${formatAiRecordedAt(candidate.createdAt)}`}
                  className="w-full rounded text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-wait"
                  data-testid={`ai-select-candidate-${candidate.id}`}
                  disabled={disabled}
                  role="radio"
                  tabIndex={isSelected ? 0 : -1}
                  type="button"
                  onClick={() => onSelect(candidate.id)}
                  onKeyDown={(event) => {
                    const nextIndex = nextAiCandidateIndex(
                      index,
                      candidates.length,
                      event.key,
                    );
                    if (nextIndex === null) {
                      return;
                    }
                    event.preventDefault();
                    const radios = Array.from(
                      event.currentTarget
                        .closest('[role="radiogroup"]')
                        ?.querySelectorAll<HTMLButtonElement>('[role="radio"]') ??
                        [],
                    );
                    radios[nextIndex]?.focus();
                    radios[nextIndex]?.click();
                  }}
                >
                  <span className="flex items-start gap-2">
                    {candidate.isAvailable ? (
                      <img
                        alt=""
                        className="size-14 shrink-0 rounded border border-border bg-preview object-contain"
                        draggable={false}
                        src={candidate.source.originalImageUrl}
                      />
                    ) : (
                      <AiUnavailableImagePlaceholder
                        label={candidate.source.originalFilename}
                        size="candidate"
                      />
                    )}
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-xs font-semibold">
                        후보 {index + 1}
                      </span>
                      <span className="mt-0.5 block truncate text-[11px] text-muted">
                        {candidate.source.originalFilename}
                      </span>
                      <span className="mt-0.5 block truncate text-[10px] text-muted">
                        {aiServiceSurfaceLabel(candidate.serviceSurface)}
                      </span>
                      {isActive ? (
                        <span className="mt-1 inline-flex items-center gap-1 rounded-full bg-accent px-1.5 py-0.5 text-[10px] font-semibold text-accent-foreground">
                          <Check aria-hidden="true" className="size-3" />
                          적용됨
                        </span>
                      ) : null}
                    </span>
                  </span>
                  <span className="mt-2 block text-[10px] leading-4 text-muted">
                    {candidate.source.width}×{candidate.source.height}px ·{" "}
                    {formatAiRecordedAt(candidate.createdAt)}
                  </span>
                </button>
                {!candidate.isAvailable ? (
                  <p
                    className="mt-2 text-[10px] leading-4 text-danger"
                    data-testid={`ai-candidate-unavailable-${candidate.id}`}
                    role="note"
                  >
                    사용할 수 없는 후보 ·{" "}
                    {candidate.unavailableReason ??
                      "저장된 이미지를 읽을 수 없습니다."}
                  </p>
                ) : null}
                {candidate.isStale && !candidate.isMaterialized ? (
                  <p className="mt-2 text-[10px] leading-4 text-muted" role="note">
                    {actionState.reason}
                  </p>
                ) : null}
              </article>
            );
          })}
        </div>
      )}
    </aside>
  );
}

export function AiCandidateCompareStage({
  checkerboardEnabled,
  compareView,
  compareZoom,
  normalizationPreview,
  selectedCandidate,
  visualSource,
  warnings,
  onCheckerboardChange,
  onCompareViewChange,
  onCompareZoomChange,
}: {
  checkerboardEnabled: boolean;
  compareView: AiCompareView;
  compareZoom: AiCompareZoom;
  normalizationPreview: AiNormalizationPreview | null;
  selectedCandidate: AiCandidate | null;
  visualSource: EffectiveVisualSource;
  warnings: ReadonlyArray<NormalizationDisplayWarning>;
  onCheckerboardChange: (enabled: boolean) => void;
  onCompareViewChange: (view: AiCompareView) => void;
  onCompareZoomChange: (zoom: AiCompareZoom) => void;
}) {
  const originalUrl = visualSource.originalSource.originalImageUrl;
  const rawUrl =
    selectedCandidate?.isAvailable === true
      ? selectedCandidate.source.originalImageUrl
      : null;
  const normalizedUrl = normalizationPreview?.normalizedPreviewPath ?? null;
  const finalUrl = normalizationPreview?.finalPreviewPath ?? null;
  const selectedUrl =
    compareView === "original"
      ? originalUrl
      : compareView === "raw"
        ? rawUrl
        : compareView === "normalized"
          ? normalizedUrl
          : compareView === "final"
            ? finalUrl
            : null;
  const compareLabel =
    AI_COMPARE_VIEWS.find((view) => view.value === compareView)?.label ??
    "비교 이미지";
  const previewNeeded =
    (compareView === "normalized" || compareView === "final") &&
    !selectedUrl;

  return (
    <main
      className="flex min-h-[300px] min-w-0 flex-1 flex-col overflow-hidden rounded-md border border-border bg-white"
      data-testid="ai-candidate-compare-stage"
    >
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border p-2">
        <div
          aria-label="비교 이미지"
          className="flex flex-wrap gap-1"
          role="group"
        >
          {AI_COMPARE_VIEWS.map((view) => (
            <button
              aria-pressed={compareView === view.value}
              className={cn(
                "min-h-9 rounded-md border px-2 py-1 text-[11px] font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
                compareView === view.value
                  ? "border-focus bg-selected text-foreground"
                  : "border-border bg-white text-muted hover:bg-menu-hover",
              )}
              data-testid={`ai-compare-view-${view.value}`}
              key={view.value}
              type="button"
              onClick={() => onCompareViewChange(view.value)}
            >
              {view.label}
            </button>
          ))}
        </div>
        <div className="flex flex-wrap items-center gap-1">
          <button
            aria-pressed={compareZoom === "fit"}
            className={cn(
              "inline-flex min-h-9 items-center gap-1 rounded-md border px-2 py-1 text-[11px] focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
              compareZoom === "fit"
                ? "border-focus bg-selected"
                : "border-border bg-white hover:bg-menu-hover",
            )}
            data-testid="ai-compare-zoom-fit"
            type="button"
            onClick={() => onCompareZoomChange("fit")}
          >
            <Maximize2 aria-hidden="true" className="size-3" />
            화면 맞춤
          </button>
          <button
            aria-pressed={compareZoom === "actual"}
            className={cn(
              "min-h-9 rounded-md border px-2 py-1 text-[11px] focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
              compareZoom === "actual"
                ? "border-focus bg-selected"
                : "border-border bg-white hover:bg-menu-hover",
            )}
            data-testid="ai-compare-zoom-actual"
            type="button"
            onClick={() => onCompareZoomChange("actual")}
          >
            100%
          </button>
          <button
            aria-pressed={checkerboardEnabled}
            className={cn(
              "inline-flex min-h-9 items-center gap-1 rounded-md border px-2 py-1 text-[11px] focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
              checkerboardEnabled
                ? "border-focus bg-selected"
                : "border-border bg-white hover:bg-menu-hover",
            )}
            data-testid="ai-compare-checkerboard"
            type="button"
            onClick={() => onCheckerboardChange(!checkerboardEnabled)}
          >
            <Layers3 aria-hidden="true" className="size-3" />
            체커보드 배경
          </button>
        </div>
      </div>

      <div
        className="relative flex min-h-[220px] flex-1 items-center justify-center overflow-auto p-4"
        data-testid="ai-compare-canvas"
        style={checkerboardEnabled ? checkerboardStyle : undefined}
      >
        {!selectedCandidate && compareView !== "original" ? (
          <p className="text-sm text-muted">먼저 후보를 선택하세요.</p>
        ) : previewNeeded ? (
          <div className="text-center text-sm leading-6 text-muted">
            <Eye aria-hidden="true" className="mx-auto mb-2 size-5" />
            현재 설정으로 규격화 미리보기를 만들어 주세요.
          </div>
        ) : compareView === "overlay" ? (
          <div
            className={cn(
              "relative grid place-items-center",
              compareZoom === "fit" && "h-full w-full",
            )}
            data-testid="ai-compare-overlay"
          >
            <img
              alt="원본 비교"
              className={cn(
                "col-start-1 row-start-1 object-contain",
                compareZoom === "fit"
                  ? "max-h-full max-w-full"
                  : "max-h-none max-w-none",
              )}
              draggable={false}
              src={originalUrl}
            />
            {finalUrl ?? normalizedUrl ?? rawUrl ? (
              <img
                alt="AI 결과 비교"
                className={cn(
                  "col-start-1 row-start-1 object-contain opacity-50",
                  compareZoom === "fit"
                    ? "max-h-full max-w-full"
                    : "max-h-none max-w-none",
                )}
                draggable={false}
                src={finalUrl ?? normalizedUrl ?? rawUrl ?? undefined}
              />
            ) : null}
          </div>
        ) : selectedUrl ? (
          <img
            alt={`${compareLabel} 미리보기`}
            className={cn(
              "object-contain",
              compareZoom === "fit"
                ? "max-h-full max-w-full"
                : "max-h-none max-w-none",
            )}
            data-testid="ai-compare-image"
            draggable={false}
            src={selectedUrl}
          />
        ) : selectedCandidate && !selectedCandidate.isAvailable ? (
          <AiUnavailableImagePlaceholder
            label={selectedCandidate.source.originalFilename}
            size="candidate"
          />
        ) : (
          <p className="text-sm text-muted">표시할 이미지가 없습니다.</p>
        )}
      </div>

      <div className="max-h-40 overflow-y-auto border-t border-border p-3">
        <ol
          className="grid grid-cols-1 gap-1 text-[11px] leading-5 text-muted sm:grid-cols-2"
          data-testid="ai-compare-metadata"
        >
          <li>
            1. AI 원본:{" "}
            {selectedCandidate
              ? `${selectedCandidate.source.width}×${selectedCandidate.source.height}px · ${formatBytes(selectedCandidate.source.byteSize)}`
              : "후보 없음"}
          </li>
          <li>
            2. 대상 캔버스: {visualSource.effectiveRenderSource.width}×
            {visualSource.effectiveRenderSource.height}px
          </li>
          <li>
            3. 최종 출력:{" "}
            {normalizationPreview
              ? `${normalizationPreview.finalRenderWidth}×${normalizationPreview.finalRenderHeight}px`
              : "미리보기 필요"}
          </li>
          <li>
            4. 투명 픽셀:{" "}
            {normalizationPreview
              ? formatAlphaLabel(normalizationPreview.normalizedHasAlpha)
              : selectedCandidate
                ? formatAlphaLabel(selectedCandidate.source.hasAlpha)
                : "확인 전"}
          </li>
        </ol>
        <NormalizationWarningList warnings={warnings} />
      </div>
    </main>
  );
}

function AiNormalizationInspector({
  expanded,
  isPreviewing,
  normalizationOptions,
  normalizationStatus,
  placement,
  reviewState,
  selectedCandidate,
  viewingDisabled,
  onExpandedChange,
  onNormalizationOptionsChange,
  onPreview,
}: {
  expanded: boolean;
  isPreviewing: boolean;
  normalizationOptions: AiNormalizationOptions;
  normalizationStatus: AiNormalizationPreviewStatus;
  placement: "right" | "bottom";
  reviewState: AiReviewState;
  selectedCandidate: AiCandidate | null;
  viewingDisabled: boolean;
  onExpandedChange: (expanded: boolean) => void;
  onNormalizationOptionsChange: (options: AiNormalizationOptions) => void;
  onPreview: () => void;
}) {
  const isBottom = placement === "bottom";
  return (
    <aside
      className={cn(
        "min-h-0 rounded-md border border-border bg-white",
        isBottom ? "shrink-0" : "overflow-y-auto",
      )}
      data-placement={placement}
      data-testid="ai-normalization-inspector"
    >
      <div className="flex min-h-10 items-center justify-between gap-2 border-b border-border px-3 py-2">
        <div>
          <h3 className="text-xs font-semibold">크기 맞춤 설정</h3>
          {selectedCandidate ? (
            <p className="mt-0.5 max-w-[220px] truncate text-[10px] text-muted">
              {selectedCandidate.source.originalFilename}
            </p>
          ) : null}
        </div>
        {isBottom ? (
          <button
            aria-expanded={expanded}
            className="rounded-md border border-border bg-white px-2 py-1 text-[11px] hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            data-testid="ai-inspector-toggle"
            type="button"
            onClick={() => onExpandedChange(!expanded)}
          >
            {expanded ? "접기" : "펼치기"}
          </button>
        ) : null}
      </div>
      <div
        className={cn("p-3", isBottom && "max-h-[320px] overflow-y-auto")}
        hidden={isBottom && !expanded}
      >
        {selectedCandidate ? (
          <>
            {!selectedCandidate.isAvailable ? (
              <p
                className="mb-3 rounded border border-danger/30 bg-danger/5 px-3 py-2 text-xs leading-5 text-danger"
                data-testid="ai-selected-candidate-unavailable"
              >
                사용할 수 없는 후보 ·{" "}
                {selectedCandidate.unavailableReason ??
                  "저장된 후보 이미지를 찾거나 읽을 수 없습니다."}
              </p>
            ) : null}
            <AiNormalizationControls
              disabled={viewingDisabled || !selectedCandidate.isAvailable}
              isPreviewing={isPreviewing}
              options={normalizationOptions}
              status={normalizationStatus}
              targetCanvasHeight={
                reviewState.visualSource.effectiveRenderSource.height
              }
              targetCanvasWidth={
                reviewState.visualSource.effectiveRenderSource.width
              }
              onOptionsChange={onNormalizationOptionsChange}
              onPreview={onPreview}
            />
          </>
        ) : (
          <p className="text-xs leading-5 text-muted">
            후보를 선택하면 규격화 방식과 정렬을 설정할 수 있습니다.
          </p>
        )}
      </div>
    </aside>
  );
}

function AiVersionHistory({
  busyAction,
  mutationDisabled,
  mutationLockReason,
  reviewState,
  viewingDisabled,
  onRefresh,
  onRestore,
}: {
  busyAction: BusyAction | null;
  mutationDisabled: boolean;
  mutationLockReason: string | null;
  reviewState: AiReviewState;
  viewingDisabled: boolean;
  onRefresh: () => void;
  onRestore: (versionId: string | null) => void;
}) {
  const originalIsCurrent = reviewState.visualSource.activeVersionId === null;
  const refreshDisabledReason = viewingDisabled
    ? (mutationLockReason ?? "진행 중인 AI 작업이 끝나면 이력을 새로고침할 수 있습니다.")
    : null;
  const originalDisabledReason = mutationDisabled
    ? (mutationLockReason ?? "진행 중인 AI 작업이 끝나면 원본으로 돌아갈 수 있습니다.")
    : originalIsCurrent
      ? "보존된 원본을 현재 편집 소스로 사용 중입니다."
      : null;
  return (
    <div className="h-full overflow-y-auto p-4" data-testid="ai-version-history">
      <div className="mx-auto flex w-full max-w-4xl flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="inline-flex items-center gap-2 text-sm font-semibold">
              <History aria-hidden="true" className="size-4" />
              AI 소스 이력
            </h3>
            <p className="mt-1 text-xs leading-5 text-muted">
              전환은 저장된 소스를 사용하며 AI 공급자를 다시 호출하지 않습니다.
            </p>
            {refreshDisabledReason ? (
              <p className="mt-1 text-[11px] leading-4 text-muted" id="ai-history-refresh-disabled-reason">
                {refreshDisabledReason}
              </p>
            ) : null}
          </div>
          <button
            aria-busy={viewingDisabled}
            aria-describedby={refreshDisabledReason ? "ai-history-refresh-disabled-reason" : undefined}
            className="inline-flex min-h-10 items-center gap-1 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
            data-testid="ai-refresh-history"
            disabled={viewingDisabled}
            type="button"
            onClick={onRefresh}
          >
            <RefreshCw
              aria-hidden="true"
              className={cn("size-3", viewingDisabled && "animate-spin motion-reduce:animate-none")}
            />
            이력 새로고침
          </button>
        </div>
        <article
          className={cn(
            "flex items-center gap-3 rounded-md border p-3",
            originalIsCurrent ? "border-focus bg-selected/40" : "border-border bg-white",
          )}
          data-testid="ai-history-original"
        >
          <img
            alt="보존된 원본 미리보기"
            className="size-14 rounded border border-border bg-preview object-contain"
            draggable={false}
            src={reviewState.visualSource.originalSource.originalImageUrl}
          />
          <div className="min-w-0 flex-1">
            <p className="text-xs font-semibold">원본 · 영구 보존</p>
            <p className="mt-1 truncate text-[11px] text-muted">
              {reviewState.visualSource.originalSource.originalFilename}
            </p>
            {originalDisabledReason ? (
              <p className="mt-1 text-[11px] leading-4 text-muted" id="ai-restore-original-disabled-reason">
                {originalDisabledReason}
              </p>
            ) : null}
          </div>
          <button
            aria-busy={busyAction === "restore-original"}
            aria-describedby={originalDisabledReason ? "ai-restore-original-disabled-reason" : undefined}
            className="shrink-0 rounded-md border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
            data-testid="ai-restore-original"
            disabled={mutationDisabled || originalIsCurrent}
            title={mutationLockReason ?? "AI 소스 이력을 유지한 채 보존된 원본으로 전환합니다."}
            type="button"
            onClick={() => onRestore(null)}
          >
            {busyAction === "restore-original"
              ? "원본으로 전환 중"
              : originalIsCurrent
                ? "현재 소스"
                : "원본으로 돌아가기"}
          </button>
        </article>
        {reviewState.versions.length === 0 ? (
          <p className="rounded-md border border-dashed border-border p-4 text-xs leading-5 text-muted">
            후보를 한 번 적용하면 다시 API를 호출하지 않고 선택할 수 있는 AI 소스가 여기에 남습니다.
          </p>
        ) : (
          reviewState.versions.map((version, index) => {
            const action = `restore:${version.id}` as const;
            const candidateSource = reviewState.candidates.find(
              (candidate) => candidate.id === version.candidateId,
            )?.source ?? null;
            const versionDisabled = mutationDisabled || version.isActive || !version.isAvailable;
            const versionDisabledReason = !version.isAvailable
              ? (version.unavailableReason ?? "저장된 소스 이미지를 찾거나 읽을 수 없습니다.")
              : mutationLockReason ??
                (version.isActive
                  ? "현재 편집 소스로 사용 중입니다."
                  : mutationDisabled
                    ? "진행 중인 AI 작업이 끝나면 이 소스로 전환할 수 있습니다."
                    : null);
            const versionReasonId = `ai-version-disabled-reason-${version.id}`;
            return (
              <article
                className={cn(
                  "flex items-center gap-3 rounded-md border p-3",
                  version.isActive ? "border-focus bg-selected/40" : "border-border bg-white",
                )}
                data-testid={`ai-history-version-${version.id}`}
                key={version.id}
              >
                {version.isAvailable ? (
                  <img
                    alt={`${version.source.originalFilename} AI 소스 미리보기`}
                    className="size-14 rounded border border-border bg-preview object-contain"
                    draggable={false}
                    src={version.source.originalImageUrl}
                  />
                ) : (
                  <AiUnavailableImagePlaceholder label={version.source.originalFilename} size="version" />
                )}
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs font-semibold">
                    AI 소스 {reviewState.versions.length - index}
                    {version.isActive ? " · 현재 소스" : ""}
                  </p>
                  <p className="mt-0.5 text-[11px] text-muted">
                    <span className="block truncate">{version.source.originalFilename}</span>
                    {formatAiRecordedAt(version.createdAt)}
                  </p>
                  <AiVersionRecipeDetails candidateSource={candidateSource} version={version} />
                  {versionDisabledReason ? (
                    <p
                      className={cn(
                        "mt-1 text-[11px] leading-4",
                        version.isAvailable ? "text-muted" : "text-danger",
                      )}
                      data-testid={version.isAvailable
                        ? `ai-version-disabled-${version.id}`
                        : `ai-version-unavailable-${version.id}`}
                      id={versionReasonId}
                      role="note"
                    >
                      {!version.isAvailable ? "복귀할 수 없는 소스 · " : ""}
                      {versionDisabledReason}
                    </p>
                  ) : null}
                </div>
                <button
                  aria-busy={busyAction === action}
                  aria-describedby={versionDisabledReason ? versionReasonId : undefined}
                  className="shrink-0 rounded-md border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
                  disabled={versionDisabled}
                  title={versionDisabledReason ?? "저장된 AI 소스로 전환하며 AI 공급자를 다시 호출하지 않습니다."}
                  type="button"
                  onClick={() => onRestore(version.id)}
                >
                  {busyAction === action
                    ? "AI 소스로 전환 중"
                    : version.isActive
                      ? "현재 소스"
                      : "이 소스로 전환"}
                </button>
              </article>
            );
          })
        )}
      </div>
    </div>
  );
}

function AiWorkspaceLoading() {
  return (
    <div
      className="flex h-full items-center justify-center gap-2 p-6 text-sm text-muted"
      data-testid="ai-workspace-loading"
    >
      <LoaderCircle aria-hidden="true" className="size-5 animate-spin motion-reduce:animate-none" />
      저장된 AI 후보와 소스 이력을 불러오는 중입니다.
    </div>
  );
}

function AiWorkspaceLoadError({
  message,
  onRetry,
}: {
  message: string;
  onRetry: () => void;
}) {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="flex max-w-md flex-col items-start gap-3 rounded-md border border-danger/30 bg-danger/5 p-4">
        <p className="text-sm leading-6 text-danger">
          {message}
        </p>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
          type="button"
          onClick={onRetry}
        >
          다시 시도
        </button>
      </div>
    </div>
  );
}

export function AiCreatedIconRefreshWarning({
  busy,
  detail,
  disabled,
  onRetry,
}: {
  busy: boolean;
  detail: string;
  disabled: boolean;
  onRetry: () => void;
}) {
  return (
    <div
      className="flex flex-col gap-2 rounded-md border border-accent/40 bg-selected/40 p-3"
      data-testid="ai-created-icon-refresh-warning"
    >
      <p className="text-xs font-medium leading-5">
        저장은 완료됐지만 아이콘 목록에 표시하지 못했습니다.
      </p>
      <p className="text-[11px] leading-5 text-muted">
        새 아이콘은 이미 저장되어 있습니다. 서버를 다시 호출하지 않고 방금 받은
        아이콘 정보만 목록에 다시 반영합니다. {detail}
      </p>
      <button
        aria-busy={busy}
        className="self-start rounded-md border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
        data-testid="ai-retry-created-icon-refresh"
        disabled={disabled}
        type="button"
        onClick={onRetry}
      >
        {busy ? "아이콘 목록 반영 중" : "아이콘 목록 다시 반영"}
      </button>
    </div>
  );
}

export function AiEditorRefreshWarning({
  busy,
  detail,
  disabled,
  onRetry,
}: {
  busy: boolean;
  detail: string;
  disabled: boolean;
  onRetry: () => void;
}) {
  return (
    <div
      className="flex flex-col gap-2 rounded-md border border-accent/40 bg-selected/40 p-3"
      data-testid="ai-editor-refresh-warning"
    >
      <p className="text-xs font-medium leading-5">
        저장은 완료됐지만 편집기 표시를 적용하지 못했습니다.
      </p>
      <p className="text-[11px] leading-5 text-muted">
        AI 소스 이력은 방금 받은 응답으로 유지됩니다. 서버를 다시 호출하지 않고
        편집기 상태만 다시 적용합니다. {detail}
      </p>
      <button
        aria-busy={busy}
        className="self-start rounded-md border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
        data-testid="ai-retry-editor-refresh"
        disabled={disabled}
        type="button"
        onClick={onRetry}
      >
        {busy ? "편집기 표시 적용 중" : "편집기 표시 다시 적용"}
      </button>
    </div>
  );
}

export function AiUnavailableImagePlaceholder({
  label,
  size,
}: {
  label: string;
  size: "candidate" | "version";
}) {
  return (
    <div
      aria-label={`${label} 미리보기를 사용할 수 없음`}
      className={cn(
        "flex shrink-0 flex-col items-center justify-center gap-1 rounded border border-danger/30 bg-danger/5 text-danger",
        size === "candidate" ? "size-16" : "size-12",
      )}
      data-testid={`ai-unavailable-image-${size}`}
      role="img"
    >
      <ImageOff aria-hidden="true" className="size-4" />
      <span className="text-[9px] font-medium">없음</span>
    </div>
  );
}

export function AiVersionRecipeDetails({
  candidateSource,
  version,
}: {
  candidateSource: SourceFileSummary | null;
  version: AiVersion;
}) {
  const summary = formatAiVersionRecipeSummary(version);
  const sourceDimensions = candidateSource
    ? `후보 원본 ${candidateSource.width}×${candidateSource.height}px → `
    : "";
  const canvasWidth =
    version.normalizationSummary?.targetCanvasWidth ?? version.source.width;
  const canvasHeight =
    version.normalizationSummary?.targetCanvasHeight ?? version.source.height;

  return (
    <div
      className="mt-1 text-[11px] leading-4 text-muted"
      data-testid={`ai-version-recipe-${version.id}`}
    >
      <p>{summary}</p>
      <p>{`${sourceDimensions}캔버스 ${canvasWidth}×${canvasHeight}px`}</p>
    </div>
  );
}

function formatAiVersionRecipeSummary(version: AiVersion) {
  const summary = version.normalizationSummary;
  if (!summary) {
    return unknownAiVersionRecipeLabel(version.normalizationRecipeHash);
  }
  if (summary.kind === "identity") {
    return "크기 유지 · 원본과 같은 크기";
  }
  if (!summary.mode || !summary.alignment || !summary.resizeFilter) {
    return unknownAiVersionRecipeLabel(version.normalizationRecipeHash);
  }
  const modeLabel = AI_NORMALIZATION_MODE_OPTIONS.find(
    (option) => option.value === summary.mode,
  )?.label;
  const alignmentLabel = AI_NORMALIZATION_ALIGNMENT_OPTIONS.find(
    (option) => option.value === summary.alignment,
  )?.label;
  const filterLabel = AI_NORMALIZATION_RESIZE_FILTER_OPTIONS.find(
    (option) => option.value === summary.resizeFilter,
  )?.label;
  if (!modeLabel || !alignmentLabel || !filterLabel) {
    return unknownAiVersionRecipeLabel(version.normalizationRecipeHash);
  }
  return `${modeLabel} · ${alignmentLabel} · ${filterLabel}`;
}

function unknownAiVersionRecipeLabel(recipeHash: string) {
  return `규격화 설정 미상 · 레시피 #${recipeHash.slice(0, 8)}`;
}

export function AiNormalizationControls({
  disabled,
  isPreviewing,
  options,
  status,
  targetCanvasHeight,
  targetCanvasWidth,
  onOptionsChange,
  onPreview,
}: {
  disabled: boolean;
  isPreviewing: boolean;
  options: AiNormalizationOptions;
  status: AiNormalizationPreviewStatus;
  targetCanvasHeight: number;
  targetCanvasWidth: number;
  onOptionsChange: (options: AiNormalizationOptions) => void;
  onPreview: () => void;
}) {
  return (
    <div className="flex flex-col gap-3" data-testid="ai-normalization-controls">
      <fieldset className="flex flex-col gap-2" disabled={disabled}>
        <legend className="text-xs font-semibold">크기 맞춤 방식</legend>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {AI_NORMALIZATION_MODE_OPTIONS.map((option) => (
            <label
              className={cn(
                "flex min-h-10 cursor-pointer items-start gap-2 rounded-md border p-2 text-xs focus-within:outline focus-within:outline-2 focus-within:outline-focus",
                options.mode === option.value
                  ? "border-focus bg-selected/50"
                  : "border-border bg-white",
              )}
              key={option.value}
            >
              <input
                checked={options.mode === option.value}
                className="mt-0.5"
                name="ai-normalization-mode"
                type="radio"
                value={option.value}
                onChange={() =>
                  onOptionsChange({ ...options, mode: option.value })
                }
              />
              <span>
                <span className="block font-semibold text-foreground">
                  {option.label}
                </span>
                <span className="mt-0.5 block leading-4 text-muted">
                  {option.description}
                </span>
              </span>
            </label>
          ))}
        </div>
      </fieldset>

      <fieldset className="flex flex-col gap-2" disabled={disabled}>
        <legend className="text-xs font-semibold">정렬 기준</legend>
        <div className="grid grid-cols-3 gap-1" role="group">
          {AI_NORMALIZATION_ALIGNMENT_OPTIONS.map((option) => (
            <button
              aria-label={`정렬: ${option.label}`}
              aria-pressed={options.alignment === option.value}
              className={cn(
                "min-h-10 rounded-md border px-2 py-1 text-[11px] font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50",
                options.alignment === option.value
                  ? "border-focus bg-selected text-foreground"
                  : "border-border bg-white text-muted hover:bg-menu-hover",
              )}
              data-testid={`ai-normalization-align-${option.value}`}
              key={option.value}
              title={option.description}
              type="button"
              onClick={() =>
                onOptionsChange({ ...options, alignment: option.value })
              }
            >
              {option.label}
            </button>
          ))}
        </div>
      </fieldset>

      <fieldset className="flex flex-col gap-2" disabled={disabled}>
        <legend className="text-xs font-semibold">크기 조절 품질</legend>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {AI_NORMALIZATION_RESIZE_FILTER_OPTIONS.map((option) => (
            <label
              className={cn(
                "flex min-h-10 cursor-pointer items-start gap-2 rounded-md border p-2 text-xs focus-within:outline focus-within:outline-2 focus-within:outline-focus",
                options.resizeFilter === option.value
                  ? "border-focus bg-selected/50"
                  : "border-border bg-white",
              )}
              key={option.value}
            >
              <input
                checked={options.resizeFilter === option.value}
                className="mt-0.5"
                name="ai-normalization-filter"
                type="radio"
                value={option.value}
                onChange={() =>
                  onOptionsChange({ ...options, resizeFilter: option.value })
                }
              />
              <span>
                <span className="block font-semibold text-foreground">
                  {option.label}
                </span>
                <span className="mt-0.5 block leading-4 text-muted">
                  {option.description}
                </span>
              </span>
            </label>
          ))}
        </div>
      </fieldset>

      <div className="flex items-center justify-between gap-3 rounded-md bg-preview px-3 py-2 text-xs">
        <span>{`대상 캔버스: ${targetCanvasWidth}×${targetCanvasHeight}px`}</span>
        <span className="shrink-0 text-muted">여백: 투명</span>
      </div>

      <div
        className={cn(
          "rounded-md border px-3 py-2 text-xs leading-5",
          status.tone === "error"
            ? "border-danger/30 bg-danger/5 text-danger"
            : status.tone === "warning"
              ? "border-accent/40 bg-selected/40"
              : status.tone === "success"
                ? "border-focus/30 bg-selected/40"
                : "border-border bg-preview text-muted",
        )}
        data-status={status.code}
        data-testid="ai-normalization-status"
        id="ai-normalization-status"
      >
        <p className="font-semibold text-foreground">{status.label}</p>
        <p>{status.message}</p>
      </div>

      <button
        aria-busy={isPreviewing}
        aria-describedby="ai-normalization-status"
        className="inline-flex min-h-10 items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
        data-testid="ai-preview-normalization"
        disabled={disabled || isPreviewing}
        type="button"
        onClick={onPreview}
      >
        {isPreviewing ? (
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin motion-reduce:animate-none" />
        ) : (
          <RefreshCw aria-hidden="true" className="size-4" />
        )}
        {isPreviewing
          ? "규격화 미리보기 만드는 중"
          : status.code === "ready"
            ? "규격화 미리보기 다시 만들기"
            : "규격화 미리보기"}
      </button>
    </div>
  );
}

type NormalizationDisplayWarning =
  | AiNormalizationWarning
  | AiNormalizationPreviewWarning;

export function AiNormalizationPreviewComparison({
  preview,
  warnings,
}: {
  preview: AiNormalizationPreview;
  warnings: ReadonlyArray<NormalizationDisplayWarning>;
}) {
  return (
    <div className="flex flex-col gap-3" data-testid="ai-normalization-comparison">
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        <NormalizationPreviewCard
          alphaLabel={formatAlphaLabel(preview.rawSource.hasAlpha)}
          dimensions={`${preview.rawSource.width}×${preview.rawSource.height}px`}
          imageUrl={preview.rawSource.originalImageUrl}
          label="AI 원본 · raw"
          testId="ai-preview-raw"
        />
        <NormalizationPreviewCard
          alphaLabel={
            preview.normalizedHasAlpha ? "투명 영역 있음" : "불투명"
          }
          dimensions={`${preview.targetCanvasWidth}×${preview.targetCanvasHeight}px`}
          imageUrl={preview.normalizedPreviewPath}
          label="규격화 캔버스"
          testId="ai-preview-normalized"
        />
        <NormalizationPreviewCard
          alphaLabel="편집 효과 포함"
          dimensions={`${preview.finalRenderWidth}×${preview.finalRenderHeight}px`}
          imageUrl={preview.finalPreviewPath}
          label="최종 편집기 렌더"
          testId="ai-preview-final"
        />
      </div>

      <p className="rounded-md bg-preview px-3 py-2 text-[11px] leading-5 text-muted">
        {formatNormalizationGeometry(preview)}
      </p>
      <p
        className="rounded-md bg-preview px-3 py-2 text-[11px] leading-5 text-muted"
        data-testid="ai-final-output-size"
      >
        {formatFinalOutputSize(preview)}
      </p>
      <NormalizationWarningList warnings={warnings} />
    </div>
  );
}

function NormalizationPreviewCard({
  alphaLabel,
  dimensions,
  imageUrl,
  label,
  testId,
}: {
  alphaLabel: string;
  dimensions: string;
  imageUrl: string;
  label: string;
  testId: string;
}) {
  return (
    <article className="min-w-0 rounded-md border border-border bg-white p-2" data-testid={testId}>
      <div
        className="flex aspect-square items-center justify-center overflow-hidden rounded border border-border p-1"
        style={checkerboardStyle}
      >
        {imageUrl ? (
          <img
            alt={`${label} 미리보기`}
            className="max-h-full max-w-full object-contain"
            draggable={false}
            src={imageUrl}
          />
        ) : (
          <span className="text-[11px] text-muted">미리보기 없음</span>
        )}
      </div>
      <p className="mt-2 truncate text-xs font-semibold">{label}</p>
      <p className="mt-1 text-[11px] leading-4 text-muted">
        {`${dimensions} · ${alphaLabel}`}
      </p>
    </article>
  );
}

function NormalizationWarningList({
  warnings,
}: {
  warnings: ReadonlyArray<NormalizationDisplayWarning>;
}) {
  if (warnings.length === 0) {
    return null;
  }
  return (
    <ul className="flex flex-col gap-1" data-testid="ai-normalization-warnings">
      {warnings.map((warning) => (
        <li
          className={cn(
            "rounded-md border px-3 py-2 text-xs leading-5",
            warning.severity === "warning"
              ? "border-accent/40 bg-selected/40"
              : "border-border bg-preview text-muted",
          )}
          data-warning-code={warning.code}
          key={warning.code}
        >
          {warning.message}
        </li>
      ))}
    </ul>
  );
}

function mergeNormalizationWarnings(
  clientWarnings: ReadonlyArray<AiNormalizationWarning>,
  backendWarnings: ReadonlyArray<AiNormalizationPreviewWarning>,
) {
  const warnings = new Map<string, NormalizationDisplayWarning>();
  for (const warning of clientWarnings) {
    warnings.set(warning.code, warning);
  }
  for (const warning of backendWarnings) {
    warnings.set(warning.code, warning);
  }
  return [...warnings.values()];
}

function formatNormalizationGeometry(preview: AiNormalizationPreview) {
  if (preview.geometry.kind === "identity") {
    return "AI 원본과 대상 캔버스 크기가 같아 픽셀 크기를 바꾸지 않았습니다.";
  }
  const cropText =
    preview.geometry.kind === "cover_crop"
      ? ` · 자르기 기준 ${preview.geometry.cropX}, ${preview.geometry.cropY}`
      : "";
  const pasteText =
    preview.geometry.kind === "contain_pad"
      ? ` · 배치 위치 ${preview.geometry.pasteX}, ${preview.geometry.pasteY}`
      : "";
  return `리사이즈 ${preview.geometry.resizedWidth}×${preview.geometry.resizedHeight}px${cropText}${pasteText}`;
}

function formatFinalOutputSize(preview: AiNormalizationPreview) {
  const finalArea = preview.finalRenderWidth * preview.finalRenderHeight;
  const pieceArea = preview.pieceWidth * preview.pieceHeight;
  const pieceCount =
    pieceArea > 0 && finalArea > 0 && finalArea % pieceArea === 0
      ? finalArea / pieceArea
      : null;
  const pieceCountLabel = pieceCount ? ` · ${pieceCount}조각` : "";
  return `최종 렌더: ${preview.finalRenderWidth}×${preview.finalRenderHeight}px · 조각 규격: ${preview.pieceWidth}×${preview.pieceHeight}px${pieceCountLabel}`;
}

function formatAlphaLabel(hasAlpha: boolean | null) {
  if (hasAlpha === null) {
    return "투명도 확인 전";
  }
  return hasAlpha ? "투명 영역 있음" : "불투명";
}

const checkerboardStyle = {
  backgroundColor: "#ffffff",
  backgroundImage:
    "linear-gradient(45deg, rgba(15, 23, 42, 0.08) 25%, transparent 25%), linear-gradient(-45deg, rgba(15, 23, 42, 0.08) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, rgba(15, 23, 42, 0.08) 75%), linear-gradient(-45deg, transparent 75%, rgba(15, 23, 42, 0.08) 75%)",
  backgroundPosition: "0 0, 0 8px, 8px -8px, -8px 0",
  backgroundSize: "16px 16px",
};

export function AiCandidateActionButtons({
  actionLockReason,
  candidate,
  currentCompatibility,
  disabled,
  isCurrentRecipe,
  isActivating,
  isCreating,
  newIconCompatibility,
  previewReady,
  onActivate,
  onCreate,
  onRevealLatestCreatedIcon,
}: {
  actionLockReason: string | null;
  candidate: AiCandidate;
  currentCompatibility: AiNormalizationCompatibility | null;
  disabled: boolean;
  isCurrentRecipe: boolean;
  isActivating: boolean;
  isCreating: boolean;
  newIconCompatibility: AiNormalizationCompatibility | null;
  previewReady: boolean;
  onActivate: () => void;
  onCreate: () => void;
  onRevealLatestCreatedIcon?: (
    createdIcon: IconSummary,
  ) => boolean | Promise<boolean>;
}) {
  const externalHandoff = useContext(AiExternalHandoffContext);
  const previewRequiredReason =
    "사용하기 전에 현재 설정으로 규격화 미리보기를 만들어 확인해 주세요.";
  const compatibleCreateReason = candidate.isStale
    ? "현재 아이콘과 맞지 않는 오래된 후보지만 별도의 새 아이콘으로는 안전하게 추가할 수 있습니다."
    : "원본 아이콘과 분리된 새 아이콘으로 추가합니다.";
  const unavailableReason = !candidate.isAvailable
    ? (candidate.unavailableReason ?? "저장된 후보 이미지를 찾거나 읽을 수 없습니다.")
    : null;
  const createReason = unavailableReason ?? actionLockReason ??
    (!previewReady
      ? previewRequiredReason
      : newIconCompatibility?.allowed !== true
        ? (newIconCompatibility?.reason ?? "이 규격화 결과는 새 아이콘으로 추가할 수 없습니다.")
        : compatibleCreateReason);
  const currentReason = unavailableReason ?? actionLockReason ??
    (!previewReady
      ? previewRequiredReason
      : isCurrentRecipe
        ? "현재 설정의 규격화 결과를 이미 편집 소스로 사용 중입니다."
        : currentCompatibility?.allowed !== true
          ? (currentCompatibility?.reason ?? "이 규격화 결과는 현재 아이콘에 사용할 수 없습니다.")
          : "현재 crop·효과는 유지되며 원본과 이전 AI 소스로 언제든 돌아갈 수 있습니다.");
  const createDisabled = disabled || !candidate.isAvailable || !previewReady ||
    newIconCompatibility?.allowed !== true;
  const activateDisabled = disabled || !candidate.isAvailable || !previewReady ||
    isCurrentRecipe || currentCompatibility?.allowed !== true;
  const createdIconCount = candidate.createdIconUsage.createdIconCount;
  const latestCreatedIcon = candidate.createdIconUsage.latestCreatedIcon;
  const createLabel = createdIconCount > 0
    ? "이 후보로 하나 더 추가"
    : "새 아이콘으로 추가 · 권장";
  return (
    <div className="flex flex-col items-end gap-2" data-testid={`ai-candidate-actions-${candidate.id}`}>
      <div className="flex flex-wrap items-center justify-end gap-2">
        <button
          aria-busy={isActivating}
          aria-describedby={`ai-current-reason-${candidate.id}`}
          className="inline-flex min-h-10 items-center gap-1 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
          data-testid={`ai-activate-current-${candidate.id}`}
          disabled={activateDisabled}
          title={currentReason}
          type="button"
          onClick={onActivate}
        >
          {isActivating ? (
            <LoaderCircle aria-hidden="true" className="size-3 animate-spin motion-reduce:animate-none" />
          ) : (
            <Sparkles aria-hidden="true" className="size-3" />
          )}
          {isActivating ? "현재 아이콘에 사용하는 중" : "현재 아이콘에 사용"}
        </button>
        <button
          aria-busy={isCreating}
          aria-describedby={`ai-create-reason-${candidate.id}`}
          className="inline-flex min-h-10 items-center gap-1 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
          data-testid={`ai-create-icon-${candidate.id}`}
          disabled={createDisabled}
          title={createReason}
          type="button"
          onClick={onCreate}
        >
          {isCreating ? (
            <LoaderCircle aria-hidden="true" className="size-3 animate-spin motion-reduce:animate-none" />
          ) : (
            <Plus aria-hidden="true" className="size-3" />
          )}
          {isCreating ? "새 아이콘으로 추가하는 중" : createLabel}
        </button>
      </div>
      <div className="max-w-2xl text-right text-[11px] leading-4 text-muted">
        <p id={`ai-current-reason-${candidate.id}`}>{currentReason}</p>
        <p className="mt-1" id={`ai-create-reason-${candidate.id}`}>{createReason}</p>
        {createdIconCount > 0 ? (
          <div
            className="mt-2 flex flex-wrap items-center justify-end gap-2"
            data-testid={`ai-created-icon-usage-${candidate.id}`}
          >
            <span>{`이 후보로 만든 아이콘 ${createdIconCount}개`}</span>
            {latestCreatedIcon && onRevealLatestCreatedIcon ? (
              <button
                aria-describedby={`ai-create-reason-${candidate.id}`}
                className="rounded text-focus underline underline-offset-2 hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted disabled:no-underline"
                data-testid={`ai-reveal-latest-created-${candidate.id}`}
                disabled={disabled}
                type="button"
                onClick={() => {
                  void externalHandoff(() =>
                    onRevealLatestCreatedIcon(latestCreatedIcon),
                  );
                }}
              >
                최근 만든 아이콘 보기
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function VisualSourceCard({
  label,
  badge,
  source,
}: {
  label: string;
  badge: string;
  source: SourceFileSummary;
}) {
  return (
    <article className="min-w-0 rounded-md border border-border bg-white p-2">
      <img
        alt={`${label} 미리보기`}
        className="aspect-square w-full rounded border border-border bg-preview object-contain"
        draggable={false}
        src={source.originalImageUrl}
      />
      <div className="mt-2 flex items-center justify-between gap-1">
        <p className="truncate text-xs font-semibold">{label}</p>
        <span className="shrink-0 rounded-full border border-border px-1.5 py-0.5 text-[10px] text-muted">
          {badge}
        </span>
      </div>
      <p className="mt-1 truncate text-[11px] text-muted">
        {source.originalFilename}
      </p>
    </article>
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

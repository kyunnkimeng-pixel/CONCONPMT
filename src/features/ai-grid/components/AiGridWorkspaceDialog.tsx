import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  DragEvent,
  KeyboardEvent,
  MouseEvent,
  ReactNode,
  RefObject,
} from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  ExternalLink,
  FolderOpen,
  GripVertical,
  ImagePlus,
  Trash2,
  Upload,
  X,
} from "lucide-react";

import {
  NovelAiWebGuide,
  type NovelAiPromptCopyOutcome,
} from "@/features/ai-web/components/NovelAiWebGuide";
import { needsNovelAiEnglishInputHint } from "@/features/ai-web/novelai-web-model";
import {
  analyzeAiGridOutput,
  attachAiGridOutput,
  cancelAiGridWorkspace,
  commitAiGeneratedIcons,
  commitAiGridReview,
  getLatestAiGridWorkspace,
  markAiGridWorkspaceAwaitingResult,
  MAX_AI_REFERENCE_EXTERNAL_BYTES,
  prepareAiGenerationWorkspace,
  prepareAiGridEditWorkspace,
  revealAiGridInput,
  startAiGridInputDrag,
} from "@/features/ai-grid/api";
import {
  buildAiGridCorrectionPrompt,
  buildAiGridMissingAlphaCorrectionPrompt,
} from "@/features/ai-grid/ai-grid-correction";
import {
  aiGridStepForStatus,
  buildAiGridPrompt,
  defaultResultMapping,
  reviewDecisions,
  selectAiGridResultFile,
  sheetSettingsFromLayout,
  validateReviewDecisions,
  type AiGridResultBackgroundPolicy,
  type AiGridWebService,
} from "@/features/ai-grid/ai-grid-workspace-model";
import type {
  AiGridWorkspace,
  FinalizeGeneratedIconInput,
} from "@/features/ai-grid/types";
import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import { openAiOfficialResource } from "@/features/editor/api";
import { SheetGridOverlay } from "@/features/sheets/components/SheetGridOverlay";
import { SheetGridSettingsPanel } from "@/features/sheets/components/SheetGridSettingsPanel";
import type {
  SheetGridAnalysis,
  SheetGridSettings,
} from "@/features/sheets/types";
import { CommandError, getCommandErrorMessage } from "@/lib/tauri";
import { useModalFocus } from "@/lib/use-modal-focus";

export type AiGridWorkspaceMode = "edit" | "generate";
interface FinalizedDraft {
  displayName: string;
  altText: string;
}

export function AiGridWorkspaceDialog({
  collection,
  icons,
  mode,
  selectedIconIds,
  onClose,
  onCompleted,
}: {
  collection: CollectionSummary;
  icons: IconSummary[];
  mode: AiGridWorkspaceMode;
  selectedIconIds: string[];
  onClose: () => void;
  onCompleted: () => Promise<void>;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const promptCopyGenerationRef = useRef(0);
  useModalFocus(dialogRef, onClose);

  const selectedIcons = useMemo(() => {
    const selected = new Set(selectedIconIds);
    return icons.filter((icon) => selected.has(icon.id));
  }, [icons, selectedIconIds]);
  const eligibleReferenceIcons = useMemo(
    () =>
      icons.filter(
        (icon) =>
          icon.iconKind === "image" &&
          icon.shape === "single" &&
          Boolean(
            icon.currentPreviewUrl ??
              icon.thumbnailOverrideUrl ??
              icon.thumbnailUrl,
          ),
      ),
    [icons],
  );
  const [step, setStep] = useState(1);
  const [workspace, setWorkspace] = useState<AiGridWorkspace | null>(null);
  const [targetCount, setTargetCount] = useState(
    mode === "edit" ? selectedIcons.length : 1,
  );
  const [targetNames, setTargetNames] = useState<string[]>(() =>
    mode === "edit"
      ? selectedIcons.map((icon) => icon.displayName)
      : ["새 이모티콘 1"],
  );
  const [userPrompt, setUserPrompt] = useState("");
  const [referenceIconIds, setReferenceIconIds] = useState<string[]>([]);
  const [referenceFiles, setReferenceFiles] = useState<File[]>([]);
  const [service, setService] = useState<AiGridWebService>("gemini_web");
  const [resultBackgroundPolicy, setResultBackgroundPolicy] =
    useState<AiGridResultBackgroundPolicy>("preserve_transparency");
  const [pendingOpaqueResultFile, setPendingOpaqueResultFile] =
    useState<File | null>(null);
  const [backgroundReviewConfirmed, setBackgroundReviewConfirmed] =
    useState(false);
  const [analysis, setAnalysis] = useState<SheetGridAnalysis | null>(null);
  const [reviewSettings, setReviewSettings] =
    useState<SheetGridSettings | null>(null);
  const [mapping, setMapping] = useState<Map<number, number>>(new Map());
  const [includedItemIndexes, setIncludedItemIndexes] = useState<Set<number>>(
    new Set(),
  );
  const [finalizedDrafts, setFinalizedDrafts] = useState<
    Record<number, FinalizedDraft>
  >({});
  const [isLoading, setIsLoading] = useState(true);
  const [isWorking, setIsWorking] = useState(false);
  const [isDraggingResult, setIsDraggingResult] = useState(false);
  const [isRestoredWorkspace, setIsRestoredWorkspace] = useState(false);
  const [restoredDeliveryConfirmed, setRestoredDeliveryConfirmed] =
    useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isMissingAlphaResult, setIsMissingAlphaResult] = useState(false);
  const [promptCopyOutcome, setPromptCopyOutcome] =
    useState<NovelAiPromptCopyOutcome>("idle");
  const [promptCopyRevision, setPromptCopyRevision] = useState(0);

  const initializeFinalizedDrafts = useCallback(
    (nextWorkspace: AiGridWorkspace) => {
      setFinalizedDrafts(
        Object.fromEntries(
          nextWorkspace.items
            .filter(
              (item) =>
                item.outputCandidateId && item.reviewStatus !== "excluded",
            )
            .map((item) => [
              item.itemIndex,
              { displayName: item.targetNameSnapshot, altText: "" },
            ]),
        ),
      );
    },
    [],
  );

  const loadReview = useCallback(async (nextWorkspace: AiGridWorkspace) => {
    if (!nextWorkspace.outputArtifact) return;
    const settings = sheetSettingsFromLayout(nextWorkspace.layout);
    setReviewSettings(settings);
    setAnalysis(null);
    setIncludedItemIndexes(
      new Set(nextWorkspace.items.map((item) => item.itemIndex)),
    );
    const nextAnalysis = await analyzeAiGridOutput(
      nextWorkspace.requestId,
      settings,
    );
    setAnalysis(nextAnalysis);
    setMapping(defaultResultMapping(nextWorkspace, nextAnalysis.cells));
  }, []);

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    void getLatestAiGridWorkspace(collection.id)
      .then(async (existing) => {
        if (cancelled || !existing) return;
        setWorkspace(existing);
        setStep(aiGridStepForStatus(existing));
        setTargetCount(existing.itemCount);
        setUserPrompt("");
        setService("gemini_web");
        setResultBackgroundPolicy(
          existing.outputArtifact && !existing.outputArtifact.hasAlpha
            ? "allow_opaque"
            : "preserve_transparency",
        );
        setPendingOpaqueResultFile(null);
        setBackgroundReviewConfirmed(false);
        setIsRestoredWorkspace(true);
        setRestoredDeliveryConfirmed(false);
        setTargetNames(
          existing.items.map((item) => item.targetNameSnapshot),
        );
        if (existing.outputArtifact && existing.candidateCount === 0) {
          await loadReview(existing);
        }
        if (existing.candidateCount > 0) {
          initializeFinalizedDrafts(existing);
        }
        if (!cancelled) {
          setMessage(
            "앱을 다시 연 뒤에도 이어갈 수 있도록 진행 중인 AI 그리드 작업을 복원했습니다.",
          );
        }
      })
      .catch((error) => {
        if (!cancelled) setErrorMessage(getCommandErrorMessage(error));
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [collection.id, initializeFinalizedDrafts, loadReview]);

  const effectiveMode: AiGridWorkspaceMode = workspace
    ? workspace.requestScope === "grid_edit"
      ? "edit"
      : "generate"
    : mode;
  const finalPrompt = workspace
    ? buildAiGridPrompt(workspace, userPrompt, service, resultBackgroundPolicy)
    : "";
  const hasBlankTargetName =
    effectiveMode === "generate" &&
    targetNames.some((name) => !name.trim());
  const referenceCount = referenceIconIds.length + referenceFiles.length;
  const cells = analysis?.cells ?? [];
  const mappedCellIndexes = useMemo(
    () =>
      new Set(
        workspace
          ? workspace.items
              .filter(
                (item) =>
                  effectiveMode === "edit" ||
                  includedItemIndexes.has(item.itemIndex),
              )
              .map((item) => mapping.get(item.itemIndex))
              .filter(
                (value): value is number => typeof value === "number",
              )
          : [],
      ),
    [effectiveMode, includedItemIndexes, mapping, workspace],
  );
  const decisions = useMemo(
    () =>
      workspace
        ? reviewDecisions(
            workspace,
            cells,
            mapping,
            includedItemIndexes,
          )
        : [],
    [cells, includedItemIndexes, mapping, workspace],
  );
  const reviewError =
    workspace && analysis
      ? validateReviewDecisions(workspace, decisions, analysis)
      : "결과 시트를 먼저 분석해 주세요.";
  const needsRestoredDeliveryConfirmation =
    isRestoredWorkspace &&
    (!userPrompt.trim() || !restoredDeliveryConfirmed);
  const isCancellableWorkspace =
    workspace !== null &&
    workspace.candidateCount === 0 &&
    [
      "draft",
      "prepared",
      "awaiting_result",
      "running",
      "layout_review_pending",
    ].includes(workspace.status);
  const correctionPrompt =
    workspace && analysis
      ? buildAiGridCorrectionPrompt(
          workspace,
          analysis,
          resultBackgroundPolicy,
        )
      : null;
  const missingAlphaCorrectionPrompt =
    buildAiGridMissingAlphaCorrectionPrompt();
  const generationItems =
    workspace?.items.filter(
      (item) =>
        item.outputCandidateId && item.reviewStatus !== "excluded",
    ) ?? [];
  const needsGenerationBackgroundConfirmation = Boolean(
    workspace &&
      workspace.requestScope !== "grid_edit" &&
      (workspace.outputArtifact || workspace.candidateCount > 0) &&
      !backgroundReviewConfirmed,
  );

  const updateTargetCount = (count: number) => {
    const bounded = Math.min(16, Math.max(1, Math.round(count)));
    setTargetCount(bounded);
    setTargetNames((current) =>
      Array.from(
        { length: bounded },
        (_, index) => current[index] ?? `새 이모티콘 ${index + 1}`,
      ),
    );
  };

  const prepareWorkspace = async () => {
    if (isWorking) return;
    if (hasBlankTargetName) {
      setErrorMessage("각 생성 아이콘의 이름을 입력해 주세요.");
      setStep(1);
      return;
    }
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    setIsMissingAlphaResult(false);
    try {
      const prepared =
        effectiveMode === "edit"
          ? await prepareAiGridEditWorkspace(
              collection.id,
              selectedIconIds,
            )
          : await prepareAiGenerationWorkspace(
              collection.id,
              targetNames,
              userPrompt.trim() ||
                `source-free-${targetCount}-${targetNames.join("|")}`,
              null,
              referenceIconIds,
              referenceFiles,
            );
      setWorkspace(prepared);
      setIsRestoredWorkspace(false);
      setRestoredDeliveryConfirmed(false);
      setTargetCount(prepared.itemCount);
      setTargetNames(
        prepared.items.map((item) => item.targetNameSnapshot),
      );
      setStep(3);
      setMessage(
        effectiveMode === "edit"
          ? "선택한 아이콘을 한 장의 투명 그리드로 준비했습니다. 원본과 현재 적용 이미지는 바뀌지 않았습니다."
          : referenceCount > 0
            ? `${referenceCount}개 참고 이미지를 한 장의 안전한 참고 시트로 준비했습니다. 참고 원본은 바뀌지 않았습니다.`
            : "가짜 빈 아이콘 없이 생성 항목과 그리드 구조만 준비했습니다.",
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const resetPromptCopySequence = () => {
    promptCopyGenerationRef.current += 1;
    setPromptCopyOutcome("idle");
    setPromptCopyRevision((revision) => revision + 1);
  };

  const publishPromptCopy = (
    outcome: NovelAiPromptCopyOutcome,
    copyGeneration: number,
  ) => {
    if (copyGeneration !== promptCopyGenerationRef.current) return false;
    setPromptCopyOutcome(outcome);
    setPromptCopyRevision((revision) => revision + 1);
    return true;
  };

  const copyPrompt = async () => {
    if (!finalPrompt) return false;
    if (needsRestoredDeliveryConfirmation) {
      setErrorMessage(
        "복원된 작업은 요청 내용과 웹 서비스를 다시 확인한 뒤 프롬프트를 복사할 수 있습니다.",
      );
      return false;
    }
    setMessage(null);
    setErrorMessage(null);
    const copyGeneration = ++promptCopyGenerationRef.current;
    try {
      await navigator.clipboard.writeText(finalPrompt);
      if (!publishPromptCopy("copied", copyGeneration)) return false;
      setMessage(
        service === "novelai_web"
          ? "NovelAI Prompt용 태그와 짧은 구조 문장을 복사했습니다."
          : "그리드 구조가 포함된 프롬프트를 복사했습니다.",
      );
      return true;
    } catch {
      if (copyGeneration !== promptCopyGenerationRef.current) return false;
      const textarea = promptRef.current;
      if (textarea) {
        textarea.focus();
        textarea.select();
        if (document.execCommand("copy")) {
          if (!publishPromptCopy("copied", copyGeneration)) return false;
          setMessage("프롬프트를 선택 영역에서 복사했습니다.");
          return true;
        }
      }
      if (!publishPromptCopy("failed", copyGeneration)) return false;
      setErrorMessage(
        "프롬프트 자동 복사에 실패했습니다. 아래 내용을 직접 복사해 주세요.",
      );
      return false;
    }
  };

  const copyCorrectionPrompt = async () => {
    if (!correctionPrompt) return;
    try {
      await navigator.clipboard.writeText(correctionPrompt);
      setMessage("현재 구조 오류에 맞춘 추가 프롬프트를 복사했습니다.");
    } catch {
      setErrorMessage(
        "교정 프롬프트 자동 복사에 실패했습니다. 아래 내용을 직접 선택해 복사해 주세요.",
      );
    }
  };

  const copyMissingAlphaCorrectionPrompt = async () => {
    try {
      await navigator.clipboard.writeText(missingAlphaCorrectionPrompt);
      setMessage("실제 투명 PNG를 다시 요청할 수정 프롬프트를 복사했습니다.");
      setErrorMessage(null);
    } catch {
      setErrorMessage(
        "투명 배경 수정 프롬프트를 자동 복사하지 못했습니다. 아래 내용을 직접 선택해 복사해 주세요.",
      );
    }
  };

  const beginWebDelivery = async () => {
    if (!workspace || isWorking) return;
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      let current = workspace;
      if (workspace.status === "prepared") {
        current = await markAiGridWorkspaceAwaitingResult(
          workspace.requestId,
        );
        setWorkspace(current);
      }
      if (!(await copyPrompt())) return;
      await openAiOfficialResource(
        service === "gemini_web"
          ? "gemini_ai_studio"
          : "novelai_app",
      );
      setMessage(
        current.inputArtifact
          ? "공식 웹사이트를 열었습니다. 아래 파일을 끌어 놓고 복사된 프롬프트를 붙여넣으세요."
          : "공식 웹사이트를 열었습니다. 복사된 생성 프롬프트를 붙여넣으세요.",
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const openSelectedWebSite = async () => {
    if (!workspace || isWorking) return;
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      if (workspace.status === "prepared") {
        const awaiting = await markAiGridWorkspaceAwaitingResult(
          workspace.requestId,
        );
        setWorkspace(awaiting);
      }
      await openAiOfficialResource(
        service === "gemini_web" ? "gemini_ai_studio" : "novelai_app",
      );
      setMessage(
        "공식 웹사이트만 열었습니다. 아래 프롬프트를 직접 복사하고, NovelAI라면 Undesired Content도 별도로 복사하세요.",
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const runInputAction = async (action: "drag" | "reveal") => {
    if (!workspace || isWorking) return;
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      if (action === "drag") {
        const result = await startAiGridInputDrag(workspace.requestId);
        setMessage(result.message);
      } else {
        await revealAiGridInput(workspace.requestId);
        setMessage("탐색기에서 그리드 입력 파일을 선택했습니다.");
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const acceptResultFile = async (file: File | null, allowOpaqueOverride?: boolean) => {
    if (!workspace || !file || isWorking) return;
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    setIsMissingAlphaResult(false);
    setPendingOpaqueResultFile(null);
    try {
      let attached: AiGridWorkspace;
      try {
        attached = await attachAiGridOutput(
          workspace.requestId,
          file,
          allowOpaqueOverride ?? resultBackgroundPolicy === "allow_opaque",
        );
      } catch (error) {
        if (
          error instanceof CommandError &&
          error.code === "ai_grid_output_alpha_required"
        ) {
          setIsMissingAlphaResult(true);
          setPendingOpaqueResultFile(file);
          setErrorMessage(null);
        } else {
          setErrorMessage(getCommandErrorMessage(error));
        }
        return;
      }
      setPendingOpaqueResultFile(null);
      setWorkspace(attached);
      setBackgroundReviewConfirmed(false);
      setStep(4);
      try {
        await loadReview(attached);
        setMessage(
          "결과 시트를 보관하고 셀 검토를 준비했습니다. 아직 후보나 새 아이콘은 만들지 않았습니다.",
        );
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
      }
    } finally {
      setIsWorking(false);
    }
  };

  const acceptResultFiles = (files: Iterable<File> | ArrayLike<File>) => {
    if (!workspace || isWorking) return;
    const selected = selectAiGridResultFile(files, workspace.requestScope);
    setMessage(null);
    setErrorMessage(selected.error);
    if (!selected.file) return;
    void acceptResultFile(selected.file);
  };

  const continueWithOpaqueBackground = () => {
    const file = pendingOpaqueResultFile;
    if (!file || isWorking) return;
    setResultBackgroundPolicy("allow_opaque");
    resetPromptCopySequence();
    void acceptResultFile(file, true);
  };

  const refreshAnalysis = async () => {
    if (!workspace || isWorking) return;
    const settings =
      reviewSettings ?? sheetSettingsFromLayout(workspace.layout);
    setReviewSettings(settings);
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      const nextAnalysis = await analyzeAiGridOutput(
        workspace.requestId,
        settings,
      );
      setAnalysis(nextAnalysis);
      setMapping(defaultResultMapping(workspace, nextAnalysis.cells));
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const saveReview = async () => {
    if (needsGenerationBackgroundConfirmation) {
      setErrorMessage(
        "후보를 저장하기 전에 배경 상태와 가짜 체커무늬 여부를 직접 확인해 주세요.",
      );
      return;
    }
    if (!workspace || reviewError || isWorking) return;
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      const result = await commitAiGridReview(
        workspace.requestId,
        decisions,
      );
      setWorkspace(result.workspace);
      if (result.workspace.requestScope === "grid_edit") {
        setMessage(
          `${result.commit.candidateIds.length}개 결과를 한 번에 비활성 후보로 저장했습니다. 원본과 현재 적용 이미지는 그대로입니다.`,
        );
      } else {
        initializeFinalizedDrafts(result.workspace);
        setMessage(
          `${result.commit.candidateIds.length}개 셀을 확정했습니다. 이름을 확인한 뒤 새 아이콘을 한 번에 만드세요.`,
        );
      }
      setStep(5);
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const createGeneratedIcons = async () => {
    if (!workspace || isWorking) return;
    if (needsGenerationBackgroundConfirmation) {
      setErrorMessage(
        "새 아이콘을 만들기 전에 후보 배경 상태와 가짜 체커무늬 여부를 다시 확인해 주세요.",
      );
      return;
    }
    const finalizedItems: FinalizeGeneratedIconInput[] =
      generationItems.map((item) => ({
        itemIndex: item.itemIndex,
        displayName:
          finalizedDrafts[item.itemIndex]?.displayName.trim() ||
          item.targetNameSnapshot,
        altText:
          finalizedDrafts[item.itemIndex]?.altText.trim() || "",
      }));
    if (finalizedItems.length === 0) {
      setErrorMessage("새 아이콘으로 만들 결과 셀이 없습니다.");
      return;
    }
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      const result = await commitAiGeneratedIcons(
        collection.id,
        workspace.requestId,
        finalizedItems,
      );
      setWorkspace(result.workspace);
      await onCompleted();
      setMessage(
        `${result.commit.createdIcons.length}개 새 아이콘을 원자적으로 만들었습니다. 원본 없는 생성 계보도 함께 저장했습니다.`,
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const resetForNewWorkspace = () => {
    const nextNames =
      mode === "edit"
        ? selectedIcons.map((icon) => icon.displayName)
        : ["새 이모티콘 1"];
    setWorkspace(null);
    setStep(1);
    setTargetCount(mode === "edit" ? selectedIcons.length : 1);
    setTargetNames(nextNames);
    setUserPrompt("");
    setReferenceIconIds([]);
    setReferenceFiles([]);
    setService("gemini_web");
    setResultBackgroundPolicy("preserve_transparency");
    setPendingOpaqueResultFile(null);
    setBackgroundReviewConfirmed(false);
    resetPromptCopySequence();
    setAnalysis(null);
    setReviewSettings(null);
    setMapping(new Map());
    setIncludedItemIndexes(new Set());
    setFinalizedDrafts({});
    setIsRestoredWorkspace(false);
    setRestoredDeliveryConfirmed(false);
    setIsMissingAlphaResult(false);
    setErrorMessage(null);
    setMessage("기존 요청을 취소했습니다. 새 작업을 준비할 수 있습니다.");
  };

  const cancelWorkspace = async (startNew = false) => {
    if (!workspace || isWorking) return;
    if (
      !window.confirm(
        startNew
          ? "진행 중인 AI 그리드 요청을 취소하고 새 작업을 시작할까요? 원본 아이콘과 현재 이미지는 바뀌지 않습니다."
          : "이 AI 그리드 요청을 취소할까요? 원본 아이콘과 현재 이미지는 바뀌지 않습니다.",
      )
    ) {
      return;
    }
    setIsWorking(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      await cancelAiGridWorkspace(workspace.requestId);
      if (startNew) {
        resetForNewWorkspace();
      } else {
        onClose();
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsWorking(false);
    }
  };

  const toggleReferenceIcon = (iconId: string) => {
    setErrorMessage(null);
    setReferenceIconIds((current) => {
      if (current.includes(iconId)) {
        return current.filter((id) => id !== iconId);
      }
      if (current.length + referenceFiles.length >= 16) {
        setErrorMessage("참고 이미지는 내부 아이콘과 외부 파일을 합쳐 최대 16개까지 선택할 수 있습니다.");
        return current;
      }
      return [...current, iconId];
    });
  };

  const addReferenceFiles = (files: File[]) => {
    setErrorMessage(null);
    const supported = files.filter(isSupportedReferenceFile);
    if (supported.length !== files.length) {
      setErrorMessage("외부 참고 이미지는 PNG, JPG 또는 GIF 파일만 선택할 수 있습니다.");
      return;
    }
    const nextFiles = [...referenceFiles, ...supported];
    if (referenceIconIds.length + nextFiles.length > 16) {
      setErrorMessage("참고 이미지는 내부 아이콘과 외부 파일을 합쳐 최대 16개까지 선택할 수 있습니다.");
      return;
    }
    const totalExternalBytes = nextFiles.reduce(
      (total, file) => total + file.size,
      0,
    );
    if (totalExternalBytes > MAX_AI_REFERENCE_EXTERNAL_BYTES) {
      setErrorMessage("외부 참고 이미지는 합계 16MB까지 사용할 수 있습니다.");
      return;
    }
    setReferenceFiles(nextFiles);
  };

  const handleResultDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setIsDraggingResult(false);
    acceptResultFiles(event.dataTransfer.files);
  };

  if (isLoading) {
    return (
      <ModalFrame dialogRef={dialogRef} onClose={onClose}>
        <div className="flex min-h-72 items-center justify-center text-sm text-muted">
          <h2 className="sr-only" id="ai-grid-workspace-title">
            AI 그리드 작업공간 불러오기
          </h2>
          AI 그리드 작업을 확인하는 중입니다.
        </div>
      </ModalFrame>
    );
  }

  return (
    <ModalFrame dialogRef={dialogRef} onClose={onClose}>
      <header className="flex flex-wrap items-start justify-between gap-4 border-b border-border px-5 py-4">
        <div>
          <h2
            className="text-lg font-semibold"
            id="ai-grid-workspace-title"
          >
            {effectiveMode === "edit"
              ? "선택 아이콘 AI 일괄 수정"
              : "AI 아이콘 만들기"}
          </h2>
          <p className="mt-1 text-sm text-muted">
            2–16개 편집은 전부 함께 검토·저장하며, 생성은 원본 없는
            단일/그리드를 지원합니다.
          </p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {isCancellableWorkspace ? (
            <button
              className="rounded-md border border-danger/30 bg-white px-3 py-2 text-xs font-semibold text-danger hover:bg-danger/5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
              disabled={isWorking}
              type="button"
              onClick={() => void cancelWorkspace(true)}
            >
              요청 취소 후 새 작업
            </button>
          ) : null}
          <button
            aria-label="AI 그리드 작업공간 닫기"
            className="rounded-md p-2 text-muted hover:bg-menu-hover hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </div>
      </header>

      <ol
        aria-label="AI 그리드 진행 단계"
        className="grid grid-cols-5 border-b border-border bg-canvas px-4 py-3 text-center text-[11px] sm:text-xs"
      >
        {["작업", "배치", "웹 전달", "셀 검토", "저장"].map(
          (label, index) => (
            <li
              className={
                step === index + 1
                  ? "font-semibold text-accent"
                  : step > index + 1
                    ? "text-success"
                    : "text-muted"
              }
              key={label}
            >
              <span className="mr-1">{index + 1}</span>
              {label}
            </li>
          ),
        )}
      </ol>

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        {message ? (
          <p
            className="mb-4 rounded-md border border-success/30 bg-success/5 p-3 text-sm text-success"
            role="status"
          >
            {message}
          </p>
        ) : null}
        {errorMessage ? (
          <p
            className="mb-4 rounded-md border border-danger/30 bg-danger/5 p-3 text-sm text-danger"
            role="alert"
          >
            {errorMessage}
          </p>
        ) : null}
        {isRestoredWorkspace && step === 3 ? (
          <div
            className="mb-4 rounded-md border border-warning/40 bg-warning/5 p-3 text-sm leading-6"
            data-testid="ai-grid-restored-delivery-notice"
            role="status"
          >
            <p className="font-semibold">전달 정보 재확인이 필요합니다.</p>
            <p className="mt-1 text-muted">
              보안을 위해 사용자 프롬프트 원문과 웹 서비스 선택은 저장하지
              않습니다. Gemini 웹으로 초기화했으므로 다시 전달하려면 요청
              내용을 재입력하고 서비스를 확인해 주세요. 이미 받은 결과는
              아래에 바로 놓을 수 있습니다.
            </p>
          </div>
        ) : null}

        {step === 1 ? (
          <section
            className="grid gap-5"
            data-testid="ai-grid-step-targets"
          >
            <div>
              <h3 className="text-base font-semibold">
                1. 작업과 대상 확인
              </h3>
              <p className="mt-1 text-sm text-muted">
                {effectiveMode === "edit"
                  ? "현재 선택 순서를 그대로 그리드 순서로 사용합니다. GIF·다중 조각은 이 흐름에서 제외됩니다."
                  : "가짜 빈 아이콘을 만들지 않고 결과를 검토한 뒤 새 아이콘으로 저장합니다."}
              </p>
            </div>
            {effectiveMode === "generate" ? (
              <label className="grid max-w-xs gap-1 text-sm font-medium">
                만들 아이콘 수
                <input
                  className="rounded-md border border-border bg-white px-3 py-2"
                  max={16}
                  min={1}
                  type="number"
                  value={targetCount}
                  onChange={(event) =>
                    updateTargetCount(Number(event.currentTarget.value))
                  }
                />
              </label>
            ) : null}
            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
              {targetNames.map((name, index) => (
                <label
                  className="grid gap-1 text-xs font-medium text-muted"
                  key={index}
                >
                  {index + 1}번 이름
                  <input
                    className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
                    disabled={effectiveMode === "edit"}
                    maxLength={255}
                    value={name}
                    onChange={(event) =>
                      setTargetNames((current) =>
                        current.map((value, itemIndex) =>
                          itemIndex === index
                            ? event.currentTarget.value
                            : value,
                        ),
                      )
                    }
                  />
                </label>
              ))}
            </div>
            {hasBlankTargetName ? (
              <p className="text-sm text-danger" role="alert">
                각 생성 아이콘의 이름을 입력해 주세요.
              </p>
            ) : null}
            {effectiveMode === "generate" ? (
              <div
                className="grid gap-3 rounded-md border border-border bg-canvas p-4"
                data-testid="ai-generation-references"
              >
                <div>
                  <h4 className="text-sm font-semibold">
                    참고 이미지 (선택) · {referenceCount}/16
                  </h4>
                  <p className="mt-1 text-xs leading-5 text-muted">
                    모음의 아이콘 또는 외부 PNG/JPG/GIF를 캐릭터·그림체 참고로 사용합니다. GIF는 첫 프레임 포스터가 들어가며, 결과 배치 틀로 사용하지 않습니다.
                  </p>
                </div>
                {eligibleReferenceIcons.length > 0 ? (
                  <div className="grid max-h-56 grid-cols-3 gap-2 overflow-y-auto sm:grid-cols-5 lg:grid-cols-8">
                    {eligibleReferenceIcons.map((icon) => {
                      const checked = referenceIconIds.includes(icon.id);
                      const preview =
                        icon.currentPreviewUrl ??
                        icon.thumbnailOverrideUrl ??
                        icon.thumbnailUrl ??
                        "";
                      return (
                        <label
                          className={
                            checked
                              ? "grid cursor-pointer gap-1 rounded-md border border-accent bg-selected p-2"
                              : "grid cursor-pointer gap-1 rounded-md border border-border bg-white p-2 hover:bg-menu-hover"
                          }
                          key={icon.id}
                        >
                          <input
                            checked={checked}
                            className="sr-only"
                            type="checkbox"
                            onChange={() => toggleReferenceIcon(icon.id)}
                          />
                          <img
                            alt=""
                            className="aspect-square w-full rounded object-contain"
                            src={preview}
                          />
                          <span className="truncate text-[11px]" title={icon.displayName}>
                            {icon.displayName}
                          </span>
                        </label>
                      );
                    })}
                  </div>
                ) : (
                  <p className="text-xs text-muted">참고로 선택할 수 있는 단일 이미지 아이콘이 없습니다.</p>
                )}
                <div className="flex flex-wrap items-center gap-2">
                  <label className="inline-flex cursor-pointer items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover">
                    <ImagePlus aria-hidden="true" />
                    외부 참고 이미지 추가
                    <input
                      accept="image/png,image/jpeg,image/gif,.png,.jpg,.jpeg,.gif"
                      className="sr-only"
                      multiple
                      type="file"
                      onChange={(event) => {
                        const files = Array.from(event.currentTarget.files ?? []);
                        event.currentTarget.value = "";
                        addReferenceFiles(files);
                      }}
                    />
                  </label>
                  <span className="text-[11px] text-muted">외부 파일 합계 최대 16MB</span>
                </div>
                {referenceFiles.length > 0 ? (
                  <ul className="grid gap-1 text-xs">
                    {referenceFiles.map((file, index) => (
                      <li
                        className="flex items-center justify-between gap-2 rounded border border-border bg-white px-2 py-1"
                        key={`${file.name}-${file.size}-${file.lastModified}-${index}`}
                      >
                        <span className="min-w-0 truncate">{file.name}</span>
                        <button
                          aria-label={`${file.name} 참고 파일 제거`}
                          className="rounded p-1 text-muted hover:bg-menu-hover hover:text-foreground"
                          type="button"
                          onClick={() =>
                            setReferenceFiles((current) =>
                              current.filter((_, fileIndex) => fileIndex !== index),
                            )
                          }
                        >
                          <X aria-hidden="true" />
                        </button>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </div>
            ) : null}
            <label className="grid gap-1 text-sm font-medium">
              원하는 수정·생성 내용
              <textarea
                className="min-h-24 rounded-md border border-border bg-white px-3 py-2 text-sm"
                disabled={isWorking}
                maxLength={2000}
                placeholder="예: 같은 캐릭터를 픽셀 아트로, 표정은 각 셀마다 다르게"
                value={userPrompt}
                onChange={(event) => {
                  setUserPrompt(event.currentTarget.value);
                  resetPromptCopySequence();
                }}
              />
              <span className="text-xs font-normal text-muted">
                그리드 구조·투명 배경·셀 순서는 기본 프롬프트에 자동으로
                들어갑니다.
              </span>
            </label>
            <div className="flex justify-end">
              <button
                className="rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-50"
                disabled={hasBlankTargetName}
                title={hasBlankTargetName ? "각 생성 아이콘의 이름을 입력해 주세요." : undefined}
                type="button"
                onClick={() => setStep(2)}
              >
                배치 확인
              </button>
            </div>
          </section>
        ) : null}

        {step === 2 ? (
          <section
            className="grid gap-5"
            data-testid="ai-grid-step-layout"
          >
            <div>
              <h3 className="text-base font-semibold">
                2. 한 장 그리드 배치
              </h3>
              <p className="mt-1 text-sm text-muted">
                항목 수에 맞춰 최대 4×4, 1024×1024 한 페이지 배치를
                계산합니다.
              </p>
            </div>
            <div className="grid gap-3 sm:grid-cols-3">
              <Metric label="항목" value={`${targetCount}개`} />
              <Metric
                label="예상 배치"
                value={defaultLayoutLabel(targetCount)}
              />
              {effectiveMode === "generate" ? (
                <Metric label="참고" value={referenceCount > 0 ? `${referenceCount}개` : "없음"} />
              ) : null}
              <Metric
                label="저장 정책"
                value={
                  effectiveMode === "edit"
                    ? "전부 또는 없음"
                    : "포함 셀 전부 또는 없음"
                }
              />
            </div>
            <div className="rounded-md border border-focus/20 bg-selected/30 p-4 text-sm leading-6">
              {effectiveMode === "edit"
                ? "준비 단계는 현재 화면을 새 PNG 입력 시트로 렌더링할 뿐입니다. 원본, crop, 활성 AI 버전은 변경하지 않습니다."
                : referenceCount > 0
                  ? "참고 이미지는 한 장의 별도 시트로 복사해 전달합니다. 원본 아이콘·외부 파일은 바꾸지 않으며, 결과 셀을 확정하기 전에는 새 아이콘을 만들지 않습니다."
                  : "생성 단계는 요청 항목만 저장합니다. 결과 셀을 확정하기 전에는 아이콘이나 원본 파일 행을 만들지 않습니다."}
            </div>
            <div className="flex flex-wrap justify-between gap-2">
              <button
                className="rounded-md border border-border bg-white px-4 py-2 text-sm font-medium hover:bg-menu-hover"
                type="button"
                onClick={() => setStep(1)}
              >
                이전
              </button>
              <button
                className="rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-50"
                disabled={isWorking}
                type="button"
                onClick={() => void prepareWorkspace()}
              >
                {isWorking ? "준비하는 중" : "작업공간 준비"}
              </button>
            </div>
          </section>
        ) : null}

        {step === 3 && workspace ? (
          <section
            className="grid gap-5"
            data-testid="ai-grid-step-delivery"
          >
            <div>
              <h3 className="text-base font-semibold">
                3. 브라우저로 전달하고 결과 받기
              </h3>
              <p className="mt-1 text-sm text-muted">
                앱은 로그인·업로드 성공·생성 완료를 감시하지 않습니다.
                파일과 프롬프트만 빠르게 준비합니다.
              </p>
            </div>
            <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
              <div className="grid gap-3">
                {workspace.inputArtifact ? (
                  <div className="overflow-hidden rounded-md border border-border bg-checkerboard p-3">
                    <img
                      alt={workspace.requestScope === "grid_edit" ? "선택 아이콘 AI 입력 그리드" : "AI 생성 참고 이미지 시트"}
                      className="mx-auto max-h-80 max-w-full object-contain"
                      src={workspace.inputArtifact.previewUrl}
                    />
                  </div>
                ) : (
                  <div className="flex min-h-40 items-center justify-center rounded-md border border-dashed border-border bg-canvas p-5 text-center text-sm text-muted">
                    참고 이미지를 선택하지 않은 원본 없는 생성입니다. 프롬프트만 웹에 붙여넣으세요.
                  </div>
                )}
                <label className="grid gap-1 text-sm font-medium">
                  {service === "novelai_web"
                    ? "NovelAI Prompt (태그 + 짧은 구조 문장)"
                    : "자동 구성 프롬프트"}
                  <textarea
                    ref={promptRef}
                    className="min-h-48 rounded-md border border-border bg-white p-3 text-xs leading-5"
                    readOnly
                    value={finalPrompt}
                  />
                </label>
              </div>
              <aside className="grid content-start gap-3 rounded-md border border-border bg-white p-4">
                {isRestoredWorkspace ? (
                  <label className="grid gap-1 text-xs font-medium text-muted">
                    복원 후 요청 내용 다시 입력
                    <textarea
                      className="min-h-24 rounded-md border border-warning/50 bg-white px-3 py-2 text-sm text-foreground"
                      disabled={isWorking}
                      maxLength={2000}
                      placeholder="원래 웹에 전달할 수정·생성 요청을 다시 입력하세요."
                      value={userPrompt}
                      onChange={(event) => {
                        setUserPrompt(event.currentTarget.value);
                        setRestoredDeliveryConfirmed(false);
                        resetPromptCopySequence();
                      }}
                    />
                  </label>
                ) : null}
                <label className="grid gap-1 text-xs font-medium text-muted">
                  웹 서비스{isRestoredWorkspace ? " (다시 확인)" : ""}
                  <select
                    className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
                    data-testid="ai-grid-web-service"
                    disabled={isWorking}
                    value={service}
                    onChange={(event) => {
                      setService(
                        event.currentTarget.value as AiGridWebService,
                      );
                      setRestoredDeliveryConfirmed(false);
                      setMessage(null);
                      setErrorMessage(null);
                      resetPromptCopySequence();
                    }}
                  >
                    <option value="gemini_web">Gemini 웹</option>
                    <option value="novelai_web">NovelAI 웹</option>
                  </select>
                </label>
                {isRestoredWorkspace ? (
                  <label className="flex items-start gap-2 rounded-md border border-border bg-canvas p-3 text-xs leading-5">
                    <input
                      checked={restoredDeliveryConfirmed}
                      className="mt-0.5"
                      disabled={isWorking || !userPrompt.trim()}
                      type="checkbox"
                      onChange={(event) =>
                        setRestoredDeliveryConfirmed(event.currentTarget.checked)
                      }
                    />
                    프롬프트 원문을 다시 입력했고 사용할 웹 서비스를 확인했습니다.
                  </label>
                ) : null}
                <button
                  className="inline-flex items-center justify-center gap-2 rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-50"
                  disabled={isWorking || needsRestoredDeliveryConfirmation}
                  title={
                    needsRestoredDeliveryConfirmation
                      ? "요청 내용을 다시 입력하고 웹 서비스를 확인해 주세요."
                      : undefined
                  }
                  type="button"
                  onClick={() => void beginWebDelivery()}
                >
                  <ExternalLink aria-hidden="true" />
                  {service === "novelai_web" ? "1. Prompt 복사 + NovelAI 열기" : "프롬프트 복사 + 웹 열기"}
                </button>
                <button
                  className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:opacity-50"
                  disabled={isWorking || needsRestoredDeliveryConfirmation}
                  type="button"
                  onClick={() => void copyPrompt()}
                >
                  <Copy aria-hidden="true" />
                  {service === "novelai_web" ? "1. Prompt 다시 복사" : "프롬프트 다시 복사"}
                </button>
                <button
                  className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:opacity-50"
                  data-testid="ai-grid-open-site-only"
                  disabled={isWorking || needsRestoredDeliveryConfirmation}
                  type="button"
                  onClick={() => void openSelectedWebSite()}
                >
                  <ExternalLink aria-hidden="true" />
                  웹사이트만 열기
                </button>
                {workspace.inputArtifact &&
                workspace.status === "awaiting_result" ? (
                  <GridInputActions
                    disabled={isWorking || needsRestoredDeliveryConfirmation}
                    onAction={runInputAction}
                  />
                ) : null}
              </aside>
            </div>
            {service === "novelai_web" ? (
              <div className="grid gap-3">
                {needsNovelAiEnglishInputHint(userPrompt) ? (
                  <p
                    className="rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs leading-5 text-warning"
                    data-testid="ai-grid-novelai-language-hint"
                  >
                    NovelAI는 영문 소문자 태그를 쉼표로 나눈 입력을 권장합니다.
                    한국어 요청은 임의 번역하지 않으므로 짧은 영문 태그로 바꾸면
                    결과를 더 일관되게 제어할 수 있습니다.
                  </p>
                ) : null}
                <NovelAiWebGuide
                  disabled={isWorking || needsRestoredDeliveryConfirmation}
                  expectedCanvas={`${workspace.layout.canvasWidth}×${workspace.layout.canvasHeight}px`}
                  promptCopyOutcome={promptCopyOutcome}
                  promptCopyRevision={promptCopyRevision}
                  hasReference={
                    workspace.requestScope === "grid_generate" &&
                    Boolean(workspace.inputArtifact)
                  }
                  backgroundPolicy={resultBackgroundPolicy}
                  task={
                    workspace.requestScope === "grid_edit"
                      ? "grid_edit"
                      : "grid_generate"
                  }
                />
              </div>
            ) : null}
            {workspace.requestScope !== "grid_edit" ? (
              <fieldset
                className="grid gap-2 rounded-md border border-border bg-canvas p-3"
                data-testid="ai-grid-result-background-policy"
              >
                <legend className="px-1 text-sm font-semibold">결과 배경 처리</legend>
                <label className="flex items-start gap-2 rounded-md border border-border bg-white p-3 text-xs leading-5">
                  <input
                    checked={resultBackgroundPolicy === "preserve_transparency"}
                    className="mt-1"
                    disabled={isWorking}
                    name="ai-grid-result-background-policy"
                    type="radio"
                    onChange={() => {
                      setResultBackgroundPolicy("preserve_transparency");
                      setBackgroundReviewConfirmed(false);
                      resetPromptCopySequence();
                    }}
                  />
                  <span>
                    <strong className="block text-foreground">투명 배경 유지</strong>
                    실제 alpha가 있는 PNG/WebP만 사용합니다. 체커무늬나 불투명 JPG는
                    다시 생성하도록 안내합니다.
                  </span>
                </label>
                <label className="flex items-start gap-2 rounded-md border border-warning/40 bg-warning/5 p-3 text-xs leading-5">
                  <input
                    checked={resultBackgroundPolicy === "allow_opaque"}
                    className="mt-1"
                    disabled={isWorking}
                    name="ai-grid-result-background-policy"
                    type="radio"
                    onChange={() => {
                      setResultBackgroundPolicy("allow_opaque");
                      setBackgroundReviewConfirmed(false);
                      resetPromptCopySequence();
                    }}
                  />
                  <span>
                    <strong className="block text-foreground">배경 포함 결과 허용</strong>
                    JPG·불투명 PNG/WebP도 가져옵니다. 단색·체커무늬를 포함한 배경은
                    이모티콘에 그대로 남으며 자동 제거하지 않습니다.
                  </span>
                </label>
              </fieldset>
            ) : null}
            {isMissingAlphaResult ? (
              <section
                className="grid gap-3 rounded-md border border-danger/35 bg-danger/5 p-4"
                data-testid="ai-grid-missing-alpha-result"
                role="alert"
              >
                <div className="flex items-start gap-2">
                  <AlertTriangle
                    aria-hidden="true"
                    className="mt-0.5 size-4 shrink-0 text-danger"
                  />
                  <div>
                    <h4 className="text-sm font-semibold text-danger">
                      결과에 실제 투명 배경이 없습니다
                    </h4>
                    <p className="mt-1 text-xs leading-5 text-muted">
                      단색이나 체커무늬처럼 이미지에 그려진 배경은 이모티콘에
                      그대로 남습니다. 실제 투명 결과를 다시 요청하거나, 같은
                      파일을 배경 포함 상태로 가져와 나중에 직접 정리할 수 있습니다.
                    </p>
                  </div>
                </div>
                <label className="grid gap-1 text-xs font-semibold">
                  웹 AI에 추가할 투명 배경 수정 프롬프트
                  <textarea
                    className="min-h-32 rounded-md border border-border bg-white p-3 font-mono text-[11px] leading-5 text-foreground"
                    data-testid="ai-grid-missing-alpha-prompt"
                    readOnly
                    value={missingAlphaCorrectionPrompt}
                  />
                </label>
                <div className="flex flex-wrap gap-2">
                  {pendingOpaqueResultFile ? (
                    <button
                      className="inline-flex min-h-9 items-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-50"
                      data-testid="ai-grid-continue-opaque"
                      disabled={isWorking}
                      type="button"
                      onClick={continueWithOpaqueBackground}
                    >
                      배경 포함으로 이 파일 가져오기
                    </button>
                  ) : null}
                  <button
                    className="inline-flex min-h-9 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover"
                    data-testid="ai-grid-copy-missing-alpha-prompt"
                    type="button"
                    onClick={() => void copyMissingAlphaCorrectionPrompt()}
                  >
                    <Copy aria-hidden="true" />
                    투명 배경 수정 프롬프트 복사
                  </button>
                </div>
              </section>
            ) : null}
            <div
              className={
                isDraggingResult
                  ? "rounded-lg border-2 border-accent bg-selected p-6 text-center"
                  : "rounded-lg border-2 border-dashed border-border bg-canvas p-6 text-center"
              }
              data-testid="ai-grid-result-drop"
              onDragEnter={(event) => {
                event.preventDefault();
                setIsDraggingResult(true);
              }}
              onDragLeave={(event) => {
                if (
                  !event.currentTarget.contains(
                    event.relatedTarget as Node | null,
                  )
                ) {
                  setIsDraggingResult(false);
                }
              }}
              onDragOver={(event) => event.preventDefault()}
              onDrop={handleResultDrop}
            >
              <Upload aria-hidden="true" className="mx-auto" />
              <p className="mt-2 text-sm font-semibold">
                {workspace.requestScope === "grid_edit"
                  ? "Download Image로 받은 정적 PNG·JPG·WebP 한 장을 놓으세요."
                  : resultBackgroundPolicy === "allow_opaque"
                    ? "Download Image로 받은 PNG·JPG·WebP 한 장을 놓으세요."
                    : "Download Image로 받은 투명 PNG·WebP 한 장을 놓으세요."}
              </p>
              <p className="mt-1 text-xs text-muted">
                {workspace.requestScope === "grid_edit"
                  ? "최대 16MB · 2048×2048 · GIF 불가. WebP는 PNG로 안전하게 변환합니다."
                  : resultBackgroundPolicy === "allow_opaque"
                    ? "최대 16MB · 2048×2048 · GIF 불가. 불투명 배경은 그대로 유지됩니다."
                    : "최대 16MB · 2048×2048 · GIF 불가. 불투명 결과는 가져오기 전에 확인합니다."}{" "}
                잘못된 파일은 artifact나 후보를 남기지 않습니다.
              </p>
              <label className="mt-3 inline-flex cursor-pointer items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover">
                <ImagePlus aria-hidden="true" />
                결과 파일 선택
                <input
                  accept="image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp"
                  className="sr-only"
                  data-testid="ai-grid-result-file-input"
                  type="file"
                  onChange={(event) => {
                    acceptResultFiles(event.currentTarget.files ?? []);
                    event.currentTarget.value = "";
                  }}
                />
              </label>
            </div>
          </section>
        ) : null}

        {step === 4 && workspace && !analysis ? (
          <section
            className="grid gap-4"
            data-testid="ai-grid-step-review"
          >
            <div>
              <h3 className="text-base font-semibold">
                4. 전체 시트와 셀 매핑 검토
              </h3>
              <p className="mt-1 text-sm text-muted">
                결과 파일은 안전하게 보관됐지만 셀 분석을 완료하지 못했습니다.
              </p>
            </div>
            <div
              className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-warning/40 bg-warning/5 p-4"
              role="alert"
            >
              <p className="text-sm text-muted">
                작업은 검토 대기 상태로 유지됩니다. 파일을 다시 올리지 말고
                분석만 다시 시도하세요.
              </p>
              <button
                className="rounded-md border border-border bg-white px-3 py-2 text-sm font-semibold hover:bg-menu-hover disabled:opacity-50"
                data-testid="ai-grid-analysis-retry"
                disabled={isWorking}
                type="button"
                onClick={() => void refreshAnalysis()}
              >
                {isWorking ? "다시 분석 중" : "결과 다시 분석"}
              </button>
            </div>
          </section>
        ) : null}

        {step === 4 && workspace && analysis && reviewSettings ? (
          <section
            className="grid gap-4"
            data-testid="ai-grid-step-review"
          >
            <div>
              <h3 className="text-base font-semibold">
                4. 전체 시트와 셀 매핑 검토
              </h3>
              <p className="mt-1 text-sm text-muted">
                편집은 모든 대상이 필수입니다. 생성은 제외할 수 있지만
                포함한 셀은 한 번에 모두 저장됩니다.
              </p>
            </div>
            {workspace.requestScope !== "grid_edit" ? (
              <GenerationBackgroundReview
                artifact={workspace.outputArtifact}
                confirmed={backgroundReviewConfirmed}
                testId="ai-grid-generation-background-review"
                onConfirmedChange={(checked) => {
                  setBackgroundReviewConfirmed(checked);
                  if (checked) setErrorMessage(null);
                }}
              />
            ) : null}
            <div className="overflow-hidden rounded-md border border-border lg:flex lg:min-h-[500px]">
              <div className="min-w-0 flex-1">
                <SheetGridOverlay
                  cells={analysis.cells}
                  imageUrl={
                    workspace.outputArtifact?.previewUrl ?? null
                  }
                  selectedIndexes={mappedCellIndexes}
                  sheetHeight={analysis.sheetHeight}
                  sheetWidth={analysis.sheetWidth}
                  onToggleCell={(cellIndex) =>
                    setMapping((current) => {
                      const first = workspace.items.find(
                        (item) =>
                          (effectiveMode === "edit" ||
                            includedItemIndexes.has(item.itemIndex)) &&
                          current.get(item.itemIndex) !== cellIndex,
                      );
                      if (!first) return current;
                      const next = new Map(current);
                      next.set(first.itemIndex, cellIndex);
                      return next;
                    })
                  }
                />
              </div>
              <SheetGridSettingsPanel
                settings={reviewSettings}
                onChange={setReviewSettings}
                onPreview={() => void refreshAnalysis()}
                onReset={() =>
                  setReviewSettings(
                    sheetSettingsFromLayout(workspace.layout),
                  )
                }
              />
            </div>
            {analysis.warnings.length ? (
              <ul className="rounded-md border border-warning/30 bg-warning/5 p-3 text-sm text-muted">
                {analysis.warnings.map((warning) => (
                  <li key={warning}>- {warning}</li>
                ))}
              </ul>
            ) : null}
            {correctionPrompt ? (
              <div className="grid gap-2 rounded-md border border-warning/40 bg-warning/5 p-3">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <h4 className="text-sm font-semibold">웹에 추가할 구조 수정 프롬프트</h4>
                    <p className="mt-1 text-xs text-muted">
                      감지한 캔버스·셀 구조 오류만 설명합니다. 인증·네트워크 오류에는 프롬프트를 제안하지 않습니다.
                    </p>
                  </div>
                  <button
                    className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover"
                    type="button"
                    onClick={() => void copyCorrectionPrompt()}
                  >
                    <Copy aria-hidden="true" />
                    수정 프롬프트 복사
                  </button>
                </div>
                <textarea
                  aria-label="구조 수정 프롬프트"
                  className="min-h-28 rounded-md border border-border bg-white p-3 text-xs leading-5"
                  readOnly
                  value={correctionPrompt}
                />
              </div>
            ) : null}
            <div className="grid gap-2 md:grid-cols-2">
              {workspace.items.map((item) => {
                const included =
                  effectiveMode === "edit" ||
                  includedItemIndexes.has(item.itemIndex);
                return (
                  <div
                    className="grid grid-cols-[auto_minmax(0,1fr)_120px] items-center gap-2 rounded-md border border-border bg-white p-3"
                    key={item.id}
                  >
                    <input
                      aria-label={`${item.targetNameSnapshot} 포함`}
                      checked={included}
                      disabled={effectiveMode === "edit"}
                      type="checkbox"
                      onChange={(event) =>
                        setIncludedItemIndexes((current) => {
                          const next = new Set(current);
                          if (event.currentTarget.checked) {
                            next.add(item.itemIndex);
                          } else {
                            next.delete(item.itemIndex);
                          }
                          return next;
                        })
                      }
                    />
                    <span
                      className="truncate text-sm"
                      title={item.targetNameSnapshot}
                    >
                      {item.itemIndex + 1}. {item.targetNameSnapshot}
                    </span>
                    <select
                      aria-label={`${item.targetNameSnapshot} 결과 셀`}
                      className="rounded-md border border-border bg-white px-2 py-1.5 text-sm"
                      disabled={!included}
                      value={mapping.get(item.itemIndex) ?? ""}
                      onChange={(event) =>
                        setMapping((current) =>
                          new Map(current).set(
                            item.itemIndex,
                            Number(event.currentTarget.value),
                          ),
                        )
                      }
                    >
                      <option value="">셀 선택</option>
                      {analysis.cells
                        .filter((cell) => !cell.outOfBounds)
                        .map((cell) => (
                          <option key={cell.index} value={cell.index}>
                            결과 셀 {cell.index + 1}
                          </option>
                        ))}
                    </select>
                  </div>
                );
              })}
            </div>
            {reviewError ? (
              <p className="text-sm text-danger" role="alert">
                {reviewError}
              </p>
            ) : (
              <p className="text-sm text-success" role="status">
                모든 매핑이 유효합니다. 저장 중 하나라도 실패하면 전부
                되돌립니다.
              </p>
            )}
            <div className="flex justify-end">
              <button
                className="rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-50"
                disabled={
                  Boolean(reviewError) ||
                  isWorking ||
                  needsGenerationBackgroundConfirmation
                }
                type="button"
                onClick={() => void saveReview()}
              >
                {isWorking
                  ? "검토 저장 중"
                  : effectiveMode === "edit"
                    ? `${workspace.itemCount}개 후보 모두 저장`
                    : "포함 셀 후보 저장"}
              </button>
            </div>
          </section>
        ) : null}

        {step === 5 && workspace ? (
          <section
            className="grid gap-5"
            data-testid="ai-grid-step-save"
          >
            <div>
              <h3 className="text-base font-semibold">5. 저장 결과</h3>
              <p className="mt-1 text-sm text-muted">
                {workspace.requestScope === "grid_edit"
                  ? "결과는 각 원본 아이콘의 비활성 후보입니다. 개별 검토에서 적용하기 전까지 현재 이미지는 바뀌지 않습니다."
                  : "새 아이콘을 만들 때 icon·piece·crop·AI 계보·순서를 한 transaction으로 저장합니다."}
              </p>
            </div>
            {workspace.requestScope !== "grid_edit" ? (
              <GenerationBackgroundReview
                artifact={workspace.outputArtifact}
                completed={workspace.status === "completed"}
                confirmed={backgroundReviewConfirmed}
                showPreview
                testId="ai-grid-final-background-review"
                onConfirmedChange={(checked) => {
                  setBackgroundReviewConfirmed(checked);
                  if (checked) setErrorMessage(null);
                }}
              />
            ) : null}
            {workspace.requestScope === "grid_edit" ? (
              <SuccessCard
                detail="원본 보존 · 현재 활성 소스 변경 없음 · 요청 단위 all-or-none 완료"
                title={`${workspace.candidateCount}개 후보를 전부 저장했습니다.`}
              />
            ) : workspace.status === "completed" ? (
              <SuccessCard
                detail="원본 없는 생성 계보와 컬렉션 순서가 함께 저장됐습니다."
                title={`새 아이콘 ${workspace.createdIconCount}개를 만들었습니다.`}
              />
            ) : (
              <div className="grid gap-3 sm:grid-cols-2">
                {generationItems.map((item) => {
                  const draft = finalizedDrafts[item.itemIndex] ?? {
                    displayName: item.targetNameSnapshot,
                    altText: "",
                  };
                  return (
                    <div
                      className="grid gap-2 rounded-md border border-border bg-white p-3"
                      key={item.id}
                    >
                      <label className="grid gap-1 text-xs font-medium text-muted">
                        {item.itemIndex + 1}번 이름
                        <input
                          className="rounded-md border border-border px-3 py-2 text-sm text-foreground"
                          maxLength={255}
                          value={draft.displayName}
                          onChange={(event) =>
                            setFinalizedDrafts((current) => ({
                              ...current,
                              [item.itemIndex]: {
                                ...draft,
                                displayName:
                                  event.currentTarget.value,
                              },
                            }))
                          }
                        />
                      </label>
                      <label className="grid gap-1 text-xs font-medium text-muted">
                        alt (선택)
                        <input
                          className="rounded-md border border-border px-3 py-2 text-sm text-foreground"
                          maxLength={16}
                          placeholder="나중에 입력 가능"
                          value={draft.altText}
                          onChange={(event) =>
                            setFinalizedDrafts((current) => ({
                              ...current,
                              [item.itemIndex]: {
                                ...draft,
                                altText: event.currentTarget.value,
                              },
                            }))
                          }
                        />
                      </label>
                    </div>
                  );
                })}
              </div>
            )}
            <div className="flex flex-wrap justify-between gap-2">
              {workspace.status !== "completed" &&
              workspace.candidateCount === 0 ? (
                <button
                  className="inline-flex items-center gap-2 rounded-md border border-danger/30 bg-white px-4 py-2 text-sm font-medium text-danger hover:bg-danger/5"
                  disabled={isWorking}
                  type="button"
                  onClick={() => void cancelWorkspace(false)}
                >
                  <Trash2 aria-hidden="true" />
                  요청 취소
                </button>
              ) : (
                <span />
              )}
              <div className="flex gap-2">
                {workspace.requestScope !== "grid_edit" &&
                workspace.status !== "completed" ? (
                  <button
                    className="rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-50"
                    disabled={
                      isWorking ||
                      generationItems.length === 0 ||
                      needsGenerationBackgroundConfirmation
                    }
                    type="button"
                    onClick={() => void createGeneratedIcons()}
                  >
                    {isWorking
                      ? "새 아이콘 저장 중"
                      : `${generationItems.length}개 새 아이콘 모두 만들기`}
                  </button>
                ) : null}
                <button
                  className="rounded-md border border-border bg-white px-4 py-2 text-sm font-medium hover:bg-menu-hover"
                  type="button"
                  onClick={onClose}
                >
                  닫기
                </button>
              </div>
            </div>
          </section>
        ) : null}
      </div>
    </ModalFrame>
  );
}

function isSupportedReferenceFile(file: File) {
  if (["image/png", "image/jpeg", "image/gif"].includes(file.type)) {
    return true;
  }
  return /\.(png|jpe?g|gif)$/i.test(file.name);
}

function GridInputActions({
  disabled,
  onAction,
}: {
  disabled: boolean;
  onAction: (action: "drag" | "reveal") => Promise<void>;
}) {
  const handleKeyboard = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    void onAction("reveal");
  };
  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (event.detail === 0) void onAction("reveal");
  };
  return (
    <>
      <button
        aria-describedby="ai-grid-native-drag-keyboard-help"
        className="inline-flex items-center justify-center gap-2 rounded-md border border-accent bg-white px-3 py-2 text-sm font-semibold hover:bg-selected focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
        disabled={disabled}
        title="마우스로 누른 채 브라우저 업로드 영역까지 끌어 놓습니다."
        type="button"
        onClick={handleClick}
        onKeyDown={handleKeyboard}
        onPointerDown={(event) => {
          if (event.pointerType === "mouse" && event.button === 0) {
            void onAction("drag");
          }
        }}
      >
        <GripVertical aria-hidden="true" />
        입력 파일 끌기
      </button>
      <button
        className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:opacity-50"
        disabled={disabled}
        type="button"
        onClick={() => void onAction("reveal")}
      >
        <FolderOpen aria-hidden="true" />
        탐색기에서 선택
      </button>
      <p
        className="text-[11px] leading-4 text-muted"
        id="ai-grid-native-drag-keyboard-help"
      >
        마우스는 파일 끌기를 시작합니다. 키보드로 활성화하면 안전한
        탐색기 선택 방식으로 전환합니다.
      </p>
    </>
  );
}

function GenerationBackgroundReview({
  artifact,
  completed = false,
  confirmed,
  onConfirmedChange,
  showPreview = false,
  testId,
}: {
  artifact: AiGridWorkspace["outputArtifact"];
  completed?: boolean;
  confirmed: boolean;
  onConfirmedChange: (checked: boolean) => void;
  showPreview?: boolean;
  testId: string;
}) {
  const serverStatus = artifact
    ? artifact.hasAlpha
      ? `서버 검사: ${artifact.extension.toUpperCase()} 결과에서 투명/반투명 픽셀을 감지했습니다.`
      : `서버 검사: ${artifact.extension.toUpperCase()} 결과는 불투명합니다. 배경을 포함한 상태로 저장됩니다.`
    : "저장된 후보에 배경 검사 메타데이터가 없어 자동 판정할 수 없습니다.";

  return (
    <section
      className="grid gap-2 rounded-md border border-warning/40 bg-warning/5 p-3 text-xs leading-5"
      data-testid={testId}
      role="status"
    >
      <p className="font-semibold text-foreground">후보 배경 검사</p>
      <p className="text-muted">{serverStatus}</p>
      {showPreview && artifact ? (
        <a
          className="grid gap-2 rounded border border-border bg-white p-2 hover:bg-menu-hover"
          data-testid={`${testId}-preview-link`}
          href={artifact.previewUrl}
          rel="noreferrer"
          target="_blank"
        >
          <img
            alt={`${artifact.originalFilename} AI 결과 전체 미리보기`}
            className="max-h-96 w-full object-contain [image-rendering:auto]"
            data-testid={`${testId}-preview`}
            src={artifact.previewUrl}
          />
          <span className="text-center text-[11px] text-muted">
            전체 결과를 크게 확인하세요. 이미지를 누르면 원본 크기로 엽니다.
          </span>
        </a>
      ) : null}
      <p className="font-medium text-warning">
        자동 검사가 alpha 유무를 확인해도 체커무늬가 실제 픽셀로 그려진 가짜
        투명 배경인지는 판별할 수 없습니다. 각 후보를 확대해 직접 확인하세요.
      </p>
      {!completed ? (
        <label className="flex items-start gap-2 rounded border border-warning/30 bg-white p-2 text-foreground">
          <input
            checked={confirmed}
            className="mt-1"
            data-testid={`${testId}-confirm`}
            type="checkbox"
            onChange={(event) => onConfirmedChange(event.currentTarget.checked)}
          />
          <span>
            후보를 확대해 체커무늬가 실제 픽셀로 칠해지지 않았는지 확인했습니다.
          </span>
        </label>
      ) : null}
    </section>
  );
}

function ModalFrame({
  dialogRef,
  children,
  onClose,
}: {
  dialogRef: RefObject<HTMLDivElement | null>;
  children: ReactNode;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-[75] flex items-center justify-center bg-black/40 p-3"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        aria-labelledby="ai-grid-workspace-title"
        aria-modal="true"
        className="flex max-h-[min(820px,calc(100vh-24px))] w-full max-w-6xl flex-col overflow-hidden rounded-xl border border-border bg-surface shadow-2xl"
        data-testid="ai-grid-workspace-dialog"
        role="dialog"
        tabIndex={-1}
      >
        {children}
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-white p-3">
      <p className="text-xs text-muted">{label}</p>
      <p className="mt-1 text-sm font-semibold">{value}</p>
    </div>
  );
}

function SuccessCard({
  detail,
  title,
}: {
  detail: string;
  title: string;
}) {
  return (
    <div className="rounded-lg border border-success/30 bg-success/5 p-5">
      <CheckCircle2 aria-hidden="true" className="text-success" />
      <p className="mt-2 font-semibold">{title}</p>
      <p className="mt-1 text-sm text-muted">{detail}</p>
    </div>
  );
}

function defaultLayoutLabel(itemCount: number) {
  if (itemCount === 1) return "1×1";
  if (itemCount === 2) return "1×2";
  if (itemCount <= 4) return "2×2";
  if (itemCount <= 9) return "3×3";
  return "4×4";
}

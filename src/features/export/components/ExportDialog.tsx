import { useEffect, useMemo, useState } from "react";
import type { MouseEvent, ReactNode } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  ExternalLink,
  FileText,
  FolderOpen,
  RefreshCw,
  Save,
  Settings2,
  X,
} from "lucide-react";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import { EditorPanel } from "@/features/editor/components/EditorPanel";
import {
  applyOptimizationCandidate,
  clearOptimizationCandidate,
  exportCollection,
  exportSelectedCollectionItems,
  generateGifOptimizationCandidates,
  generateStaticOptimizationCandidates,
  listExportProfiles,
  openExportPath,
  pickExportDirectory,
  saveExportProfileSettings,
  validateExportCollection,
} from "@/features/export/api";
import {
  EXPORT_WORKSPACE_FILTER_LABELS,
  filterExportItems,
  formatExportIndex,
  hasOversizedIssue,
  issueSummary,
  issuesForItem,
  mergeExportSessionValidation,
  problemExportNumbers,
  statusLabel,
  statusTone,
  summarizeExportWorkspace,
  type ExportWorkspaceFilter,
} from "@/features/export/export-workspace-model";
import type {
  ExportCollectionResult,
  ExportFormat,
  ExportPlanItem,
  ExportProfile,
  ExportRequestPayload,
  ExportValidationResult,
  FilenameMode,
  OptimizationCandidate,
  OptimizationResult,
  ResizeFilter,
} from "@/features/export/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import {
  bytesToMegabytesInput,
  megabytesInputToBytes,
} from "@/lib/byte-size";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

interface ExportDialogProps {
  collection: CollectionSummary;
  onClose: () => void;
  onExported: () => Promise<void>;
  onIconUpdated: (icon: IconSummary) => void;
}

interface ExportDraft {
  profileId: string;
  targetFormat: ExportFormat;
  targetCellWidth: number;
  targetCellHeight: number;
  maxBytes: number;
  filenameMode: FilenameMode;
  includeAltTxt: boolean;
  strictWarnings: boolean;
  outputDirectory: string;
  openFolderAfterExport: boolean;
  openAltTxtAfterExport: boolean;
  resizeFilter: ResizeFilter;
}

const FORMAT_OPTIONS: Array<{ value: ExportFormat; label: string }> = [
  { value: "png", label: "PNG" },
  { value: "jpg", label: "JPG" },
  { value: "gif", label: "GIF" },
  { value: "source", label: "원본 형식" },
];

const FILENAME_OPTIONS: Array<{ value: FilenameMode; label: string }> = [
  { value: "sequence", label: "001, 002, 003" },
  { value: "alt", label: "alt 값" },
];

const RESIZE_FILTER_OPTIONS: Array<{ value: ResizeFilter; label: string }> = [
  { value: "nearest", label: "Nearest" },
  { value: "triangle", label: "Bilinear" },
  { value: "catmull_rom", label: "Bicubic" },
  { value: "gaussian", label: "Gaussian" },
  { value: "lanczos3", label: "Lanczos" },
];

const FILTERS: ExportWorkspaceFilter[] = [
  "all",
  "included",
  "excluded",
  "completed",
  "pending",
  "warnings",
  "not_upload_ready",
  "failed",
  "gif",
  "oversized",
];

export function ExportDialog({
  collection,
  onClose,
  onExported,
  onIconUpdated,
}: ExportDialogProps) {
  const [profiles, setProfiles] = useState<ExportProfile[]>([]);
  const [draft, setDraft] = useState<ExportDraft | null>(null);
  const [validation, setValidation] = useState<ExportValidationResult | null>(null);
  const [exportResult, setExportResult] = useState<ExportCollectionResult | null>(null);
  const [excludedPieceIds, setExcludedPieceIds] = useState<Set<string>>(() => new Set());
  const [filter, setFilter] = useState<ExportWorkspaceFilter>("all");
  const [selectedPieceId, setSelectedPieceId] = useState<string | null>(null);
  const [selectedPieceIds, setSelectedPieceIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [selectionAnchorPieceId, setSelectionAnchorPieceId] = useState<string | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isValidating, setIsValidating] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [optimizationItem, setOptimizationItem] = useState<ExportPlanItem | null>(null);
  const [optimizationResult, setOptimizationResult] =
    useState<OptimizationResult | null>(null);
  const [optimizationError, setOptimizationError] = useState<string | null>(null);
  const [isOptimizing, setIsOptimizing] = useState(false);
  const [editingIconId, setEditingIconId] = useState<string | null>(null);

  useEffect(() => {
    let isActive = true;

    async function loadProfiles() {
      setIsLoading(true);
      setErrorMessage(null);

      try {
        const nextProfiles = await listExportProfiles(collection.id);
        if (!isActive) {
          return;
        }

        const nextDraft = draftFromProfile(preferredProfile(nextProfiles), collection);
        const nextValidation = await validateExportCollection(
          collection.id,
          payloadFromDraft(nextDraft, new Set()),
        );
        if (!isActive) {
          return;
        }

        setProfiles(nextProfiles);
        setDraft(nextDraft);
        setValidation(nextValidation);
        setSelectedPieceId(nextValidation.items[0]?.pieceId ?? null);
      } catch (error) {
        if (isActive) {
          setErrorMessage(getCommandErrorMessage(error));
        }
      } finally {
        if (isActive) {
          setIsLoading(false);
        }
      }
    }

    void loadProfiles();

    return () => {
      isActive = false;
    };
  }, [
    collection.id,
    collection.defaultCellWidth,
    collection.defaultCellHeight,
    collection.exportFormat,
    collection.maxBytes,
  ]);

  const selectedProfile = useMemo(() => {
    if (!draft) {
      return null;
    }

    return profiles.find((profile) => profile.id === draft.profileId) ?? null;
  }, [draft, profiles]);

  const isBusy = isLoading || isSaving || isValidating || isExporting || isOptimizing;
  const payload = draft ? payloadFromDraft(draft, excludedPieceIds) : null;
  const summary = useMemo(() => summarizeExportWorkspace(validation), [validation]);
  const filteredItems = useMemo(
    () => filterExportItems(validation, filter),
    [filter, validation],
  );
  const filteredPieceIds = useMemo(
    () => filteredItems.map((item) => item.pieceId),
    [filteredItems],
  );
  const allPieceIds = useMemo(
    () => (validation?.items ?? []).map((item) => item.pieceId),
    [validation],
  );
  const selectedExportItems = useMemo(
    () => filteredItems.filter((item) => selectedPieceIds.has(item.pieceId)),
    [filteredItems, selectedPieceIds],
  );
  const selectedIncludedCount = useMemo(
    () =>
      (validation?.items ?? []).filter(
        (item) => selectedPieceIds.has(item.pieceId) && item.included,
      ).length,
    [selectedPieceIds, validation],
  );
  const selectedItem =
    filteredItems.find((item) => item.pieceId === selectedPieceId) ??
    selectedExportItems[0] ??
    filteredItems[0] ??
    null;
  const selectedIssues = selectedItem ? issuesForItem(validation, selectedItem) : [];
  const problemNumbers = problemExportNumbers(validation);
  const canStartExport = Boolean(payload && summary.included > 0 && !isBusy);
  const selectedOversizedItems = useMemo(
    () =>
      selectedExportItems.filter(
        (item) => item.included && hasOversizedIssue(validation, item),
      ),
    [selectedExportItems, validation],
  );
  const canOptimizeSelected = selectedOversizedItems.length > 0 && !isBusy;
  const canRerunSelected = Boolean(
    payload &&
      exportResult?.exportDirectory &&
      selectedIncludedCount > 0 &&
      !isBusy,
  );

  useEffect(() => {
    const validPieceIds = new Set(allPieceIds);
    setSelectedPieceIds((current) => {
      const next = new Set(
        Array.from(current).filter((pieceId) => validPieceIds.has(pieceId)),
      );
      return next.size === current.size ? current : next;
    });
    if (selectedPieceId && !validPieceIds.has(selectedPieceId)) {
      setSelectedPieceId(allPieceIds[0] ?? null);
    }
    if (selectionAnchorPieceId && !validPieceIds.has(selectionAnchorPieceId)) {
      setSelectionAnchorPieceId(null);
    }
  }, [allPieceIds, selectedPieceId, selectionAnchorPieceId]);

  const runValidation = async (
    nextDraft: ExportDraft,
    nextExcludedPieceIds: Set<string>,
    options: {
      dirtyIconIds?: Set<string>;
      dirtyPieceIds?: Set<string>;
      preserveNonDirtyExcluded?: boolean;
      preserveSession?: boolean;
      quiet?: boolean;
    } = {},
  ) => {
    setIsValidating(true);
    setErrorMessage(null);
    if (!options.quiet) {
      setStatusMessage(null);
    }

    try {
      const result = await validateExportCollection(
        collection.id,
        payloadFromDraft(nextDraft, nextExcludedPieceIds),
      );
      const nextValidation = options.preserveSession
        ? mergeExportSessionValidation(result, validation, {
            dirtyIconIds: options.dirtyIconIds,
            dirtyPieceIds: options.dirtyPieceIds,
            preserveNonDirtyExcluded: options.preserveNonDirtyExcluded,
          })
        : result;
      setValidation(nextValidation);
      if (!options.preserveSession) {
        setExportResult(null);
      }
      setSelectedPieceId((current) => current ?? result.items[0]?.pieceId ?? null);
      if (!options.quiet) {
        setStatusMessage("사전 확인을 완료했습니다.");
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsValidating(false);
    }
  };

  const selectProfile = (profileId: string) => {
    const profile = profiles.find((candidate) => candidate.id === profileId);
    if (!profile) {
      return;
    }

    const nextDraft = draftFromProfile(profile, collection);
    const nextExcludedPieceIds = new Set<string>();
    setDraft(nextDraft);
    setExcludedPieceIds(nextExcludedPieceIds);
    setExportResult(null);
    setStatusMessage(null);
    void runValidation(nextDraft, nextExcludedPieceIds, { quiet: true });
  };

  const updateDraft = (partial: Partial<ExportDraft>) => {
    setDraft((current) => (current ? { ...current, ...partial } : current));
    setExportResult(null);
    setStatusMessage(null);
  };

  const setPiecesIncluded = (pieceIds: string[], included: boolean) => {
    if (!draft) {
      return;
    }

    const nextExcludedPieceIds = new Set(excludedPieceIds);
    for (const pieceId of pieceIds) {
      if (included) {
        nextExcludedPieceIds.delete(pieceId);
      } else {
        nextExcludedPieceIds.add(pieceId);
      }
    }
    setExcludedPieceIds(nextExcludedPieceIds);
    void runValidation(draft, nextExcludedPieceIds, {
      dirtyPieceIds: new Set(pieceIds),
      preserveSession: Boolean(exportResult),
      quiet: true,
    });
  };

  const setPieceIncluded = (pieceId: string, included: boolean) => {
    setPiecesIncluded([pieceId], included);
  };

  const handleSelectItem = (event: MouseEvent, item: ExportPlanItem) => {
    setSelectedPieceId(item.pieceId);

    setSelectedPieceIds((current) => {
      if (event.shiftKey && selectionAnchorPieceId) {
        const anchorIndex = filteredPieceIds.indexOf(selectionAnchorPieceId);
        const targetIndex = filteredPieceIds.indexOf(item.pieceId);
        if (anchorIndex !== -1 && targetIndex !== -1) {
          const [start, end] = [
            Math.min(anchorIndex, targetIndex),
            Math.max(anchorIndex, targetIndex),
          ];
          const range = filteredPieceIds.slice(start, end + 1);
          if (event.ctrlKey || event.metaKey) {
            return new Set([...current, ...range]);
          }
          return new Set(range);
        }
      }

      if (event.ctrlKey || event.metaKey) {
        const next = new Set(current);
        if (next.has(item.pieceId)) {
          next.delete(item.pieceId);
        } else {
          next.add(item.pieceId);
        }
        return next;
      }

      return new Set([item.pieceId]);
    });

    if (!event.shiftKey || !selectionAnchorPieceId) {
      setSelectionAnchorPieceId(item.pieceId);
    }
  };

  const selectVisibleItems = () => {
    setSelectedPieceIds(new Set(filteredPieceIds));
    setSelectedPieceId(filteredPieceIds[0] ?? null);
    setSelectionAnchorPieceId(filteredPieceIds[0] ?? null);
  };

  const clearSelection = () => {
    setSelectedPieceIds(new Set());
    setSelectionAnchorPieceId(null);
  };

  const handlePickOutputDirectory = async () => {
    if (!draft) {
      return;
    }

    setErrorMessage(null);
    try {
      const selectedDirectory = await pickExportDirectory(
        draft.outputDirectory.trim() ? draft.outputDirectory.trim() : null,
      );
      if (selectedDirectory) {
        updateDraft({ outputDirectory: selectedDirectory });
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleSave = async () => {
    if (!payload) {
      return;
    }

    setIsSaving(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const profile = await saveExportProfileSettings(collection.id, payload);
      setProfiles((current) =>
        current.map((candidate) => (candidate.id === profile.id ? profile : candidate)),
      );
      setStatusMessage("내보내기 설정을 저장했습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  };

  const handleValidate = async () => {
    if (draft) {
      await runValidation(draft, excludedPieceIds, {
        preserveSession: Boolean(exportResult),
      });
    }
  };

  const handleExport = async () => {
    if (!payload) {
      return;
    }

    setIsExporting(true);
    setErrorMessage(null);
    setStatusMessage("내보내기를 실행하는 중입니다.");
    setExportResult(null);

    try {
      const result = await exportCollection(collection.id, payload);
      setValidation(result.validation);
      setExportResult(result);
      setSelectedPieceId(result.validation.items[0]?.pieceId ?? null);
      await onExported();
      setStatusMessage(
        result.exportDirectory
          ? "내보내기를 완료했습니다. 문제 항목은 보고서에서 확인할 수 있습니다."
          : "생성 가능한 내보내기 항목이 없습니다.",
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsExporting(false);
    }
  };

  const handleExportPieces = async (pieceIds: string[]) => {
    if (!draft || !payload || !exportResult?.exportDirectory || pieceIds.length === 0) {
      return;
    }
    const requestedPieceIds = new Set(pieceIds);
    const selectedIncludedPieceIds = (validation?.items ?? [])
      .filter((item) => requestedPieceIds.has(item.pieceId) && item.included)
      .map((item) => item.pieceId);
    if (selectedIncludedPieceIds.length === 0) {
      return;
    }

    setIsExporting(true);
    setErrorMessage(null);
    setStatusMessage(`선택 항목 ${selectedIncludedPieceIds.length}개를 다시 내보내는 중입니다.`);

    try {
      const result = await exportSelectedCollectionItems(
        collection.id,
        payloadFromDraft(draft, excludedPieceIds),
        selectedIncludedPieceIds,
        exportResult.exportDirectory,
      );
      const mergedValidation = mergeExportSessionValidation(
        result.validation,
        validation,
        {
          dirtyPieceIds: new Set(selectedIncludedPieceIds),
          preserveNonDirtyExcluded: true,
        },
      );
      setValidation(mergedValidation);
      setExportResult({
        ...result,
        validation: mergedValidation,
      });
      await onExported();
      setStatusMessage(
        `선택 항목 ${selectedIncludedPieceIds.length}개를 다시 내보냈습니다. 나머지 완료 상태는 유지했습니다.`,
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsExporting(false);
    }
  };

  const handleExportSelected = async () => {
    await handleExportPieces(Array.from(selectedPieceIds));
  };

  const handleOpenOptimization = async (item: ExportPlanItem) => {
    if (!draft) {
      return;
    }

    setOptimizationItem(item);
    setOptimizationResult(null);
    setOptimizationError(null);
    setIsOptimizing(true);

    try {
      const result =
        item.outputFormat === "gif"
          ? await generateGifOptimizationCandidates(item.iconId, draft.profileId, item.pieceId)
          : await generateStaticOptimizationCandidates(
              item.iconId,
              draft.profileId,
              item.pieceId,
            );
      setOptimizationResult(result);
    } catch (error) {
      setOptimizationError(getCommandErrorMessage(error));
    } finally {
      setIsOptimizing(false);
    }
  };

  const handleOptimizeSelected = async () => {
    if (!draft || selectedOversizedItems.length === 0) {
      return;
    }

    setIsOptimizing(true);
    setErrorMessage(null);
    setStatusMessage(`선택 항목 ${selectedOversizedItems.length}개의 용량 후보를 생성하는 중입니다.`);

    const failures: string[] = [];
    let appliedCount = 0;

    for (const item of selectedOversizedItems) {
      try {
        const result =
          item.outputFormat === "gif"
            ? await generateGifOptimizationCandidates(item.iconId, draft.profileId, item.pieceId)
            : await generateStaticOptimizationCandidates(
                item.iconId,
                draft.profileId,
                item.pieceId,
              );
        const candidate = chooseBatchCandidate(result.candidates);
        if (!candidate) {
          failures.push(`${formatExportIndex(item)} 후보 없음`);
          continue;
        }
        await applyOptimizationCandidate(candidate.id);
        appliedCount += 1;
      } catch (error) {
        failures.push(`${formatExportIndex(item)} ${getCommandErrorMessage(error)}`);
      }
    }

    try {
      await runValidation(draft, excludedPieceIds, {
        dirtyPieceIds: new Set(selectedOversizedItems.map((item) => item.pieceId)),
        preserveSession: Boolean(exportResult),
        quiet: true,
      });
    } finally {
      setIsOptimizing(false);
    }

    setStatusMessage(
      failures.length > 0
        ? `용량 압축 ${appliedCount}개 적용, ${failures.length}개 실패: ${failures
            .slice(0, 3)
            .join(" / ")}`
        : `용량 압축 후보 ${appliedCount}개를 적용했습니다. 필요한 항목만 다시 내보내세요.`,
    );
  };

  const handleApplyOptimization = async (candidate: OptimizationCandidate) => {
    if (!draft) {
      return;
    }

    setIsOptimizing(true);
    setOptimizationError(null);
    try {
      const applied = await applyOptimizationCandidate(candidate.id);
      await runValidation(draft, excludedPieceIds, {
        dirtyPieceIds: new Set([candidate.pieceId]),
        preserveSession: Boolean(exportResult),
        quiet: true,
      });
      setStatusMessage(applied.message);
      setOptimizationResult((current) =>
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
    } catch (error) {
      setOptimizationError(getCommandErrorMessage(error));
    } finally {
      setIsOptimizing(false);
    }
  };

  const handleClearOptimization = async () => {
    if (!draft || !optimizationItem) {
      return;
    }

    setIsOptimizing(true);
    setOptimizationError(null);
    try {
      const cleared = await clearOptimizationCandidate(
        optimizationItem.iconId,
        draft.profileId,
        optimizationItem.pieceId,
      );
      await runValidation(draft, excludedPieceIds, {
        dirtyPieceIds: new Set([optimizationItem.pieceId]),
        preserveSession: Boolean(exportResult),
        quiet: true,
      });
      setStatusMessage(cleared.message);
      setOptimizationResult(null);
    } catch (error) {
      setOptimizationError(getCommandErrorMessage(error));
    } finally {
      setIsOptimizing(false);
    }
  };

  const handleEditedIcon = (icon: IconSummary) => {
    onIconUpdated(icon);
    if (draft) {
      void runValidation(draft, excludedPieceIds, {
        dirtyIconIds: new Set([icon.id]),
        preserveSession: Boolean(exportResult),
        quiet: true,
      });
    }
  };

  return (
    <div
      aria-modal="true"
      className="fixed inset-0 z-50 flex bg-surface text-foreground"
      role="dialog"
    >
      <div className="flex min-h-0 w-full flex-col">
        <header className="flex shrink-0 flex-col gap-3 border-b border-border bg-surface px-5 py-4">
          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <h2 className="truncate text-lg font-semibold tracking-normal">
                내보내기 작업공간
              </h2>
              <p className="mt-1 truncate text-xs text-muted">
                {collection.name} · 파일 생성과 업로드 가능 여부를 분리해서 확인합니다.
              </p>
            </div>
            <button
              aria-label="내보내기 작업공간 닫기"
              className="inline-flex size-9 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              disabled={isBusy}
              type="button"
              onClick={onClose}
            >
              <X aria-hidden="true" />
            </button>
          </div>

          {draft && selectedProfile ? (
            <div className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_auto]">
              <div className="grid gap-2 md:grid-cols-[180px_minmax(180px,1fr)_180px_170px_160px]">
                <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
                  프로필
                  <select
                    className="h-9 rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    value={draft.profileId}
                    onChange={(event) => selectProfile(event.currentTarget.value)}
                  >
                    {profiles.map((profile) => (
                      <option key={profile.id} value={profile.id}>
                        {profileLabel(profile)}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
                  출력 폴더
                  <div className="flex min-w-0 gap-2">
                    <input
                      className="h-9 min-w-0 flex-1 rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                      placeholder="기본 exports 폴더"
                      readOnly
                      value={draft.outputDirectory}
                    />
                    <button
                      className="inline-flex h-9 items-center gap-1 rounded-md border border-border bg-white px-3 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                      disabled={isBusy}
                      type="button"
                      onClick={() => {
                        void handlePickOutputDirectory();
                      }}
                    >
                      <FolderOpen aria-hidden="true" className="size-4" />
                      선택
                    </button>
                  </div>
                </label>
                <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
                  파일명
                  <select
                    className="h-9 rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    value={draft.filenameMode}
                    onChange={(event) =>
                      updateDraft({
                        filenameMode: event.currentTarget.value as FilenameMode,
                      })
                    }
                  >
                    {FILENAME_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
                  형식
                  <select
                    className="h-9 rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    value={draft.targetFormat}
                    onChange={(event) =>
                      updateDraft({
                        targetFormat: event.currentTarget.value as ExportFormat,
                      })
                    }
                  >
                    {FORMAT_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
                  리사이즈
                  <select
                    className="h-9 rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    value={draft.resizeFilter}
                    onChange={(event) =>
                      updateDraft({
                        resizeFilter: event.currentTarget.value as ResizeFilter,
                      })
                    }
                  >
                    {RESIZE_FILTER_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
              </div>

              <div className="flex flex-wrap items-end gap-2">
                <button
                  className="inline-flex h-9 items-center gap-2 rounded-md border border-border bg-white px-3 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                  disabled={!payload || isBusy}
                  type="button"
                  onClick={() => {
                    void handleSave();
                  }}
                >
                  <Save aria-hidden="true" className="size-4" />
                  저장
                </button>
                <button
                  className="inline-flex h-9 items-center gap-2 rounded-md border border-border bg-white px-3 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                  disabled={!payload || isBusy}
                  type="button"
                  onClick={() => {
                    void handleValidate();
                  }}
                >
                  <RefreshCw aria-hidden="true" className="size-4" />
                  사전 확인
                </button>
                <button
                  className="inline-flex h-9 items-center gap-2 rounded-md bg-accent px-3 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
                  disabled={!canStartExport}
                  type="button"
                  onClick={() => {
                    void handleExport();
                  }}
                >
                  <Download aria-hidden="true" className="size-4" />
                  내보내기 시작
                </button>
              </div>
            </div>
          ) : null}

          {draft ? (
            <div className="flex flex-wrap items-center gap-3">
              <NumberField
                label="셀 너비"
                min={1}
                value={draft.targetCellWidth}
                onChange={(targetCellWidth) => updateDraft({ targetCellWidth })}
              />
              <NumberField
                label="셀 높이"
                min={1}
                value={draft.targetCellHeight}
                onChange={(targetCellHeight) => updateDraft({ targetCellHeight })}
              />
              <MegabytesField
                label="용량 제한"
                value={draft.maxBytes}
                onChange={(maxBytes) => updateDraft({ maxBytes })}
              />
              <CheckboxField
                checked={draft.includeAltTxt}
                label="alts.txt"
                onChange={(includeAltTxt) => updateDraft({ includeAltTxt })}
              />
              <CheckboxField
                checked={draft.strictWarnings}
                label="경고 시 내보내기 차단"
                onChange={(strictWarnings) => updateDraft({ strictWarnings })}
              />
              <CheckboxField
                checked={draft.openFolderAfterExport}
                label="완료 후 폴더 열기"
                onChange={(openFolderAfterExport) =>
                  updateDraft({ openFolderAfterExport })
                }
              />
            </div>
          ) : null}

          {isExporting ? (
            <div className="rounded-md border border-border bg-canvas px-3 py-2">
              <div className="flex items-center justify-between text-xs text-muted">
                <span>내보내기 처리 중</span>
                <span>포함 항목 {summary.included}개</span>
              </div>
              <div
                aria-label="내보내기 처리 중"
                aria-valuetext="내보내기 결과를 생성하는 중입니다."
                className="mt-2 h-2 overflow-hidden rounded-full bg-preview"
                role="progressbar"
              >
                <div className="h-full w-1/2 rounded-full bg-accent animate-pulse" />
              </div>
            </div>
          ) : null}
        </header>

        <main className="grid min-h-0 flex-1 grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-3 bg-canvas px-5 py-4">
          <SummaryStrip result={exportResult} summary={summary} />

          <ExportSelectionToolbar
            allCount={allPieceIds.length}
            canOptimizeSelected={canOptimizeSelected}
            canRerunSelected={canRerunSelected}
            isBusy={isBusy}
            selectedCount={selectedPieceIds.size}
            selectedIncludedCount={selectedIncludedCount}
            selectedOversizedCount={selectedOversizedItems.length}
            visibleCount={filteredItems.length}
            onClearSelection={clearSelection}
            onExcludeAll={() => setPiecesIncluded(allPieceIds, false)}
            onIncludeAll={() => setPiecesIncluded(allPieceIds, true)}
            onOptimizeSelected={() => {
              void handleOptimizeSelected();
            }}
            onRerunSelected={() => {
              void handleExportSelected();
            }}
            onSelectVisible={selectVisibleItems}
            onToggleSelectedIncluded={() =>
              setPiecesIncluded(
                Array.from(selectedPieceIds),
                selectedIncludedCount === 0,
              )
            }
          />

          <div className="grid min-h-0 gap-4 xl:grid-cols-[minmax(300px,0.95fr)_minmax(460px,1.35fr)_320px]">
            <section className="flex min-h-0 flex-col rounded-md border border-border bg-surface">
              <PaneHeader count={filteredItems.length} title="전: 현재 디시콘" />
              <div className="grid min-h-0 grid-cols-2 gap-3 overflow-auto p-3 2xl:grid-cols-3">
                {filteredItems.map((item) => (
                  <SourceCard
                    item={item}
                    key={item.pieceId}
                    selected={selectedPieceIds.has(item.pieceId)}
                    onToggleIncluded={setPieceIncluded}
                    onSelect={handleSelectItem}
                  />
                ))}
              </div>
            </section>

            <section className="flex min-h-0 flex-col rounded-md border border-border bg-surface">
              <PaneHeader count={filteredItems.length} title="후: 내보내기 결과" />
              <div className="border-b border-border px-3 py-2">
                <div className="flex flex-wrap gap-1">
                  {FILTERS.map((nextFilter) => (
                    <button
                      className={cn(
                        "rounded px-2 py-1 text-xs font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
                        filter === nextFilter
                          ? "bg-accent text-accent-foreground"
                          : "bg-white text-muted hover:bg-menu-hover",
                      )}
                      key={nextFilter}
                      type="button"
                      onClick={() => setFilter(nextFilter)}
                    >
                      {EXPORT_WORKSPACE_FILTER_LABELS[nextFilter]}
                    </button>
                  ))}
                </div>
              </div>
              <div className="min-h-0 overflow-auto">
                <table className="w-full border-collapse text-left text-xs">
                  <thead className="sticky top-0 z-10 bg-surface text-muted">
                    <tr>
                      <th className="w-10 border-b border-border px-3 py-2 font-medium">
                        포함
                      </th>
                      <th className="border-b border-border px-3 py-2 font-medium">번호</th>
                      <th className="border-b border-border px-3 py-2 font-medium">
                        이미지
                      </th>
                      <th className="border-b border-border px-3 py-2 font-medium">이름</th>
                      <th className="border-b border-border px-3 py-2 font-medium">alt</th>
                      <th className="border-b border-border px-3 py-2 font-medium">형식</th>
                      <th className="border-b border-border px-3 py-2 font-medium">
                        크기 / 제한
                      </th>
                      <th className="border-b border-border px-3 py-2 font-medium">상태</th>
                      <th className="border-b border-border px-3 py-2 font-medium">작업</th>
                    </tr>
                  </thead>
                  <tbody>
                    {filteredItems.map((item) => (
                      <ExportRow
                        exportDirectory={exportResult?.exportDirectory ?? null}
                        item={item}
                        key={item.pieceId}
                        result={validation}
                        disabled={isBusy}
                        selected={selectedPieceIds.has(item.pieceId)}
                        canRerunExport={Boolean(exportResult?.exportDirectory && item.included)}
                        onEditIcon={setEditingIconId}
                        onIncludedChange={setPieceIncluded}
                        onOptimize={(nextItem) => {
                          void handleOpenOptimization(nextItem);
                        }}
                        onOpenPath={openExportPath}
                        onRerunExport={() => {
                          void handleExportPieces([item.pieceId]);
                        }}
                        onSelect={handleSelectItem}
                      />
                    ))}
                  </tbody>
                </table>
                {filteredItems.length === 0 ? (
                  <p className="px-3 py-8 text-center text-sm text-muted">
                    현재 필터에 해당하는 항목이 없습니다.
                  </p>
                ) : null}
              </div>
            </section>

            <IssuePanel
              exportResult={exportResult}
              item={selectedItem}
              issues={selectedIssues}
              isBusy={isBusy}
              problemNumbers={problemNumbers}
              onEditIcon={setEditingIconId}
              onOptimize={(nextItem) => {
                void handleOpenOptimization(nextItem);
              }}
              onOpenPath={openExportPath}
              onRerunExport={() => {
                if (selectedItem) {
                  void handleExportPieces([selectedItem.pieceId]);
                }
              }}
              onSetFilter={setFilter}
            />
          </div>

          <div className="min-h-[24px]">
            {isLoading ? (
              <p className="text-sm text-muted">내보내기 작업공간을 불러오는 중입니다.</p>
            ) : null}
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
        </main>
      </div>
      {optimizationItem ? (
        <OptimizationPanel
          errorMessage={optimizationError}
          isBusy={isOptimizing}
          item={optimizationItem}
          result={optimizationResult}
          onApply={(candidate) => {
            void handleApplyOptimization(candidate);
          }}
          onClear={() => {
            void handleClearOptimization();
          }}
          onClose={() => {
            if (!isOptimizing) {
              setOptimizationItem(null);
              setOptimizationResult(null);
              setOptimizationError(null);
            }
          }}
        />
      ) : null}
      {editingIconId ? (
        <div className="absolute inset-0 z-[55] flex justify-end bg-black/20">
          <EditorPanel
            collection={collection}
            iconId={editingIconId}
            onClose={() => setEditingIconId(null)}
            onIconUpdated={handleEditedIcon}
          />
        </div>
      ) : null}
    </div>
  );
}

function ExportSelectionToolbar({
  allCount,
  canOptimizeSelected,
  canRerunSelected,
  isBusy,
  selectedCount,
  selectedIncludedCount,
  selectedOversizedCount,
  visibleCount,
  onClearSelection,
  onExcludeAll,
  onIncludeAll,
  onOptimizeSelected,
  onRerunSelected,
  onSelectVisible,
  onToggleSelectedIncluded,
}: {
  allCount: number;
  canOptimizeSelected: boolean;
  canRerunSelected: boolean;
  isBusy: boolean;
  selectedCount: number;
  selectedIncludedCount: number;
  selectedOversizedCount: number;
  visibleCount: number;
  onClearSelection: () => void;
  onExcludeAll: () => void;
  onIncludeAll: () => void;
  onOptimizeSelected: () => void;
  onRerunSelected: () => void;
  onSelectVisible: () => void;
  onToggleSelectedIncluded: () => void;
}) {
  const selectedIncludeAction =
    selectedIncludedCount > 0 ? "선택 항목 제외" : "선택 항목 포함";

  return (
    <section className="flex max-h-28 select-none flex-wrap items-center justify-between gap-2 overflow-auto rounded-md border border-border bg-surface px-3 py-2">
      <div className="text-sm">
        <span className="font-semibold">선택 {selectedCount}개</span>
        <span className="ml-2 text-xs text-muted">
          화면 {visibleCount}개 · 전체 {allCount}개
        </span>
        <span className="ml-2 text-xs text-muted">Shift 범위 · Ctrl 개별 선택</span>
      </div>
      <div className="flex flex-wrap gap-1">
        <ToolbarButton disabled={isBusy || visibleCount === 0} onClick={onSelectVisible}>
          보이는 항목 선택
        </ToolbarButton>
        <ToolbarButton disabled={isBusy || selectedCount === 0} onClick={onClearSelection}>
          단순 선택 비우기
        </ToolbarButton>
        <ToolbarButton disabled={isBusy || allCount === 0} onClick={onIncludeAll}>
          전체 내보내기 포함
        </ToolbarButton>
        <ToolbarButton disabled={isBusy || allCount === 0} onClick={onExcludeAll}>
          전체 내보내기 제외
        </ToolbarButton>
        <ToolbarButton
          active={selectedCount > 0}
          disabled={isBusy || selectedCount === 0}
          onClick={onToggleSelectedIncluded}
        >
          {selectedIncludeAction}
        </ToolbarButton>
        <ToolbarButton disabled={!canOptimizeSelected} onClick={onOptimizeSelected}>
          선택 항목 용량 압축
          {selectedOversizedCount > 0 ? ` (${selectedOversizedCount})` : ""}
        </ToolbarButton>
        <ToolbarButton disabled={!canRerunSelected} onClick={onRerunSelected}>
          선택 항목 다시 내보내기
        </ToolbarButton>
      </div>
    </section>
  );
}

function ToolbarButton({
  active = false,
  children,
  disabled,
  onClick,
}: {
  active?: boolean;
  children: ReactNode;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={cn(
        "rounded border px-2 py-1 text-xs font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted",
        active && !disabled
          ? "border-focus bg-selected text-focus hover:bg-selected/80"
          : "border-border bg-white hover:bg-menu-hover",
      )}
      disabled={disabled}
      type="button"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function SummaryStrip({
  result,
  summary,
}: {
  result: ExportCollectionResult | null;
  summary: ReturnType<typeof summarizeExportWorkspace>;
}) {
  const cards = result
    ? [
        ["생성 성공", summary.success],
        ["업로드 가능", summary.uploadReady],
        ["경고", summary.warnings],
        ["업로드 불가", summary.notUploadReady],
        ["생성 실패", summary.failed],
      ]
    : [
        ["포함", summary.included],
        ["제외", summary.excluded],
        ["업로드 가능", summary.uploadReady],
        ["경고", summary.warnings],
        ["업로드 불가", summary.notUploadReady],
      ];

  return (
    <section className="grid gap-2 md:grid-cols-5">
      {cards.map(([label, value]) => (
        <div className="rounded-md border border-border bg-surface px-3 py-2" key={label}>
          <p className="text-xs text-muted">{label}</p>
          <p className="mt-1 text-lg font-semibold tabular-nums">{value}</p>
        </div>
      ))}
    </section>
  );
}

function PaneHeader({ count, title }: { count: number; title: string }) {
  return (
    <header className="flex items-center justify-between border-b border-border px-3 py-2">
      <h3 className="text-sm font-semibold tracking-normal">{title}</h3>
      <span className="text-xs text-muted">{count}개</span>
    </header>
  );
}

function SourceCard({
  item,
  selected,
  onSelect,
  onToggleIncluded,
}: {
  item: ExportPlanItem;
  selected: boolean;
  onSelect: (event: MouseEvent, item: ExportPlanItem) => void;
  onToggleIncluded: (pieceId: string, included: boolean) => void;
}) {
  return (
    <button
      aria-pressed={selected}
      className={cn(
        "flex min-w-0 select-none flex-col gap-2 rounded-md border border-border bg-white p-2 text-left hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
        selected && "border-focus bg-selected",
        !item.included && "opacity-55",
      )}
      type="button"
      onClick={(event) => onSelect(event, item)}
      onDoubleClick={(event) => {
        event.preventDefault();
        onToggleIncluded(item.pieceId, !item.included);
      }}
    >
      <PreviewImage src={item.sourcePreviewUrl} />
      <span className="truncate text-xs font-medium">{item.displayName}</span>
      <span className="truncate text-[11px] text-muted">
        {pieceRoleLabel(item.pieceRole)} · alt {item.altText || "-"}
      </span>
    </button>
  );
}

function ExportRow({
  canRerunExport,
  disabled,
  exportDirectory,
  item,
  result,
  selected,
  onEditIcon,
  onIncludedChange,
  onOptimize,
  onOpenPath,
  onRerunExport,
  onSelect,
}: {
  canRerunExport: boolean;
  disabled: boolean;
  exportDirectory: string | null;
  item: ExportPlanItem;
  result: ExportValidationResult | null;
  selected: boolean;
  onEditIcon: (iconId: string) => void;
  onIncludedChange: (pieceId: string, included: boolean) => void;
  onOptimize: (item: ExportPlanItem) => void;
  onOpenPath: (path: string) => Promise<void>;
  onRerunExport: () => void;
  onSelect: (event: MouseEvent, item: ExportPlanItem) => void;
}) {
  const issues = issuesForItem(result, item);
  const isOversized = hasOversizedIssue(result, item);
  const exportPath =
    item.exportPath ?? exportFilePathFromDirectory(exportDirectory, item.fileName);

  return (
    <tr
      aria-selected={selected}
      className={cn(
        "cursor-default select-none border-b border-border/70",
        selected ? "bg-selected odd:bg-selected" : "odd:bg-canvas",
        !item.included && "text-muted opacity-70",
      )}
      data-testid="export-result-row"
      tabIndex={0}
      onClick={(event) => onSelect(event, item)}
    >
      <td className="px-3 py-2">
        <input
          aria-label={`${item.displayName} 내보내기 포함`}
          checked={item.included}
          disabled={disabled}
          type="checkbox"
          onChange={(event) => onIncludedChange(item.pieceId, event.currentTarget.checked)}
          onClick={(event) => event.stopPropagation()}
        />
      </td>
      <td className="px-3 py-2 tabular-nums">{formatExportIndex(item)}</td>
      <td className="px-3 py-2">
        <div className="h-14 w-14 overflow-hidden rounded border border-border bg-preview">
          <PreviewImage compact src={item.exportPath ?? item.sourcePreviewUrl} />
        </div>
      </td>
      <td className="max-w-[150px] px-3 py-2">
        <p className="truncate font-medium">{item.displayName}</p>
        <p className="truncate text-[11px] text-muted">{pieceRoleLabel(item.pieceRole)}</p>
      </td>
      <td className="max-w-[88px] truncate px-3 py-2">{item.altText || "-"}</td>
      <td className="px-3 py-2 uppercase">{item.outputFormat}</td>
      <td className="whitespace-nowrap px-3 py-2">
        {item.byteSize ? formatBytes(item.byteSize) : "-"} / {formatBytes(item.limitBytes)}
      </td>
      <td className="px-3 py-2">
        <StatusBadge status={item.status} />
      </td>
      <td className="px-3 py-2">
        <div className="flex flex-wrap gap-1">
          <button
            className="rounded border border-border bg-white px-2 py-1 font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
            disabled={disabled}
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              onEditIcon(item.iconId);
            }}
          >
            수정
          </button>
          {exportPath ? (
            <button
              className="rounded border border-border bg-white px-2 py-1 font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={disabled}
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                void onOpenPath(exportPath);
              }}
            >
              파일
            </button>
          ) : null}
          {canRerunExport ? (
            <button
              className="rounded border border-border bg-white px-2 py-1 font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={disabled}
              title="현재 포함 설정으로 다시 내보냅니다."
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                onRerunExport();
              }}
            >
              다시 내보내기
            </button>
          ) : null}
          {isOversized ? (
            <button
              className="rounded border border-amber-300 bg-amber-50 px-2 py-1 font-medium text-amber-900 hover:bg-amber-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
              disabled={disabled}
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                onOptimize(item);
              }}
            >
              자동 최적화
            </button>
          ) : null}
        </div>
        {issues.length > 0 ? (
          <p className="mt-1 line-clamp-2 max-w-[220px] text-[11px] text-muted">
            {issueSummary(issues)}
          </p>
        ) : null}
      </td>
    </tr>
  );
}

function IssuePanel({
  exportResult,
  isBusy,
  item,
  issues,
  problemNumbers,
  onEditIcon,
  onOptimize,
  onOpenPath,
  onRerunExport,
  onSetFilter,
}: {
  exportResult: ExportCollectionResult | null;
  isBusy: boolean;
  item: ExportPlanItem | null;
  issues: ReturnType<typeof issuesForItem>;
  problemNumbers: string[];
  onEditIcon: (iconId: string) => void;
  onOptimize: (item: ExportPlanItem) => void;
  onOpenPath: (path: string) => Promise<void>;
  onRerunExport: () => void;
  onSetFilter: (filter: ExportWorkspaceFilter) => void;
}) {
  return (
    <aside className="flex min-h-0 flex-col gap-3 overflow-auto rounded-md border border-border bg-surface p-3">
      <section>
        <h3 className="text-sm font-semibold tracking-normal">선택 항목</h3>
        {item ? (
          <div className="mt-3 flex flex-col gap-3">
            <PreviewImage src={item.exportPath ?? item.sourcePreviewUrl} />
            <div className="text-sm">
              <p className="font-medium">{item.displayName}</p>
              <p className="mt-1 text-xs text-muted">
                {formatExportIndex(item)} · {item.fileName || "파일명 없음"} · alt{" "}
                {item.altText || "-"}
              </p>
            </div>
            <StatusBadge status={item.status} />
            {issues.length > 0 ? (
              <div className="flex flex-col gap-2">
                {issues.map((issue) => (
                  <p
                    className={cn(
                      "rounded-md border px-3 py-2 text-xs",
                      issue.severity === "warning"
                        ? "border-amber-200 bg-amber-50 text-amber-900"
                        : "border-red-200 bg-red-50 text-danger",
                    )}
                    key={`${issue.code}-${issue.pieceId}-${issue.message}`}
                  >
                    {issue.message}
                  </p>
                ))}
              </div>
            ) : (
              <p className="rounded-md border border-border bg-canvas px-3 py-2 text-xs text-muted">
                현재 항목에는 표시할 문제가 없습니다.
              </p>
            )}
            {hasOversizedIssueFromIssues(issues) ? (
              <button
                className="inline-flex items-center justify-center rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-sm font-medium text-amber-900 hover:bg-amber-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
                disabled={isBusy}
                type="button"
                onClick={() => onOptimize(item)}
              >
                자동 최적화
              </button>
            ) : null}
            <button
              className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={isBusy}
              type="button"
              onClick={() => onEditIcon(item.iconId)}
            >
              <Settings2 aria-hidden="true" className="size-4" />
              편집기에서 수정
            </button>
          </div>
        ) : (
          <p className="mt-3 text-sm text-muted">항목을 선택하세요.</p>
        )}
      </section>

      {exportResult ? (
        <section className="mt-auto flex flex-col gap-2 border-t border-border pt-3">
          <h3 className="text-sm font-semibold tracking-normal">완료 보고서</h3>
          {problemNumbers.length > 0 ? (
            <p className="text-xs text-muted">
              문제 번호: {problemNumbers.slice(0, 12).join(", ")}
              {problemNumbers.length > 12 ? " ..." : ""}
            </p>
          ) : (
            <p className="text-xs text-muted">문제 항목이 없습니다.</p>
          )}
          <div className="grid gap-2">
            <button
              className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={isBusy || !item?.included}
              title="선택 항목만 기존 export 폴더에 다시 씁니다."
              type="button"
              onClick={onRerunExport}
            >
              <RefreshCw aria-hidden="true" className="size-4" />
              다시 내보내기
            </button>
            {exportResult.exportDirectory ? (
              <OpenPathButton
                icon="folder"
                label="export 폴더 열기"
                path={exportResult.exportDirectory}
                onOpenPath={onOpenPath}
              />
            ) : null}
            {exportResult.altTxtPath ? (
              <OpenPathButton
                icon="text"
                label="alts.txt 열기"
                path={exportResult.altTxtPath}
                onOpenPath={onOpenPath}
              />
            ) : null}
            {exportResult.reportTxtPath ? (
              <OpenPathButton
                icon="text"
                label="report 열기"
                path={exportResult.reportTxtPath}
                onOpenPath={onOpenPath}
              />
            ) : null}
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              className="rounded border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="button"
              onClick={() => onSetFilter("not_upload_ready")}
            >
              업로드 불가 보기
            </button>
            <button
              className="rounded border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="button"
              onClick={() => onSetFilter("failed")}
            >
              실패 보기
            </button>
            <button
              className="rounded border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="button"
              onClick={() => onSetFilter("warnings")}
            >
              경고 보기
            </button>
          </div>
        </section>
      ) : null}
    </aside>
  );
}

function OptimizationPanel({
  errorMessage,
  isBusy,
  item,
  result,
  onApply,
  onClear,
  onClose,
}: {
  errorMessage: string | null;
  isBusy: boolean;
  item: ExportPlanItem;
  result: OptimizationResult | null;
  onApply: (candidate: OptimizationCandidate) => void;
  onClear: () => void;
  onClose: () => void;
}) {
  const candidates = result?.candidates ?? [];

  return (
    <div className="absolute inset-0 z-[60] flex items-center justify-center bg-black/30 p-6">
      <section className="flex max-h-full w-full max-w-5xl flex-col rounded-md border border-border bg-surface shadow-xl">
        <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
          <div className="min-w-0">
            <h3 className="truncate text-base font-semibold tracking-normal">용량 최적화</h3>
            <p className="mt-1 truncate text-xs text-muted">
              {formatExportIndex(item)} · {item.displayName} · {item.outputFormat.toUpperCase()}
            </p>
          </div>
          <button
            aria-label="용량 최적화 닫기"
            className="inline-flex size-8 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
            disabled={isBusy}
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </header>

        <div className="grid min-h-0 gap-4 overflow-auto p-4 lg:grid-cols-[280px_minmax(0,1fr)]">
          <aside className="flex flex-col gap-3">
            <div className="rounded-md border border-border bg-canvas p-3">
              <p className="text-xs text-muted">현재 크기</p>
              <p className="mt-1 text-lg font-semibold">
                {item.byteSize ? formatBytes(item.byteSize) : "-"} /{" "}
                {formatBytes(item.limitBytes)}
              </p>
              {result ? (
                <p className="mt-2 text-xs text-muted">
                  {result.analysis.explanationForUser}
                </p>
              ) : (
                <p className="mt-2 text-xs text-muted">
                  후보 파일을 실제로 생성하고 측정하는 중입니다.
                </p>
              )}
            </div>
            <div className="rounded-md border border-border bg-white p-3">
              <PreviewImage src={item.exportPath ?? item.sourcePreviewUrl} />
              <p className="mt-2 text-xs text-muted">원본 파일은 보존됩니다.</p>
              <p className="mt-1 text-xs text-muted">적용 후 export 검증을 다시 실행합니다.</p>
            </div>
            <button
              className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={isBusy}
              type="button"
              onClick={onClear}
            >
              원본 사용
            </button>
          </aside>

          <div className="flex min-h-0 flex-col gap-3">
            {isBusy ? (
              <div className="rounded-md border border-border bg-canvas px-3 py-2">
                <div className="flex items-center justify-between text-xs text-muted">
                  <span>최적화 후보 생성 중</span>
                  <span>실제 파일 측정</span>
                </div>
                <div className="mt-2 h-2 overflow-hidden rounded-full bg-preview">
                  <div className="h-full w-1/2 animate-pulse rounded-full bg-accent" />
                </div>
              </div>
            ) : null}

            {errorMessage ? (
              <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-danger">
                {errorMessage}
              </p>
            ) : null}

            {result?.alreadyPasses ? (
              <p className="rounded-md border border-border bg-canvas px-3 py-2 text-sm text-muted">
                {result.message}
              </p>
            ) : null}

            <div className="grid gap-3 md:grid-cols-3">
              {candidates.map((candidate) => (
                <OptimizationCandidateCard
                  candidate={candidate}
                  disabled={isBusy}
                  key={candidate.id}
                  onApply={onApply}
                />
              ))}
            </div>

            {result && candidates.length === 0 && !result.alreadyPasses ? (
              <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900">
                <p>{result.message}</p>
                {result.fallbackSuggestions.length > 0 ? (
                  <p className="mt-1 text-xs">
                    제안: {result.fallbackSuggestions.join(", ")}
                  </p>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>
      </section>
    </div>
  );
}

function OptimizationCandidateCard({
  candidate,
  disabled,
  onApply,
}: {
  candidate: OptimizationCandidate;
  disabled: boolean;
  onApply: (candidate: OptimizationCandidate) => void;
}) {
  return (
    <article className="flex min-w-0 flex-col gap-2 rounded-md border border-border bg-white p-3">
      <PreviewImage src={candidate.previewUrl || candidate.path} />
      <div className="flex items-center justify-between gap-2">
        <h4 className="truncate text-sm font-semibold tracking-normal">
          {candidatePresetLabel(candidate.preset)}
        </h4>
        <span
          className={cn(
            "rounded px-2 py-1 text-[11px] font-medium",
            candidate.passes ? "bg-selected text-foreground" : "bg-orange-50 text-orange-900",
          )}
        >
          {candidate.passes ? "통과" : "제한 초과"}
        </span>
      </div>
      <dl className="grid gap-1 text-xs text-muted">
        <div className="flex justify-between gap-2">
          <dt>크기</dt>
          <dd>
            {formatBytes(candidate.measuredByteSize)} / {formatBytes(candidate.targetMaxBytes)}
          </dd>
        </div>
        {candidate.frameCount !== null ? (
          <div className="flex justify-between gap-2">
            <dt>프레임</dt>
            <dd>
              {candidate.originalFrameCount ?? "-"} → {candidate.frameCount}
            </dd>
          </div>
        ) : null}
        {candidate.quality !== null ? (
          <div className="flex justify-between gap-2">
            <dt>품질</dt>
            <dd>{candidate.quality}</dd>
          </div>
        ) : null}
        <div className="flex justify-between gap-2">
          <dt>손상</dt>
          <dd>{candidate.qualityImpact}</dd>
        </div>
      </dl>
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

function StatusBadge({ status }: { status: ExportPlanItem["status"] }) {
  const tone = statusTone(status);
  const Icon = tone === "ok" ? CheckCircle2 : AlertTriangle;

  return (
    <span
      className={cn(
        "inline-flex w-fit items-center gap-1 rounded px-2 py-1 text-xs font-medium",
        tone === "ok" && "bg-selected text-foreground",
        tone === "warning" && "bg-amber-50 text-amber-900",
        tone === "danger" && "bg-orange-50 text-orange-900",
        tone === "error" && "bg-red-50 text-danger",
        tone === "muted" && "bg-slate-100 text-muted",
        tone === "neutral" && "bg-canvas text-muted",
      )}
    >
      <Icon aria-hidden="true" className="size-3.5" />
      {statusLabel(status)}
    </span>
  );
}

function PreviewImage({
  compact = false,
  src,
}: {
  compact?: boolean;
  src: string | null;
}) {
  const assetUrl = filePathToAssetUrl(src);

  if (!assetUrl) {
    return (
      <span
        className={cn(
          "flex items-center justify-center rounded border border-border bg-preview text-xs text-muted",
          compact ? "size-full" : "aspect-square w-full",
        )}
      >
        이미지 없음
      </span>
    );
  }

  return (
    <img
      alt=""
      className={cn(
        "rounded border border-border bg-preview object-contain",
        compact ? "size-full" : "aspect-square w-full",
      )}
      draggable={false}
      src={assetUrl}
      onDragStart={(event) => event.preventDefault()}
    />
  );
}

function NumberField({
  label,
  min,
  value,
  onChange,
}: {
  label: string;
  min: number;
  value: number;
  onChange: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  return (
    <label className="flex items-center gap-2 text-xs font-medium text-muted">
      {label}
      <input
        className="h-8 w-24 select-text rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        min={min}
        type="number"
        value={draft}
        onBlur={() => {
          const parsed = Number.parseInt(draft, 10);
          if (!Number.isFinite(parsed) || parsed < min) {
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
          if (Number.isFinite(parsed) && parsed >= min) {
            onChange(parsed);
          }
        }}
      />
    </label>
  );
}

function MegabytesField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  const [draft, setDraft] = useState(() => bytesToMegabytesInput(value));

  useEffect(() => {
    setDraft(bytesToMegabytesInput(value));
  }, [value]);

  return (
    <label className="flex items-center gap-2 text-xs font-medium text-muted">
      {label}
      <span className="inline-flex h-8 items-center rounded-md border border-border bg-white focus-within:outline focus-within:outline-2 focus-within:outline-focus">
        <input
          aria-label={`${label} MB`}
          className="h-full w-20 min-w-0 select-text bg-transparent px-2 text-sm text-foreground outline-none"
          inputMode="decimal"
          type="text"
          value={draft}
          onBlur={() => {
            if (megabytesInputToBytes(draft) === null) {
              setDraft(bytesToMegabytesInput(value));
            }
          }}
          onChange={(event) => {
            const nextValue = event.currentTarget.value;
            setDraft(nextValue);
            const nextBytes = megabytesInputToBytes(nextValue);
            if (nextBytes !== null) {
              onChange(nextBytes);
            }
          }}
        />
        <span className="border-l border-border px-2 text-xs text-muted">MB</span>
      </span>
    </label>
  );
}

function CheckboxField({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="flex h-8 items-center gap-2 rounded-md border border-border bg-white px-2 text-xs font-medium text-foreground">
      <input
        checked={checked}
        className="size-4"
        type="checkbox"
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      {label}
    </label>
  );
}

function OpenPathButton({
  icon,
  label,
  path,
  onOpenPath,
}: {
  icon: "folder" | "text";
  label: string;
  path: string;
  onOpenPath: (path: string) => Promise<void>;
}) {
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const Icon = icon === "folder" ? FolderOpen : FileText;

  return (
    <div className="flex flex-col gap-1">
      <button
        className="inline-flex items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        type="button"
        onClick={() => {
          setErrorMessage(null);
          void onOpenPath(path).catch((error) => {
            setErrorMessage(getCommandErrorMessage(error));
          });
        }}
      >
        <Icon aria-hidden="true" className="size-4" />
        {label}
        <ExternalLink aria-hidden="true" className="size-4" />
      </button>
      {errorMessage ? <span className="text-xs text-danger">{errorMessage}</span> : null}
    </div>
  );
}

function preferredProfile(profiles: ExportProfile[]): ExportProfile {
  return (
    profiles.find((profile) => profile.profileType === "dcinside") ??
    profiles[0] ?? {
      id: "",
      collectionId: "",
      name: "DCInside",
      profileType: "dcinside",
      targetFormat: "png",
      targetCellWidth: 200,
      targetCellHeight: 200,
      previewWidth: 100,
      previewHeight: 100,
      maxBytes: 2_097_152,
      allowedFormats: ["jpg", "png", "gif"],
      filenameMode: "sequence",
      includeAltTxt: true,
      strictWarnings: false,
      createdAt: "",
      updatedAt: "",
    }
  );
}

function draftFromProfile(
  profile: ExportProfile,
  collection: CollectionSummary,
): ExportDraft {
  return {
    profileId: profile.id,
    targetFormat: profile.targetFormat,
    targetCellWidth: profile.targetCellWidth || collection.defaultCellWidth,
    targetCellHeight: profile.targetCellHeight || collection.defaultCellHeight,
    maxBytes: profile.maxBytes || collection.maxBytes,
    filenameMode: profile.filenameMode,
    includeAltTxt: profile.includeAltTxt,
    strictWarnings: profile.strictWarnings,
    outputDirectory: "",
    openFolderAfterExport: true,
    openAltTxtAfterExport: profile.includeAltTxt,
    resizeFilter: "lanczos3",
  };
}

function payloadFromDraft(
  draft: ExportDraft,
  excludedPieceIds: Set<string>,
): ExportRequestPayload {
  return {
    profileId: draft.profileId,
    targetFormat: draft.targetFormat,
    targetCellWidth: normalizedNumber(draft.targetCellWidth, 200),
    targetCellHeight: normalizedNumber(draft.targetCellHeight, 200),
    maxBytes: normalizedNumber(draft.maxBytes, 2_097_152),
    filenameMode: draft.filenameMode,
    includeAltTxt: draft.includeAltTxt,
    strictWarnings: draft.strictWarnings,
    outputDirectory: draft.outputDirectory.trim() ? draft.outputDirectory.trim() : null,
    openFolderAfterExport: draft.openFolderAfterExport,
    openAltTxtAfterExport: draft.includeAltTxt && draft.openAltTxtAfterExport,
    excludedPieceIds: Array.from(excludedPieceIds),
    resizeFilter: draft.resizeFilter,
  };
}

function normalizedNumber(value: number, fallback: number) {
  if (!Number.isFinite(value) || value < 1) {
    return fallback;
  }

  return Math.round(value);
}

function profileLabel(profile: ExportProfile) {
  return profile.profileType === "dcinside" ? "DCInside" : "Custom";
}

function pieceRoleLabel(role: ExportPlanItem["pieceRole"]) {
  switch (role) {
    case "left":
      return "왼쪽";
    case "right":
      return "오른쪽";
    case "top":
      return "위";
    case "bottom":
      return "아래";
    case "single":
    default:
      return "단일";
  }
}

function exportFilePathFromDirectory(exportDirectory: string | null, fileName: string) {
  if (!exportDirectory || !fileName) {
    return null;
  }

  const normalizedDirectory = exportDirectory.replace(/[\\/]+$/, "");
  const separator = normalizedDirectory.includes("\\") ? "\\" : "/";
  return `${normalizedDirectory}${separator}files${separator}${fileName}`;
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
    default:
      return "사용자 후보";
  }
}

function chooseBatchCandidate(candidates: OptimizationCandidate[]) {
  const passingBalanced = candidates.find(
    (candidate) => candidate.preset === "balanced" && candidate.passes,
  );
  if (passingBalanced) {
    return passingBalanced;
  }

  const passing = candidates.find((candidate) => candidate.passes);
  if (passing) {
    return passing;
  }

  return [...candidates].sort(
    (left, right) => left.measuredByteSize - right.measuredByteSize,
  )[0];
}

function hasOversizedIssueFromIssues(issues: ReturnType<typeof issuesForItem>) {
  return issues.some((issue) => issue.code === "max_bytes");
}

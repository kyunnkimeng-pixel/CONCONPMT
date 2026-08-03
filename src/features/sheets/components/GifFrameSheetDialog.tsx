import { useEffect, useMemo, useRef, useState } from "react";
import type { DragEvent, ReactNode } from "react";

import {
  NovelAiWebGuide,
  type NovelAiPromptCopyOutcome,
} from "@/features/ai-web/components/NovelAiWebGuide";
import {
  needsNovelAiEnglishInputHint,
  normalizeNovelAiPromptInput,
} from "@/features/ai-web/novelai-web-model";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import { copyAiHandoffPrompt } from "@/features/editor/ai-provider-model";
import { listExportProfiles, openExportPath } from "@/features/export/api";
import type { ExportProfile } from "@/features/export/types";
import {
  analyzeGifFrameSheetExport,
  exportGifFrameSheet,
  reimportGifFrameSheet,
  revealGifFrameSheetPage,
  startGifFrameSheetPageDrag,
  validateGifFrameSheetReimport,
  type GifFrameManifestSource,
} from "@/features/sheets/api";
import { SheetGridPresetSelect } from "@/features/sheets/components/SheetGridPresetSelect";
import {
  assignGifFrameFileToSlot,
  autoAssignGifFrameFiles,
  classifyGifFrameReimportFiles,
  gifFrameResultNeedsOpaqueWarning,
  mappedGifFrameFiles,
  readGifFrameReimportPageSlots,
  type GifFrameReimportPageSlot,
  type GifFrameTransparencyMode,
} from "@/features/sheets/gif-frame-reimport-model";
import {
  applyPresetToGifFrameSettings,
  defaultGifFrameSheetSettings,
  estimateGifFrameSheetPages,
  presetInputFromGifFrameSettings,
} from "@/features/sheets/sheet-ui-model";
import type {
  GifFrameSheetExportAnalysis,
  GifFrameSheetExportResult,
  GifFrameSheetReimportResult,
  GifFrameSheetReimportValidation,
  GifFrameSheetSettings,
  SheetBackground,
} from "@/features/sheets/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import { getCommandErrorMessage } from "@/lib/tauri";
import { useModalFocus } from "@/lib/use-modal-focus";

type GifFrameSheetMode = "export" | "reimport";
export type GifAiWebResource = "gemini_ai_studio" | "novelai_app";

export interface GifFrameHandoffSession {
  manifestPath: string;
  pageSlots: GifFrameReimportPageSlot[];
}

interface GifFrameSheetDialogProps {
  aiWebWorkflow?: boolean;
  collection: CollectionSummary;
  icon: IconSummary;
  mode: GifFrameSheetMode;
  onClose: () => void;
  onOpenAiSite?: (resource: GifAiWebResource) => Promise<void>;
  onVariantCreated: () => Promise<void>;
}

export function GifFrameSheetDialog({
  aiWebWorkflow = false,
  collection,
  icon,
  mode,
  onClose,
  onOpenAiSite,
  onVariantCreated,
}: GifFrameSheetDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);
  useModalFocus(dialogRef, onClose);
  const defaultCellWidth = icon.cellWidthOverride ?? collection.defaultCellWidth;
  const defaultCellHeight = icon.cellHeightOverride ?? collection.defaultCellHeight;
  const [activeMode, setActiveMode] = useState<GifFrameSheetMode>(mode);
  const [handoffSession, setHandoffSession] =
    useState<GifFrameHandoffSession | null>(null);
  const [transparencyMode, setTransparencyMode] =
    useState<GifFrameTransparencyMode>("preserve_alpha");
  const [settings, setSettings] = useState<GifFrameSheetSettings>(() => {
    const defaults = defaultGifFrameSheetSettings(
      defaultCellWidth,
      defaultCellHeight,
    );
    return aiWebWorkflow
      ? {
          ...defaults,
          background: "transparent",
          includeCleanSheet: true,
          includeManifest: true,
        }
      : defaults;
  });

  return (
    <div className={`fixed inset-0 ${aiWebWorkflow ? "z-[110]" : "z-50"} flex items-center justify-center bg-slate-900/35 px-4 py-5`}>
      <section
        ref={dialogRef}
        aria-labelledby="gif-frame-sheet-dialog-title"
        aria-modal="true"
        className="flex max-h-[calc(100vh-40px)] w-[min(1040px,100%)] flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-xl"
        data-testid="gif-frame-sheet-dialog"
        role="dialog"
        tabIndex={-1}
      >
        <header className="flex items-start justify-between gap-4 border-b border-border px-5 py-4">
          <div className="min-w-0">
            <h2
              className="text-lg font-semibold tracking-normal"
              id="gif-frame-sheet-dialog-title"
            >
              {activeMode === "export" ? "GIF 프레임 시트로 내보내기" : "GIF 프레임 시트 다시 가져오기"}
            </h2>
            <p className="mt-1 text-sm text-muted">
              {icon.displayName} · GIF · 원본은 그대로 유지되며 결과는 내보내기용 GIF 처리 버전으로 저장됩니다.
            </p>
            {aiWebWorkflow ? (
              <p className="mt-1 text-xs leading-5 text-muted" data-testid="gif-ai-web-safety-note">
                수동 웹 AI용 PNG 프레임 시트 왕복입니다. 원본 GIF, 프레임별 timing, 재생 순서와
                loop는 manifest에서 보존·복원하며 GIF 자체를 AI API로 직접 호출하지 않습니다.
              </p>
            ) : null}
          </div>
          <button
            className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            type="button"
            onClick={onClose}
          >
            닫기
          </button>
        </header>

        <div className="border-b border-border px-5 py-3">
          <div className="inline-flex rounded-md border border-border bg-white p-1">
            <TabButton selected={activeMode === "export"} onClick={() => setActiveMode("export")}>
              내보내기
            </TabButton>
            <TabButton selected={activeMode === "reimport"} onClick={() => setActiveMode("reimport")}>
              다시 가져오기
            </TabButton>
          </div>
        </div>

        <div className="min-h-0 overflow-y-auto px-5 py-4">
          {activeMode === "export" ? (
            <GifFrameExportPanel
              aiWebWorkflow={aiWebWorkflow}
              collectionId={collection.id}
              icon={icon}
              settings={settings}
              transparencyMode={transparencyMode}
              onContinueToReimport={() => setActiveMode("reimport")}
              onHandoffSessionChange={setHandoffSession}
              onOpenAiSite={onOpenAiSite}
              onSettingsChange={setSettings}
            />
          ) : (
            <GifFrameReimportPanel
              collection={collection}
              icon={icon}
              retainedSession={handoffSession}
              transparencyMode={transparencyMode}
              onTransparencyModeChange={setTransparencyMode}
              onVariantCreated={onVariantCreated}
            />
          )}
        </div>
      </section>
    </div>
  );
}

export function GifFrameExportPanel({
  aiWebWorkflow = false,
  collectionId,
  icon,
  settings,
  transparencyMode,
  onContinueToReimport,
  onHandoffSessionChange,
  onOpenAiSite,
  onSettingsChange,
}: {
  aiWebWorkflow?: boolean;
  collectionId: string;
  icon: IconSummary;
  settings: GifFrameSheetSettings;
  transparencyMode?: GifFrameTransparencyMode;
  onContinueToReimport?: () => void;
  onHandoffSessionChange?: (session: GifFrameHandoffSession | null) => void;
  onOpenAiSite?: (resource: GifAiWebResource) => Promise<void>;
  onSettingsChange: (settings: GifFrameSheetSettings) => void;
}) {
  const [analysis, setAnalysis] = useState<GifFrameSheetExportAnalysis | null>(null);
  const [result, setResult] = useState<GifFrameSheetExportResult | null>(null);
  const [aiWebPrompt, setAiWebPrompt] = useState<string | null>(null);
  const [aiWebNovelAiPrompt, setAiWebNovelAiPrompt] = useState<string | null>(null);
  const [aiWebExpectedCanvas, setAiWebExpectedCanvas] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isWorking, setIsWorking] = useState(false);
  const previousSettingsRef = useRef(settings);

  useEffect(() => {
    let cancelled = false;
    setErrorMessage(null);
    void analyzeGifFrameSheetExport(icon.id, settings)
      .then((nextAnalysis) => {
        if (!cancelled) {
          setAnalysis(nextAnalysis);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setAnalysis(null);
          setErrorMessage(getCommandErrorMessage(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [icon.id, settings]);

  useEffect(() => {
    const settingsChanged = previousSettingsRef.current !== settings;
    previousSettingsRef.current = settings;
    setResult(null);
    setAiWebPrompt(null);
    setAiWebNovelAiPrompt(null);
    setAiWebExpectedCanvas(null);
    if (settingsChanged) onHandoffSessionChange?.(null);
  }, [onHandoffSessionChange, settings]);

  const estimatedPages = estimateGifFrameSheetPages(analysis?.frameCount ?? 0, settings);

  const updateNumber = (
    field: keyof Pick<
      GifFrameSheetSettings,
      | "frameCellWidth"
      | "frameCellHeight"
      | "columns"
      | "framesPerPage"
      | "gapX"
      | "gapY"
      | "borderX"
      | "borderY"
      | "maxSheetWidth"
      | "maxSheetHeight"
    >,
    value: number,
  ) => {
    onSettingsChange({
      ...settings,
      [field]: Number.isFinite(value) ? Math.max(1, Math.round(value)) : 1,
    });
  };

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
      <div className="grid gap-4">
        <section className="rounded-md border border-border bg-white p-4">
          <h3 className="text-sm font-semibold">프레임 정보</h3>
          <div className="mt-3 grid gap-2 text-sm sm:grid-cols-2 lg:grid-cols-4">
            <Metric label="Frame" value={analysis ? String(analysis.frameCount) : "-"} />
            <Metric label="Duration" value={analysis ? `${analysis.durationMs}ms` : "-"} />
            <Metric label="Loop" value={analysis ? loopLabel(analysis.loopMode, analysis.loopCount) : "-"} />
            <Metric label="Pages" value={analysis ? `${analysis.pageCount} (${estimatedPages} 예상)` : "-"} />
          </div>
          {analysis?.warnings.length ? <MessageList tone="warning" messages={analysis.warnings} /> : null}
          {errorMessage ? <MessageList tone="error" messages={[errorMessage]} /> : null}
        </section>

        <SheetGridPresetSelect
          collectionId={collectionId}
          compatibleKinds={["gif_frame_export", "static_import_export", "static_export"]}
          currentSummary={`${settings.frameCellWidth}x${settings.frameCellHeight} · ${settings.columns}열 · ${settings.framesPerPage ?? "-"} frames/page · gap ${settings.gapX}/${settings.gapY}`}
          saveKindLabel="GIF 프레임 작업시트"
          target="gif_frame"
          buildPresetInput={(name) =>
            presetInputFromGifFrameSettings(name, collectionId, settings)
          }
          onApplyPreset={(preset) => {
            const nextSettings = applyPresetToGifFrameSettings(settings, preset);
            onSettingsChange(
              aiWebWorkflow
                ? {
                    ...nextSettings,
                    background: "transparent",
                    includeCleanSheet: true,
                    includeManifest: true,
                  }
                : nextSettings,
            );
          }}
        />

        <section className="rounded-md border border-border bg-white p-4">
          <h3 className="text-sm font-semibold">프레임 시트 설정</h3>
          {aiWebWorkflow ? (
            <p className="mt-2 text-xs leading-5 text-muted">
              입력 시트는 원본 alpha와 재조립 구조를 보존하도록 transparent clean PNG와
              manifest JSON으로 만듭니다. AI 결과의 불투명 배경 허용 여부는 다시 가져올 때 선택합니다.
            </p>
          ) : null}
          <div className="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
            <NumberField label="Cell W" value={settings.frameCellWidth} onChange={(value) => updateNumber("frameCellWidth", value)} />
            <NumberField label="Cell H" value={settings.frameCellHeight} onChange={(value) => updateNumber("frameCellHeight", value)} />
            <NumberField label="Columns" value={settings.columns} onChange={(value) => updateNumber("columns", value)} />
            <NumberField label="Frames/page" value={settings.framesPerPage ?? 64} onChange={(value) => updateNumber("framesPerPage", value)} />
            <NumberField label="Max W" value={settings.maxSheetWidth} onChange={(value) => updateNumber("maxSheetWidth", value)} />
            <NumberField label="Max H" value={settings.maxSheetHeight} onChange={(value) => updateNumber("maxSheetHeight", value)} />
            <NumberField label="Gap X" value={settings.gapX} onChange={(value) => updateNumber("gapX", value)} />
            <NumberField label="Gap Y" value={settings.gapY} onChange={(value) => updateNumber("gapY", value)} />
            <NumberField label="Border X" value={settings.borderX} onChange={(value) => updateNumber("borderX", value)} />
            <NumberField label="Border Y" value={settings.borderY} onChange={(value) => updateNumber("borderY", value)} />
          </div>
          <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <label className="flex flex-col gap-1 text-xs font-medium text-muted">
              Background
              <select
                className="h-9 rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                disabled={aiWebWorkflow}
                value={settings.background}
                onChange={(event) =>
                  onSettingsChange({
                    ...settings,
                    background: event.currentTarget.value as SheetBackground,
                  })
                }
              >
                <option value="transparent">transparent</option>
                <option value="checker">checker</option>
                <option value="white">white</option>
                <option value="black">black</option>
              </select>
            </label>
            <CheckField disabled={aiWebWorkflow} label="Clean frame sheet PNG" checked={settings.includeCleanSheet} onChange={(checked) => onSettingsChange({ ...settings, includeCleanSheet: checked })} />
            <CheckField label="Guide frame sheet PNG" checked={settings.includeGuideSheet} onChange={(checked) => onSettingsChange({ ...settings, includeGuideSheet: checked })} />
            <CheckField disabled={aiWebWorkflow} label="Manifest JSON" checked={settings.includeManifest} onChange={(checked) => onSettingsChange({ ...settings, includeManifest: checked })} />
            <CheckField label="완료 후 폴더 열기" checked={settings.openOutputFolder} onChange={(checked) => onSettingsChange({ ...settings, openOutputFolder: checked })} />
          </div>
        </section>
      </div>

      <aside className="rounded-md border border-border bg-white p-4">
        <h3 className="text-sm font-semibold">산출물</h3>
        <p className="mt-2 text-sm text-muted">
          clean sheet에는 라벨/grid를 넣지 않습니다. guide sheet에만 번호와 기준선을 표시합니다.
        </p>
        <button
          className="mt-4 w-full rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
          disabled={!analysis || isWorking}
          type="button"
          onClick={() => {
            if (!analysis) return;
            const analysisAtExport = analysis;
            const settingsAtExport: GifFrameSheetSettings = { ...settings };
            setIsWorking(true);
            setResult(null);
            setAiWebPrompt(null);
            setAiWebNovelAiPrompt(null);
            setAiWebExpectedCanvas(null);
            setErrorMessage(null);
            onHandoffSessionChange?.(null);
            void exportGifFrameSheet(icon.id, settingsAtExport)
              .then((nextResult) => {
                setResult(nextResult);
                onHandoffSessionChange?.(
                  nextResult.manifestPath
                    ? {
                        manifestPath: nextResult.manifestPath,
                        pageSlots: gifFrameReimportPageSlotsFromExport(
                          analysisAtExport,
                          nextResult,
                        ),
                      }
                    : null,
                );
                setAiWebPrompt(
                  aiWebWorkflow
                    ? buildGifAiWebPrompt({
                        analysis: analysisAtExport,
                        result: nextResult,
                        settings: settingsAtExport,
                        transparencyMode: transparencyMode ?? "preserve_alpha",
                      })
                    : null,
                );
                setAiWebNovelAiPrompt(
                  aiWebWorkflow
                    ? buildNovelAiGifWebPrompt({
                        analysis: analysisAtExport,
                        result: nextResult,
                        settings: settingsAtExport,
                        transparencyMode: transparencyMode ?? "preserve_alpha",
                      })
                    : null,
                );
                setAiWebExpectedCanvas(
                  aiWebWorkflow
                    ? gifAiPageCanvasContracts({
                        analysis: analysisAtExport,
                        result: nextResult,
                      }).join(", ")
                    : null,
                );
              })
              .catch((error) => setErrorMessage(getCommandErrorMessage(error)))
              .finally(() => setIsWorking(false));
          }}
        >
          {isWorking ? "내보내는 중" : "GIF 프레임 시트 내보내기"}
        </button>
        {result ? (
          <GifFrameExportResultPanel
            aiWebExpectedCanvas={aiWebExpectedCanvas}
            aiWebNovelAiPrompt={aiWebNovelAiPrompt}
            aiWebPrompt={aiWebPrompt}
            result={result}
            onContinueToReimport={onContinueToReimport}
            onOpenAiSite={onOpenAiSite}
            onOpenFolder={(path) =>
              openExportPath(path).catch((error) =>
                setErrorMessage(getCommandErrorMessage(error)),
              )
            }
          />
        ) : null}
      </aside>
    </div>
  );
}

function GifFrameReimportPanel({
  collection,
  icon,
  retainedSession,
  transparencyMode,
  onTransparencyModeChange,
  onVariantCreated,
}: {
  collection: CollectionSummary;
  icon: IconSummary;
  retainedSession: GifFrameHandoffSession | null;
  transparencyMode: GifFrameTransparencyMode;
  onTransparencyModeChange: (mode: GifFrameTransparencyMode) => void;
  onVariantCreated: () => Promise<void>;
}) {
  const manifestLoadGenerationRef = useRef(0);
  const pendingManifestFileRef = useRef<File | null>(null);
  const sheetFilesRef = useRef<File[]>([]);
  const [manualManifestFile, setManualManifestFile] = useState<File | null>(null);
  const [usingManualRecovery, setUsingManualRecovery] = useState(
    () => retainedSession === null,
  );
  const [sheetFiles, setSheetFiles] = useState<File[]>([]);
  const [pageSlots, setPageSlots] = useState<GifFrameReimportPageSlot[]>(
    () => retainedSession?.pageSlots ?? [],
  );
  const [pageAssignments, setPageAssignments] = useState<Array<number | null>>(
    () => (retainedSession?.pageSlots ?? []).map(() => null),
  );
  const [validation, setValidation] = useState<GifFrameSheetReimportValidation | null>(null);
  const [result, setResult] = useState<GifFrameSheetReimportResult | null>(null);
  const [profiles, setProfiles] = useState<ExportProfile[]>([]);
  const [targetProfileId, setTargetProfileId] = useState<string | null>(null);
  const [setActiveVariant, setSetActiveVariant] = useState(false);
  const [isWorking, setIsWorking] = useState(false);
  const [isValidating, setIsValidating] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const canSetActive = icon.shape === "single";
  const manifestSource = useMemo<GifFrameManifestSource | null>(() => {
    if (usingManualRecovery && manualManifestFile) {
      return { kind: "manual_file", file: manualManifestFile };
    }
    if (!usingManualRecovery && retainedSession?.manifestPath) {
      return { kind: "retained_path", path: retainedSession.manifestPath };
    }
    return null;
  }, [manualManifestFile, retainedSession, usingManualRecovery]);
  const mappedSheetFiles = useMemo(
    () => mappedGifFrameFiles(pageSlots, sheetFiles, pageAssignments),
    [pageAssignments, pageSlots, sheetFiles],
  );
  const mappedPageIndexes = useMemo(
    () => pageSlots.map((slot) => slot.pageIndex),
    [pageSlots],
  );
  const assignedPageCount = pageAssignments.filter(
    (fileIndex) => fileIndex !== null,
  ).length;
  const firstUnassignedSlotIndex = pageAssignments.findIndex(
    (fileIndex) => fileIndex === null,
  );
  const usedFileIndexes = useMemo(
    () =>
      new Set(
        pageAssignments.filter(
          (fileIndex): fileIndex is number => fileIndex !== null,
        ),
      ),
    [pageAssignments],
  );
  const hasDefiniteOpaqueResult = sheetFiles.some(
    gifFrameResultNeedsOpaqueWarning,
  );
  const hasWebpResult = sheetFiles.some(
    (file) => /\.webp$/i.test(file.name) || file.type === "image/webp",
  );

  useEffect(() => {
    if (!retainedSession || manualManifestFile) return;
    setUsingManualRecovery(false);
    setPageSlots(retainedSession.pageSlots);
    setPageAssignments(
      autoAssignGifFrameFiles(retainedSession.pageSlots, sheetFilesRef.current),
    );
    setErrorMessage(null);
  }, [manualManifestFile, retainedSession]);

  useEffect(() => {
    void listExportProfiles(collection.id)
      .then((nextProfiles) => {
        setProfiles(nextProfiles);
        setTargetProfileId(nextProfiles.find((profile) => profile.profileType === "dcinside")?.id ?? nextProfiles[0]?.id ?? null);
      })
      .catch(() => {
        setProfiles([]);
      });
  }, [collection.id]);

  useEffect(() => {
    if (!manifestSource || !mappedSheetFiles) {
      setValidation(null);
      setIsValidating(false);
      return;
    }
    let cancelled = false;
    setValidation(null);
    setIsValidating(true);
    setErrorMessage(null);
    void validateGifFrameSheetReimport(
      manifestSource,
      mappedSheetFiles,
      mappedPageIndexes,
      transparencyMode,
    )
      .then((nextValidation) => {
        if (!cancelled) {
          setValidation(nextValidation);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setValidation(null);
          setErrorMessage(getCommandErrorMessage(error));
        }
      })
      .finally(() => {
        if (!cancelled) setIsValidating(false);
      });
    return () => {
      cancelled = true;
    };
  }, [manifestSource, mappedPageIndexes, mappedSheetFiles, transparencyMode]);

  const clearSheetSelection = (message: string) => {
    manifestLoadGenerationRef.current += 1;
    pendingManifestFileRef.current = null;
    sheetFilesRef.current = [];
    setSheetFiles([]);
    setPageAssignments(pageSlots.map(() => null));
    setValidation(null);
    setResult(null);
    setIsValidating(false);
    setErrorMessage(message);
  };

  const replaceSheetFiles = (files: File[]) => {
    const activeManualManifest =
      pendingManifestFileRef.current ??
      (usingManualRecovery ? manualManifestFile : null);
    const combinedSelection = classifyGifFrameReimportFiles([
      ...(activeManualManifest ? [activeManualManifest] : []),
      ...files,
    ]);
    if (combinedSelection.error) {
      clearSheetSelection(combinedSelection.error);
      return;
    }
    sheetFilesRef.current = combinedSelection.imageFiles;
    setSheetFiles(combinedSelection.imageFiles);
    setPageAssignments(
      autoAssignGifFrameFiles(pageSlots, combinedSelection.imageFiles),
    );
    setValidation(null);
    setResult(null);
    setErrorMessage(
      activeManualManifest || retainedSession
        ? null
        : "이전·외부 작업은 앱 복원용 frames_manifest.json을 먼저 선택해 주세요.",
    );
  };

  const replaceManifest = async (file: File, nextSheetFiles: File[] = []) => {
    const manifestLoadGeneration = ++manifestLoadGenerationRef.current;
    pendingManifestFileRef.current = file;
    sheetFilesRef.current = nextSheetFiles;
    setUsingManualRecovery(true);
    setValidation(null);
    setResult(null);
    setIsValidating(false);
    setErrorMessage(null);
    try {
      const slots = await readGifFrameReimportPageSlots(file);
      if (manifestLoadGeneration !== manifestLoadGenerationRef.current) return;
      pendingManifestFileRef.current = null;
      const currentSheetFiles = sheetFilesRef.current;
      setManualManifestFile(file);
      setPageSlots(slots);
      setSheetFiles(currentSheetFiles);
      setPageAssignments(autoAssignGifFrameFiles(slots, currentSheetFiles));
    } catch (error) {
      if (manifestLoadGeneration !== manifestLoadGenerationRef.current) return;
      pendingManifestFileRef.current = null;
      sheetFilesRef.current = [];
      setManualManifestFile(null);
      setSheetFiles([]);
      if (retainedSession) {
        setUsingManualRecovery(false);
        setPageSlots(retainedSession.pageSlots);
        setPageAssignments(retainedSession.pageSlots.map(() => null));
      } else {
        setPageSlots([]);
        setPageAssignments([]);
      }
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    const selection = classifyGifFrameReimportFiles(event.dataTransfer.files);
    if (selection.error) {
      clearSheetSelection(selection.error);
      return;
    }
    if (selection.manifestFile) {
      void replaceManifest(selection.manifestFile, selection.imageFiles);
      return;
    }
    if (selection.imageFiles.length === 0) {
      clearSheetSelection("수정한 프레임 시트 결과 이미지를 선택해 주세요.");
      return;
    }
    replaceSheetFiles(selection.imageFiles);
  };

  const errors = validation?.errors ?? [];

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
      <section
        className="rounded-md border border-dashed border-border bg-white p-4"
        data-testid="gif-frame-reimport-drop"
        onDragOver={(event) => event.preventDefault()}
        onDrop={handleDrop}
      >
        <h3 className="text-sm font-semibold">수정된 프레임 시트 결과 선택</h3>
        <p className="mt-2 text-sm text-muted">
          {retainedSession && !usingManualRecovery
            ? "같은 창에서 내보낸 manifest는 앱이 유지합니다. AI에서 받은 결과 이미지만 놓으세요."
            : "이전 세션이나 외부에서 만든 작업은 manifest와 결과 이미지를 함께 놓으세요."}
          {" "}PNG, JPG/JPEG, 정적 WebP를 지원하며 페이지 이름이 달라지면 아래에서 직접 연결할 수 있습니다.
        </p>

        <div
          className="mt-4 rounded-md border border-border bg-canvas p-3 text-xs"
          data-testid="gif-frame-manifest-source"
        >
          {retainedSession && !usingManualRecovery ? (
            <>
              <p className="font-semibold text-success">앱 복원용 manifest · 자동 유지됨</p>
              <p className="mt-1 truncate text-muted" title={retainedSession.manifestPath}>
                웹 AI에 업로드하지 않습니다 · {fileNameFromPath(retainedSession.manifestPath)}
              </p>
              <button
                className="mt-2 rounded border border-border bg-white px-2 py-1 font-medium hover:bg-menu-hover"
                data-testid="gif-frame-manifest-recovery-open"
                type="button"
                onClick={() => {
                  setUsingManualRecovery(true);
                  setManualManifestFile(null);
                  setPageSlots([]);
                  setPageAssignments([]);
                  setValidation(null);
                  setErrorMessage(null);
                }}
              >
                이전 작업 복구 · manifest 수동 선택
              </button>
            </>
          ) : (
            <div className="grid gap-2">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <p className="font-semibold">수동 복구용 manifest</p>
                {retainedSession ? (
                  <button
                    className="rounded border border-border bg-white px-2 py-1 font-medium hover:bg-menu-hover"
                    data-testid="gif-frame-manifest-use-retained"
                    type="button"
                    onClick={() => {
                      manifestLoadGenerationRef.current += 1;
                      pendingManifestFileRef.current = null;
                      setUsingManualRecovery(false);
                      setManualManifestFile(null);
                      setPageSlots(retainedSession.pageSlots);
                      setPageAssignments(
                        autoAssignGifFrameFiles(
                          retainedSession.pageSlots,
                          sheetFilesRef.current,
                        ),
                      );
                      setValidation(null);
                      setErrorMessage(null);
                    }}
                  >
                    이 창의 manifest 다시 사용
                  </button>
                ) : null}
              </div>
              <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                frames_manifest.json 선택
                <input
                  accept="application/json,.json"
                  className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
                  data-testid="gif-frame-manifest-file"
                  type="file"
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0] ?? null;
                    event.currentTarget.value = "";
                    if (!file) return;
                    const selection = classifyGifFrameReimportFiles([file]);
                    if (selection.error || !selection.manifestFile) {
                      clearSheetSelection(
                        selection.error ?? "manifest JSON 파일을 선택해 주세요.",
                      );
                      return;
                    }
                    const combinedSelection = classifyGifFrameReimportFiles([
                      selection.manifestFile,
                      ...sheetFilesRef.current,
                    ]);
                    if (combinedSelection.error) {
                      clearSheetSelection(combinedSelection.error);
                      return;
                    }
                    void replaceManifest(
                      selection.manifestFile,
                      combinedSelection.imageFiles,
                    );
                  }}
                />
              </label>
              <p className="leading-5 text-muted">
                수동 복구는 앱을 다시 켰거나 다른 PC·폴더에서 이어갈 때만 사용합니다.
              </p>
            </div>
          )}
        </div>

        <label className="mt-4 flex flex-col gap-1 text-xs font-medium text-muted">
          AI 결과 이미지
          <input
            accept="image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp"
            className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
            data-testid="gif-frame-result-files"
            multiple
            type="file"
            onChange={(event) => {
              const selection = classifyGifFrameReimportFiles(
                event.currentTarget.files ?? [],
              );
              event.currentTarget.value = "";
              if (selection.error || selection.manifestFile) {
                clearSheetSelection(
                  selection.error ?? "여기에는 결과 이미지만 선택해 주세요.",
                );
                return;
              }
              replaceSheetFiles(selection.imageFiles);
            }}
          />
        </label>

        <fieldset className="mt-4 rounded-md border border-border p-3" data-testid="gif-frame-transparency-mode">
          <legend className="px-1 text-xs font-semibold">배경·투명도 처리</legend>
          <div className="mt-1 grid gap-2 text-sm">
            <label className="flex items-start gap-2">
              <input
                checked={transparencyMode === "preserve_alpha"}
                name="gif-frame-transparency"
                type="radio"
                onChange={() => onTransparencyModeChange("preserve_alpha")}
              />
              <span>
                <strong>투명 유지</strong>
                <span className="block text-xs leading-5 text-muted">투명 픽셀이 없는 결과는 적용 전에 오류로 막습니다.</span>
              </span>
            </label>
            <label className="flex items-start gap-2">
              <input
                checked={transparencyMode === "allow_opaque"}
                name="gif-frame-transparency"
                type="radio"
                onChange={() => onTransparencyModeChange("allow_opaque")}
              />
              <span>
                <strong>배경 포함 허용</strong>
                <span className="block text-xs leading-5 text-muted">JPG처럼 불투명한 결과도 받지만 그 배경이 모든 GIF 프레임에 그대로 남습니다.</span>
              </span>
            </label>
          </div>
        </fieldset>

        <div className="mt-4 grid gap-2 text-xs text-muted">
          <p>
            Manifest: {manifestSource?.kind === "retained_path"
              ? `앱에 유지됨 · ${fileNameFromPath(manifestSource.path)}`
              : manualManifestFile?.name ?? "-"}
          </p>
          <p>
            선택한 결과: {sheetFiles.length > 0
              ? sheetFiles.map((file) => file.name).join(", ")
              : "-"}
          </p>
        </div>
        {hasDefiniteOpaqueResult ? (
          <p className="mt-3 text-xs leading-5 text-warning" role="status">
            JPG/JPEG에는 투명도 정보가 없습니다. 확장자만 .png로 바꿔도 투명해지지 않습니다.
            {transparencyMode === "allow_opaque"
              ? " 현재는 배경 포함을 허용하므로 보이는 배경이 GIF에 그대로 들어갑니다."
              : " 계속하려면 투명 PNG를 다시 받거나 ‘배경 포함 허용’을 선택하세요."}
          </p>
        ) : null}
        {hasWebpResult ? (
          <p className="mt-2 text-xs leading-5 text-muted" role="status">
            WebP는 정적 이미지만 사용할 수 있습니다. 투명도 유무와 애니메이션 여부는 가져오기 전에 검사합니다.
          </p>
        ) : null}

        {pageSlots.length > 0 ? (
          <div
            className="mt-4 grid gap-2 rounded-md border border-border bg-canvas p-3"
            data-testid="gif-frame-page-slots"
          >
            <div className="flex items-center justify-between gap-3 text-xs">
              <p className="font-semibold">페이지별 결과 연결</p>
              <p className="text-muted">
                {assignedPageCount}/{pageSlots.length} 페이지 지정
              </p>
            </div>
            {pageSlots.map((slot, slotIndex) => {
              const assignedFileIndex = pageAssignments[slotIndex];
              const assignedFile =
                assignedFileIndex === null || assignedFileIndex === undefined
                  ? null
                  : sheetFiles[assignedFileIndex] ?? null;
              const isNextUnassigned =
                assignedFile === null && slotIndex === firstUnassignedSlotIndex;
              return (
                <div
                  className="grid gap-1 text-xs text-muted sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)] sm:items-center"
                  key={`${slot.pageIndex}-${slot.expectedFileName}`}
                >
                  <span>
                    {slot.pageIndex + 1}페이지 · {slot.expectedFileName} · {slot.width}×
                    {slot.height}px
                  </span>
                  {assignedFile ? (
                    <div className="flex min-w-0 items-center gap-2 rounded-md border border-border bg-white px-2 py-1.5 text-foreground">
                      <span className="min-w-0 flex-1 truncate" title={assignedFile.name}>
                        {assignedFile.name}
                      </span>
                      <button
                        aria-label={`${slot.pageIndex + 1}페이지 결과 연결 해제`}
                        className="shrink-0 rounded border border-border px-2 py-0.5 font-medium hover:bg-menu-hover"
                        type="button"
                        onClick={() => {
                          setPageAssignments((current) =>
                            assignGifFrameFileToSlot(current, slotIndex, null),
                          );
                          setValidation(null);
                          setResult(null);
                          setErrorMessage(null);
                        }}
                      >
                        연결 해제
                      </button>
                    </div>
                  ) : isNextUnassigned ? (
                    <select
                      aria-label={`${slot.pageIndex + 1}페이지 결과 이미지`}
                      className="rounded-md border border-border bg-white px-2 py-1.5 text-xs text-foreground"
                      data-testid={`gif-frame-page-slot-select-${slotIndex}`}
                      value=""
                      onChange={(event) => {
                        const value = event.currentTarget.value;
                        if (value === "") return;
                        setPageAssignments((current) =>
                          assignGifFrameFileToSlot(
                            current,
                            slotIndex,
                            Number(value),
                          ),
                        );
                        setValidation(null);
                        setResult(null);
                        setErrorMessage(null);
                      }}
                    >
                      <option value="">파일 선택</option>
                      {sheetFiles.map((file, fileIndex) =>
                        usedFileIndexes.has(fileIndex) ? null : (
                          <option
                            key={`${file.name}-${file.size}-${file.lastModified}-${fileIndex}`}
                            value={fileIndex}
                          >
                            {file.name}
                          </option>
                        ),
                      )}
                    </select>
                  ) : (
                    <span className="rounded-md border border-dashed border-border px-2 py-1.5">
                      앞 페이지 연결 후 선택
                    </span>
                  )}
                </div>
              );
            })}
            {assignedPageCount < pageSlots.length ? (
              <p className="text-xs leading-5 text-warning" role="status">
                자동으로 연결되지 않은 페이지는 해당 결과 이미지를 직접 선택해 주세요.
                페이지를 모두 지정해야 구조 검사를 시작합니다.
              </p>
            ) : isValidating ? (
              <p className="text-xs text-muted" role="status">
                파일 포맷·캔버스 크기·투명도와 페이지 구조를 검사하는 중입니다.
              </p>
            ) : null}
          </div>
        ) : null}
        {validation ? (
          <div className="mt-4 rounded-md border border-border bg-canvas p-3 text-sm">
            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
              <Metric label="Frames" value={`${validation.detectedFrameCount}/${validation.frameCount}`} />
              <Metric label="Pages" value={String(validation.pageCount)} />
              <Metric label="Duration" value={`${validation.durationMs}ms`} />
              <Metric label="Loop" value={loopLabel(validation.loopMode, validation.loopCount)} />
            </div>
            {validation.warnings.length ? <MessageList tone="warning" messages={validation.warnings} /> : null}
            {validation.errors.length ? <MessageList tone="error" messages={validation.errors} /> : null}
          </div>
        ) : null}
        {errorMessage ? <MessageList tone="error" messages={[errorMessage]} /> : null}
      </section>

      <aside className="rounded-md border border-border bg-white p-4">
        <h3 className="text-sm font-semibold">내보내기용 GIF 결과</h3>
        <p className="mt-2 text-sm text-muted">
          새 GIF 처리 버전은 선택한 export profile에서만 사용할 수 있으며 원본과 현재 편집 소스는 바뀌지 않습니다.
        </p>
        <div className="mt-4 grid gap-3">
          <CheckField
            disabled={!canSetActive}
            label={canSetActive ? "선택한 export profile에서 사용" : "single GIF만 export용으로 설정 가능"}
            checked={setActiveVariant && canSetActive}
            onChange={(checked) => setSetActiveVariant(checked)}
          />
          {setActiveVariant && canSetActive ? (
            <label className="flex flex-col gap-1 text-xs font-medium text-muted">
              Export profile
              <select
                className="h-9 rounded-md border border-border bg-white px-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                value={targetProfileId ?? ""}
                onChange={(event) => setTargetProfileId(event.currentTarget.value || null)}
              >
                {profiles.map((profile) => (
                  <option key={profile.id} value={profile.id}>
                    {profile.name}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
        </div>
        <button
          className="mt-4 w-full rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
          data-testid="gif-frame-reimport-create"
          disabled={
            !manifestSource ||
            !mappedSheetFiles ||
            !validation ||
            errors.length > 0 ||
            isValidating ||
            isWorking
          }
          type="button"
          onClick={() => {
            if (!manifestSource || !mappedSheetFiles || !validation) {
              return;
            }
            setIsWorking(true);
            setResult(null);
            setErrorMessage(null);
            void reimportGifFrameSheet(
              icon.id,
              manifestSource,
              mappedSheetFiles,
              setActiveVariant && canSetActive,
              setActiveVariant && canSetActive ? targetProfileId : null,
              mappedPageIndexes,
              transparencyMode,
            )
              .then(async (nextResult) => {
                setResult(nextResult);
                if (nextResult.variantId) {
                  await onVariantCreated();
                }
              })
              .catch((error) => setErrorMessage(getCommandErrorMessage(error)))
              .finally(() => setIsWorking(false));
          }}
        >
          {isWorking ? "다시 가져오는 중" : "내보내기용 GIF 처리 버전 만들기"}
        </button>
        {result ? (
          <GifFrameVariantResult
            result={result}
            onOpenPath={(path) =>
              openExportPath(path).catch((error) =>
                setErrorMessage(getCommandErrorMessage(error)),
              )
            }
          />
        ) : null}
      </aside>
    </div>
  );
}
export function GifFrameExportResultPanel({
  aiWebExpectedCanvas,
  aiWebNovelAiPrompt,
  aiWebPrompt,
  result,
  onContinueToReimport,
  onOpenAiSite,
  onOpenFolder,
}: {
  aiWebExpectedCanvas?: string | null;
  aiWebNovelAiPrompt?: string | null;
  aiWebPrompt?: string | null;
  result: GifFrameSheetExportResult;
  onContinueToReimport?: () => void;
  onOpenAiSite?: (resource: GifAiWebResource) => Promise<void>;
  onOpenFolder: (path: string) => Promise<void> | void;
}) {
  const [selectedPageIndex, setSelectedPageIndex] = useState(0);
  const [completedPageIndexes, setCompletedPageIndexes] = useState<number[]>([]);
  const [workingPageAction, setWorkingPageAction] = useState<"drag" | "reveal" | null>(null);
  const [pageActionMessage, setPageActionMessage] = useState<string | null>(null);
  const [pageActionError, setPageActionError] = useState<string | null>(null);
  const selectedPagePath = result.frameSheetPaths[selectedPageIndex] ?? null;
  const canUseManagedPageAction = Boolean(result.manifestPath && selectedPagePath);

  const runPageAction = async (action: "drag" | "reveal") => {
    if (!result.manifestPath || !selectedPagePath || workingPageAction) return;
    setWorkingPageAction(action);
    setPageActionMessage(null);
    setPageActionError(null);
    try {
      if (action === "drag") {
        const dragResult = await startGifFrameSheetPageDrag(
          result.manifestPath,
          selectedPageIndex,
        );
        setPageActionMessage(dragResult.message);
      } else {
        await revealGifFrameSheetPage(result.manifestPath, selectedPageIndex);
        setPageActionMessage(
          `${selectedPageIndex + 1}페이지 파일을 탐색기에서 선택했습니다. 웹 업로드 영역으로 끌어 놓으세요.`,
        );
      }
    } catch (error) {
      setPageActionError(getCommandErrorMessage(error));
    } finally {
      setWorkingPageAction(null);
    }
  };

  const markSelectedPageComplete = () => {
    setCompletedPageIndexes((current) =>
      current.includes(selectedPageIndex)
        ? current.filter((pageIndex) => pageIndex !== selectedPageIndex)
        : [...current, selectedPageIndex],
    );
    const nextPageIndex = result.frameSheetPaths.findIndex(
      (_, pageIndex) =>
        pageIndex > selectedPageIndex &&
        !completedPageIndexes.includes(pageIndex),
    );
    if (nextPageIndex >= 0) setSelectedPageIndex(nextPageIndex);
  };

  return (
    <div data-testid="gif-frame-export-result">
      <ResultBlock
        title="내보내기 완료"
        rows={[
          ["Output", result.outputDirectory],
          ["Frames", String(result.frameCount)],
          ["Pages", String(result.pageCount)],
        ]}
        warnings={result.warnings}
      />

      <div className="mt-3 grid gap-3" data-testid="gif-frame-artifact-roles">
        <section className="rounded-md border border-success/40 bg-success/5 p-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h4 className="text-xs font-semibold text-success">AI 업로드 대상 · frames_sheet</h4>
              <p className="mt-1 text-[11px] leading-4 text-muted">
                아래 clean 시트만 한 페이지씩 Gemini/NovelAI에 올립니다.
              </p>
            </div>
            <span className="shrink-0 rounded-full bg-success/10 px-2 py-1 text-[10px] font-semibold text-success">
              {completedPageIndexes.length}/{result.frameSheetPaths.length} 처리 표시
            </span>
          </div>
          {result.frameSheetPaths.length > 0 ? (
            <div className="mt-2 grid gap-1.5">
              {result.frameSheetPaths.map((path, pageIndex) => {
                const selected = selectedPageIndex === pageIndex;
                const completed = completedPageIndexes.includes(pageIndex);
                return (
                  <button
                    aria-pressed={selected}
                    className={`flex min-w-0 items-center justify-between gap-2 rounded border px-2 py-1.5 text-left text-xs ${
                      selected
                        ? "border-focus bg-selected"
                        : "border-border bg-white hover:bg-menu-hover"
                    }`}
                    data-testid={`gif-frame-upload-page-${pageIndex}`}
                    key={path}
                    type="button"
                    onClick={() => {
                      setSelectedPageIndex(pageIndex);
                      setPageActionMessage(null);
                      setPageActionError(null);
                    }}
                  >
                    <span className="min-w-0 truncate">
                      {pageIndex + 1}페이지 · {fileNameFromPath(path)}
                    </span>
                    <span className={completed ? "text-success" : "text-muted"}>
                      {completed ? "처리함" : selected ? "현재" : "대기"}
                    </span>
                  </button>
                );
              })}
            </div>
          ) : (
            <p className="mt-2 text-xs text-danger">업로드할 clean 시트가 생성되지 않았습니다.</p>
          )}
          {selectedPagePath ? (
            <div className="mt-3 grid gap-2">
              <p className="truncate text-xs font-semibold" title={selectedPagePath}>
                현재 업로드: {fileNameFromPath(selectedPagePath)}
              </p>
              <div className="grid gap-2 sm:grid-cols-2">
                <button
                  aria-describedby="gif-frame-native-drag-help"
                  className="rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-50"
                  data-testid="gif-frame-page-native-drag"
                  disabled={!canUseManagedPageAction || workingPageAction !== null}
                  title="마우스로 누른 채 웹 AI의 업로드 영역까지 끌어 놓습니다."
                  type="button"
                  onClick={(event) => {
                    if (event.detail === 0) void runPageAction("reveal");
                  }}
                  onKeyDown={(event) => {
                    if (event.key !== "Enter" && event.key !== " ") return;
                    event.preventDefault();
                    void runPageAction("reveal");
                  }}
                  onPointerDown={(event) => {
                    if (event.pointerType === "mouse" && event.button === 0) {
                      void runPageAction("drag");
                    }
                  }}
                >
                  이 페이지 끌기
                </button>
                <button
                  className="rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover disabled:opacity-50"
                  data-testid="gif-frame-page-reveal"
                  disabled={!canUseManagedPageAction || workingPageAction !== null}
                  type="button"
                  onClick={() => void runPageAction("reveal")}
                >
                  탐색기에서 이 파일 선택
                </button>
              </div>
              <button
                className="rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover"
                data-testid="gif-frame-page-mark-complete"
                type="button"
                onClick={markSelectedPageComplete}
              >
                {completedPageIndexes.includes(selectedPageIndex)
                  ? "처리 표시 취소"
                  : "이 페이지 AI 처리 완료 표시"}
              </button>
              <p className="text-[11px] leading-4 text-muted" id="gif-frame-native-drag-help">
                마우스는 실제 파일 끌기를 시작합니다. 키보드로 활성화하면 안전한 탐색기 선택으로 전환합니다.
              </p>
            </div>
          ) : null}
        </section>

        <section className="rounded-md border border-warning/50 bg-warning/5 p-3">
          <h4 className="text-xs font-semibold text-warning">사람 확인용 · AI 업로드 금지 · frames_guide</h4>
          <p className="mt-1 text-[11px] leading-4 text-muted">
            번호·경계선·체커무늬를 눈으로 확인하는 파일입니다. AI에 올리면 표시가 실제 이미지에 섞일 수 있습니다.
          </p>
          <p className="mt-2 break-all text-[11px] text-muted">
            {result.guideSheetPaths.length > 0
              ? result.guideSheetPaths.map(fileNameFromPath).join(", ")
              : "guide 시트를 생성하지 않음"}
          </p>
        </section>

        <section className="rounded-md border border-border bg-canvas p-3">
          <h4 className="text-xs font-semibold">앱 복원용 · AI 업로드 금지 · manifest</h4>
          <p className="mt-1 text-[11px] leading-4 text-muted">
            같은 창에서는 앱이 자동으로 유지합니다. AI에 올리거나 정상 흐름에서 다시 선택할 필요가 없습니다.
          </p>
          <p className="mt-2 break-all text-[11px] text-muted">
            {result.manifestPath
              ? fileNameFromPath(result.manifestPath)
              : "manifest가 생성되지 않아 자동 재가져오기를 사용할 수 없음"}
          </p>
        </section>
      </div>

      <button
        className="mt-3 w-full rounded-md border border-border bg-white px-3 py-2 text-sm font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        data-testid="gif-frame-export-open-folder"
        type="button"
        onClick={() => void onOpenFolder(result.outputDirectory)}
      >
        결과 폴더 열기
      </button>
      {pageActionMessage ? <p className="mt-2 text-xs text-success" role="status">{pageActionMessage}</p> : null}
      {pageActionError ? <p className="mt-2 text-xs text-danger" role="alert">{pageActionError}</p> : null}
      {aiWebPrompt && onOpenAiSite && onContinueToReimport ? (
        <GifAiWebExportActions
          expectedCanvas={aiWebExpectedCanvas ?? "내보낸 clean PNG와 같은 크기"}
          novelAiPrompt={aiWebNovelAiPrompt ?? aiWebPrompt}
          prompt={aiWebPrompt}
          onContinueToReimport={onContinueToReimport}
          onOpenAiSite={onOpenAiSite}
        />
      ) : null}
    </div>
  );
}
export function GifAiWebExportActions({
  expectedCanvas = "내보낸 clean PNG와 같은 크기",
  novelAiPrompt = "animated emoticon, frame sequence, sprite sheet, consistent character, consistent style",
  prompt,
  onContinueToReimport,
  onOpenAiSite,
}: {
  expectedCanvas?: string;
  novelAiPrompt?: string;
  prompt: string;
  onContinueToReimport: () => void;
  onOpenAiSite: (resource: GifAiWebResource) => Promise<void>;
}) {
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const promptCopyGenerationRef = useRef(0);
  const [service, setService] =
    useState<GifAiWebResource>("gemini_ai_studio");
  const [desiredEdit, setDesiredEdit] = useState("");
  const [workingResource, setWorkingResource] =
    useState<GifAiWebResource | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [promptCopyOutcome, setPromptCopyOutcome] =
    useState<NovelAiPromptCopyOutcome>("idle");
  const [promptCopyRevision, setPromptCopyRevision] = useState(0);
  const activePrompt = buildGifAiWebPromptWithUserRequest(
    service === "novelai_app" ? novelAiPrompt : prompt,
    desiredEdit,
    service,
  );

  const resetPromptCopySequence = () => {
    promptCopyGenerationRef.current += 1;
    setPromptCopyOutcome("idle");
    setPromptCopyRevision((revision) => revision + 1);
  };

  const copyPrompt = async (resource: GifAiWebResource = service) => {
    const value = buildGifAiWebPromptWithUserRequest(
      resource === "novelai_app" ? novelAiPrompt : prompt,
      desiredEdit,
      resource,
    );
    const copyGeneration = ++promptCopyGenerationRef.current;
    const result = await copyAiHandoffPrompt(value, {
      clipboardWriteText:
        typeof navigator !== "undefined" && navigator.clipboard?.writeText
          ? (clipboardValue) => navigator.clipboard.writeText(clipboardValue)
          : undefined,
      fallbackCopy: () => {
        const input = promptRef.current;
        if (!input || typeof document === "undefined") return false;
        input.focus();
        input.select();
        return typeof document.execCommand === "function"
          ? document.execCommand("copy")
          : false;
      },
    });
    if (copyGeneration !== promptCopyGenerationRef.current) return false;
    const copied = result === "clipboard" || result === "fallback";
    if (resource === "novelai_app") {
      setPromptCopyOutcome(copied ? "copied" : "failed");
      setPromptCopyRevision((revision) => revision + 1);
    }
    setStatusMessage(
      copied
        ? resource === "novelai_app"
          ? "NovelAI Prompt를 복사했습니다. 제외 태그는 아래에서 따로 복사하세요."
          : "GIF 웹 AI 프롬프트를 복사했습니다."
        : null,
    );
    setErrorMessage(
      copied
        ? null
        : "프롬프트 자동 복사에 실패했습니다. 아래 내용을 직접 복사한 뒤 공식 사이트를 열어 주세요.",
    );
    return copied;
  };

  const openSite = async () => {
    if (workingResource) return;
    const resource = service;
    setWorkingResource(resource);
    setStatusMessage(null);
    setErrorMessage(null);
    const copied = await copyPrompt(resource);
    if (!copied) {
      setErrorMessage(
        "프롬프트를 복사하지 못해 사이트를 열지 않았습니다. 아래 내용을 직접 복사하거나 다시 시도해 주세요.",
      );
      setWorkingResource(null);
      return;
    }
    try {
      await onOpenAiSite(resource);
      setStatusMessage(
        resource === "novelai_app"
          ? "NovelAI 공식 사이트를 열었습니다. Prompt와 Undesired Content를 붙여넣고 현재 frames_sheet 페이지부터 처리하세요. guide와 manifest는 업로드하지 않습니다."
          : "Gemini AI Studio 공식 사이트를 열었습니다. 현재 frames_sheet 페이지부터 처리하세요. guide와 manifest는 업로드하지 않습니다.",
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setWorkingResource(null);
    }
  };

  return (
    <section
      className="mt-4 rounded-md border border-focus/30 bg-selected/30 p-3"
      data-testid="gif-ai-web-export-actions"
    >
      <h4 className="text-sm font-semibold">웹 AI에서 프레임 시트 수정</h4>
      <ol className="mt-2 list-decimal space-y-1 pl-4 text-xs leading-5 text-muted">
        <li><strong>frames_sheet</strong>의 현재 페이지 한 장만 공식 웹 AI에 전달합니다.</li>
        <li><strong>frames_guide와 frames_manifest는 절대 업로드하지 않습니다.</strong></li>
        <li>결과를 내려받아 다음 페이지를 같은 설정으로 처리합니다.</li>
        <li>앱으로 돌아와 결과 이미지만 넣으면 앱이 유지한 timing·loop로 복원합니다.</li>
      </ol>
      <p className="mt-2 rounded-md border border-warning/40 bg-warning/5 p-2 text-[11px] leading-4 text-warning" data-testid="gif-ai-provider-limit">
        생성형 이미지 모델은 그림체·캐릭터·정확한 셀 구조나 PNG/투명 출력을 보장하지 않습니다.
        가능한 한 “다시 그리기”가 아닌 요청 부분만 수정하고, 결과를 페이지별로 반드시 검토하세요.
      </p>
      <label className="mt-3 grid gap-1 text-xs font-semibold">
        웹 서비스
        <select
          className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
          data-testid="gif-ai-web-service"
          disabled={workingResource !== null}
          value={service}
          onChange={(event) => {
            setService(event.currentTarget.value as GifAiWebResource);
            setStatusMessage(null);
            setErrorMessage(null);
            resetPromptCopySequence();
          }}
        >
          <option value="gemini_ai_studio">Gemini AI Studio</option>
          <option value="novelai_app">NovelAI</option>
        </select>
      </label>
      <label className="mt-3 grid gap-1 text-xs font-semibold">
        원하는 GIF 수정
        <textarea
          className="min-h-20 resize-y rounded-md border border-border bg-white p-2 text-sm font-normal leading-5"
          data-testid="gif-ai-desired-edit"
          disabled={workingResource !== null}
          maxLength={2000}
          placeholder={
            service === "novelai_app"
              ? "예: same character, wavy motion, shifting colors, clean lineart"
              : "예: 캐릭터는 유지하고 일렁이는 움직임과 색 변화가 자연스럽게 이어지게 해 주세요."
          }
          value={desiredEdit}
          onChange={(event) => {
            setDesiredEdit(event.currentTarget.value);
            setStatusMessage(null);
            setErrorMessage(null);
            resetPromptCopySequence();
          }}
        />
      </label>
      {service === "novelai_app" && needsNovelAiEnglishInputHint(desiredEdit) ? (
        <p className="mt-2 text-xs leading-5 text-warning" data-testid="gif-novelai-language-hint">
          NovelAI Prompt에는 쉼표로 나눈 짧은 영문 태그를 권장합니다. 입력한 한국어는
          자동 번역하지 않습니다.
        </p>
      ) : null}
      <label className="mt-3 block text-xs font-semibold" htmlFor="gif-ai-web-prompt">
        {service === "novelai_app"
          ? "NovelAI Prompt (태그 + 짧은 구조 문장)"
          : "Gemini 구조 보호 프롬프트"}
      </label>
      <textarea
        className="mt-1 min-h-36 w-full resize-y rounded-md border border-border bg-white p-2 text-[11px] leading-4"
        data-testid="gif-ai-web-prompt"
        id="gif-ai-web-prompt"
        readOnly
        ref={promptRef}
        value={activePrompt}
      />

      <button
        className="mt-2 w-full rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
        data-testid="gif-ai-copy-prompt"
        disabled={workingResource !== null}
        type="button"
        onClick={() => void copyPrompt()}
      >
        {service === "novelai_app" ? "NovelAI Prompt 복사" : "프롬프트만 복사"}
      </button>
      {service === "novelai_app" ? (
        <div className="mt-3">
          <NovelAiWebGuide
            disabled={workingResource !== null}
            expectedCanvas={expectedCanvas}
            promptCopyOutcome={promptCopyOutcome}
            promptCopyRevision={promptCopyRevision}
            task="gif_frame_sheet"
          />
        </div>
      ) : null}
      <div className="mt-2 grid gap-2">
        <button
          className="rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
          data-testid="gif-ai-open-selected"
          disabled={workingResource !== null}
          type="button"
          onClick={() => void openSite()}
        >
          프롬프트 복사 + {service === "gemini_ai_studio" ? "Gemini AI Studio" : "NovelAI"} 열기
        </button>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
          data-testid="gif-ai-continue-reimport"
          disabled={workingResource !== null}
          type="button"
          onClick={onContinueToReimport}
        >
          결과 이미지를 받았어요 · 다시 가져오기
        </button>
      </div>
      <p className="mt-2 text-[11px] leading-4 text-muted">
        원본 GIF와 frame timing·loop는 바뀌지 않습니다. 출력 캔버스는 그대로 유지하고,
        바뀐 다운로드명은 다시 가져오기의 페이지별 결과 연결에서 지정하세요.
      </p>
      {statusMessage ? <p className="mt-2 text-xs text-success" role="status">{statusMessage}</p> : null}
      {errorMessage ? <p className="mt-2 text-xs text-danger" role="alert">{errorMessage}</p> : null}
    </section>
  );
}

export function buildGifAiWebPromptWithUserRequest(
  basePrompt: string,
  userRequest: string,
  resource: GifAiWebResource,
) {
  const request = userRequest.trim();
  if (!request) return basePrompt;
  if (resource === "gemini_ai_studio") {
    return `${basePrompt}\n\n사용자 편집 요청:\n${request}`;
  }
  const normalized = normalizeNovelAiPromptInput(request);
  if (!normalized) return basePrompt;
  const [tags, ...structure] = basePrompt.split("\n");
  return [`${tags}, ${normalized}`, ...structure].join("\n");
}

export function gifFrameReimportPageSlotsFromExport(
  analysis: GifFrameSheetExportAnalysis,
  result: GifFrameSheetExportResult,
): GifFrameReimportPageSlot[] {
  return result.frameSheetPaths.map((path, pageIndex) => {
    const page = analysis.pages[pageIndex];
    return {
      pageIndex: page?.pageIndex ?? pageIndex,
      expectedFileName: fileNameFromPath(path),
      width: page?.width ?? analysis.sheetWidth,
      height: page?.height ?? analysis.sheetHeight,
    };
  });
}

export function gifAiPageCanvasContracts({
  analysis,
  result,
}: {
  analysis: GifFrameSheetExportAnalysis;
  result: GifFrameSheetExportResult;
}) {
  if (result.frameSheetPaths.length === 0) {
    return [`clean PNG=${analysis.sheetWidth}×${analysis.sheetHeight}px`];
  }
  return result.frameSheetPaths.map((path, index) => {
    const page = analysis.pages[index];
    const width = page?.width ?? analysis.sheetWidth;
    const height = page?.height ?? analysis.sheetHeight;
    return `${fileNameFromPath(path)}=${width}×${height}px`;
  });
}

export function buildGifAiWebPrompt({
  analysis,
  result,
  settings,
  transparencyMode = "preserve_alpha",
}: {
  analysis: GifFrameSheetExportAnalysis;
  result: GifFrameSheetExportResult;
  settings: GifFrameSheetSettings;
  transparencyMode?: GifFrameTransparencyMode;
}) {
  const cleanFiles = result.frameSheetPaths
    .map((path) => fileNameFromPath(path))
    .join(", ");
  const manifestName = result.manifestPath
    ? fileNameFromPath(result.manifestPath)
    : "frames_manifest.json";
  const guideFiles = result.guideSheetPaths
    .map((path) => fileNameFromPath(path))
    .join(", ");
  const pageCanvases = gifAiPageCanvasContracts({ analysis, result }).join(", ");
  const alphaRule =
    transparencyMode === "preserve_alpha"
      ? "- 기존 투명 배경과 픽셀별 alpha를 유지하고 비어 있는 셀은 완전히 투명하게 두세요. 체크무늬를 그리지 마세요."
      : "- 투명 배경은 선택 사항입니다. 불투명 배경을 만들 경우 체크무늬로 가짜 투명을 표현하지 말고 실제 단색/그림 배경으로 일관되게 유지하세요.";
  const returnRule =
    transparencyMode === "preserve_alpha"
      ? "- 가능한 경우 alpha가 실제로 포함된 PNG 한 장으로 반환하세요. JPG는 투명도를 보존할 수 없습니다."
      : "- PNG, JPG/JPEG 또는 정적 WebP 한 장으로 반환할 수 있습니다. 불투명 배경은 최종 GIF에도 남습니다.";
  return [
    "[PMTCONCON Studio · GIF 프레임 시트 부분 수정]",
    "이 작업은 새로 다시 그리는 생성 작업이 아닙니다. 업로드한 clean 프레임 시트를 기반으로 요청한 부분만 편집하세요.",
    `업로드 대상 frames_sheet: ${cleanFiles || `${result.pageCount}개의 clean PNG 페이지`}`,
    `업로드 금지: ${guideFiles || "frames_guide PNG"}, ${manifestName}. guide와 manifest는 절대 첨부하지 마세요.`,
    "",
    "필수 그림체·캐릭터 보존:",
    "- 캐릭터를 재해석하거나 새 디자인으로 다시 그리지 마세요. 요청하지 않은 픽셀은 가능한 한 그대로 유지하세요.",
    "- 모든 페이지와 프레임에서 얼굴 비율, 눈·입 모양, 체형, 의상, 팔레트, 선 굵기와 채색법을 원본과 동일하게 유지하세요.",
    "- 요청한 효과나 움직임만 최소 범위로 바꾸고 캐릭터 정체성, 카메라 구도와 프레임별 포즈 흐름을 바꾸지 마세요.",
    "",
    "절대 변경하지 말아야 할 구조:",
    `- 총 ${analysis.frameCount}프레임, ${result.pageCount}페이지, 파일별 정확한 캔버스: ${pageCanvases}.`,
    `- 셀 ${settings.frameCellWidth}×${settings.frameCellHeight}px, ${analysis.columns}열 × 페이지당 ${analysis.rowsPerPage}행, gap ${settings.gapX}/${settings.gapY}px, border ${settings.borderX}/${settings.borderY}px.`,
    "- 현재 업로드한 frames_sheet 페이지 한 장만 결과 한 장으로 반환하세요.",
    "- 왼쪽→오른쪽, 위→아래 row-major 셀 순서와 셀 위치·크기·개수를 유지하세요.",
    "- 프레임을 추가·삭제·병합·분할하거나 라벨, 격자선, 셀 번호, 체커무늬를 삽입하지 마세요.",
    alphaRule,
    "",
    "반환 형식:",
    returnRule,
    "- 캔버스 크기가 정확하지 않으면 앱에서 구조 오류로 표시됩니다.",
    "",
    `원본 GIF 메타데이터는 앱이 ${manifestName}으로 내부 보존합니다: 총 재생시간 ${analysis.durationMs}ms, loop ${gifAiLoopPromptLabel(analysis.loopMode, analysis.loopCount)}.`,
    "timing·재생 순서·loop와 원본 GIF는 수정 대상이 아닙니다. manifest를 AI에 올릴 필요도, 앱에서 같은 세션에 다시 선택할 필요도 없습니다.",
  ].join("\n");
}

export function buildNovelAiGifWebPrompt({
  analysis,
  result,
  settings,
  transparencyMode = "preserve_alpha",
}: {
  analysis: GifFrameSheetExportAnalysis;
  result: GifFrameSheetExportResult;
  settings: GifFrameSheetSettings;
  transparencyMode?: GifFrameTransparencyMode;
}) {
  const pageCanvases = gifAiPageCanvasContracts({ analysis, result }).join(", ");
  const backgroundTags =
    transparencyMode === "preserve_alpha"
      ? "transparent background, real alpha, no checkerboard"
      : "consistent background, no checkerboard pattern";
  const alphaRule =
    transparencyMode === "preserve_alpha"
      ? "Preserve real alpha and transparent gaps. JPEG cannot preserve alpha."
      : "Opaque output is allowed and will remain visible in the rebuilt GIF.";
  return [
    `animated emoticon, frame sequence, sprite sheet, edit only, preserve original character, preserve original style, consistent face, consistent proportions, consistent colors, clean lineart, ${backgroundTags}`,
    `Keep exactly ${analysis.frameCount} frames across ${result.pageCount} page(s); keep these exact canvas sizes: ${pageCanvases}.`,
    `Upload only the current frames_sheet page. Never upload frames_guide or frames_manifest. Process one page at a time with the same Prompt, Strength, Noise, sampler, and style controls. Return only the current page; keep the original ${analysis.columns} column layout, ${settings.frameCellWidth} by ${settings.frameCellHeight} pixel cells, row-major order, gaps, and borders unchanged. Do not redraw or redesign the character; edit only the requested detail. ${alphaRule}`,
  ].join("\n");
}function fileNameFromPath(path: string) {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function gifAiLoopPromptLabel(loopMode: string, loopCount: number | null) {
  if (loopMode === "count") return `${loopCount ?? 1}회 반복`;
  if (loopMode === "once") return "1회 재생";
  if (loopMode === "infinite") return "무한 반복";
  if (loopMode === "preserve") return "원본 loop 유지";
  return `${loopMode}${loopCount === null ? "" : ` ${loopCount}`}`;
}
export function GifFrameVariantResult({
  result,
  onOpenPath,
}: {
  result: GifFrameSheetReimportResult;
  onOpenPath: (path: string) => Promise<void> | void;
}) {
  const previewUrl = filePathToAssetUrl(result.outputPath, result.variantId);

  return (
    <div data-testid="gif-frame-variant-result">
      <ResultBlock
        title="다시 가져오기 완료"
        rows={[
          ["Variant", result.variantId ?? "-"],
          ["Output", result.outputPath ?? "-"],
          ["Frames", String(result.frameCount)],
          ["Duration", `${result.durationMs}ms`],
          ["Export", result.activeVariantSet ? "선택한 profile에서 사용" : "아직 사용 안 함"],
        ]}
        warnings={result.warnings}
        errors={result.errors}
      />
      {previewUrl ? (
        <div className="mt-3 rounded-md border border-border bg-canvas p-3">
          <img
            alt="재조립한 GIF 처리 버전 미리보기"
            className="mx-auto max-h-48 max-w-full rounded border border-border bg-checker object-contain"
            data-testid="gif-frame-variant-preview"
            src={previewUrl}
          />
          <p className="mt-2 text-xs text-muted">
            이 미리보기는 새 내보내기용 처리 버전입니다. 원본 GIF와 현재 편집 소스는 그대로 유지됩니다.
          </p>
          <button
            className="mt-3 w-full rounded-md border border-border bg-white px-3 py-2 text-sm font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            data-testid="gif-frame-variant-open-path"
            type="button"
            onClick={() => {
              if (result.outputPath) {
                void onOpenPath(result.outputPath);
              }
            }}
          >
            결과 GIF 위치 열기
          </button>
        </div>
      ) : null}
    </div>
  );
}

function TabButton({
  selected,
  children,
  onClick,
}: {
  selected: boolean;
  children: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      className={`rounded px-3 py-1.5 text-sm font-medium ${
        selected ? "bg-selected text-foreground" : "text-muted hover:bg-menu-hover"
      }`}
      type="button"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-white px-3 py-2">
      <div className="text-xs text-muted">{label}</div>
      <div className="mt-1 truncate text-sm font-semibold">{value}</div>
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
    <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <input
        className="min-w-0 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        min={1}
        type="number"
        value={value}
        onChange={(event) => onChange(Number.parseInt(event.currentTarget.value, 10))}
      />
    </label>
  );
}

function CheckField({
  label,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm text-foreground">
      <input
        checked={checked}
        disabled={disabled}
        type="checkbox"
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      {label}
    </label>
  );
}

function MessageList({
  tone,
  messages,
}: {
  tone: "warning" | "error";
  messages: string[];
}) {
  return (
    <ul
      className={`mt-3 grid gap-1 text-sm ${tone === "error" ? "text-danger" : "text-muted"}`}
      role={tone === "error" ? "alert" : "status"}
    >
      {messages.map((message, index) => (
        <li key={`${tone}-${index}`}>- {message}</li>
      ))}
    </ul>
  );
}

function ResultBlock({
  title,
  rows,
  warnings = [],
  errors = [],
}: {
  title: string;
  rows: Array<[string, string]>;
  warnings?: string[];
  errors?: string[];
}) {
  return (
    <div className="mt-4 rounded-md border border-border bg-canvas p-3 text-sm">
      <h4 className="font-semibold">{title}</h4>
      <dl className="mt-2 grid gap-1 text-xs">
        {rows.map(([label, value]) => (
          <div className="grid grid-cols-[72px_minmax(0,1fr)] gap-2" key={label}>
            <dt className="text-muted">{label}</dt>
            <dd className="truncate text-foreground" title={value}>
              {value}
            </dd>
          </div>
        ))}
      </dl>
      {warnings.length ? <MessageList tone="warning" messages={warnings} /> : null}
      {errors.length ? <MessageList tone="error" messages={errors} /> : null}
    </div>
  );
}

function loopLabel(loopMode: string, loopCount: number | null) {
  if (loopMode === "count") {
    return `count ${loopCount ?? 1}`;
  }
  if (loopMode === "once") {
    return "once";
  }
  return loopMode || "preserve";
}

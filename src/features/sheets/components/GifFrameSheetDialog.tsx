import { useEffect, useRef, useState } from "react";
import type { DragEvent, ReactNode } from "react";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import { copyAiHandoffPrompt } from "@/features/editor/ai-provider-model";
import { listExportProfiles, openExportPath } from "@/features/export/api";
import type { ExportProfile } from "@/features/export/types";
import {
  analyzeGifFrameSheetExport,
  exportGifFrameSheet,
  reimportGifFrameSheet,
  validateGifFrameSheetReimport,
} from "@/features/sheets/api";
import { SheetGridPresetSelect } from "@/features/sheets/components/SheetGridPresetSelect";
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
              onContinueToReimport={() => setActiveMode("reimport")}
              onOpenAiSite={onOpenAiSite}
              onSettingsChange={setSettings}
            />
          ) : (
            <GifFrameReimportPanel
              collection={collection}
              icon={icon}
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
  onContinueToReimport,
  onOpenAiSite,
  onSettingsChange,
}: {
  aiWebWorkflow?: boolean;
  collectionId: string;
  icon: IconSummary;
  settings: GifFrameSheetSettings;
  onContinueToReimport?: () => void;
  onOpenAiSite?: (resource: GifAiWebResource) => Promise<void>;
  onSettingsChange: (settings: GifFrameSheetSettings) => void;
}) {
  const [analysis, setAnalysis] = useState<GifFrameSheetExportAnalysis | null>(null);
  const [result, setResult] = useState<GifFrameSheetExportResult | null>(null);
  const [aiWebPrompt, setAiWebPrompt] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isWorking, setIsWorking] = useState(false);

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
    setResult(null);
    setAiWebPrompt(null);
  }, [settings]);

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
              AI 왕복에서는 alpha와 재조립 정보를 지키기 위해 transparent 배경, clean PNG,
              manifest JSON을 필수로 고정합니다.
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
            setErrorMessage(null);
            void exportGifFrameSheet(icon.id, settingsAtExport)
              .then((nextResult) => {
                setResult(nextResult);
                setAiWebPrompt(
                  aiWebWorkflow
                    ? buildGifAiWebPrompt({
                        analysis: analysisAtExport,
                        result: nextResult,
                        settings: settingsAtExport,
                      })
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
  onVariantCreated,
}: {
  collection: CollectionSummary;
  icon: IconSummary;
  onVariantCreated: () => Promise<void>;
}) {
  const [manifestFile, setManifestFile] = useState<File | null>(null);
  const [sheetFiles, setSheetFiles] = useState<File[]>([]);
  const [validation, setValidation] = useState<GifFrameSheetReimportValidation | null>(null);
  const [result, setResult] = useState<GifFrameSheetReimportResult | null>(null);
  const [profiles, setProfiles] = useState<ExportProfile[]>([]);
  const [targetProfileId, setTargetProfileId] = useState<string | null>(null);
  const [setActiveVariant, setSetActiveVariant] = useState(false);
  const [isWorking, setIsWorking] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const canSetActive = icon.shape === "single";

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
    if (!manifestFile || sheetFiles.length === 0) {
      setValidation(null);
      return;
    }
    let cancelled = false;
    setErrorMessage(null);
    void validateGifFrameSheetReimport(manifestFile, sheetFiles)
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
      });
    return () => {
      cancelled = true;
    };
  }, [manifestFile, sheetFiles]);

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    const files = Array.from(event.dataTransfer.files);
    const manifest = files.find((file) => /\.json$/i.test(file.name));
    const sheets = files.filter((file) => /\.png$/i.test(file.name));
    if (manifest) {
      setManifestFile(manifest);
    }
    if (sheets.length > 0) {
      setSheetFiles(sheets);
    }
  };

  const errors = validation?.errors ?? [];

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
      <section
        className="rounded-md border border-dashed border-border bg-white p-4"
        onDragOver={(event) => event.preventDefault()}
        onDrop={handleDrop}
      >
        <h3 className="text-sm font-semibold">수정된 프레임 시트 선택</h3>
        <p className="mt-2 text-sm text-muted">
          frames_manifest.json과 수정한 frames_sheet PNG를 함께 놓으세요. frames_sheet_001.png 같은 파일명을 바꾸면 안전을 위해 가져오지 않습니다.
        </p>
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          <label className="flex flex-col gap-1 text-xs font-medium text-muted">
            Manifest JSON
            <input
              accept="application/json,.json"
              className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
              type="file"
              onChange={(event) => setManifestFile(event.currentTarget.files?.[0] ?? null)}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs font-medium text-muted">
            Edited frame sheet PNG
            <input
              accept="image/png,.png"
              className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
              multiple
              type="file"
              onChange={(event) => setSheetFiles(Array.from(event.currentTarget.files ?? []))}
            />
          </label>
        </div>
        <div className="mt-4 grid gap-2 text-xs text-muted">
          <p>Manifest: {manifestFile?.name ?? "-"}</p>
          <p>Sheets: {sheetFiles.length > 0 ? sheetFiles.map((file) => file.name).join(", ") : "-"}</p>
        </div>
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
          disabled={!manifestFile || sheetFiles.length === 0 || errors.length > 0 || isWorking}
          type="button"
          onClick={() => {
            if (!manifestFile) {
              return;
            }
            setIsWorking(true);
            setResult(null);
            setErrorMessage(null);
            void reimportGifFrameSheet(
              icon.id,
              manifestFile,
              sheetFiles,
              setActiveVariant && canSetActive,
              setActiveVariant && canSetActive ? targetProfileId : null,
            )
              .then(async (nextResult) => {
                setResult(nextResult);
                await onVariantCreated();
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
  aiWebPrompt,
  result,
  onContinueToReimport,
  onOpenAiSite,
  onOpenFolder,
}: {
  aiWebPrompt?: string | null;
  result: GifFrameSheetExportResult;
  onContinueToReimport?: () => void;
  onOpenAiSite?: (resource: GifAiWebResource) => Promise<void>;
  onOpenFolder: (path: string) => Promise<void> | void;
}) {
  return (
    <div data-testid="gif-frame-export-result">
      <ResultBlock
        title="내보내기 완료"
        rows={[
          ["Output", result.outputDirectory],
          ["Clean", `${result.frameSheetPaths.length} files`],
          ["Guide", `${result.guideSheetPaths.length} files`],
          ["Manifest", result.manifestPath ?? "-"],
          ["Frames", String(result.frameCount)],
          ["Pages", String(result.pageCount)],
        ]}
        warnings={result.warnings}
      />
      <button
        className="mt-3 w-full rounded-md border border-border bg-white px-3 py-2 text-sm font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        data-testid="gif-frame-export-open-folder"
        type="button"
        onClick={() => void onOpenFolder(result.outputDirectory)}
      >
        결과 폴더 열기
      </button>
      {aiWebPrompt && onOpenAiSite && onContinueToReimport ? (
        <GifAiWebExportActions
          prompt={aiWebPrompt}
          onContinueToReimport={onContinueToReimport}
          onOpenAiSite={onOpenAiSite}
        />
      ) : null}
    </div>
  );
}

export function GifAiWebExportActions({
  prompt,
  onContinueToReimport,
  onOpenAiSite,
}: {
  prompt: string;
  onContinueToReimport: () => void;
  onOpenAiSite: (resource: GifAiWebResource) => Promise<void>;
}) {
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const [workingResource, setWorkingResource] =
    useState<GifAiWebResource | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const copyPrompt = async () => {
    const result = await copyAiHandoffPrompt(prompt, {
      clipboardWriteText:
        typeof navigator !== "undefined" && navigator.clipboard?.writeText
          ? (value) => navigator.clipboard.writeText(value)
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
    const copied = result === "clipboard" || result === "fallback";
    setStatusMessage(
      copied ? "GIF 웹 AI 프롬프트를 복사했습니다." : null,
    );
    setErrorMessage(
      copied
        ? null
        : "프롬프트 자동 복사에 실패했습니다. 아래 내용을 직접 복사한 뒤 공식 사이트를 열어 주세요.",
    );
    return copied;
  };

  const openSite = async (resource: GifAiWebResource) => {
    if (workingResource) return;
    setWorkingResource(resource);
    setStatusMessage(null);
    setErrorMessage(null);
    const copied = await copyPrompt();
    try {
      await onOpenAiSite(resource);
      if (copied) {
        setStatusMessage(
          `${resource === "gemini_ai_studio" ? "Gemini AI Studio" : "NovelAI"} 공식 사이트를 열었습니다. 수정된 clean PNG를 받은 뒤 다시 가져오세요.`,
        );
        onContinueToReimport();
      } else {
        setErrorMessage(
          "공식 사이트는 열었지만 프롬프트를 복사하지 못했습니다. 아래 내용을 직접 복사하면 다시 가져오기 단계로 이동할 수 있습니다.",
        );
      }
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
        <li>clean PNG 페이지와 아래 프롬프트를 공식 웹 AI에 전달합니다.</li>
        <li>같은 파일명의 PNG만 내려받습니다. GIF·JPG·WebP 결과는 사용하지 않습니다.</li>
        <li>앱으로 돌아오면 다시 가져오기 탭에서 manifest와 수정 PNG를 선택합니다.</li>
      </ol>
      <label className="mt-3 block text-xs font-semibold" htmlFor="gif-ai-web-prompt">
        구조 보호 프롬프트
      </label>
      <textarea
        className="mt-1 min-h-36 w-full resize-y rounded-md border border-border bg-white p-2 text-[11px] leading-4"
        data-testid="gif-ai-web-prompt"
        id="gif-ai-web-prompt"
        readOnly
        ref={promptRef}
        value={prompt}
      />
      <button
        className="mt-2 w-full rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
        data-testid="gif-ai-copy-prompt"
        disabled={workingResource !== null}
        type="button"
        onClick={() => void copyPrompt()}
      >
        프롬프트만 복사
      </button>
      <div className="mt-2 grid gap-2">
        <button
          className="rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
          data-testid="gif-ai-open-gemini"
          disabled={workingResource !== null}
          type="button"
          onClick={() => void openSite("gemini_ai_studio")}
        >
          프롬프트 복사 + Gemini AI Studio 열기
        </button>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
          data-testid="gif-ai-open-novelai"
          disabled={workingResource !== null}
          type="button"
          onClick={() => void openSite("novelai_app")}
        >
          프롬프트 복사 + NovelAI 열기
        </button>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
          disabled={workingResource !== null}
          type="button"
          onClick={onContinueToReimport}
        >
          수정 PNG를 받았어요 · 다시 가져오기
        </button>
      </div>
      <p className="mt-2 text-[11px] leading-4 text-muted">
        공식 사이트를 열면 프롬프트를 먼저 복사하고 앱은 다시 가져오기 탭으로 이어집니다.
        원본 GIF와 frame timing·loop는 바뀌지 않습니다.
      </p>
      {statusMessage ? <p className="mt-2 text-xs text-success" role="status">{statusMessage}</p> : null}
      {errorMessage ? <p className="mt-2 text-xs text-danger" role="alert">{errorMessage}</p> : null}
    </section>
  );
}

export function buildGifAiWebPrompt({
  analysis,
  result,
  settings,
}: {
  analysis: GifFrameSheetExportAnalysis;
  result: GifFrameSheetExportResult;
  settings: GifFrameSheetSettings;
}) {
  const cleanFiles = result.frameSheetPaths
    .map((path) => fileNameFromPath(path))
    .join(", ");
  const manifestName = result.manifestPath
    ? fileNameFromPath(result.manifestPath)
    : "frames_manifest.json";
  return [
    "[PMTCONCON Studio · GIF 프레임 시트 수정]",
    "이 요청은 GIF 생성이 아니라 clean PNG 프레임 시트의 셀 내부 이미지만 수정하는 작업입니다.",
    `편집 대상 PNG: ${cleanFiles || `${result.pageCount}개의 clean PNG 페이지`}`,
    `참조 manifest: ${manifestName}`,
    "",
    "필수 일관성:",
    "- 모든 페이지와 모든 프레임에서 캐릭터의 얼굴, 체형, 의상, 색상, 선화와 전체 그림체를 동일하게 유지하세요.",
    "- 프레임 사이의 움직임만 자연스럽게 이어지게 하고 캐릭터 정체성이나 카메라 구도를 임의로 바꾸지 마세요.",
    "",
    "절대 변경하지 말아야 할 구조:",
    `- 총 ${analysis.frameCount}프레임, ${result.pageCount}페이지, 각 PNG 캔버스 ${analysis.sheetWidth}×${analysis.sheetHeight}px.`,
    `- 셀 ${settings.frameCellWidth}×${settings.frameCellHeight}px, ${analysis.columns}열 × 페이지당 ${analysis.rowsPerPage}행, gap ${settings.gapX}/${settings.gapY}px, border ${settings.borderX}/${settings.borderY}px.`,
    "- 페이지 번호 순서와 각 페이지의 왼쪽→오른쪽, 위→아래 row-major 셀 순서를 그대로 유지하세요.",
    "- 셀 위치·크기·개수, 페이지 수, 파일 수와 파일명을 바꾸거나 프레임을 추가·삭제·병합·분할하지 마세요.",
    "- 투명 배경과 픽셀별 alpha를 그대로 유지하고, 비어 있는 셀은 완전히 투명한 상태로 두세요.",
    "- guide PNG와 manifest JSON은 참조 전용이며 수정하거나 결과물로 다시 만들지 마세요.",
    "",
    "반환 형식:",
    "- 입력 clean PNG 한 장당 같은 파일명의 PNG 한 장만 반환하세요.",
    "- PNG만 반환하세요. GIF, JPG, JPEG, WebP, PDF, ZIP 또는 설명문은 반환하지 마세요.",
    "",
    `원본 GIF 메타데이터는 앱이 별도로 보존합니다: 총 재생시간 ${analysis.durationMs}ms, loop ${gifAiLoopPromptLabel(analysis.loopMode, analysis.loopCount)}.`,
    "프레임 timing·재생 순서·loop는 수정 대상이 아니며 다시 가져올 때 manifest에서 복원됩니다. 원본 GIF도 덮어쓰지 않습니다.",
  ].join("\n");
}

function fileNameFromPath(path: string) {
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
    <ul className={`mt-3 grid gap-1 text-sm ${tone === "error" ? "text-danger" : "text-muted"}`}>
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

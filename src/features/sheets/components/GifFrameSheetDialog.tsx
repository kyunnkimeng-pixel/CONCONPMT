import { useEffect, useRef, useState } from "react";
import type { DragEvent, ReactNode } from "react";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import { listExportProfiles } from "@/features/export/api";
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
import { getCommandErrorMessage } from "@/lib/tauri";
import { useModalFocus } from "@/lib/use-modal-focus";

type GifFrameSheetMode = "export" | "reimport";

interface GifFrameSheetDialogProps {
  collection: CollectionSummary;
  icon: IconSummary;
  mode: GifFrameSheetMode;
  onClose: () => void;
  onVariantCreated: () => Promise<void>;
}

export function GifFrameSheetDialog({
  collection,
  icon,
  mode,
  onClose,
  onVariantCreated,
}: GifFrameSheetDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);
  useModalFocus(dialogRef, onClose);
  const defaultCellWidth = icon.cellWidthOverride ?? collection.defaultCellWidth;
  const defaultCellHeight = icon.cellHeightOverride ?? collection.defaultCellHeight;
  const [activeMode, setActiveMode] = useState<GifFrameSheetMode>(mode);
  const [settings, setSettings] = useState<GifFrameSheetSettings>(() =>
    defaultGifFrameSheetSettings(defaultCellWidth, defaultCellHeight),
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/35 px-4 py-5">
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
            <p className="mt-1 truncate text-sm text-muted">
              {icon.displayName} · GIF · 원본은 보존되고 결과는 processed variant로 생성됩니다.
            </p>
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
              collectionId={collection.id}
              icon={icon}
              settings={settings}
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

function GifFrameExportPanel({
  collectionId,
  icon,
  settings,
  onSettingsChange,
}: {
  collectionId: string;
  icon: IconSummary;
  settings: GifFrameSheetSettings;
  onSettingsChange: (settings: GifFrameSheetSettings) => void;
}) {
  const [analysis, setAnalysis] = useState<GifFrameSheetExportAnalysis | null>(null);
  const [result, setResult] = useState<GifFrameSheetExportResult | null>(null);
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
          onApplyPreset={(preset) =>
            onSettingsChange(applyPresetToGifFrameSettings(settings, preset))
          }
        />

        <section className="rounded-md border border-border bg-white p-4">
          <h3 className="text-sm font-semibold">프레임 시트 설정</h3>
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
            <CheckField label="Clean frame sheet PNG" checked={settings.includeCleanSheet} onChange={(checked) => onSettingsChange({ ...settings, includeCleanSheet: checked })} />
            <CheckField label="Guide frame sheet PNG" checked={settings.includeGuideSheet} onChange={(checked) => onSettingsChange({ ...settings, includeGuideSheet: checked })} />
            <CheckField label="Manifest JSON" checked={settings.includeManifest} onChange={(checked) => onSettingsChange({ ...settings, includeManifest: checked })} />
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
            setIsWorking(true);
            setResult(null);
            setErrorMessage(null);
            void exportGifFrameSheet(icon.id, settings)
              .then(setResult)
              .catch((error) => setErrorMessage(getCommandErrorMessage(error)))
              .finally(() => setIsWorking(false));
          }}
        >
          {isWorking ? "내보내는 중" : "GIF 프레임 시트 내보내기"}
        </button>
        {result ? (
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
          frames_manifest.json과 수정한 frames_sheet PNG 파일을 선택하거나 이 영역으로 드래그해서 놓습니다.
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
        <h3 className="text-sm font-semibold">Reimport 결과</h3>
        <p className="mt-2 text-sm text-muted">
          새 GIF processed variant를 만들고 원본 GIF는 변경하지 않습니다.
        </p>
        <div className="mt-4 grid gap-3">
          <CheckField
            disabled={!canSetActive}
            label={canSetActive ? "Export 활성 variant로 설정" : "single GIF만 export 활성 설정 가능"}
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
          {isWorking ? "다시 가져오는 중" : "GIF variant 만들기"}
        </button>
        {result ? (
          <ResultBlock
            title="다시 가져오기 완료"
            rows={[
              ["Variant", result.variantId ?? "-"],
              ["Output", result.outputPath ?? "-"],
              ["Frames", String(result.frameCount)],
              ["Duration", `${result.durationMs}ms`],
              ["Active", result.activeVariantSet ? "set" : "not set"],
            ]}
            warnings={result.warnings}
            errors={result.errors}
          />
        ) : null}
      </aside>
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

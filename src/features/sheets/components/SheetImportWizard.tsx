import { useEffect, useMemo, useState } from "react";
import { Grid3X3, X } from "lucide-react";

import type { CollectionSummary } from "@/features/collections/types";
import { analyzeSheetGrid, autoDetectSheetGrid, importSheetCells } from "@/features/sheets/api";
import { SheetCellReviewGrid } from "@/features/sheets/components/SheetCellReviewGrid";
import { SheetAutoDetectPanel } from "@/features/sheets/components/SheetAutoDetectPanel";
import { SheetGridPresetSelect } from "@/features/sheets/components/SheetGridPresetSelect";
import { SheetGridOverlay } from "@/features/sheets/components/SheetGridOverlay";
import { SheetGridSettingsPanel } from "@/features/sheets/components/SheetGridSettingsPanel";
import { SheetImagePicker } from "@/features/sheets/components/SheetImagePicker";
import { ManualSliceCanvas } from "@/features/sheets/components/ManualSliceCanvas";
import { SheetReimportDialog } from "@/features/sheets/components/SheetReimportDialog";
import {
  applyPresetToImportSettings,
  defaultSheetGridSettings,
  nextSelectionAfterCellClick,
  presetInputFromImportSettings,
} from "@/features/sheets/sheet-ui-model";
import type {
  AutoDetectSheetGridProposal,
  AutoDetectSheetGridResult,
  SheetGridAnalysis,
  SheetGridMode,
  SheetGridSettings,
} from "@/features/sheets/types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

type Step = "source" | "mode" | "auto" | "grid" | "review" | "manual" | "manifest";

export function SheetImportWizard({
  collection,
  onClose,
  onImported,
}: {
  collection: CollectionSummary;
  onClose: () => void;
  onImported: () => Promise<void>;
}) {
  const [step, setStep] = useState<Step>("source");
  const [file, setFile] = useState<File | null>(null);
  const [settings, setSettings] = useState<SheetGridSettings>(() => defaultSheetGridSettings());
  const [analysis, setAnalysis] = useState<SheetGridAnalysis | null>(null);
  const [autoDetectResult, setAutoDetectResult] = useState<AutoDetectSheetGridResult | null>(null);
  const [selectedIndexes, setSelectedIndexes] = useState<Set<number>>(new Set());
  const [displayNamePattern, setDisplayNamePattern] = useState("sheet_{number}");
  const [isRunning, setIsRunning] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const imageUrl = useObjectUrl(file);
  const selectedImportableCount = useMemo(
    () =>
      analysis
        ? analysis.cells.filter(
            (cell) =>
              selectedIndexes.has(cell.index) && !cell.outOfBounds && !cell.emptyCandidate,
          ).length
        : 0,
    [analysis, selectedIndexes],
  );

  const runPreview = async (overrideSettings?: SheetGridSettings) => {
    if (!file) {
      return;
    }
    const previewSettings = overrideSettings ?? settings;
    setIsRunning(true);
    setErrorMessage(null);
    setMessage(null);
    try {
      const nextAnalysis = await analyzeSheetGrid(file, previewSettings);
      if (overrideSettings) {
        setSettings(overrideSettings);
      }
      setAnalysis(nextAnalysis);
      setSelectedIndexes(
        new Set(
          nextAnalysis.cells
            .filter((cell) => !cell.outOfBounds && !cell.emptyCandidate)
            .map((cell) => cell.index),
        ),
      );
      setStep("grid");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsRunning(false);
    }
  };

  const runAutoDetect = async () => {
    if (!file) {
      return;
    }
    setIsRunning(true);
    setErrorMessage(null);
    setMessage(null);
    try {
      const result = await autoDetectSheetGrid(file);
      setAutoDetectResult(result);
      setStep("auto");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsRunning(false);
    }
  };

  const applyAutoDetectProposal = (proposal: AutoDetectSheetGridProposal) => {
    void runPreview(proposal.gridSettings);
  };

  const runImport = async () => {
    if (!file || !analysis) {
      return;
    }
    setIsRunning(true);
    setErrorMessage(null);
    setMessage(null);
    try {
      const result = await importSheetCells(
        collection.id,
        file,
        settings,
        [...selectedIndexes],
        displayNamePattern,
      );
      await onImported();
      setMessage(
        `${result.importedCount}개 셀을 가져왔습니다. ${result.skippedCells.length}개 셀은 건너뛰었습니다.`,
      );
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsRunning(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/35 p-3 sm:p-6">
      <div className="flex max-h-[92vh] w-full max-w-7xl flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-xl">
        <header className="flex items-center justify-between gap-3 border-b border-border px-5 py-4">
          <div>
            <h2 className="flex items-center gap-2 text-base font-semibold">
              <Grid3X3 aria-hidden="true" />
              시트 가져오기
            </h2>
            <p className="mt-1 text-sm text-muted">{collection.name} 모음에 고정 그리드 시트를 가져옵니다.</p>
          </div>
          <button
            aria-label="닫기"
            className="flex size-9 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover"
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </header>

        <div className="flex min-h-0 flex-1 flex-col overflow-hidden lg:flex-row">
          <main className="flex min-w-0 flex-1 flex-col gap-4 overflow-auto p-5">
            <StepTabs step={step} onStepChange={setStep} />
            <SheetGridPresetSelect
              collectionId={collection.id}
              compatibleKinds={["static_import", "static_import_export", "static_export"]}
              currentSummary={`${settings.cellWidth ?? 200}x${settings.cellHeight ?? 200} · ${
                settings.columns ?? "-"
              }열 · gap ${settings.gapX}/${settings.gapY} · border ${
                settings.borderLeft
              }/${settings.borderTop}`}
              saveKindLabel="가져오기/내보내기 공유"
              target="import"
              buildPresetInput={(name) =>
                presetInputFromImportSettings(name, collection.id, settings)
              }
              onApplyPreset={(preset) =>
                setSettings((current) => applyPresetToImportSettings(current, preset))
              }
            />
            {step === "source" ? (
              <div className="grid gap-4 lg:grid-cols-[1fr_320px]">
                <SheetImagePicker file={file} onFileChange={setFile} />
                <div className="rounded-md border border-border bg-white p-4 text-sm text-muted">
                  {file ? (
                    <div className="flex flex-col gap-2">
                      <span className="font-medium text-foreground">{file.name}</span>
                      <span>{Math.round(file.size / 1024)} KB</span>
                      <span>PNG는 알파를 보존하고 JPG는 투명도가 없다는 경고를 표시합니다.</span>
                    </div>
                  ) : (
                    "먼저 시트 파일을 선택하세요."
                  )}
                </div>
              </div>
            ) : null}

            {step === "mode" ? (
              <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                <ModeButton
                  label="Grid로 자르기"
                  selected={settings.mode === "rows_columns"}
                  onClick={() => {
                    setSettingsForMode("rows_columns", settings, setSettings);
                    setStep("grid");
                  }}
                />
                <ModeButton
                  label="셀 크기로 자르기"
                  selected={settings.mode === "cell_size"}
                  onClick={() => {
                    setSettingsForMode("cell_size", settings, setSettings);
                    setStep("grid");
                  }}
                />
                <button
                  className="rounded-md border border-border bg-white p-4 text-left hover:bg-menu-hover"
                  type="button"
                  onClick={() => setStep("manifest")}
                >
                  <span className="font-semibold">Manifest로 복원</span>
                  <span className="mt-1 block text-sm text-muted">pmtcon-sheet-v1 기반 다시 가져오기</span>
                </button>
                <button
                  className="rounded-md border border-border bg-white p-4 text-left hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
                  disabled={!file}
                  type="button"
                  onClick={() => setStep("manual")}
                >
                  <span className="font-semibold">직접 Slice 지정</span>
                  <span className="mt-1 block text-sm text-muted">
                    사각형 Slice를 직접 추가하고 좌표를 입력해서 가져오기
                  </span>
                </button>
                <button
                  className="rounded-md border border-border bg-white p-4 text-left hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
                  disabled={!file || isRunning}
                  type="button"
                  onClick={() => void runAutoDetect()}
                >
                  <span className="font-semibold">자동 감지 (실험)</span>
                  <span className="mt-1 block text-sm text-muted">
                    투명/단색 separator를 찾아 grid 설정 후보만 제안합니다.
                  </span>
                </button>
              </div>
            ) : null}

            {step === "auto" ? (
              <SheetAutoDetectPanel
                errorMessage={errorMessage}
                file={file}
                isRunning={isRunning}
                result={autoDetectResult}
                onApplyProposal={applyAutoDetectProposal}
                onRun={() => void runAutoDetect()}
              />
            ) : null}

            {step === "grid" && file && !analysis ? (
              <div className="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_280px]">
                <div className="sheet-checkerboard flex min-h-[360px] items-center justify-center overflow-hidden rounded-md border border-border bg-white p-3">
                  {imageUrl ? (
                    <img
                      alt="시트 미리보기"
                      className="max-h-[58vh] max-w-full object-contain"
                      src={imageUrl}
                    />
                  ) : null}
                </div>
                <div className="rounded-md border border-border bg-white p-4 text-sm text-muted">
                  오른쪽 분할 설정에서 행/열 또는 셀 크기, 여백, 간격을 입력한 뒤 미리보기 갱신을 누르세요.
                  프리셋을 적용한 경우에도 수치는 직접 수정할 수 있습니다.
                </div>
              </div>
            ) : null}

            {step === "grid" && analysis ? (
              <SheetGridOverlay
                cells={analysis.cells}
                imageUrl={imageUrl}
                selectedIndexes={selectedIndexes}
                sheetHeight={analysis.sheetHeight}
                sheetWidth={analysis.sheetWidth}
                onToggleCell={(cellIndex, multi) =>
                  setSelectedIndexes((current) =>
                    nextSelectionAfterCellClick(current, cellIndex, { multi }),
                  )
                }
              />
            ) : null}

            {step === "review" && analysis ? (
              <div className="flex min-h-0 flex-1 flex-col gap-4">
                <label className="flex max-w-sm flex-col gap-1 text-xs font-medium text-muted">
                  표시 이름 패턴
                  <input
                    className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground"
                    value={displayNamePattern}
                    onChange={(event) => setDisplayNamePattern(event.currentTarget.value)}
                  />
                </label>
                <SheetCellReviewGrid
                  cells={analysis.cells}
                  selectedIndexes={selectedIndexes}
                  onSelectionChange={setSelectedIndexes}
                />
              </div>
            ) : null}

            {step === "manifest" ? (
              <SheetReimportDialog
                collectionId={collection.id}
                onImported={async () => {
                  await onImported();
                }}
              />
            ) : null}

            {step === "manual" && file ? (
              <ManualSliceCanvas
                collection={collection}
                file={file}
                imageUrl={imageUrl}
                onImported={onImported}
              />
            ) : null}

            {analysis?.warnings.length ? (
              <div className="rounded-md border border-border bg-white p-3 text-sm text-muted">
                {analysis.warnings.join(" / ")}
              </div>
            ) : null}
            {message ? <p className="text-sm text-muted">{message}</p> : null}
            {errorMessage ? (
              <p className="text-sm text-danger" role="alert">
                {errorMessage}
              </p>
            ) : null}
          </main>

          {(step === "mode" || step === "grid" || step === "review") && file ? (
            <SheetGridSettingsPanel
              settings={settings}
              onChange={setSettings}
              onPreview={() => void runPreview()}
              onReset={() => setSettings(defaultSheetGridSettings())}
            />
          ) : null}
        </div>

        <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-4">
          <span className="text-sm text-muted">
            {analysis ? `${analysis.cellCount}개 셀, 가져오기 대상 ${selectedImportableCount}개` : "검토 후에만 가져옵니다."}
          </span>
          <div className="flex flex-wrap justify-end gap-2">
            <button className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover" type="button" onClick={onClose}>
              취소
            </button>
            {step === "source" ? (
              <button className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-60" disabled={!file} type="button" onClick={() => setStep("mode")}>
                다음
              </button>
            ) : null}
            {step === "mode" ? (
              <button className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-60" disabled={!file || isRunning} type="button" onClick={() => void runPreview()}>
                Grid 미리보기
              </button>
            ) : null}
            {step === "grid" ? (
              <button className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong" type="button" onClick={() => setStep("review")}>
                셀 검토
              </button>
            ) : null}
            {step === "review" ? (
              <button className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-60" disabled={selectedImportableCount === 0 || isRunning} type="button" onClick={() => void runImport()}>
                {isRunning ? "가져오는 중" : "선택한 셀 가져오기"}
              </button>
            ) : null}
          </div>
        </footer>
      </div>
    </div>
  );
}

function StepTabs({ step, onStepChange }: { step: Step; onStepChange: (step: Step) => void }) {
  const tabs: Array<{ id: Step; label: string }> = [
    { id: "source", label: "1. 파일" },
    { id: "mode", label: "2. 방식" },
    { id: "auto", label: "자동 감지" },
    { id: "grid", label: "3. Grid" },
    { id: "review", label: "4. 검토" },
    { id: "manual", label: "직접 Slice" },
    { id: "manifest", label: "Manifest" },
  ];
  return (
    <div className="-mx-1 overflow-x-auto pb-1" data-testid="sheet-step-tabs-scroll">
      <div className="flex w-max min-w-full gap-1 px-1">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={cn(
              "flex-none whitespace-nowrap rounded-md px-3 py-2 text-sm font-medium hover:bg-menu-hover",
              step === tab.id ? "bg-selected text-foreground" : "text-muted",
            )}
            type="button"
            onClick={() => onStepChange(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function ModeButton({
  label,
  selected,
  onClick,
}: {
  label: string;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={cn(
        "rounded-md border border-border bg-white p-4 text-left hover:bg-menu-hover",
        selected ? "outline outline-2 outline-focus" : "",
      )}
      type="button"
      onClick={onClick}
    >
      <span className="font-semibold">{label}</span>
      <span className="mt-1 block text-sm text-muted">고정 수치 기반 수동 분할</span>
    </button>
  );
}

function setSettingsForMode(
  mode: SheetGridMode,
  settings: SheetGridSettings,
  onChange: (settings: SheetGridSettings) => void,
) {
  onChange({
    ...settings,
    mode,
    cellWidth: mode === "cell_size" ? (settings.cellWidth ?? 200) : settings.cellWidth,
    cellHeight: mode === "cell_size" ? (settings.cellHeight ?? 200) : settings.cellHeight,
  });
}

function useObjectUrl(file: File | null) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!file) {
      setUrl(null);
      return undefined;
    }
    const nextUrl = URL.createObjectURL(file);
    setUrl(nextUrl);
    return () => URL.revokeObjectURL(nextUrl);
  }, [file]);
  return url;
}

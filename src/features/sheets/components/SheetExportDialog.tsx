import { useState } from "react";
import { Download, FolderOpen, X } from "lucide-react";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import { openExportPath, pickExportDirectory } from "@/features/export/api";
import { exportEditSheet } from "@/features/sheets/api";
import { SheetExportPreview } from "@/features/sheets/components/SheetExportPreview";
import { defaultExportSheetRequest } from "@/features/sheets/sheet-ui-model";
import type { ExportEditSheetRequest, ExportEditSheetResult, SheetBackground } from "@/features/sheets/types";
import { getCommandErrorMessage } from "@/lib/tauri";

export function SheetExportDialog({
  collection,
  icons,
  onClose,
}: {
  collection: CollectionSummary;
  icons: IconSummary[];
  onClose: () => void;
}) {
  const [request, setRequest] = useState<ExportEditSheetRequest>(() =>
    defaultExportSheetRequest(collection.id),
  );
  const [isExporting, setIsExporting] = useState(false);
  const [result, setResult] = useState<ExportEditSheetResult | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const itemCount = icons.reduce((count, icon) => count + Math.max(1, icon.pieces.length), 0);

  const updateNumber = (
    field: keyof Pick<
      ExportEditSheetRequest,
      | "cellWidth"
      | "cellHeight"
      | "columns"
      | "gapX"
      | "gapY"
      | "borderX"
      | "borderY"
      | "maxSheetWidth"
      | "maxSheetHeight"
    >,
    value: string,
  ) => {
    const parsed = Number.parseInt(value, 10);
    setRequest((current) => ({
      ...current,
      [field]: Number.isFinite(parsed) ? Math.max(1, parsed) : current[field],
    }));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/35 p-6">
      <div className="flex max-h-[92vh] w-full max-w-5xl flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-xl">
        <header className="flex items-center justify-between gap-3 border-b border-border px-5 py-4">
          <div>
            <h2 className="flex items-center gap-2 text-base font-semibold">
              <Download aria-hidden="true" />
              작업 시트로 내보내기
            </h2>
            <p className="mt-1 text-sm text-muted">
              고정 grid clean sheet, guide sheet, pmtcon-sheet-v1 manifest를 생성합니다.
            </p>
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

        <main className="grid min-h-0 flex-1 gap-4 overflow-auto p-5 lg:grid-cols-[1fr_320px]">
          <div className="flex flex-col gap-4">
            <section className="rounded-md border border-border bg-white p-4">
              <h3 className="text-sm font-semibold">대상</h3>
              <div className="mt-3 rounded-md bg-preview p-3 text-sm">
                현재 모음 전체: {icons.length}개 아이콘, {itemCount}개 셀
              </div>
              <p className="mt-2 text-xs text-muted">
                GIF 아이콘은 정적 contact sheet 기준으로 첫 프레임만 포함됩니다. GIF 재조립은 GIF 프레임 시트 단계에서 처리합니다.
              </p>
            </section>

            <section className="rounded-md border border-border bg-white p-4">
              <h3 className="text-sm font-semibold">Layout</h3>
              <div className="mt-3 grid gap-3 md:grid-cols-4">
                <NumberField label="Cell W" value={request.cellWidth} onChange={(value) => updateNumber("cellWidth", value)} />
                <NumberField label="Cell H" value={request.cellHeight} onChange={(value) => updateNumber("cellHeight", value)} />
                <NumberField label="Columns" value={request.columns} onChange={(value) => updateNumber("columns", value)} />
                <NumberField label="Gap X" value={request.gapX} onChange={(value) => updateNumber("gapX", value)} />
                <NumberField label="Gap Y" value={request.gapY} onChange={(value) => updateNumber("gapY", value)} />
                <NumberField label="Border X" value={request.borderX} onChange={(value) => updateNumber("borderX", value)} />
                <NumberField label="Border Y" value={request.borderY} onChange={(value) => updateNumber("borderY", value)} />
                <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                  Background
                  <select
                    className="rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
                    value={request.background}
                    onChange={(event) =>
                      setRequest((current) => ({
                        ...current,
                        background: event.currentTarget.value as SheetBackground,
                      }))
                    }
                  >
                    <option value="transparent">transparent</option>
                    <option value="checker">checker</option>
                    <option value="white">white</option>
                    <option value="black">black</option>
                  </select>
                </label>
                <NumberField label="Max W" value={request.maxSheetWidth} onChange={(value) => updateNumber("maxSheetWidth", value)} />
                <NumberField label="Max H" value={request.maxSheetHeight} onChange={(value) => updateNumber("maxSheetHeight", value)} />
              </div>
            </section>

            <section className="rounded-md border border-border bg-white p-4">
              <h3 className="text-sm font-semibold">Output</h3>
              <div className="mt-3 grid gap-2 md:grid-cols-3">
                <CheckField label="Clean sheet PNG" checked={request.includeCleanSheet} onChange={(checked) => setRequest((current) => ({ ...current, includeCleanSheet: checked }))} />
                <CheckField label="Guide sheet PNG" checked={request.includeGuideSheet} onChange={(checked) => setRequest((current) => ({ ...current, includeGuideSheet: checked }))} />
                <CheckField label="Manifest JSON" checked={request.includeManifest} onChange={(checked) => setRequest((current) => ({ ...current, includeManifest: checked }))} />
                <CheckField label="Guide cell number" checked={request.labelOptions.cellNumber} onChange={(checked) => setRequest((current) => ({ ...current, labelOptions: { ...current.labelOptions, cellNumber: checked } }))} />
                <CheckField label="Guide export number" checked={request.labelOptions.exportNumber} onChange={(checked) => setRequest((current) => ({ ...current, labelOptions: { ...current.labelOptions, exportNumber: checked } }))} />
                <CheckField label="Open folder" checked={request.openOutputFolder} onChange={(checked) => setRequest((current) => ({ ...current, openOutputFolder: checked }))} />
              </div>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <button
                  className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover"
                  type="button"
                  onClick={() => {
                    void pickExportDirectory(request.outputDirectory).then((directory) => {
                      if (directory) {
                        setRequest((current) => ({ ...current, outputDirectory: directory }));
                      }
                    });
                  }}
                >
                  <FolderOpen aria-hidden="true" />
                  출력 폴더
                </button>
                <span className="min-w-0 truncate text-sm text-muted">
                  {request.outputDirectory ?? "앱 데이터 sheet_exports/static"}
                </span>
              </div>
            </section>
          </div>

          <aside className="flex flex-col gap-4">
            <SheetExportPreview itemCount={itemCount} request={request} />
            {result ? (
              <section className="rounded-md border border-border bg-white p-4 text-sm">
                <h3 className="font-semibold">완료</h3>
                <p className="mt-2 text-muted">
                  {result.itemCount}개 셀, {result.pageCount}개 페이지를 생성했습니다.
                </p>
                {result.warnings.length ? (
                  <p className="mt-2 text-muted">{result.warnings.join(" / ")}</p>
                ) : null}
                <button
                  className="mt-3 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover"
                  type="button"
                  onClick={() => void openExportPath(result.outputDirectory)}
                >
                  결과 폴더 열기
                </button>
              </section>
            ) : null}
            {errorMessage ? (
              <p className="text-sm text-danger" role="alert">
                {errorMessage}
              </p>
            ) : null}
          </aside>
        </main>

        <footer className="flex justify-end gap-2 border-t border-border px-5 py-4">
          <button className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover" type="button" onClick={onClose}>
            닫기
          </button>
          <button
            className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:cursor-not-allowed disabled:opacity-60"
            disabled={isExporting || itemCount === 0}
            type="button"
            onClick={() => {
              setIsExporting(true);
              setErrorMessage(null);
              setResult(null);
              void exportEditSheet(request)
                .then(setResult)
                .catch((error) => setErrorMessage(getCommandErrorMessage(error)))
                .finally(() => setIsExporting(false));
            }}
          >
            {isExporting ? "내보내는 중" : "작업 시트 내보내기"}
          </button>
        </footer>
      </div>
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
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <input
        className="min-w-0 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
        min={1}
        type="number"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}

function CheckField({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input
        checked={checked}
        type="checkbox"
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      {label}
    </label>
  );
}

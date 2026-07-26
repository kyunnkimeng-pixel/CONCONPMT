import type { SheetGridSettings, SheetReadOrder } from "@/features/sheets/types";

export function SheetGridSettingsPanel({
  settings,
  disabled = false,
  onChange,
  onPreview,
  onReset,
}: {
  settings: SheetGridSettings;
  disabled?: boolean;
  onChange: (settings: SheetGridSettings) => void;
  onPreview: () => void;
  onReset: () => void;
}) {
  const updateNumber = (
    field: keyof Omit<SheetGridSettings, "mode" | "readOrder">,
    value: string,
  ) => {
    const parsed = Number.parseInt(value, 10);
    onChange({
      ...settings,
      [field]: value.trim() === "" || !Number.isFinite(parsed) ? null : Math.max(0, parsed),
    });
  };

  return (
    <aside
      className="flex max-h-80 w-full shrink-0 flex-col gap-3 overflow-y-auto border-t border-border bg-surface p-4 lg:max-h-none lg:w-80 lg:min-w-72 lg:border-l lg:border-t-0"
      data-testid="sheet-grid-settings-panel"
    >
      <div>
        <h3 className="text-sm font-semibold">분할 설정</h3>
        <p className="mt-1 text-xs text-muted">정확한 수치를 입력하고 미리보기로 확인합니다.</p>
      </div>
      <label className="flex flex-col gap-1 text-xs font-medium text-muted">
        분할 기준
        <select
          className="rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
          disabled={disabled}
          value={settings.mode}
          onChange={(event) =>
            onChange({ ...settings, mode: event.currentTarget.value as SheetGridSettings["mode"] })
          }
        >
          <option value="rows_columns">행/열</option>
          <option value="cell_size">셀 크기</option>
        </select>
      </label>
      <div className="grid grid-cols-2 gap-2">
        <NumberField disabled={disabled} label="열" value={settings.columns} onChange={(value) => updateNumber("columns", value)} />
        <NumberField disabled={disabled} label="행" value={settings.rows} onChange={(value) => updateNumber("rows", value)} />
        <NumberField disabled={disabled} label="셀 너비" value={settings.cellWidth} onChange={(value) => updateNumber("cellWidth", value)} />
        <NumberField disabled={disabled} label="셀 높이" value={settings.cellHeight} onChange={(value) => updateNumber("cellHeight", value)} />
        <NumberField disabled={disabled} label="좌측 여백" value={settings.borderLeft} onChange={(value) => updateNumber("borderLeft", value)} />
        <NumberField disabled={disabled} label="상단 여백" value={settings.borderTop} onChange={(value) => updateNumber("borderTop", value)} />
        <NumberField disabled={disabled} label="우측 여백" value={settings.borderRight} onChange={(value) => updateNumber("borderRight", value)} />
        <NumberField disabled={disabled} label="하단 여백" value={settings.borderBottom} onChange={(value) => updateNumber("borderBottom", value)} />
        <NumberField disabled={disabled} label="가로 간격" value={settings.gapX} onChange={(value) => updateNumber("gapX", value)} />
        <NumberField disabled={disabled} label="세로 간격" value={settings.gapY} onChange={(value) => updateNumber("gapY", value)} />
      </div>
      <label className="flex flex-col gap-1 text-xs font-medium text-muted">
        읽기 순서
        <select
          className="rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
          disabled={disabled}
          value={settings.readOrder}
          onChange={(event) =>
            onChange({ ...settings, readOrder: event.currentTarget.value as SheetReadOrder })
          }
        >
          <option value="row_major">좌→우, 위→아래</option>
          <option value="column_major">위→아래, 좌→우</option>
        </select>
      </label>
      <label className="flex flex-col gap-1 text-xs font-medium text-muted">
        빈 셀 alpha 기준
        <input
          className="w-full"
          disabled={disabled}
          max={1}
          min={0.5}
          step={0.01}
          type="range"
          value={settings.emptyCellThreshold}
          onChange={(event) =>
            onChange({ ...settings, emptyCellThreshold: Number(event.currentTarget.value) })
          }
        />
        <span>{Math.round(settings.emptyCellThreshold * 100)}%</span>
      </label>
      <div className="flex gap-2">
        <button
          className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:cursor-not-allowed disabled:opacity-60"
          disabled={disabled}
          type="button"
          onClick={onPreview}
        >
          미리보기 갱신
        </button>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
          disabled={disabled}
          title="분할 칸, 여백, 간격, 읽기 순서를 앱의 초기값으로 되돌립니다."
          type="button"
          onClick={onReset}
        >
          분할 설정 초기값
        </button>
      </div>
    </aside>
  );
}

function NumberField({
  disabled = false,
  label,
  value,
  onChange,
}: {
  disabled?: boolean;
  label: string;
  value: number | null;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <input
        className="min-w-0 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
        disabled={disabled}
        min={0}
        type="number"
        value={value ?? ""}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}

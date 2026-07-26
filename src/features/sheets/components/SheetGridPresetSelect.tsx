import { useEffect, useMemo, useRef, useState } from "react";
import { Copy, Save, Star, Trash2 } from "lucide-react";

import {
  createSheetGridPreset,
  deleteSheetGridPreset,
  duplicateSheetGridPreset,
  getDefaultSheetGridPreset,
  listSheetGridPresets,
  setDefaultSheetGridPreset,
} from "@/features/sheets/api";
import type {
  SheetGridPreset,
  SheetGridPresetInput,
  SheetGridPresetKind,
  SheetGridPresetTarget,
} from "@/features/sheets/types";
import { getCommandErrorMessage } from "@/lib/tauri";

interface SheetGridPresetSelectProps {
  collectionId: string;
  target: SheetGridPresetTarget;
  compatibleKinds: SheetGridPresetKind[];
  currentSummary: string;
  saveKindLabel: string;
  autoApplyDefault?: boolean;
  disabled?: boolean;
  buildPresetInput: (name: string) => SheetGridPresetInput;
  onApplyPreset: (preset: SheetGridPreset) => void;
}

export function SheetGridPresetSelect({
  collectionId,
  target,
  compatibleKinds,
  currentSummary,
  saveKindLabel,
  autoApplyDefault = true,
  disabled = false,
  buildPresetInput,
  onApplyPreset,
}: SheetGridPresetSelectProps) {
  const [presets, setPresets] = useState<SheetGridPreset[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [name, setName] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const didApplyDefaultRef = useRef(false);
  const compatibleKindSet = useMemo(() => new Set(compatibleKinds), [compatibleKinds]);
  const visiblePresets = useMemo(
    () =>
      presets.filter(
        (preset) =>
          compatibleKindSet.has(preset.kind) ||
          preset.kind === "static_import_export",
      ),
    [compatibleKindSet, presets],
  );
  const selectedPreset =
    visiblePresets.find((preset) => preset.id === selectedId) ?? visiblePresets[0] ?? null;

  useEffect(() => {
    let cancelled = false;
    setErrorMessage(null);
    void listSheetGridPresets(collectionId)
      .then((nextPresets) => {
        if (cancelled) {
          return;
        }
        setPresets(nextPresets);
        setSelectedId((current) =>
          current && nextPresets.some((preset) => preset.id === current)
            ? current
            : nextPresets[0]?.id ?? "",
        );
      })
      .catch((error) => {
        if (!cancelled) {
          setErrorMessage(getCommandErrorMessage(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [collectionId]);

  useEffect(() => {
    if (disabled || !autoApplyDefault || didApplyDefaultRef.current) {
      return;
    }
    let cancelled = false;
    void getDefaultSheetGridPreset(target, collectionId)
      .then((preset) => {
        if (!preset || cancelled || didApplyDefaultRef.current) {
          return;
        }
        if (
          compatibleKindSet.has(preset.kind) ||
          preset.kind === "static_import_export"
        ) {
          didApplyDefaultRef.current = true;
          setSelectedId(preset.id);
          onApplyPreset(preset);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [autoApplyDefault, collectionId, compatibleKindSet, disabled, onApplyPreset, target]);

  const reloadAndSelect = async (presetId: string) => {
    const nextPresets = await listSheetGridPresets(collectionId);
    setPresets(nextPresets);
    setSelectedId(presetId);
  };

  const run = async (action: () => Promise<void>) => {
    setStatus(null);
    setErrorMessage(null);
    try {
      await action();
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  return (
    <section
      className="rounded-md border border-border bg-white p-3"
      data-testid="sheet-grid-preset-select"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <h3 className="text-sm font-semibold">시트 프리셋</h3>
          <p className="mt-1 text-xs text-muted">{currentSummary}</p>
        </div>
        <span className="rounded bg-canvas px-2 py-1 text-[11px] font-medium text-muted">
          {saveKindLabel}
        </span>
      </div>

      <div className="mt-3 grid gap-2 lg:grid-cols-[minmax(0,1fr)_auto]">
        <select
          className="min-w-0 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
          value={selectedPreset?.id ?? ""}
          onChange={(event) => setSelectedId(event.currentTarget.value)}
        >
          {visiblePresets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
              {preset.isBuiltin ? " · 기본 제공" : ""}
            </option>
          ))}
        </select>
        <div className="flex flex-wrap gap-1">
          <button
            className="rounded border border-border bg-white px-2 py-1.5 text-xs font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
            disabled={disabled || !selectedPreset}
            type="button"
            onClick={() => selectedPreset && onApplyPreset(selectedPreset)}
          >
            프리셋 적용
          </button>
          <button
            className="inline-flex items-center gap-1 rounded border border-border bg-white px-2 py-1.5 text-xs font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
            disabled={!selectedPreset}
            type="button"
            onClick={() =>
              selectedPreset &&
              void run(async () => {
                await setDefaultSheetGridPreset(selectedPreset.id, target, collectionId);
                setStatus("기본 프리셋으로 설정했습니다.");
                await reloadAndSelect(selectedPreset.id);
              })
            }
          >
            <Star aria-hidden="true" className="size-3.5" />
            기본 설정
          </button>
        </div>
      </div>

      <div className="mt-3 grid gap-2 lg:grid-cols-[minmax(0,1fr)_auto]">
        <input
          className="min-w-0 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
          placeholder="새 프리셋 이름"
          value={name}
          onChange={(event) => setName(event.currentTarget.value)}
        />
        <div className="flex flex-wrap gap-1">
          <button
            className="inline-flex items-center gap-1 rounded border border-border bg-white px-2 py-1.5 text-xs font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
            disabled={!name.trim()}
            type="button"
            onClick={() =>
              void run(async () => {
                const created = await createSheetGridPreset(buildPresetInput(name));
                setName("");
                setStatus("현재 설정을 프리셋으로 저장했습니다.");
                await reloadAndSelect(created.id);
              })
            }
          >
            <Save aria-hidden="true" className="size-3.5" />
            현재 설정 저장
          </button>
          <button
            className="inline-flex items-center gap-1 rounded border border-border bg-white px-2 py-1.5 text-xs font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
            disabled={!selectedPreset}
            type="button"
            onClick={() =>
              selectedPreset &&
              void run(async () => {
                const duplicated = await duplicateSheetGridPreset(selectedPreset.id);
                setStatus("프리셋을 복제했습니다.");
                await reloadAndSelect(duplicated.id);
              })
            }
          >
            <Copy aria-hidden="true" className="size-3.5" />
            복제
          </button>
          <button
            className="inline-flex items-center gap-1 rounded border border-border bg-white px-2 py-1.5 text-xs font-medium text-danger hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
            disabled={!selectedPreset || selectedPreset.isBuiltin}
            title={selectedPreset?.isBuiltin ? "기본 제공 프리셋은 삭제할 수 없습니다." : undefined}
            type="button"
            onClick={() =>
              selectedPreset &&
              void run(async () => {
                await deleteSheetGridPreset(selectedPreset.id);
                setStatus("프리셋을 삭제했습니다.");
                await reloadAndSelect("");
              })
            }
          >
            <Trash2 aria-hidden="true" className="size-3.5" />
            삭제
          </button>
        </div>
      </div>
      {status ? <p className="mt-2 text-xs text-muted">{status}</p> : null}
      {errorMessage ? (
        <p className="mt-2 text-xs text-danger" role="alert">
          {errorMessage}
        </p>
      ) : null}
    </section>
  );
}

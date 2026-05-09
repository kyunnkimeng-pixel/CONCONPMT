import { useEffect, useMemo, useState } from "react";
import {
  Download,
  ExternalLink,
  FileText,
  FolderOpen,
  RefreshCw,
  Save,
  X,
} from "lucide-react";

import type { CollectionSummary } from "@/features/collections/types";
import {
  exportCollection,
  listExportProfiles,
  openExportPath,
  saveExportProfileSettings,
  validateExportCollection,
} from "@/features/export/api";
import { ValidationResultList } from "@/features/export/components/ValidationResultList";
import type {
  ExportCollectionResult,
  ExportFormat,
  ExportProfile,
  ExportRequestPayload,
  ExportValidationResult,
  FilenameMode,
} from "@/features/export/types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

interface ExportDialogProps {
  collection: CollectionSummary;
  onClose: () => void;
  onExported: () => Promise<void>;
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

export function ExportDialog({ collection, onClose, onExported }: ExportDialogProps) {
  const [profiles, setProfiles] = useState<ExportProfile[]>([]);
  const [draft, setDraft] = useState<ExportDraft | null>(null);
  const [validation, setValidation] = useState<ExportValidationResult | null>(null);
  const [exportResult, setExportResult] = useState<ExportCollectionResult | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isValidating, setIsValidating] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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

        setProfiles(nextProfiles);
        setDraft(draftFromProfile(preferredProfile(nextProfiles), collection));
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
  }, [collection]);

  const selectedProfile = useMemo(() => {
    if (!draft) {
      return null;
    }

    return profiles.find((profile) => profile.id === draft.profileId) ?? null;
  }, [draft, profiles]);

  const payload = draft ? payloadFromDraft(draft) : null;
  const isBusy = isLoading || isSaving || isValidating || isExporting;
  const validationBlocksExport = validation ? !validation.canExport : false;

  const selectProfile = (profileId: string) => {
    const profile = profiles.find((candidate) => candidate.id === profileId);
    if (!profile) {
      return;
    }

    setDraft(draftFromProfile(profile, collection));
    setValidation(null);
    setExportResult(null);
    setStatusMessage(null);
    setErrorMessage(null);
  };

  const updateDraft = (partial: Partial<ExportDraft>) => {
    setDraft((current) => (current ? { ...current, ...partial } : current));
    setValidation(null);
    setExportResult(null);
    setStatusMessage(null);
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
    if (!payload) {
      return;
    }

    setIsValidating(true);
    setErrorMessage(null);
    setStatusMessage(null);
    setExportResult(null);

    try {
      const result = await validateExportCollection(collection.id, payload);
      setValidation(result);
      setStatusMessage(result.canExport ? "내보낼 수 있습니다." : "수정이 필요한 항목이 있습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsValidating(false);
    }
  };

  const handleExport = async () => {
    if (!payload || validationBlocksExport) {
      return;
    }

    setIsExporting(true);
    setErrorMessage(null);
    setStatusMessage(null);
    setExportResult(null);

    try {
      const result = await exportCollection(collection.id, payload);
      setValidation(result.validation);
      setExportResult(result);

      if (result.exportDirectory) {
        await onExported();
        setStatusMessage("내보내기를 완료했습니다.");
      } else {
        setStatusMessage("내보내기 전에 수정이 필요한 항목이 있습니다.");
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <div
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/30 px-4 py-6"
      role="dialog"
    >
      <div className="flex max-h-full w-full max-w-3xl flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-xl">
        <header className="flex items-center justify-between gap-4 border-b border-border px-5 py-4">
          <div className="min-w-0">
            <h2 className="truncate text-base font-semibold tracking-normal">내보내기</h2>
            <p className="mt-1 truncate text-xs text-muted">{collection.name}</p>
          </div>
          <button
            aria-label="내보내기 닫기"
            className="inline-flex size-9 items-center justify-center rounded-md border border-border bg-white hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            disabled={isBusy}
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </header>

        <div className="flex-1 overflow-auto px-5 py-4">
          {isLoading ? (
            <p className="text-sm text-muted">내보내기 설정을 불러오는 중입니다.</p>
          ) : null}

          {!isLoading && draft && selectedProfile ? (
            <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(280px,0.9fr)]">
              <div className="flex flex-col gap-4">
                <section className="flex flex-col gap-3">
                  <h3 className="text-sm font-semibold tracking-normal">프로필</h3>
                  <select
                    className="rounded-md border border-border bg-white px-3 py-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    value={draft.profileId}
                    onChange={(event) => selectProfile(event.currentTarget.value)}
                  >
                    {profiles.map((profile) => (
                      <option key={profile.id} value={profile.id}>
                        {profileLabel(profile)}
                      </option>
                    ))}
                  </select>
                </section>

                <section className="grid grid-cols-2 gap-3 border-t border-border pt-4">
                  <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                    형식
                    <select
                      className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
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
                  <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                    파일명
                    <select
                      className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
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
                </section>

                <section className="grid grid-cols-3 gap-3 border-t border-border pt-4">
                  <NumberField
                    label="기준 너비"
                    min={1}
                    value={draft.targetCellWidth}
                    onChange={(targetCellWidth) => updateDraft({ targetCellWidth })}
                  />
                  <NumberField
                    label="기준 높이"
                    min={1}
                    value={draft.targetCellHeight}
                    onChange={(targetCellHeight) => updateDraft({ targetCellHeight })}
                  />
                  <NumberField
                    label="최대 용량"
                    min={1}
                    value={draft.maxBytes}
                    onChange={(maxBytes) => updateDraft({ maxBytes })}
                  />
                </section>

                <section className="flex flex-col gap-3 border-t border-border pt-4">
                  <label className="flex flex-col gap-1 text-xs font-medium text-muted">
                    출력 폴더
                    <input
                      className="rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                      placeholder="기본 exports 폴더"
                      value={draft.outputDirectory}
                      onChange={(event) =>
                        updateDraft({ outputDirectory: event.currentTarget.value })
                      }
                    />
                  </label>
                </section>

                <section className="grid gap-2 border-t border-border pt-4 sm:grid-cols-2">
                  <CheckboxField
                    checked={draft.includeAltTxt}
                    label="alts.txt 생성"
                    onChange={(includeAltTxt) => updateDraft({ includeAltTxt })}
                  />
                  <CheckboxField
                    checked={draft.strictWarnings}
                    label="경고도 차단"
                    onChange={(strictWarnings) => updateDraft({ strictWarnings })}
                  />
                  <CheckboxField
                    checked={draft.openFolderAfterExport}
                    label="완료 후 폴더 열기"
                    onChange={(openFolderAfterExport) =>
                      updateDraft({ openFolderAfterExport })
                    }
                  />
                  <CheckboxField
                    checked={draft.openAltTxtAfterExport}
                    disabled={!draft.includeAltTxt}
                    label="완료 후 alts.txt 열기"
                    onChange={(openAltTxtAfterExport) =>
                      updateDraft({ openAltTxtAfterExport })
                    }
                  />
                </section>

                {exportResult?.exportDirectory ? (
                  <section className="flex flex-wrap gap-2 border-t border-border pt-4">
                    <OpenPathButton
                      icon="folder"
                      label="폴더 열기"
                      path={exportResult.exportDirectory}
                    />
                    {exportResult.altTxtPath ? (
                      <OpenPathButton
                        icon="text"
                        label="alts.txt 열기"
                        path={exportResult.altTxtPath}
                      />
                    ) : null}
                  </section>
                ) : null}
              </div>

              <div className="flex min-w-0 flex-col gap-3">
                <div className="rounded-md border border-border bg-canvas px-3 py-3 text-sm">
                  <div className="flex items-center justify-between gap-3">
                    <span className="font-medium text-foreground">프로필 상태</span>
                    <span className="text-xs text-muted">
                      {selectedProfile.profileType === "dcinside" ? "DCInside" : "Custom"}
                    </span>
                  </div>
                  <p className="mt-2 text-xs leading-5 text-muted">
                    {selectedProfile.profileType === "dcinside"
                      ? "10~200개, 200×200, 2MB 제한을 검사합니다."
                      : "모음/아이콘에 저장된 크기를 기준으로 출력합니다."}
                  </p>
                </div>

                <ValidationResultList result={validation} />

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
            </div>
          ) : null}
        </div>

        <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-4">
          <button
            className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
            disabled={!payload || isBusy}
            type="button"
            onClick={() => {
              void handleSave();
            }}
          >
            <Save aria-hidden="true" />
            {isSaving ? "저장 중" : "설정 저장"}
          </button>
          <div className="flex items-center gap-2">
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={!payload || isBusy}
              type="button"
              onClick={() => {
                void handleValidate();
              }}
            >
              <RefreshCw aria-hidden="true" />
              {isValidating ? "검증 중" : "검증"}
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
              disabled={!payload || isBusy || validationBlocksExport}
              type="button"
              onClick={() => {
                void handleExport();
              }}
            >
              <Download aria-hidden="true" />
              {isExporting ? "내보내는 중" : "내보내기"}
            </button>
          </div>
        </footer>
      </div>
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
  };
}

function payloadFromDraft(draft: ExportDraft): ExportRequestPayload {
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
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <input
        className="min-w-0 rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        min={min}
        type="number"
        value={value}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
      />
    </label>
  );
}

function CheckboxField({
  checked,
  disabled = false,
  label,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <label
      className={cn(
        "flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium",
        disabled && "cursor-not-allowed text-muted",
      )}
    >
      <input
        checked={checked}
        className="size-4"
        disabled={disabled}
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
}: {
  icon: "folder" | "text";
  label: string;
  path: string;
}) {
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const Icon = icon === "folder" ? FolderOpen : FileText;

  return (
    <div className="flex flex-col gap-1">
      <button
        className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        type="button"
        onClick={() => {
          setErrorMessage(null);
          void openExportPath(path).catch((error) => {
            setErrorMessage(getCommandErrorMessage(error));
          });
        }}
      >
        <Icon aria-hidden="true" />
        {label}
        <ExternalLink aria-hidden="true" className="size-4" />
      </button>
      {errorMessage ? <span className="text-xs text-danger">{errorMessage}</span> : null}
    </div>
  );
}

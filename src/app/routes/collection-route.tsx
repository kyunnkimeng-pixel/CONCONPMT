import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { InputHTMLAttributes } from "react";
import { Link, useParams } from "@tanstack/react-router";
import {
  ChevronLeft,
  Download,
  FileImage,
  FolderPlus,
  ImagePlus,
  Images,
  LayoutGrid,
  MessageSquareText,
  Settings,
} from "lucide-react";

import {
  getAppSettings,
  importCollectionCoverImage,
  listCollections,
  saveAppSettings,
  setCollectionCoverIcon,
  updateCollectionSettings,
} from "@/features/collections/api";
import { DropImportZone } from "@/features/collections/components/DropImportZone";
import type {
  CollectionSettingsPayload,
  CollectionSummary,
  IconPieceSummary,
  IconSummary,
} from "@/features/collections/types";
import { EditorPanel } from "@/features/editor/components/EditorPanel";
import { ExportDialog } from "@/features/export/components/ExportDialog";
import {
  deleteIcons,
  duplicateIcon,
  importImagesIntoCollection,
  listIcons,
  renameIcon,
  reorderIcons,
  revealIconExportResult,
  revealIconOriginal,
  setIconThumbnailOverride,
  updateIconPieceAlt,
} from "@/features/icons/api";
import { IconGrid } from "@/features/icons/components/IconGrid";
import { DcinsidePreview } from "@/features/preview/components/DcinsidePreview";
import {
  IMPORTABLE_IMAGE_ACCEPT,
  COVER_IMAGE_ACCEPT,
  isCoverImageFile,
  partitionImportableImageFiles,
  sortFilesForImport,
} from "@/lib/file-types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import {
  findDuplicateAltPieceIds,
  isDuplicateAltDraft,
  normalizeAltText,
  validateDcinsideAltText,
} from "@/lib/validation";

const folderInputProps = {
  webkitdirectory: "",
  directory: "",
} as InputHTMLAttributes<HTMLInputElement> & {
  webkitdirectory: string;
  directory: string;
};

export function CollectionRoute() {
  const { collectionId } = useParams({ from: "/collections/$collectionId" });
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const coverInputRef = useRef<HTMLInputElement>(null);
  const thumbnailInputRef = useRef<HTMLInputElement>(null);
  const hasLoadedRouteSettingsRef = useRef(false);
  const [collection, setCollection] = useState<CollectionSummary | null>(null);
  const [icons, setIcons] = useState<IconSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isDragging, setIsDragging] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [editingIconId, setEditingIconId] = useState<string | null>(null);
  const [isExportDialogOpen, setIsExportDialogOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [viewMode, setViewMode] = useState<"explorer" | "usagePreview">("explorer");
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [thumbnailOverrideIconId, setThumbnailOverrideIconId] = useState<string | null>(null);
const duplicatePieceIds = useMemo(() => findDuplicateAltPieceIds(icons), [icons]);

  const refreshCollectionAndIcons = useCallback(async () => {
    const [collections, nextIcons] = await Promise.all([
      listCollections(),
      listIcons(collectionId),
    ]);
    const nextCollection =
      collections.find((candidate) => candidate.id === collectionId) ?? null;

    setCollection(nextCollection);
    setIcons(nextCollection ? nextIcons : []);

    return nextCollection;
  }, [collectionId]);

  const loadCollection = useCallback(async () => {
    setIsLoading(true);
    setErrorMessage(null);

    try {
      hasLoadedRouteSettingsRef.current = false;
      const [nextCollection, settings] = await Promise.all([
        refreshCollectionAndIcons(),
        getAppSettings(),
      ]);
      if (nextCollection && settings.lastOpenCollectionId === collectionId) {
        setViewMode(settings.lastViewMode);
      }
      hasLoadedRouteSettingsRef.current = true;
    } catch (error) {
      hasLoadedRouteSettingsRef.current = true;
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  }, [refreshCollectionAndIcons]);

  useEffect(() => {
    void loadCollection();
  }, [loadCollection]);

  useEffect(() => {
    if (!collection || !hasLoadedRouteSettingsRef.current) {
      return;
    }

    void saveAppSettings({
      lastOpenCollectionId: collection.id,
      lastViewMode: viewMode,
    }).catch(() => {
      // Route restore is a convenience; regular editing should not be interrupted.
    });
  }, [collection, viewMode]);

  const handleImportFiles = useCallback(
    async (files: File[]) => {
      const { accepted, rejected } = partitionImportableImageFiles(sortFilesForImport(files));
      setErrorMessage(null);
      setImportStatus(null);

      if (accepted.length === 0) {
        setImportStatus("가져올 수 있는 jpg, jpeg, png, gif 파일이 없습니다.");
        return;
      }

      setIsImporting(true);

      try {
        const result = await importImagesIntoCollection(collectionId, accepted);
        const skippedCount = rejected.length + result.rejectedFiles.length;
        setCollection(result.collection);
        setIcons(await listIcons(collectionId));
        setImportStatus(importStatusMessage(result.importedIcons.length, skippedCount));
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
      } finally {
        setIsImporting(false);
      }
    },
    [collectionId],
  );

  const validateCurrentAlt = useCallback((piece: IconPieceSummary) => {
    const validation = validateDcinsideAltText(piece.altText);
    return validation.isValid ? null : validation.message;
  }, []);

  const validateAltDraft = useCallback(
    (pieceId: string, value: string) => {
      const validation = validateDcinsideAltText(value);
      if (!validation.isValid) {
        return validation.message;
      }

      if (isDuplicateAltDraft(icons, pieceId, value)) {
        return "같은 모음 안에서 alt 값은 중복될 수 없습니다.";
      }

      return null;
    },
    [icons],
  );

  const handleAltCommit = useCallback(
    async (pieceId: string, value: string) => {
      setErrorMessage(null);

      try {
        const updatedIcon = await updateIconPieceAlt(
          collectionId,
          pieceId,
          normalizeAltText(value),
        );
        setIcons((currentIcons) =>
          currentIcons.map((icon) => (icon.id === updatedIcon.id ? updatedIcon : icon)),
        );
        setImportStatus("alt 값을 저장했습니다.");
        return true;
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
        return false;
      }
    },
    [collectionId],
  );

  const handleBatchAltCommit = useCallback(
    async (iconIds: string[], value: string) => {
      const targetPieceIds = icons
        .filter((icon) => iconIds.includes(icon.id))
        .flatMap((icon) => icon.pieces.map((piece) => piece.id));

      if (targetPieceIds.length === 0) {
        return false;
      }

      setErrorMessage(null);

      try {
        await Promise.all(
          targetPieceIds.map((pieceId) =>
            updateIconPieceAlt(collectionId, pieceId, normalizeAltText(value)),
          ),
        );
        setIcons(await listIcons(collectionId));
        setImportStatus(
          `${targetPieceIds.length}개의 alt 값을 일괄 변경했습니다.`,
        );
        return true;
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
        return false;
      }
    },
    [collectionId, icons],
  );

  const handleDeleteIcons = useCallback(
    async (iconIds: string[]) => {
      setErrorMessage(null);
      setImportStatus(null);

      try {
        setCollection(await deleteIcons(collectionId, iconIds));
        setIcons(await listIcons(collectionId));
        if (editingIconId && iconIds.includes(editingIconId)) {
          setEditingIconId(null);
        }
        setImportStatus(`${iconIds.length}개 아이콘을 삭제했습니다.`);
        return true;
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
        return false;
      }
    },
    [collectionId, editingIconId],
  );

  const handleDuplicateIcon = useCallback(
    async (iconId: string) => {
      setErrorMessage(null);
      setImportStatus(null);

      try {
        await duplicateIcon(collectionId, iconId);
        await refreshCollectionAndIcons();
        setImportStatus("아이콘을 복제했습니다.");
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
      }
    },
    [collectionId, refreshCollectionAndIcons],
  );

  const handleRenameIcon = useCallback(
    async (iconId: string, displayName: string) => {
      setErrorMessage(null);
      setImportStatus(null);

      try {
        const updatedIcon = await renameIcon(collectionId, iconId, displayName);
        setIcons((currentIcons) =>
          currentIcons.map((icon) => (icon.id === updatedIcon.id ? updatedIcon : icon)),
        );
        setImportStatus("아이콘 이름을 저장했습니다.");
        return true;
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
        return false;
      }
    },
    [collectionId],
  );

  const handleEditIcon = useCallback(
    (iconId: string) => {
      setEditingIconId(iconId);
      setImportStatus("아이콘 편집 패널을 열었습니다.");
    },
    [],
  );

  const handleIconUpdated = useCallback((updatedIcon: IconSummary) => {
    setIcons((currentIcons) =>
      currentIcons.map((icon) => (icon.id === updatedIcon.id ? updatedIcon : icon)),
    );
    setImportStatus("아이콘 편집값을 저장했습니다.");
  }, []);

  const handleReorderIcons = useCallback(
    async (orderedIconIds: string[]) => {
      const previousIcons = icons;
      const iconById = new Map(icons.map((icon) => [icon.id, icon]));
      const optimisticIcons = orderedIconIds
        .map((iconId) => iconById.get(iconId))
        .filter((icon): icon is IconSummary => Boolean(icon));

      setIcons(optimisticIcons);
      setErrorMessage(null);
      setImportStatus(null);

      try {
        setIcons(await reorderIcons(collectionId, orderedIconIds));
        setImportStatus("아이콘 순서를 저장했습니다.");
      } catch (error) {
        setIcons(previousIcons);
        setErrorMessage(getCommandErrorMessage(error));
      }
    },
    [collectionId, icons],
  );

  const handleSetCover = async (iconId: string) => {
    setErrorMessage(null);

    try {
      setCollection(await setCollectionCoverIcon(collectionId, iconId));
      setImportStatus("대표 이미지를 변경했습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleImportCoverImage = async (files: File[]) => {
    const file = files[0];
    setErrorMessage(null);
    setImportStatus(null);

    if (!file) {
      return;
    }
    if (!isCoverImageFile(file)) {
      setImportStatus("대표 이미지는 200×200 JPG 또는 PNG 파일만 사용할 수 있습니다.");
      return;
    }

    try {
      setCollection(await importCollectionCoverImage(collectionId, file));
      setImportStatus("모음 대표 이미지를 가져왔습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleSetThumbnailOverride = (iconId: string) => {
    setThumbnailOverrideIconId(iconId);
    thumbnailInputRef.current?.click();
  };

  const handleThumbnailOverrideFile = async (files: File[]) => {
    const file = files[0];
    const iconId = thumbnailOverrideIconId;
    setThumbnailOverrideIconId(null);
    setErrorMessage(null);
    setImportStatus(null);

    if (!file || !iconId) {
      return;
    }

    try {
      const updatedIcon = await setIconThumbnailOverride(collectionId, iconId, file);
      setIcons((currentIcons) =>
        currentIcons.map((icon) => (icon.id === updatedIcon.id ? updatedIcon : icon)),
      );
      setImportStatus("아이콘 썸네일을 바꿨습니다.");
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleRevealOriginal = useCallback(
    async (iconId: string) => {
      setErrorMessage(null);
      try {
        await revealIconOriginal(collectionId, iconId);
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
      }
    },
    [collectionId],
  );

  const handleRevealExportResult = useCallback(
    async (iconId: string) => {
      setErrorMessage(null);
      try {
        await revealIconExportResult(collectionId, iconId);
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
      }
    },
    [collectionId],
  );

  const openImportPicker = () => {
    fileInputRef.current?.click();
  };

  const openFolderImportPicker = () => {
    folderInputRef.current?.click();
  };

  const openCoverImportPicker = () => {
    coverInputRef.current?.click();
  };

  const handleExported = useCallback(async () => {
    await refreshCollectionAndIcons();
    setImportStatus("내보내기 결과를 저장했습니다.");
  }, [refreshCollectionAndIcons]);

  const changeViewMode = (nextMode: "explorer" | "usagePreview") => {
    setViewMode(nextMode);
    if (nextMode === "usagePreview") {
      setEditingIconId(null);
    }
  };

  if (isLoading) {
    return (
      <div className="flex h-screen flex-col overflow-hidden">
        <header className="border-b border-border bg-surface px-8 py-5">
          <BackLink />
        </header>
        <div className="flex flex-1 items-center justify-center text-muted">
          모음을 불러오는 중
        </div>
      </div>
    );
  }

  if (!collection) {
    return (
      <div className="flex h-screen flex-col overflow-hidden">
        <header className="border-b border-border bg-surface px-8 py-5">
          <BackLink />
        </header>
        <div className="flex flex-1 items-center justify-center text-muted">
          {errorMessage ?? "모음을 찾을 수 없습니다."}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <header className="border-b border-border bg-surface/95 px-8 py-5">
        <div className="mb-3 flex items-center gap-2 text-sm text-muted">
          <Link
            className="rounded-md hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            to="/"
          >
            홈
          </Link>
          <span aria-hidden="true">/</span>
          <span className="truncate text-foreground">{collection.name}</span>
        </div>
        <div className="flex items-end justify-between gap-4">
          <div className="min-w-0">
            <h1 className="truncate text-2xl font-semibold tracking-normal">
              {collection.name}
            </h1>
            <p className="mt-1 text-sm text-muted">{icons.length}개 항목</p>
          </div>
          <div className="flex items-center gap-2">
            <div
              aria-label="보기 모드"
              className="flex items-center gap-1 rounded-md border border-border bg-white p-1"
              role="group"
            >
              <button
                className={viewModeButtonClass(viewMode === "explorer")}
                type="button"
                onClick={() => changeViewMode("explorer")}
              >
                <LayoutGrid aria-hidden="true" />
                탐색
              </button>
              <button
                className={viewModeButtonClass(viewMode === "usagePreview")}
                type="button"
                onClick={() => changeViewMode("usagePreview")}
              >
                <MessageSquareText aria-hidden="true" />
                사용 미리보기
              </button>
            </div>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={icons.length === 0}
              title={icons.length === 0 ? "내보낼 항목이 없습니다." : undefined}
              type="button"
              onClick={() => setIsExportDialogOpen(true)}
            >
              <Download aria-hidden="true" />
              내보내기
            </button>
            <button
              className={viewModeButtonClass(isSettingsOpen)}
              type="button"
              onClick={() => setIsSettingsOpen((isOpen) => !isOpen)}
            >
              <Settings aria-hidden="true" />
              설정
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="button"
              onClick={openCoverImportPicker}
            >
              <ImagePlus aria-hidden="true" />
              대표 이미지
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={isImporting}
              type="button"
              onClick={openFolderImportPicker}
            >
              <FolderPlus aria-hidden="true" />
              폴더 추가
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={isImporting}
              type="button"
              onClick={openImportPicker}
            >
              <FileImage aria-hidden="true" />
              이미지 추가
            </button>
            <BackLink label="뒤로" />
          </div>
        </div>
        {isSettingsOpen ? (
          <CollectionSettingsPanel
            collection={collection}
            onSave={async (payload) => {
              const updatedCollection = await updateCollectionSettings(collection.id, payload);
              setCollection(updatedCollection);
              setImportStatus("모음 기준 크기 설정을 저장했습니다.");
            }}
          />
        ) : null}
      </header>

      <input
        ref={fileInputRef}
        accept={IMPORTABLE_IMAGE_ACCEPT}
        className="hidden"
        multiple
        type="file"
        onChange={(event) => {
          const files = Array.from(event.currentTarget.files ?? []);
          event.currentTarget.value = "";
          void handleImportFiles(files);
        }}
      />
      <input
        ref={folderInputRef}
        accept={IMPORTABLE_IMAGE_ACCEPT}
        className="hidden"
        multiple
        type="file"
        {...folderInputProps}
        onChange={(event) => {
          const files = Array.from(event.currentTarget.files ?? []);
          event.currentTarget.value = "";
          void handleImportFiles(files);
        }}
      />
      <input
        ref={coverInputRef}
        accept={COVER_IMAGE_ACCEPT}
        className="hidden"
        type="file"
        onChange={(event) => {
          const files = Array.from(event.currentTarget.files ?? []);
          event.currentTarget.value = "";
          void handleImportCoverImage(files);
        }}
      />
      <input
        ref={thumbnailInputRef}
        accept={IMPORTABLE_IMAGE_ACCEPT}
        className="hidden"
        type="file"
        onChange={(event) => {
          const files = Array.from(event.currentTarget.files ?? []);
          event.currentTarget.value = "";
          void handleThumbnailOverrideFile(files);
        }}
      />

      <section className="flex min-h-0 flex-1 overflow-hidden">
        <div className="min-w-0 flex-1 overflow-auto px-8 py-6">
          {viewMode === "usagePreview" ? (
            <DcinsidePreview collection={collection} icons={icons} />
          ) : (
            <DropImportZone
              isDragging={isDragging}
              label="이 모음에 이미지 파일 놓기"
              onDragStateChange={setIsDragging}
              onFilesDropped={(files) => {
                void handleImportFiles(files);
              }}
            >
              {icons.length > 0 ? (
                <IconGrid
                  collection={collection}
                  duplicatePieceIds={duplicatePieceIds}
                  editRequest={null}
                  icons={icons}
                  validateAltDraft={validateAltDraft}
                  validateCurrentAlt={validateCurrentAlt}
                  onAltCommit={handleAltCommit}
                  onBatchAltCommit={handleBatchAltCommit}
                  onDeleteIcons={handleDeleteIcons}
                  onDuplicateIcon={handleDuplicateIcon}
                  onEditIcon={handleEditIcon}
                  onRenameIcon={handleRenameIcon}
                  onReorderIcons={handleReorderIcons}
                  onRevealExportResult={handleRevealExportResult}
                  onRevealOriginal={handleRevealOriginal}
                  onSetCover={handleSetCover}
                  onSetThumbnailOverride={handleSetThumbnailOverride}
                />
              ) : (
                <div className="flex min-h-[360px] items-center justify-center">
                  <div className="flex flex-col items-center gap-3 text-center text-muted">
                    <Images aria-hidden="true" />
                    <p className="text-sm">이 모음에는 아직 항목이 없습니다.</p>
                    <button
                      className="rounded-md border border-border bg-white px-4 py-2 text-sm font-medium text-foreground hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                      disabled={isImporting}
                      type="button"
                      onClick={openImportPicker}
                    >
                      이미지 추가
                    </button>
                  </div>
                </div>
              )}
            </DropImportZone>
          )}

          {importStatus ? (
            <p className="mt-3 text-sm text-muted" role="status">
              {importStatus}
            </p>
          ) : null}

          {errorMessage ? (
            <p className="mt-3 text-sm text-danger" role="alert">
              {errorMessage}
            </p>
          ) : null}
        </div>

        {viewMode === "explorer" && editingIconId ? (
          <EditorPanel
            collection={collection}
            iconId={editingIconId}
            onClose={() => setEditingIconId(null)}
            onIconUpdated={handleIconUpdated}
          />
        ) : null}
      </section>

      {isExportDialogOpen ? (
        <ExportDialog
          collection={collection}
          onClose={() => setIsExportDialogOpen(false)}
          onExported={handleExported}
        />
      ) : null}
    </div>
  );
}

function CollectionSettingsPanel({
  collection,
  onSave,
}: {
  collection: CollectionSummary;
  onSave: (payload: CollectionSettingsPayload) => Promise<void>;
}) {
  const [draft, setDraft] = useState<CollectionSettingsPayload>(() => ({
    defaultCellWidth: collection.defaultCellWidth,
    defaultCellHeight: collection.defaultCellHeight,
    previewWidth: collection.previewWidth,
    previewHeight: collection.previewHeight,
    exportFormat: collection.exportFormat,
    maxBytes: collection.maxBytes,
  }));
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    setDraft({
      defaultCellWidth: collection.defaultCellWidth,
      defaultCellHeight: collection.defaultCellHeight,
      previewWidth: collection.previewWidth,
      previewHeight: collection.previewHeight,
      exportFormat: collection.exportFormat,
      maxBytes: collection.maxBytes,
    });
  }, [collection]);

  const updateNumber = (
    field: keyof Omit<CollectionSettingsPayload, "exportFormat">,
    value: number,
  ) => {
    setDraft((current) => ({
      ...current,
      [field]: Number.isFinite(value) ? Math.max(1, Math.round(value)) : 1,
    }));
  };

  return (
    <section className="mt-4 grid gap-3 rounded-md border border-border bg-white p-3">
      <div className="grid gap-3 md:grid-cols-6">
        <SettingsNumberField
          label="기준 너비"
          value={draft.defaultCellWidth}
          onChange={(value) => updateNumber("defaultCellWidth", value)}
        />
        <SettingsNumberField
          label="기준 높이"
          value={draft.defaultCellHeight}
          onChange={(value) => updateNumber("defaultCellHeight", value)}
        />
        <SettingsNumberField
          label="표시 너비"
          value={draft.previewWidth}
          onChange={(value) => updateNumber("previewWidth", value)}
        />
        <SettingsNumberField
          label="표시 높이"
          value={draft.previewHeight}
          onChange={(value) => updateNumber("previewHeight", value)}
        />
        <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
          기본 형식
          <select
            className="rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            value={draft.exportFormat}
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                exportFormat: event.currentTarget.value as CollectionSummary["exportFormat"],
              }))
            }
          >
            <option value="png">PNG</option>
            <option value="jpg">JPG</option>
            <option value="gif">GIF</option>
            <option value="source">원본</option>
          </select>
        </label>
        <SettingsNumberField
          label="최대 용량"
          value={draft.maxBytes}
          onChange={(value) => updateNumber("maxBytes", value)}
        />
      </div>
      <div className="flex items-center justify-between gap-3">
        {errorMessage ? (
          <p className="text-sm text-danger" role="alert">
            {errorMessage}
          </p>
        ) : (
          <p className="text-xs text-muted">
            아이콘별 크기 변경은 오른쪽 편집 패널의 셀 크기에서 저장됩니다.
          </p>
        )}
        <button
          className="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
          disabled={isSaving}
          type="button"
          onClick={() => {
            setIsSaving(true);
            setErrorMessage(null);
            void onSave(draft)
              .catch((error) => setErrorMessage(getCommandErrorMessage(error)))
              .finally(() => setIsSaving(false));
          }}
        >
          {isSaving ? "저장 중" : "설정 저장"}
        </button>
      </div>
    </section>
  );
}

function SettingsNumberField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  return (
    <label className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <input
        className="min-w-0 select-text rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        min={1}
        type="number"
        value={draft}
        onBlur={() => {
          const parsed = Number.parseInt(draft, 10);
          if (!Number.isFinite(parsed) || parsed < 1) {
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
          if (Number.isFinite(parsed) && parsed >= 1) {
            onChange(parsed);
          }
        }}
      />
    </label>
  );
}

function viewModeButtonClass(isSelected: boolean) {
  return cn(
    "inline-flex items-center gap-2 rounded px-2.5 py-1.5 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
    isSelected ? "bg-selected text-foreground" : "text-muted",
  );
}

function BackLink({ label = "홈" }: { label?: string }) {
  return (
    <Link
      className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
      to="/"
    >
      <ChevronLeft aria-hidden="true" />
      {label}
    </Link>
  );
}

function importStatusMessage(importedCount: number, skippedCount: number) {
  if (importedCount === 0) {
    return skippedCount > 0
      ? `가져온 이미지가 없습니다. ${skippedCount}개 파일은 건너뛰었습니다.`
      : "가져온 이미지가 없습니다.";
  }

  return skippedCount > 0
    ? `${importedCount}개 이미지를 가져왔습니다. ${skippedCount}개 파일은 건너뛰었습니다.`
    : `${importedCount}개 이미지를 가져왔습니다.`;
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams } from "@tanstack/react-router";
import {
  ChevronLeft,
  Download,
  FileImage,
  Images,
  LayoutGrid,
  MessageSquareText,
} from "lucide-react";

import { listCollections, setCollectionCoverIcon } from "@/features/collections/api";
import { DropImportZone } from "@/features/collections/components/DropImportZone";
import type {
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
  reorderIcons,
  updateIconPieceAlt,
} from "@/features/icons/api";
import { IconGrid } from "@/features/icons/components/IconGrid";
import { DcinsidePreview } from "@/features/preview/components/DcinsidePreview";
import {
  IMPORTABLE_IMAGE_ACCEPT,
  partitionImportableImageFiles,
} from "@/lib/file-types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import {
  findDuplicateAltPieceIds,
  isDuplicateAltDraft,
  normalizeAltText,
  validateDcinsideAltText,
} from "@/lib/validation";

export function CollectionRoute() {
  const { collectionId } = useParams({ from: "/collections/$collectionId" });
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [collection, setCollection] = useState<CollectionSummary | null>(null);
  const [icons, setIcons] = useState<IconSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isDragging, setIsDragging] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [editingIconId, setEditingIconId] = useState<string | null>(null);
  const [isExportDialogOpen, setIsExportDialogOpen] = useState(false);
  const [viewMode, setViewMode] = useState<"explorer" | "usagePreview">("explorer");
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
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
      await refreshCollectionAndIcons();
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  }, [refreshCollectionAndIcons]);

  useEffect(() => {
    void loadCollection();
  }, [loadCollection]);

  const handleImportFiles = useCallback(
    async (files: File[]) => {
      const { accepted, rejected } = partitionImportableImageFiles(files);
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
      const validationMessage = validateAltDraft(pieceId, value);
      if (validationMessage) {
        setErrorMessage(validationMessage);
        return false;
      }

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
    [collectionId, validateAltDraft],
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

  const openImportPicker = () => {
    fileInputRef.current?.click();
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
      <div className="flex min-h-screen flex-col">
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
      <div className="flex min-h-screen flex-col">
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
    <div className="flex min-h-screen flex-col">
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
                  onDeleteIcons={handleDeleteIcons}
                  onDuplicateIcon={handleDuplicateIcon}
                  onEditIcon={handleEditIcon}
                  onReorderIcons={handleReorderIcons}
                  onSetCover={handleSetCover}
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

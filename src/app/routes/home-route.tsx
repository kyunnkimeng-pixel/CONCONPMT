import { useCallback, useEffect, useRef, useState } from "react";
import type { InputHTMLAttributes } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Copy, FileImage, FolderPlus, FolderX, Plus, Trash2, Upload } from "lucide-react";

import { CollectionGrid } from "@/features/collections/components/CollectionGrid";
import { DropImportZone } from "@/features/collections/components/DropImportZone";
import {
  cleanupLibrary,
  createCollection,
  deleteCollection,
  duplicateCollection,
  getAppSettings,
  listCollections,
  previewLibraryCleanup,
  renameCollection,
} from "@/features/collections/api";
import type { CollectionSummary } from "@/features/collections/types";
import { notifyCollectionListChanged } from "@/features/collections/events";
import { importImagesIntoCollection } from "@/features/icons/api";
import {
  IMPORTABLE_IMAGE_ACCEPT,
  partitionImportableImageFiles,
  sortFilesForImport,
} from "@/lib/file-types";
import {
  formatImportResultMessage,
  partitionFilesByImportSize,
} from "@/lib/import-file";
import { getCommandErrorMessage } from "@/lib/tauri";

const folderInputProps = {
  webkitdirectory: "",
  directory: "",
} as InputHTMLAttributes<HTMLInputElement> & {
  webkitdirectory: string;
  directory: string;
};

export function HomeRoute() {
  const navigate = useNavigate();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const didAttemptRestoreRef = useRef(false);
  const [isActionPanelOpen, setIsActionPanelOpen] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [selectedCollectionId, setSelectedCollectionId] = useState<string | null>(null);
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [collections, setCollections] = useState<CollectionSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isImporting, setIsImporting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const reloadCollections = useCallback(async () => {
    setIsLoading(true);
    setErrorMessage(null);

    try {
      const nextCollections = await listCollections();
      setCollections(nextCollections);
      setSelectedCollectionId((currentId) =>
        currentId && nextCollections.some((collection) => collection.id === currentId)
          ? currentId
          : null,
      );

      if (!didAttemptRestoreRef.current && shouldAttemptStartupRestore()) {
        didAttemptRestoreRef.current = true;
        markStartupRestoreAttempted();
        const settings = await getAppSettings();
        const restoredCollection = nextCollections.find(
          (collection) => collection.id === settings.lastOpenCollectionId,
        );
        if (restoredCollection) {
          void navigate({
            to: "/collections/$collectionId",
            params: { collectionId: restoredCollection.id },
          });
        }
      }
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  }, [navigate]);

  useEffect(() => {
    void reloadCollections();
  }, [reloadCollections]);

  const openCollection = (collectionId: string) => {
    void navigate({ to: "/collections/$collectionId", params: { collectionId } });
  };

  const handleCreateCollection = async () => {
    setErrorMessage(null);

    try {
      const collection = await createCollection();
      setCollections((currentCollections) => [...currentCollections, collection]);
      setSelectedCollectionId(collection.id);
      setIsActionPanelOpen(false);
      notifyCollectionListChanged();
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleRenameCollection = async (collectionId: string, name: string) => {
    const previousCollections = collections;
    const optimisticName = name.trim() || "이름 없는 모음";

    setCollections((currentCollections) =>
      currentCollections.map((collection) =>
        collection.id === collectionId
          ? { ...collection, name: optimisticName, updatedAt: new Date().toISOString() }
          : collection,
      ),
    );

    try {
      const updatedCollection = await renameCollection(collectionId, name);
      setCollections((currentCollections) =>
        currentCollections.map((collection) =>
          collection.id === collectionId ? updatedCollection : collection,
        ),
      );
      notifyCollectionListChanged();
    } catch (error) {
      setCollections(previousCollections);
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleDuplicateCollection = async (collectionId = selectedCollectionId) => {
    if (!collectionId) {
      return;
    }

    setErrorMessage(null);
    setImportStatus(null);

    try {
      const duplicated = await duplicateCollection(collectionId);
      setCollections((currentCollections) => [...currentCollections, duplicated]);
      setSelectedCollectionId(duplicated.id);
      setImportStatus("모음을 복제했습니다.");
      notifyCollectionListChanged();
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleDeleteCollection = useCallback(
    async (collectionId = selectedCollectionId) => {
      if (!collectionId || isImporting) {
        return;
      }

      const collection = collections.find((candidate) => candidate.id === collectionId);
      if (!collection) {
        return;
      }
      if (
        !window.confirm(
          `“${collection.name}” 모음을 삭제할까요?\n원본 파일은 라이브러리 정리 전까지 보존됩니다.`,
        )
      ) {
        return;
      }

      const previousCollections = collections;
      setCollections((current) => current.filter((candidate) => candidate.id !== collectionId));
      setSelectedCollectionId((current) => (current === collectionId ? null : current));
      setErrorMessage(null);
      setImportStatus(null);

      try {
        await deleteCollection(collectionId);
        setImportStatus(`“${collection.name}” 모음을 삭제했습니다.`);
        notifyCollectionListChanged();
      } catch (error) {
        setCollections(previousCollections);
        setSelectedCollectionId(collectionId);
        setErrorMessage(getCommandErrorMessage(error));
      }
    },
    [collections, isImporting, selectedCollectionId],
  );

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        event.key !== "Delete" ||
        !selectedCollectionId ||
        isImporting ||
        (target instanceof HTMLElement && target.closest("input,textarea,select,[contenteditable]"))
      ) {
        return;
      }

      event.preventDefault();
      void handleDeleteCollection();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleDeleteCollection, isImporting, selectedCollectionId]);

  const handleCleanupLibrary = async () => {
    setErrorMessage(null);
    setImportStatus(null);

    try {
      const preview = await previewLibraryCleanup();
      const candidateCount =
        preview.removedOriginalFiles +
        preview.removedThumbnailFiles +
        preview.removedTempFiles;

      if (candidateCount === 0) {
        setImportStatus("정리할 라이브러리 파일이 없습니다.");
        return;
      }

      if (!window.confirm(cleanupConfirmMessage(preview))) {
        return;
      }

      const result = await cleanupLibrary();
      setImportStatus(cleanupResultMessage(result));
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleImportFiles = useCallback(
    async (files: File[]) => {
      const { accepted: formatAccepted, rejected: unsupportedFiles } =
        partitionImportableImageFiles(sortFilesForImport(files));
      const { accepted, rejected: oversizedFiles } =
        partitionFilesByImportSize(formatAccepted);
      setErrorMessage(null);
      setImportStatus(null);
      setIsActionPanelOpen(false);

      if (accepted.length === 0) {
        setImportStatus(
          formatImportResultMessage(0, unsupportedFiles, oversizedFiles),
        );
        return;
      }

      setIsImporting(true);

      try {
        let targetCollection =
          collections.find((collection) => collection.id === selectedCollectionId) ?? null;

        if (!targetCollection) {
          const createdCollection = await createCollection();
          targetCollection = createdCollection;
          setCollections((currentCollections) =>
            upsertCollection(currentCollections, createdCollection),
          );
          notifyCollectionListChanged();
        }

        const result = await importImagesIntoCollection(targetCollection.id, accepted);
        const rejectedFiles = [...oversizedFiles, ...result.rejectedFiles];

        setCollections((currentCollections) =>
          upsertCollection(currentCollections, result.collection),
        );
        setSelectedCollectionId(result.collection.id);
        setImportStatus(
          formatImportResultMessage(
            result.importedIcons.length,
            unsupportedFiles,
            rejectedFiles,
          ),
        );
        notifyCollectionListChanged();
      } catch (error) {
        setErrorMessage(getCommandErrorMessage(error));
      } finally {
        setIsImporting(false);
      }
    },
    [collections, selectedCollectionId],
  );

  const openImportPicker = () => {
    fileInputRef.current?.click();
  };

  const openFolderImportPicker = () => {
    folderInputRef.current?.click();
  };

  return (
    <div className="flex min-h-screen flex-col">
      <header className="border-b border-border bg-surface/95 px-8 py-5">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="mb-2 flex items-center gap-2 text-sm text-muted">
              <span>홈</span>
            </div>
            <h1 className="text-2xl font-semibold tracking-normal">디시콘 모음</h1>
          </div>

          <div className="flex items-center gap-2">
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={!selectedCollectionId || isImporting}
              title={
                isImporting
                  ? "이미지를 가져오는 동안에는 모음을 삭제할 수 없습니다."
                  : !selectedCollectionId
                    ? "삭제할 모음을 선택하세요."
                    : undefined
              }
              type="button"
              onClick={() => void handleDeleteCollection()}
            >
              <FolderX aria-hidden="true" />
              선택 삭제
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={!selectedCollectionId}
              title={!selectedCollectionId ? "복제할 모음을 선택하세요." : undefined}
              type="button"
              onClick={() => void handleDuplicateCollection()}
            >
              <Copy aria-hidden="true" />
              선택 복제
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="button"
              onClick={() => void handleCleanupLibrary()}
            >
              <Trash2 aria-hidden="true" />
              라이브러리 정리
            </button>
            <div className="relative">
            <button
              aria-expanded={isActionPanelOpen}
              aria-label="모음 추가"
              className="flex size-10 items-center justify-center rounded-md bg-accent text-accent-foreground shadow-sm transition hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="button"
              onClick={() => setIsActionPanelOpen((isOpen) => !isOpen)}
            >
              <Plus aria-hidden="true" />
            </button>

            {isActionPanelOpen ? (
              <div className="absolute right-0 top-12 z-10 w-72 rounded-lg border border-border bg-white p-2 shadow-lg">
                <button
                  className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                  type="button"
                  onClick={() => void handleCreateCollection()}
                >
                  <FolderPlus aria-hidden="true" />새 모음
                </button>
                <button
                  className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                  disabled={isImporting}
                  type="button"
                  onClick={openImportPicker}
                >
                  <FileImage aria-hidden="true" />
                  {selectedCollectionId ? "선택한 모음에 이미지 가져오기" : "새 모음으로 이미지 가져오기"}
                </button>
                <button
                  className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                  disabled={isImporting}
                  type="button"
                  onClick={openFolderImportPicker}
                >
                  <FolderPlus aria-hidden="true" />
                  {selectedCollectionId ? "선택한 모음에 폴더 가져오기" : "새 모음으로 폴더 가져오기"}
                </button>
              </div>
            ) : null}
            </div>
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

      <div className="flex-1 px-8 py-6">
        <DropImportZone
          isDragging={isDragging}
          label={
            selectedCollectionId
              ? "선택한 모음에 이미지 파일 놓기"
              : "새 모음으로 이미지 파일 놓기"
          }
          onDragStateChange={setIsDragging}
          onFilesDropped={(files) => {
            void handleImportFiles(files);
          }}
        >
          {isLoading ? (
            <div className="flex min-h-[360px] items-center justify-center text-sm text-muted">
              모음을 불러오는 중
            </div>
          ) : collections.length > 0 ? (
            <CollectionGrid
              collections={collections}
              isDeleteDisabled={isImporting}
              selectedCollectionId={selectedCollectionId}
              onOpenCollection={openCollection}
              onDuplicateCollection={(collectionId) => {
                void handleDuplicateCollection(collectionId);
              }}
              onDeleteCollection={(collectionId) => {
                void handleDeleteCollection(collectionId);
              }}
              onRenameCollection={(collectionId, name) => {
                void handleRenameCollection(collectionId, name);
              }}
              onSelectCollection={setSelectedCollectionId}
            />
          ) : (
            <div className="flex min-h-[360px] items-center justify-center">
              <div className="flex max-w-sm flex-col items-center gap-4 text-center">
                <div className="flex size-16 items-center justify-center rounded-lg border border-border bg-card text-muted shadow-sm">
                  <Upload aria-hidden="true" />
                </div>
                <div>
                  <h2 className="text-lg font-semibold">아직 모음이 없습니다</h2>
                  <p className="mt-2 text-sm leading-6 text-muted">
                    새 모음을 만들거나 이미지 파일을 가져와 첫 모음을 시작하세요.
                  </p>
                </div>
                <div className="flex flex-wrap justify-center gap-2">
                  <button
                    className="rounded-md border border-border bg-white px-4 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
                    disabled={isImporting}
                    type="button"
                    onClick={openImportPicker}
                  >
                    이미지 가져오기
                  </button>
                  <button
                    className="rounded-md border border-border bg-white px-4 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    type="button"
                    onClick={() => void handleCreateCollection()}
                  >
                    새 모음
                  </button>
                </div>
              </div>
            </div>
          )}
        </DropImportZone>

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
    </div>
  );
}

function upsertCollection(collections: CollectionSummary[], collection: CollectionSummary) {
  if (collections.some((currentCollection) => currentCollection.id === collection.id)) {
    return collections.map((currentCollection) =>
      currentCollection.id === collection.id ? collection : currentCollection,
    );
  }

  return [...collections, collection];
}

function cleanupConfirmMessage(result: {
  orphanedSourceFiles: number;
  removedOriginalFiles: number;
  removedThumbnailFiles: number;
  removedTempFiles: number;
}) {
  return [
    "사용 중이 아닌 라이브러리 파일을 정리할까요?",
    `원본 ${result.removedOriginalFiles}개, 썸네일 ${result.removedThumbnailFiles}개, 임시 파일 ${result.removedTempFiles}개가 대상입니다.`,
  ].join("\n");
}

function cleanupResultMessage(result: {
  removedOriginalFiles: number;
  removedThumbnailFiles: number;
  removedTempFiles: number;
}) {
  const total =
    result.removedOriginalFiles + result.removedThumbnailFiles + result.removedTempFiles;
  return total === 0
    ? "정리할 라이브러리 파일이 없습니다."
    : `라이브러리 파일 ${total}개를 정리했습니다.`;
}

const RESTORE_SESSION_KEY = "pmtconcon:last-route-restore-attempted";

function shouldAttemptStartupRestore() {
  return window.sessionStorage.getItem(RESTORE_SESSION_KEY) !== "1";
}

function markStartupRestoreAttempted() {
  window.sessionStorage.setItem(RESTORE_SESSION_KEY, "1");
}

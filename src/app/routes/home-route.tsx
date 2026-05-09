import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { FileImage, FolderPlus, Plus, Upload } from "lucide-react";

import { CollectionGrid } from "@/features/collections/components/CollectionGrid";
import { DropImportZone } from "@/features/collections/components/DropImportZone";
import {
  createCollection,
  listCollections,
  renameCollection,
} from "@/features/collections/api";
import type { CollectionSummary } from "@/features/collections/types";
import { importImagesIntoCollection } from "@/features/icons/api";
import {
  IMPORTABLE_IMAGE_ACCEPT,
  partitionImportableImageFiles,
} from "@/lib/file-types";
import { getCommandErrorMessage } from "@/lib/tauri";

export function HomeRoute() {
  const navigate = useNavigate();
  const fileInputRef = useRef<HTMLInputElement>(null);
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
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  }, []);

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
    } catch (error) {
      setCollections(previousCollections);
      setErrorMessage(getCommandErrorMessage(error));
    }
  };

  const handleImportFiles = useCallback(
    async (files: File[]) => {
      const { accepted, rejected } = partitionImportableImageFiles(files);
      setErrorMessage(null);
      setImportStatus(null);
      setIsActionPanelOpen(false);

      if (accepted.length === 0) {
        setImportStatus("가져올 수 있는 jpg, jpeg, png, gif 파일이 없습니다.");
        return;
      }

      setIsImporting(true);

      try {
        let targetCollection =
          collections.find((collection) => collection.id === selectedCollectionId) ?? null;

        if (!targetCollection) {
          targetCollection = await createCollection();
        }

        const result = await importImagesIntoCollection(targetCollection.id, accepted);
        const skippedCount = rejected.length + result.rejectedFiles.length;

        setCollections((currentCollections) =>
          upsertCollection(currentCollections, result.collection),
        );
        setSelectedCollectionId(result.collection.id);
        setImportStatus(importStatusMessage(result.importedIcons.length, skippedCount));
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
                  aria-disabled="true"
                  className="flex w-full cursor-not-allowed items-center gap-3 rounded-md px-3 py-2 text-left text-sm text-muted"
                  title="폴더 가져오기는 이후 단계에서 연결됩니다."
                  type="button"
                >
                  <FolderPlus aria-hidden="true" />
                  폴더 가져오기 준비 중
                </button>
              </div>
            ) : null}
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
              selectedCollectionId={selectedCollectionId}
              onOpenCollection={openCollection}
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

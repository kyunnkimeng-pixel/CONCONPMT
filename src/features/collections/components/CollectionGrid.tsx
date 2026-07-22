import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { MouseEvent } from "react";
import { Copy, Trash2 } from "lucide-react";

import { CollectionCard } from "@/features/collections/components/CollectionCard";
import type { CollectionSummary } from "@/features/collections/types";

interface CollectionGridProps {
  collections: CollectionSummary[];
  isDeleteDisabled?: boolean;
  selectedCollectionId: string | null;
  onOpenCollection: (collectionId: string) => void;
  onDuplicateCollection: (collectionId: string) => void;
  onDeleteCollection: (collectionId: string) => void;
  onRenameCollection: (collectionId: string, name: string) => void;
  onSelectCollection: (collectionId: string) => void;
}

export function CollectionGrid({
  collections,
  isDeleteDisabled = false,
  selectedCollectionId,
  onOpenCollection,
  onDuplicateCollection,
  onDeleteCollection,
  onRenameCollection,
  onSelectCollection,
}: CollectionGridProps) {
  const [contextMenu, setContextMenu] = useState<{
    collectionId: string;
    x: number;
    y: number;
  } | null>(null);
  const targetCollection = contextMenu
    ? collections.find((collection) => collection.id === contextMenu.collectionId) ?? null
    : null;

  const handleContextMenu = (event: MouseEvent, collectionId: string) => {
    event.preventDefault();
    onSelectCollection(collectionId);
    setContextMenu({
      collectionId,
      x: event.clientX,
      y: event.clientY,
    });
  };

  return (
    <>
      <section
        aria-label="디시콘 모음"
        className="grid grid-cols-[repeat(auto-fill,minmax(176px,1fr))] gap-4"
        role="listbox"
      >
        {collections.map((collection) => (
          <CollectionCard
            collection={collection}
            isSelected={collection.id === selectedCollectionId}
            key={collection.id}
            onContextMenu={(event) => handleContextMenu(event, collection.id)}
            onOpen={() => onOpenCollection(collection.id)}
            onRename={(name) => onRenameCollection(collection.id, name)}
            onSelect={() => onSelectCollection(collection.id)}
          />
        ))}
      </section>
      {contextMenu && targetCollection ? (
        <CollectionContextMenu
          collectionName={targetCollection.name}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
          onDuplicate={() => {
            onDuplicateCollection(targetCollection.id);
            setContextMenu(null);
          }}
          onDelete={() => {
            onDeleteCollection(targetCollection.id);
            setContextMenu(null);
          }}
          isDeleteDisabled={isDeleteDisabled}
        />
      ) : null}
    </>
  );
}

function CollectionContextMenu({
  collectionName,
  x,
  y,
  onClose,
  onDelete,
  onDuplicate,
  isDeleteDisabled,
}: {
  collectionName: string;
  x: number;
  y: number;
  onClose: () => void;
  onDelete: () => void;
  onDuplicate: () => void;
  isDeleteDisabled: boolean;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ left: x, top: y, measured: false });

  useEffect(() => {
    const handlePointerDown = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) {
        onClose();
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) {
      return;
    }
    const margin = 8;
    const rect = menu.getBoundingClientRect();
    setPosition({
      left: Math.min(
        Math.max(x, margin),
        Math.max(margin, window.innerWidth - rect.width - margin),
      ),
      top: Math.min(
        Math.max(y, margin),
        Math.max(margin, window.innerHeight - rect.height - margin),
      ),
      measured: true,
    });
  }, [x, y]);

  return (
    <div
      ref={menuRef}
      aria-label={`${collectionName} 모음 작업 메뉴`}
      className="fixed z-50 min-w-48 rounded-md border border-border bg-white p-1 shadow-lg"
      data-testid="collection-context-menu"
      role="menu"
      style={{
        left: position.left,
        top: position.top,
        visibility: position.measured ? "visible" : "hidden",
      }}
    >
      <button
        className="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        data-testid="collection-context-duplicate"
        role="menuitem"
        type="button"
        onClick={onDuplicate}
      >
        <Copy aria-hidden="true" className="size-4" />
        모음 복제하기
      </button>
      <button
        className="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-danger hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
        data-testid="collection-context-delete"
        disabled={isDeleteDisabled}
        role="menuitem"
        title={
          isDeleteDisabled ? "이미지를 가져오는 동안에는 삭제할 수 없습니다." : undefined
        }
        type="button"
        onClick={onDelete}
      >
        <Trash2 aria-hidden="true" className="size-4" />
        모음 삭제하기
      </button>
    </div>
  );
}

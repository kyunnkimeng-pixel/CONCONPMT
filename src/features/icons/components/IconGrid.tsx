import { useEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent } from "react";
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import type { DragEndEvent } from "@dnd-kit/core";
import {
  arrayMove,
  rectSortingStrategy,
  SortableContext,
  sortableKeyboardCoordinates,
} from "@dnd-kit/sortable";

import type {
  CollectionSummary,
  IconPieceSummary,
  IconSummary,
} from "@/features/collections/types";
import { IconContextMenu } from "@/features/icons/components/IconContextMenu";
import { IconTile } from "@/features/icons/components/IconTile";
import {
  type IconSelectionState,
  pruneSelection,
  selectIcon,
  selectIconForContextMenu,
} from "@/features/icons/selection/selection-model";

interface IconGridProps {
  collection: CollectionSummary;
  icons: IconSummary[];
  thumbnailOnly: boolean;
  duplicatePieceIds: Set<string>;
  editRequest: { pieceId: string; requestKey: number } | null;
  validateAltDraft: (pieceId: string, value: string) => string | null;
  validateCurrentAlt: (piece: IconPieceSummary) => string | null;
  onAltCommit: (pieceId: string, value: string) => Promise<boolean>;
  onBatchAltCommit: (iconIds: string[], value: string) => Promise<boolean>;
  onDeleteIcons: (iconIds: string[]) => Promise<boolean>;
  onDuplicateIcon: (iconId: string) => Promise<void>;
  onEditIcon: (iconId: string) => void;
  onRenameIcon: (iconId: string, displayName: string) => Promise<boolean>;
  onReorderIcons: (orderedIconIds: string[]) => Promise<void>;
  onRevealExportResult: (iconId: string) => Promise<void>;
  onRevealOriginal: (iconId: string) => Promise<void>;
  onReplaceImage: (iconId: string) => void;
  onSetCover: (iconId: string) => Promise<void>;
  onSetReadiness: (
    iconIds: string[],
    readiness: IconSummary["readiness"],
  ) => Promise<void>;
  onSetThumbnailOverride: (iconId: string) => void;
}

export function IconGrid({
  collection,
  icons,
  thumbnailOnly,
  duplicatePieceIds,
  editRequest,
  validateAltDraft,
  validateCurrentAlt,
  onAltCommit,
  onBatchAltCommit,
  onDeleteIcons,
  onDuplicateIcon,
  onEditIcon,
  onRenameIcon,
  onReorderIcons,
  onRevealExportResult,
  onRevealOriginal,
  onReplaceImage,
  onSetCover,
  onSetReadiness,
  onSetThumbnailOverride,
}: IconGridProps) {
  const gridRef = useRef<HTMLDivElement>(null);
  const orderedIds = useMemo(() => icons.map((icon) => icon.id), [icons]);
  const [selection, setSelection] = useState<IconSelectionState>({
    selectedIds: [],
    anchorId: null,
  });
  const selectedIdSet = useMemo(
    () => new Set(selection.selectedIds),
    [selection.selectedIds],
  );
  const [contextMenu, setContextMenu] = useState<{
    iconId: string;
    x: number;
    y: number;
  } | null>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 6 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );
  const previewScale = thumbnailOnly ? 1.42 : 1;
  const effectivePreviewWidth = Math.round(collection.previewWidth * previewScale);
  const effectivePreviewHeight = Math.round(collection.previewHeight * previewScale);
  const hasHorizontalDouble = icons.some((icon) => icon.shape === "horizontal_double");
  const minTileWidth = Math.max(
    thumbnailOnly ? 180 : 148,
    (hasHorizontalDouble ? effectivePreviewWidth * 2 : effectivePreviewWidth) +
      (thumbnailOnly ? 28 : 48),
  );

  useEffect(() => {
    setSelection((current) => {
      return pruneSelection(current, orderedIds);
    });
  }, [orderedIds]);

  const updateSelection = (next: IconSelectionState) => {
    setSelection(next);
  };

  const handleSelect = (event: MouseEvent, iconId: string) => {
    gridRef.current?.focus();
    updateSelection(
      selectIcon(selection, orderedIds, iconId, {
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey,
      }),
    );
  };

  const handleContextMenu = (event: MouseEvent, iconId: string) => {
    event.preventDefault();
    gridRef.current?.focus();
    updateSelection(selectIconForContextMenu(selection, orderedIds, iconId));
    setContextMenu({
      iconId,
      x: event.clientX,
      y: event.clientY,
    });
  };

  const handleDelete = async (iconIds: string[]) => {
    if (iconIds.length === 0) {
      return;
    }

    const message =
      iconIds.length > 1
        ? `선택한 아이콘 ${iconIds.length}개를 삭제할까요?`
        : "이 아이콘을 삭제할까요?";
    if (!window.confirm(message)) {
      return;
    }

    const didDelete = await onDeleteIcons(iconIds);
    if (didDelete) {
      updateSelection({ selectedIds: [], anchorId: null });
    }
  };

  const handleBatchAltEdit = async (iconIds: string[]) => {
    if (iconIds.length === 0) {
      return;
    }

    const nextAlt = window.prompt(
      `${iconIds.length}개 아이콘의 모든 alt 값을 변경합니다. 여러 alt가 바뀌는 경우 입력값 뒤에 1, 2...를 붙여 중복을 피합니다.`,
      "",
    );
    if (nextAlt === null) {
      return;
    }

    await onBatchAltCommit(iconIds, nextAlt);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }

    const oldIndex = orderedIds.indexOf(String(active.id));
    const newIndex = orderedIds.indexOf(String(over.id));
    if (oldIndex === -1 || newIndex === -1) {
      return;
    }

    const nextIds = arrayMove(orderedIds, oldIndex, newIndex);
    void onReorderIcons(nextIds);
  };

  const targetIcon = contextMenu
    ? icons.find((icon) => icon.id === contextMenu.iconId) ?? null
    : null;
  const contextSelectionIds =
    targetIcon && selectedIdSet.has(targetIcon.id)
      ? selection.selectedIds
      : targetIcon
        ? [targetIcon.id]
        : [];

  return (
    <>
      <DndContext
        collisionDetection={closestCenter}
        sensors={sensors}
        onDragEnd={handleDragEnd}
      >
        <SortableContext items={orderedIds} strategy={rectSortingStrategy}>
          <div
            ref={gridRef}
            aria-label={`${collection.name} 아이콘`}
            className="grid select-none gap-4 focus:outline-none"
            role="listbox"
            style={{
              gridTemplateColumns: `repeat(auto-fill, minmax(${minTileWidth}px, 1fr))`,
            }}
            tabIndex={0}
            onClick={(event) => {
              if (event.target === event.currentTarget) {
                updateSelection({ selectedIds: [], anchorId: null });
              }
            }}
            onKeyDown={(event) => {
              if (event.key === "Delete" && selection.selectedIds.length > 0) {
                event.preventDefault();
                void handleDelete(selection.selectedIds);
              }
            }}
          >
            {icons.map((icon) => (
              <IconTile
                duplicatePieceIds={duplicatePieceIds}
                editRequest={editRequest}
                icon={icon}
                isCover={collection.coverIconId === icon.id}
                isSelected={selectedIdSet.has(icon.id)}
                key={icon.id}
                previewHeight={effectivePreviewHeight}
                previewWidth={effectivePreviewWidth}
                showDetails={!thumbnailOnly}
                validateAltDraft={validateAltDraft}
                validateCurrentAlt={validateCurrentAlt}
                onAltCommit={onAltCommit}
                onContextMenu={handleContextMenu}
                onOpenEditor={onEditIcon}
                onRename={onRenameIcon}
                onSelect={handleSelect}
              />
            ))}
          </div>
        </SortableContext>
      </DndContext>

      {contextMenu && targetIcon ? (
        <IconContextMenu
          isCover={collection.coverIconId === targetIcon.id}
          hasExportResult={targetIcon.pieces.some((piece) => piece.lastExportUrl)}
          selectionCount={contextSelectionIds.length}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
          onDelete={() => {
            void handleDelete(contextSelectionIds);
          }}
          onBatchAltEdit={() => {
            void handleBatchAltEdit(contextSelectionIds);
          }}
          onDuplicate={() => {
            void onDuplicateIcon(targetIcon.id);
          }}
          onEdit={() => onEditIcon(targetIcon.id)}
          onRevealExportResult={() => {
            void onRevealExportResult(targetIcon.id);
          }}
          onRevealOriginal={() => {
            void onRevealOriginal(targetIcon.id);
          }}
          onReplaceImage={() => onReplaceImage(targetIcon.id)}
          onRename={() => {
            const nextName = window.prompt("아이콘 이름", targetIcon.displayName);
            if (nextName !== null) {
              void onRenameIcon(targetIcon.id, nextName);
            }
          }}
          onSetCover={() => {
            void onSetCover(targetIcon.id);
          }}
          onSetReadiness={(readiness) => {
            void onSetReadiness(contextSelectionIds, readiness);
          }}
          onSetThumbnailOverride={() => onSetThumbnailOverride(targetIcon.id)}
        />
      ) : null}
    </>
  );
}

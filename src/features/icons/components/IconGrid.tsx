import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent, MouseEvent, PointerEvent as ReactPointerEvent } from "react";
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
  const [batchAltDialog, setBatchAltDialog] = useState<{
    iconIds: string[];
    pieceCount: number;
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

  const handleBatchAltEdit = (iconIds: string[]) => {
    if (iconIds.length === 0) {
      return;
    }

    setBatchAltDialog({
      iconIds,
      pieceCount: altPieceCountForIconIds(icons, iconIds),
    });
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
  const contextAltSelectionCount = altPieceCountForIconIds(icons, contextSelectionIds);

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
          altSelectionCount={contextAltSelectionCount}
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
      {batchAltDialog ? (
        <BatchAltDialog
          iconCount={batchAltDialog.iconIds.length}
          pieceCount={batchAltDialog.pieceCount}
          onClose={() => setBatchAltDialog(null)}
          onSubmit={(value) => {
            void onBatchAltCommit(batchAltDialog.iconIds, value).then((didCommit) => {
              if (didCommit) {
                setBatchAltDialog(null);
              }
            });
          }}
        />
      ) : null}
    </>
  );
}

function altPieceCountForIconIds(icons: IconSummary[], iconIds: string[]) {
  const iconIdSet = new Set(iconIds);
  return icons
    .filter((icon) => iconIdSet.has(icon.id))
    .reduce((count, icon) => count + icon.pieces.length, 0);
}

function BatchAltDialog({
  iconCount,
  pieceCount,
  onClose,
  onSubmit,
}: {
  iconCount: number;
  pieceCount: number;
  onClose: () => void;
  onSubmit: (value: string) => void;
}) {
  const [value, setValue] = useState("");
  const [position, setPosition] = useState({ x: 96, y: 96 });
  const dragRef = useRef<{
    pointerId: number;
    startClientX: number;
    startClientY: number;
    startX: number;
    startY: number;
  } | null>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const startDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const target = event.target;
    if (target instanceof HTMLElement && target.closest("button,input,textarea,select")) {
      return;
    }
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      pointerId: event.pointerId,
      startClientX: event.clientX,
      startClientY: event.clientY,
      startX: position.x,
      startY: position.y,
    };
  };

  const moveDialog = (event: ReactPointerEvent<HTMLElement>) => {
    const dragState = dragRef.current;
    if (!dragState) {
      return;
    }

    event.preventDefault();
    setPosition({
      x: Math.max(8, dragState.startX + event.clientX - dragState.startClientX),
      y: Math.max(8, dragState.startY + event.clientY - dragState.startClientY),
    });
  };

  const stopDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) {
      dragRef.current = null;
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSubmit(value);
  };

  return (
    <div className="fixed inset-0 z-[60] pointer-events-none">
      <form
        className="pointer-events-auto fixed flex w-[min(420px,calc(100vw-32px))] flex-col rounded-md border border-border bg-white shadow-xl"
        data-testid="batch-alt-dialog"
        style={{
          left: position.x,
          top: position.y,
        }}
        onSubmit={submit}
      >
        <header
          className="flex cursor-move select-none items-center justify-between gap-3 border-b border-border bg-canvas px-4 py-3"
          onPointerCancel={stopDrag}
          onPointerDown={startDrag}
          onPointerMove={moveDialog}
          onPointerUp={stopDrag}
        >
          <div className="min-w-0">
            <h3 className="text-sm font-semibold tracking-normal">alt 값 일괄 변경</h3>
            <p className="mt-1 text-xs text-muted">
              선택 아이콘 {iconCount}개 · 실제 alt {pieceCount}개 변경
            </p>
          </div>
          <button
            aria-label="alt 일괄 변경 닫기"
            className="rounded border border-border bg-white px-2 py-1 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            type="button"
            onClick={onClose}
          >
            닫기
          </button>
        </header>
        <div className="flex flex-col gap-3 p-4">
          <label className="flex flex-col gap-1 text-xs font-medium text-muted">
            입력값
            <textarea
              autoFocus
              className="min-h-24 resize-y rounded-md border border-border bg-white px-3 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              placeholder="예: 가,나,다"
              value={value}
              onChange={(event) => setValue(event.currentTarget.value)}
            />
          </label>
          <div className="rounded-md border border-border bg-canvas px-3 py-2 text-xs text-muted">
            쉼표로 구분하면 실제 alt {pieceCount}개에 순서대로 적용됩니다. 입력 수가
            부족하면 마지막 값에 1, 2...를 붙입니다. 빈칸이면 1, 2, 3... 순번을 넣습니다.
          </div>
          <div className="flex justify-end gap-2">
            <button
              className="rounded border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="button"
              onClick={onClose}
            >
              취소
            </button>
            <button
              className="rounded bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              type="submit"
            >
              적용
            </button>
          </div>
        </div>
      </form>
    </div>
  );
}

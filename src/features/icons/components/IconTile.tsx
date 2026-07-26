import type { CSSProperties, MouseEvent } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Images, NotebookText, Star } from "lucide-react";

import { InlineNameEditor } from "@/components/explorer/InlineNameEditor";
import type { IconPieceSummary, IconSummary } from "@/features/collections/types";
import { AltInlineEditor } from "@/features/icons/components/AltInlineEditor";
import { cn } from "@/lib/utils";

interface IconTileProps {
  icon: IconSummary;
  isSelected: boolean;
  isCover: boolean;
  previewWidth: number;
  previewHeight: number;
  showDetails: boolean;
  duplicatePieceIds: Set<string>;
  editRequest: { pieceId: string; requestKey: number } | null;
  validateAltDraft: (pieceId: string, value: string) => string | null;
  validateCurrentAlt: (piece: IconPieceSummary) => string | null;
  onAltCommit: (pieceId: string, value: string) => Promise<boolean>;
  onContextMenu: (event: MouseEvent, iconId: string) => void;
  onEditNote: (iconId: string) => void;
  onOpenEditor: (iconId: string) => void;
  onRename: (iconId: string, displayName: string) => Promise<boolean>;
  onSelect: (event: MouseEvent, iconId: string) => void;
}

const nonDraggableImageStyle: CSSProperties & { WebkitUserDrag: string } = {
  WebkitUserDrag: "none",
  userSelect: "none",
};

export function IconTile({
  icon,
  isSelected,
  isCover,
  previewWidth,
  previewHeight,
  showDetails,
  duplicatePieceIds,
  editRequest,
  validateAltDraft,
  validateCurrentAlt,
  onAltCommit,
  onContextMenu,
  onEditNote,
  onOpenEditor,
  onRename,
  onSelect,
}: IconTileProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: icon.id });
  const previewUrl =
    icon.thumbnailOverrideUrl ?? icon.currentPreviewUrl ?? icon.thumbnailUrl;
  const previewSize = tilePreviewSize(icon.shape, previewWidth, previewHeight);
  const style = {
    transform: CSS.Transform.toString(transform),
    transition: isDragging ? "none" : transition,
  };

  return (
    <figure
      ref={setNodeRef}
      {...attributes}
      {...listeners}
      aria-label={`${icon.displayName} 아이콘`}
      aria-selected={isSelected}
      className={cn(
        "group flex cursor-default select-none flex-col items-center rounded-lg border border-border bg-card shadow-sm transition hover:border-border-strong hover:shadow-md focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus aria-selected:border-focus aria-selected:bg-selected",
        showDetails ? "min-h-[188px] p-3" : "min-h-0 p-2",
        icon.readiness === "working" && "bg-slate-100 opacity-90",
        icon.iconKind === "placeholder" && "border-dashed",
        isDragging && "opacity-70 shadow-lg",
      )}
      data-testid="icon-tile"
      role="option"
      style={style}
      tabIndex={0}
      onClick={(event) => {
        if (event.detail >= 2) {
          onOpenEditor(icon.id);
          return;
        }

        onSelect(event, icon.id);
      }}
      onContextMenu={(event) => onContextMenu(event, icon.id)}
      onDoubleClick={() => onOpenEditor(icon.id)}
      onDragStart={(event) => event.preventDefault()}
    >
      <div
        className="relative flex max-w-full items-center justify-center overflow-hidden rounded-md border border-border bg-preview"
        style={{
          height: previewSize.height,
          width: previewSize.width,
        }}
      >
        <PiecePreviewGrid
          fallbackUrl={previewUrl}
          icon={icon}
          previewHeight={previewHeight}
          previewWidth={previewWidth}
        />
        {isCover ? (
          <span className="absolute right-1 top-1 inline-flex items-center gap-1 rounded bg-white/90 px-1.5 py-0.5 text-[11px] font-medium text-foreground">
            <Star aria-hidden="true" />
            대표
          </span>
        ) : null}
        {icon.readiness === "working" ? (
          <span className="absolute left-1 top-1 rounded bg-slate-700 px-1.5 py-0.5 text-[11px] font-medium text-white">
            작업중
          </span>
        ) : null}
      </div>

      {showDetails ? (
        <figcaption className="mt-1.5 flex w-full flex-col items-center gap-1">
          <div className="flex max-w-full items-center justify-center gap-1">
            <InlineNameEditor
              ariaLabel={`${icon.displayName} 아이콘명 변경`}
              value={icon.displayName}
              onCommit={(value) => {
                void onRename(icon.id, value);
              }}
            />
            <IconMemoButton
              displayName={icon.displayName}
              note={icon.note}
              onEdit={() => onEditNote(icon.id)}
            />
          </div>
          <div
            className={cn(
              "w-full gap-1",
              icon.shape === "single" ? "flex flex-col" : "grid grid-cols-2",
            )}
          >
            {icon.pieces.map((piece) => (
              <div className="min-w-0" data-testid="icon-alt-field" key={piece.id}>
                <div className="flex min-w-0 items-start gap-1">
                  <span
                    className="mt-1.5 w-8 shrink-0 text-right text-[11px] font-medium text-muted"
                    data-testid="icon-alt-label"
                  >
                    {pieceAltLabel(piece)}
                  </span>
                  <AltInlineEditor
                    ariaLabel={`${icon.displayName} ${pieceLabel(piece)} alt 수정`}
                    compact
                    editRequestKey={
                      editRequest?.pieceId === piece.id
                        ? editRequest.requestKey
                        : undefined
                    }
                    validationMessage={
                      duplicatePieceIds.has(piece.id)
                        ? "중복된 alt 값입니다."
                        : validateCurrentAlt(piece)
                    }
                    validateDraft={(value) => validateAltDraft(piece.id, value)}
                    value={piece.altText}
                    onCommit={(value) => onAltCommit(piece.id, value)}
                  />
                </div>
              </div>
            ))}
          </div>
        </figcaption>
      ) : null}
    </figure>
  );
}

function PiecePreviewGrid({
  fallbackUrl,
  icon,
  previewWidth,
  previewHeight,
}: {
  fallbackUrl: string | null;
  icon: IconSummary;
  previewWidth: number;
  previewHeight: number;
}) {
  if (icon.iconKind === "placeholder") {
    return (
      <div className="flex size-full items-center justify-center bg-slate-100 px-3 text-center text-sm font-semibold text-muted">
        {icon.placeholderText || icon.displayName}
      </div>
    );
  }

  if (icon.shape === "single") {
    return <PreviewImage src={fallbackUrl} />;
  }

  const isHorizontal = icon.shape === "horizontal_double";

  return (
    <div
      className="relative overflow-hidden bg-preview"
      data-testid="icon-piece-preview-grid"
      style={{
        height: isHorizontal ? previewHeight : previewHeight * 2,
        width: isHorizontal ? previewWidth * 2 : previewWidth,
      }}
    >
      <PreviewImage src={fallbackUrl} />
      <span
        aria-hidden="true"
        className={cn(
          "pointer-events-none absolute border-border",
          isHorizontal
            ? "bottom-0 left-1/2 top-0 border-l"
            : "left-0 right-0 top-1/2 border-t",
        )}
      />
    </div>
  );
}

function PreviewImage({ src }: { src: string | null }) {
  if (!src) {
    return (
      <span className="flex size-full items-center justify-center">
        <Images aria-hidden="true" className="text-muted" />
      </span>
    );
  }

  return (
    <img
      alt=""
      className="size-full object-contain"
      draggable={false}
      src={src}
      style={nonDraggableImageStyle}
      onDragStart={(event) => event.preventDefault()}
    />
  );
}

export function IconMemoButton({
  displayName,
  note,
  onEdit,
}: {
  displayName: string;
  note: string | null;
  onEdit: () => void;
}) {
  const trimmed = note?.trim();
  const hasNote = Boolean(trimmed);
  const actionLabel = `${displayName} 메모 ${hasNote ? "수정" : "추가"}`;

  return (
    <button
      aria-label={actionLabel}
      className={cn(
        "group/memo relative inline-flex size-6 shrink-0 items-center justify-center rounded text-muted hover:bg-menu-hover hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
        hasNote
          ? "opacity-100"
          : "opacity-45 group-hover:opacity-100 focus-visible:opacity-100",
      )}
      data-testid={hasNote ? "icon-memo-indicator" : "icon-memo-add"}
      title={actionLabel}
      type="button"
      onClick={(event) => {
        event.stopPropagation();
        onEdit();
      }}
      onDoubleClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
    >
      <NotebookText aria-hidden="true" className="size-3.5" />
      <span
        className={cn(
          "pointer-events-none absolute left-1/2 top-full z-20 mt-2 hidden -translate-x-1/2 rounded-md border border-border bg-white px-3 py-2 text-left text-xs leading-5 text-foreground shadow-lg group-hover/memo:inline-block group-focus/memo:inline-block",
          trimmed ? "w-64 whitespace-pre-wrap" : "whitespace-nowrap",
        )}
      >
        {trimmed || "메모 추가"}
      </span>
    </button>
  );
}

function tilePreviewSize(shape: IconSummary["shape"], width: number, height: number) {
  return {
    width: shape === "horizontal_double" ? width * 2 : width,
    height: shape === "vertical_double" ? height * 2 : height,
  };
}

function pieceLabel(piece: IconPieceSummary) {
  switch (piece.pieceRole) {
    case "left":
      return "왼쪽";
    case "right":
      return "오른쪽";
    case "top":
      return "위쪽";
    case "bottom":
      return "아래쪽";
    case "single":
      return "단일";
  }
}

function pieceAltLabel(piece: IconPieceSummary) {
  switch (piece.pieceRole) {
    case "left":
      return "왼";
    case "right":
      return "오";
    case "top":
      return "위";
    case "bottom":
      return "아래";
    case "single":
      return "alt";
  }
}

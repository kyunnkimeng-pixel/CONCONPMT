import type { MouseEvent } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Images, Star } from "lucide-react";

import type { IconPieceSummary, IconSummary } from "@/features/collections/types";
import { AltInlineEditor } from "@/features/icons/components/AltInlineEditor";
import { cn } from "@/lib/utils";

interface IconTileProps {
  icon: IconSummary;
  isSelected: boolean;
  isCover: boolean;
  previewWidth: number;
  previewHeight: number;
  duplicatePieceIds: Set<string>;
  editRequest: { pieceId: string; requestKey: number } | null;
  validateAltDraft: (pieceId: string, value: string) => string | null;
  validateCurrentAlt: (piece: IconPieceSummary) => string | null;
  onAltCommit: (pieceId: string, value: string) => Promise<boolean>;
  onContextMenu: (event: MouseEvent, iconId: string) => void;
  onOpenEditor: (iconId: string) => void;
  onSelect: (event: MouseEvent, iconId: string) => void;
}

export function IconTile({
  icon,
  isSelected,
  isCover,
  previewWidth,
  previewHeight,
  duplicatePieceIds,
  editRequest,
  validateAltDraft,
  validateCurrentAlt,
  onAltCommit,
  onContextMenu,
  onOpenEditor,
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
  const previewUrl = icon.currentPreviewUrl ?? icon.thumbnailUrl;
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <figure
      ref={setNodeRef}
      {...attributes}
      {...listeners}
      aria-label={`${icon.displayName} 아이콘`}
      aria-selected={isSelected}
      className={cn(
        "group flex min-h-[196px] cursor-default flex-col items-center rounded-lg border border-border bg-card p-3 shadow-sm transition hover:border-border-strong hover:shadow-md focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus aria-selected:border-focus aria-selected:bg-selected",
        isDragging && "opacity-70 shadow-lg",
      )}
      role="option"
      style={style}
      tabIndex={0}
      onClick={(event) => onSelect(event, icon.id)}
      onContextMenu={(event) => onContextMenu(event, icon.id)}
      onDoubleClick={() => onOpenEditor(icon.id)}
    >
      <div
        className="relative flex max-w-full items-center justify-center overflow-hidden rounded-md border border-border bg-preview"
        style={{
          height: previewHeight,
          width: previewWidth,
        }}
      >
        {previewUrl ? (
          <img
            alt=""
            className="size-full object-contain"
            draggable={false}
            src={previewUrl}
          />
        ) : (
          <Images aria-hidden="true" className="text-muted" />
        )}
        {isCover ? (
          <span className="absolute right-1 top-1 inline-flex items-center gap-1 rounded bg-white/90 px-1.5 py-0.5 text-[11px] font-medium text-foreground">
            <Star aria-hidden="true" />
            대표
          </span>
        ) : null}
      </div>

      <figcaption className="mt-2 flex w-full flex-col items-center gap-1">
        {icon.pieces.map((piece) => (
          <AltInlineEditor
            ariaLabel={`${icon.displayName} ${pieceLabel(piece)} alt 수정`}
            editRequestKey={
              editRequest?.pieceId === piece.id ? editRequest.requestKey : undefined
            }
            key={piece.id}
            validationMessage={
              duplicatePieceIds.has(piece.id)
                ? "중복된 alt 값입니다."
                : validateCurrentAlt(piece)
            }
            validateDraft={(value) => validateAltDraft(piece.id, value)}
            value={piece.altText}
            onCommit={(value) => onAltCommit(piece.id, value)}
          />
        ))}
      </figcaption>
    </figure>
  );
}

function pieceLabel(piece: IconPieceSummary) {
  switch (piece.pieceRole) {
    case "left":
      return "왼쪽";
    case "right":
      return "오른쪽";
    case "top":
      return "위";
    case "bottom":
      return "아래";
    case "single":
      return "단일";
  }
}

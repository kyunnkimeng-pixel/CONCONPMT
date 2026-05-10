import type { CSSProperties, MouseEvent } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Images, Star } from "lucide-react";

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
  duplicatePieceIds: Set<string>;
  editRequest: { pieceId: string; requestKey: number } | null;
  validateAltDraft: (pieceId: string, value: string) => string | null;
  validateCurrentAlt: (piece: IconPieceSummary) => string | null;
  onAltCommit: (pieceId: string, value: string) => Promise<boolean>;
  onContextMenu: (event: MouseEvent, iconId: string) => void;
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
  duplicatePieceIds,
  editRequest,
  validateAltDraft,
  validateCurrentAlt,
  onAltCommit,
  onContextMenu,
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
        "group flex min-h-[220px] cursor-default select-none flex-col items-center rounded-lg border border-border bg-card p-3 shadow-sm transition hover:border-border-strong hover:shadow-md focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus aria-selected:border-focus aria-selected:bg-selected",
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
      </div>

      <figcaption className="mt-2 flex w-full flex-col items-center gap-2">
        <div className="w-full">
          <span
            className="mb-1 block text-[11px] font-medium text-muted"
            data-testid="icon-file-name-label"
          >
            파일명
          </span>
          <InlineNameEditor
            ariaLabel={`${icon.displayName} 아이콘명 변경`}
            value={icon.displayName}
            onCommit={(value) => {
              void onRename(icon.id, value);
            }}
          />
        </div>
        <div
          className={cn(
            "w-full gap-1",
            icon.shape === "horizontal_double" ? "grid grid-cols-2" : "flex flex-col",
          )}
        >
          {icon.pieces.map((piece) => (
            <div className="min-w-0" data-testid="icon-alt-field" key={piece.id}>
              <span
                className="mb-1 block text-[11px] font-medium text-muted"
                data-testid="icon-alt-label"
              >
                {pieceAltLabel(piece)}
              </span>
              <AltInlineEditor
                ariaLabel={`${icon.displayName} ${pieceLabel(piece)} alt 수정`}
                editRequestKey={
                  editRequest?.pieceId === piece.id ? editRequest.requestKey : undefined
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
          ))}
        </div>
      </figcaption>
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
  return piece.pieceRole === "single" ? "alt 값" : `${pieceLabel(piece)} alt`;
}

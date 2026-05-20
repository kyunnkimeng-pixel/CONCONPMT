import { Images } from "lucide-react";
import type { MouseEvent } from "react";

import { InlineNameEditor } from "@/components/explorer/InlineNameEditor";
import type { CollectionSummary } from "@/features/collections/types";

interface CollectionCardProps {
  collection: CollectionSummary;
  isSelected: boolean;
  onOpen: () => void;
  onContextMenu: (event: MouseEvent) => void;
  onRename: (name: string) => void;
  onSelect: () => void;
}

export function CollectionCard({
  collection,
  isSelected,
  onOpen,
  onContextMenu,
  onRename,
  onSelect,
}: CollectionCardProps) {
  return (
    <article
      aria-selected={isSelected}
      className="group flex min-h-[250px] cursor-default flex-col rounded-lg border border-border bg-card p-3 shadow-sm transition hover:border-border-strong hover:shadow-md aria-selected:border-focus aria-selected:bg-selected"
      role="option"
      tabIndex={0}
      onClick={onSelect}
      onContextMenu={onContextMenu}
      onDoubleClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          onOpen();
        }
      }}
    >
      <div className="flex aspect-square items-center justify-center overflow-hidden rounded-md border border-border bg-preview">
        {collection.coverImageUrl ? (
          <img
            alt=""
            className="size-full object-cover"
            draggable={false}
            src={collection.coverImageUrl}
          />
        ) : (
          <div className="flex size-full items-center justify-center text-muted">
            <Images aria-hidden="true" />
          </div>
        )}
      </div>

      <div className="mt-3 flex min-w-0 flex-1 flex-col items-center justify-end gap-1">
        <InlineNameEditor
          ariaLabel={`${collection.name} 이름 변경`}
          value={collection.name}
          onCommit={onRename}
        />
        <p className="text-xs text-muted">{collection.iconCount}개 항목</p>
      </div>
    </article>
  );
}

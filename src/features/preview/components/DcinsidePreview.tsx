import { useEffect, useMemo, useState } from "react";
import { ImagePlus, MessageSquareText } from "lucide-react";

import type { CollectionSummary, IconSummary } from "@/features/collections/types";
import {
  appendUsagePreviewIcon,
  buildUsagePreviewIcons,
  hasAnimatedPreview,
  type InsertedPreviewIcon,
  type UsagePreviewIcon,
} from "@/features/preview/preview-model";
import {
  PreviewComposer,
  PreviewPieceImage,
  previewGroupDirectionClass,
} from "@/features/preview/components/PreviewComposer";
import { cn } from "@/lib/utils";

interface DcinsidePreviewProps {
  collection: CollectionSummary;
  icons: IconSummary[];
}

export function DcinsidePreview({ collection, icons }: DcinsidePreviewProps) {
  const previewIcons = useMemo(
    () => buildUsagePreviewIcons(collection, icons),
    [collection, icons],
  );
  const [commentText, setCommentText] = useState("");
  const [insertedItems, setInsertedItems] = useState<InsertedPreviewIcon[]>([]);
  const [activeIconId, setActiveIconId] = useState<string | null>(null);
  const gifRefreshKey = useContinuousGifRefresh(previewIcons, insertedItems);

  const insertIcon = (icon: UsagePreviewIcon) => {
    setInsertedItems((currentItems) =>
      appendUsagePreviewIcon(currentItems, icon, `${Date.now()}-${currentItems.length}`),
    );
    setActiveIconId(icon.id);
  };

  return (
    <div className="flex min-h-full flex-col gap-5">
      <header className="flex items-end justify-between gap-4">
        <div className="min-w-0">
          <h2 className="truncate text-xl font-semibold tracking-normal">사용 미리보기</h2>
          <p className="mt-1 text-sm text-muted">
            {collection.name} · 아이콘 {icons.length}개 · 로컬 전용
          </p>
        </div>
        <div className="flex items-center gap-2 rounded-md border border-border bg-surface px-3 py-2 text-sm text-muted">
          <MessageSquareText aria-hidden="true" />
          댓글 화면
        </div>
      </header>

      <div className="grid min-h-0 flex-1 gap-5 xl:grid-cols-[minmax(280px,420px)_minmax(0,1fr)]">
        <aside className="min-w-0 rounded-lg border border-border bg-surface shadow-sm">
          <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
            <div className="min-w-0">
              <h3 className="truncate text-base font-semibold tracking-normal">
                아이콘 팔레트
              </h3>
              <p className="mt-1 text-xs text-muted">노출 크기 100×100</p>
            </div>
            <ImagePlus aria-hidden="true" className="text-muted" />
          </header>

          <div className="max-h-[calc(100vh-250px)] overflow-auto p-4">
            {previewIcons.length > 0 ? (
              <div className="grid gap-3">
                {previewIcons.map((icon) => (
                  <button
                    aria-label={`${icon.displayName} 댓글 미리보기에 삽입`}
                    className={cn(
                      "flex w-full flex-col items-start gap-3 rounded-md border border-border bg-white p-3 text-left transition hover:border-border-strong hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
                      activeIconId === icon.id && "border-focus bg-selected",
                    )}
                    key={icon.id}
                    type="button"
                    onClick={() => insertIcon(icon)}
                  >
                    <div className="flex w-full items-center justify-between gap-3">
                      <span className="min-w-0 truncate text-sm font-semibold">
                        {icon.displayName}
                      </span>
                      <span className="shrink-0 rounded bg-preview px-2 py-1 text-xs text-muted">
                        {icon.usesProcessedOutput ? "처리됨" : "원본"}
                      </span>
                    </div>

                    <div
                      className={cn(
                        "inline-flex max-w-full gap-1 overflow-auto",
                        previewGroupDirectionClass(icon.shape),
                      )}
                    >
                      {icon.pieces.map((piece) => (
                        <PreviewPieceImage
                          className="shrink-0"
                          gifRefreshKey={gifRefreshKey}
                          key={piece.id}
                          piece={piece}
                          showAltLabel
                        />
                      ))}
                    </div>
                  </button>
                ))}
              </div>
            ) : (
              <div className="flex min-h-[240px] items-center justify-center text-center text-sm text-muted">
                미리볼 아이콘이 없습니다.
              </div>
            )}
          </div>
        </aside>

        <PreviewComposer
          commentText={commentText}
          defaultCellHeight={collection.defaultCellHeight}
          defaultCellWidth={collection.defaultCellWidth}
          gifRefreshKey={gifRefreshKey}
          insertedItems={insertedItems}
          onClear={() => {
            setCommentText("");
            setInsertedItems([]);
            setActiveIconId(null);
          }}
          onRemoveLast={() => {
            setInsertedItems((currentItems) => currentItems.slice(0, -1));
          }}
          onTextChange={setCommentText}
        />
      </div>
    </div>
  );
}

function useContinuousGifRefresh(
  previewIcons: UsagePreviewIcon[],
  insertedItems: InsertedPreviewIcon[],
) {
  const [gifRefreshKey, setGifRefreshKey] = useState(0);
  const shouldRefreshGif = useMemo(
    () => hasAnimatedPreview(previewIcons, insertedItems),
    [insertedItems, previewIcons],
  );

  useEffect(() => {
    if (!shouldRefreshGif) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      setGifRefreshKey((currentKey) => (currentKey + 1) % 10_000);
    }, 2500);

    return () => window.clearInterval(intervalId);
  }, [shouldRefreshGif]);

  return gifRefreshKey;
}

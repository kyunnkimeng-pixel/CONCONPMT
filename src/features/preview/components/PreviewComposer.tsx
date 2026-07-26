import { Images, RotateCcw, Undo2 } from "lucide-react";

import type {
  InsertedPreviewIcon,
  UsagePreviewPiece,
} from "@/features/preview/preview-model";
import { isGifPreviewUrl } from "@/features/preview/preview-model";
import { cn } from "@/lib/utils";

interface PreviewComposerProps {
  commentText: string;
  insertedItems: InsertedPreviewIcon[];
  gifRefreshKey: number;
  defaultCellWidth: number;
  defaultCellHeight: number;
  onClear: () => void;
  onRemoveLast: () => void;
  onTextChange: (value: string) => void;
}

interface PreviewPieceImageProps {
  piece: UsagePreviewPiece;
  gifRefreshKey: number;
  showAltLabel?: boolean;
  className?: string;
}

export function PreviewComposer({
  commentText,
  insertedItems,
  gifRefreshKey,
  defaultCellWidth,
  defaultCellHeight,
  onClear,
  onRemoveLast,
  onTextChange,
}: PreviewComposerProps) {
  const hasPreviewContent = commentText.trim().length > 0 || insertedItems.length > 0;

  return (
    <section className="flex min-w-0 flex-1 flex-col gap-4">
      <div className="rounded-lg border border-border bg-surface shadow-sm">
        <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
          <div className="min-w-0">
            <h2 className="truncate text-base font-semibold tracking-normal">
              디시인사이드 댓글 미리보기
            </h2>
            <p className="mt-1 text-xs text-muted">
              노출 100×100 · 기본 셀 {defaultCellWidth}×{defaultCellHeight}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={insertedItems.length === 0}
              type="button"
              onClick={onRemoveLast}
            >
              <Undo2 aria-hidden="true" />
              마지막 삭제
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
              disabled={!hasPreviewContent}
              title="입력한 댓글과 미리보기 아이콘만 비웁니다."
              type="button"
              onClick={onClear}
            >
              <RotateCcw aria-hidden="true" />
              미리보기 비우기
            </button>
          </div>
        </header>

        <div className="border-b border-border bg-white px-4 py-3">
          <textarea
            aria-label="댓글 내용"
            className="min-h-20 w-full resize-none rounded-md border border-border bg-surface px-3 py-2 text-sm leading-6 text-foreground outline-none focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            placeholder="댓글 내용"
            value={commentText}
            onChange={(event) => onTextChange(event.currentTarget.value)}
          />
        </div>

        <article className="bg-canvas px-4 py-4">
          <div className="rounded-md border border-border bg-white">
            <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
              <div className="flex min-w-0 items-center gap-2">
                <strong className="truncate text-sm font-semibold">ㅇㅇ</strong>
                <span className="text-xs text-muted">방금 전</span>
              </div>
              <span className="rounded bg-preview px-2 py-1 text-xs text-muted">
                로컬 미리보기
              </span>
            </header>

            <div className="min-h-[220px] px-4 py-4">
              {hasPreviewContent ? (
                <div className="flex flex-col gap-3">
                  {commentText.trim().length > 0 ? (
                    <p className="whitespace-pre-wrap text-sm leading-6 text-foreground">
                      {commentText}
                    </p>
                  ) : null}

                  {insertedItems.length > 0 ? (
                    <div className="flex flex-wrap items-start gap-2">
                      {insertedItems.map((item) => (
                        <div
                          aria-label={`${item.displayName} 삽입됨`}
                          className={cn(
                            "inline-flex rounded-sm align-top",
                            previewGroupDirectionClass(item.shape),
                          )}
                          key={item.id}
                        >
                          {item.pieces.map((piece) => (
                            <PreviewPieceImage
                              gifRefreshKey={gifRefreshKey}
                              key={`${item.id}-${piece.id}`}
                              piece={piece}
                            />
                          ))}
                        </div>
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : (
                <div className="flex min-h-[188px] items-center justify-center text-sm text-muted">
                  댓글 미리보기 내용이 없습니다.
                </div>
              )}
            </div>

            {insertedItems.length > 0 ? (
              <footer className="border-t border-border px-4 py-3">
                <div className="flex flex-wrap gap-2">
                  {insertedItems.map((item) => (
                    <span
                      className="rounded bg-preview px-2 py-1 text-xs text-muted"
                      key={`${item.id}-alt`}
                    >
                      {item.displayName}:{" "}
                      {item.pieces
                        .map((piece) => piece.altText || "alt 없음")
                        .join(", ")}
                    </span>
                  ))}
                </div>
              </footer>
            ) : null}
          </div>
        </article>
      </div>
    </section>
  );
}

export function PreviewPieceImage({
  piece,
  gifRefreshKey,
  showAltLabel = false,
  className,
}: PreviewPieceImageProps) {
  return (
    <span className={cn("inline-flex flex-col items-center gap-1", className)}>
      <span
        className="inline-flex items-center justify-center overflow-hidden border border-border bg-preview"
        style={{
          height: piece.displayHeight,
          width: piece.displayWidth,
        }}
      >
        {piece.imageUrl ? (
          <img
            alt={piece.altText || piece.displayName}
            className="size-full object-contain"
            draggable={false}
            src={previewImageUrl(piece.imageUrl, gifRefreshKey)}
          />
        ) : (
          <Images aria-hidden="true" className="text-muted" />
        )}
      </span>
      {showAltLabel ? (
        <span className="max-w-[100px] truncate text-xs font-medium text-muted">
          {piece.altText || "alt 없음"}
        </span>
      ) : null}
    </span>
  );
}

export function previewGroupDirectionClass(shape: InsertedPreviewIcon["shape"]) {
  return shape === "vertical_double" ? "flex-col" : "flex-row";
}

function previewImageUrl(url: string, gifRefreshKey: number) {
  if (!isGifPreviewUrl(url)) {
    return url;
  }

  const separator = url.includes("?") ? "&" : "?";
  return `${url}${separator}usagePreviewLoop=${gifRefreshKey}`;
}

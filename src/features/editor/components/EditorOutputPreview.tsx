import type { ReactNode } from "react";

import type { IconShape } from "@/features/editor/types";

interface EditorOutputPreviewProps {
  cellHeight: number;
  cellWidth: number;
  children: ReactNode;
  previewHeight: number;
  previewWidth: number;
  shape: IconShape;
}

const MAX_RENDERED_PREVIEW_WIDTH = 220;
const MAX_RENDERED_PREVIEW_HEIGHT = 128;

export function fitEditorOutputPreview(displayWidth: number, displayHeight: number) {
  const safeWidth = Math.max(1, displayWidth);
  const safeHeight = Math.max(1, displayHeight);
  const scale = Math.min(
    1,
    MAX_RENDERED_PREVIEW_WIDTH / safeWidth,
    MAX_RENDERED_PREVIEW_HEIGHT / safeHeight,
  );

  return {
    height: safeHeight * scale,
    scale,
    width: safeWidth * scale,
  };
}

export function EditorOutputPreview({
  cellHeight,
  cellWidth,
  children,
  previewHeight,
  previewWidth,
  shape,
}: EditorOutputPreviewProps) {
  const displayWidth = shape === "horizontal_double" ? previewWidth * 2 : previewWidth;
  const displayHeight = shape === "vertical_double" ? previewHeight * 2 : previewHeight;
  const pieceCount = shape === "single" ? 1 : 2;
  const fittedPreview = fitEditorOutputPreview(displayWidth, displayHeight);

  return (
    <section
      aria-label="출력 미리보기"
      className="sticky top-0 z-20 -mx-5 border-y border-border bg-surface px-5 py-2.5 shadow-sm"
      data-testid="editor-output-preview"
    >
      <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-sm font-semibold tracking-normal">출력 미리보기</h3>
            <span className="rounded-full border border-border bg-white px-2 py-0.5 text-[11px] font-medium text-foreground">
              적용 전
            </span>
          </div>
          <p className="mt-1 text-[11px] leading-4 text-muted">
            표시 {displayWidth}×{displayHeight}px
            <br />
            출력 조각 {cellWidth}×{cellHeight}px · {pieceCount}개
          </p>
        </div>
        <div className="flex items-center justify-center overflow-hidden rounded-md border border-border bg-preview p-2">
          <div style={{ height: fittedPreview.height, width: fittedPreview.width }}>
            <div
              style={{
                height: displayHeight,
                transform: `scale(${fittedPreview.scale})`,
                transformOrigin: "top left",
                width: displayWidth,
              }}
            >
              {children}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

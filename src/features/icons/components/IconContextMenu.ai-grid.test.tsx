import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { IconContextMenu } from "@/features/icons/components/IconContextMenu";

function renderMenu(aiGridEditDisabledReason: string | null) {
  return renderToStaticMarkup(
    <IconContextMenu
      aiGridEditDisabledReason={aiGridEditDisabledReason}
      altSelectionCount={2}
      hasExportResult={false}
      hasNote={false}
      isCover={false}
      isGifIcon={false}
      selectionCount={2}
      x={10}
      y={10}
      onAiGridEdit={() => undefined}
      onBatchAltEdit={() => undefined}
      onClearNote={() => undefined}
      onClose={() => undefined}
      onDelete={() => undefined}
      onDuplicate={() => undefined}
      onEdit={() => undefined}
      onEditNote={() => undefined}
      onExportGifFrameSheet={() => undefined}
      onExportSelectedSheet={() => undefined}
      onReimportGifFrameSheet={() => undefined}
      onRename={() => undefined}
      onReplaceImage={() => undefined}
      onRevealExportResult={() => undefined}
      onRevealOriginal={() => undefined}
      onSetCover={() => undefined}
      onSetReadiness={() => undefined}
      onSetThumbnailOverride={() => undefined}
    />,
  );
}

describe("IconContextMenu AI grid action", () => {
  it("shows an implemented action for an eligible multi-selection", () => {
    const html = renderMenu(null);
    expect(html).toContain('data-testid="icon-context-ai-grid-edit"');
    expect(html).toContain("선택 2개 AI로 수정");
    expect(html).not.toMatch(
      /data-testid="icon-context-ai-grid-edit"[^>]*disabled/,
    );
  });

  it("keeps the action disabled with an exact reason when unsupported", () => {
    const html = renderMenu("GIF는 프레임 작업시트를 사용해 주세요.");
        expect(html).toContain('aria-disabled="true"');
    expect(html).not.toMatch(
      /data-testid="icon-context-ai-grid-edit"[^>]* disabled=""/,
    );
    expect(html).toContain("GIF는 프레임 작업시트를 사용해 주세요.");
  });
});

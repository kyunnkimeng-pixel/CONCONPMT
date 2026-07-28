import { DndContext } from "@dnd-kit/core";
import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { IconSummary } from "@/features/collections/types";
import { IconTile } from "@/features/icons/components/IconTile";

const icon: IconSummary = {
  id: "icon_2",
  collectionId: "collection_1",
  sourceFileId: "source_2",
  displayName: "새 AI 아이콘",
  note: null,
  iconKind: "image",
  readiness: "working",
  placeholderText: null,
  shape: "single",
  orderIndex: 1,
  cellWidthOverride: null,
  cellHeightOverride: null,
  thumbnailUrl: null,
  thumbnailOverrideUrl: null,
  currentPreviewUrl: null,
  transformQuarterTurns: 0,
  transformFlipHorizontal: false,
  transformFlipVertical: false,
  gifLoopMode: "preserve",
  gifLoopCount: null,
  createdAt: "2026-07-28T00:00:00Z",
  updatedAt: "2026-07-28T00:00:00Z",
  pieces: [],
};

describe("IconTile reveal target", () => {
  it("exposes a stable icon identity on the focusable listbox option", () => {
    const html = renderToString(
      <DndContext>
        <IconTile
          duplicatePieceIds={new Set()}
          editRequest={null}
          icon={icon}
          isCover={false}
          isSelected
          previewHeight={100}
          previewWidth={100}
          showDetails={false}
          validateAltDraft={() => null}
          validateCurrentAlt={() => null}
          onAltCommit={async () => true}
          onContextMenu={() => {}}
          onEditNote={() => {}}
          onOpenEditor={() => {}}
          onRename={async () => true}
          onSelect={() => {}}
        />
      </DndContext>,
    );

    expect(html).toContain('data-icon-id="icon_2"');
    expect(html).toContain('role="option"');
    expect(html).toContain('tabindex="0"');
    expect(html).toContain('aria-selected="true"');
  });
});
